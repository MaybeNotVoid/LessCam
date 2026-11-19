//! LessCam - High-Performance Video Streaming & Virtual Camera Engine (Rust Edition)
//! Handles video decoding via FFmpeg raw RGB24 pipe, static image streaming,
//! aspect-ratio scaling (letterbox/pillarbox), high-precision framerate pacing,
//! and broadcasting to Windows DirectShow Virtual Camera (UnityCapture / OBS VirtualCam).

use crate::engine::deps::{check_virtual_cam_driver_installed, resolve_binary_path};
use crate::engine::video_trimmer::get_video_metadata;
use crossbeam_channel::{unbounded, Receiver, Sender};
use image::{imageops::FilterType, RgbImage};
use parking_lot::Mutex;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Standby,
    LocalVideo,
    YouTube,
    Image,
}

#[derive(Debug, Clone)]
pub struct VideoFramePacket {
    pub width: u32,
    pub height: u32,
    pub rgb_data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct VideoEngineState {
    pub source_type: SourceType,
    pub source_title: String,
    pub is_playing: bool,
    pub loop_enabled: bool,
    pub current_pos_sec: f64,
    pub duration_sec: f64,
    pub target_w: u32,
    pub target_h: u32,
    pub target_fps: u32,
    pub vcam_active: bool,
    pub vcam_status_msg: String,
    pub last_error: Option<String>,
}

impl Default for VideoEngineState {
    fn default() -> Self {
        Self {
            source_type: SourceType::Standby,
            source_title: "Standby".to_string(),
            is_playing: false,
            loop_enabled: true,
            current_pos_sec: 0.0,
            duration_sec: 0.0,
            target_w: 1280,
            target_h: 720,
            target_fps: 30,
            vcam_active: false,
            vcam_status_msg: "Webcam Virtuale Disattivata".to_string(),
            last_error: None,
        }
    }
}

/// Scale an input RGB buffer (`in_w x in_h x 3`) to fit inside (`target_w x target_h x 3`)
/// preserving the original aspect ratio and filling borders with black (letterbox/pillarbox).
pub fn letterbox_frame(
    frame_rgb: &[u8],
    in_w: u32,
    in_h: u32,
    target_w: u32,
    target_h: u32,
) -> Vec<u8> {
    let out_len = (target_w as usize) * (target_h as usize) * 3;
    if in_w == 0 || in_h == 0 || target_w == 0 || target_h == 0 {
        return vec![0u8; out_len];
    }

    if in_w == target_w && in_h == target_h && frame_rgb.len() == out_len {
        return frame_rgb.to_vec();
    }

    let scale_x = (target_w as f64) / (in_w as f64);
    let scale_y = (target_h as f64) / (in_h as f64);
    let scale = scale_x.min(scale_y);

    let new_w = ((in_w as f64 * scale).round() as u32).max(1).min(target_w);
    let new_h = ((in_h as f64 * scale).round() as u32).max(1).min(target_h);

    let src_img = match RgbImage::from_raw(in_w, in_h, frame_rgb.to_vec()) {
        Some(img) => img,
        None => return vec![0u8; out_len],
    };

    let resized = image::imageops::resize(&src_img, new_w, new_h, FilterType::Triangle);
    let mut canvas = vec![0u8; out_len];

    let x_offset = ((target_w - new_w) / 2) as usize;
    let y_offset = ((target_h - new_h) / 2) as usize;
    let resized_raw = resized.as_raw();
    let row_bytes = (new_w as usize) * 3;
    let target_row_bytes = (target_w as usize) * 3;

    for y in 0..(new_h as usize) {
        let dst_row_start = (y_offset + y) * target_row_bytes + (x_offset * 3);
        let src_row_start = y * row_bytes;
        canvas[dst_row_start..dst_row_start + row_bytes]
            .copy_from_slice(&resized_raw[src_row_start..src_row_start + row_bytes]);
    }

    canvas
}

/// Generate a sleek dark standby frame with animated pulsing cyan accent bar.
pub fn generate_standby_frame(width: u32, height: u32, tick: u64) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut buf = vec![0u8; w * h * 3];

    // Deep studio obsidian background with subtle vertical gradient
    for y in 0..h {
        let v = 14u8.saturating_add(((y * 8) / h.max(1)) as u8);
        let row_offset = y * w * 3;
        for x in 0..w {
            let idx = row_offset + x * 3;
            buf[idx] = v;
            buf[idx + 1] = v.saturating_add(2);
            buf[idx + 2] = v.saturating_add(6);
        }
    }

    // Animated glowing horizon line
    let pulse = 0.55 + 0.45 * ((tick as f64 * 0.07).sin());
    let r_acc = (0.0 * pulse) as u8;
    let g_acc = (175.0 * pulse) as u8;
    let b_acc = (255.0 * pulse) as u8;

    let cy = h / 2;
    let cx = w / 2;
    let bar_half = (w / 6).max(60);
    let x_start = cx.saturating_sub(bar_half);
    let x_end = (cx + bar_half).min(w);

    for dy in 0..3 {
        let y = (cy + dy).min(h.saturating_sub(1));
        for x in x_start..x_end {
            let dist = ((x as isize - cx as isize).unsigned_abs() as f64) / (bar_half as f64);
            let fade = (1.0 - dist * dist).max(0.0);
            let idx = (y * w + x) * 3;
            buf[idx] = (r_acc as f64 * fade) as u8;
            buf[idx + 1] = (g_acc as f64 * fade) as u8;
            buf[idx + 2] = (b_acc as f64 * fade) as u8;
        }
    }

    buf
}

enum VideoCommand {
    LoadLocalVideo(String),
    LoadYouTube {
        video_url: String,
        duration: f64,
        title: String,
    },
    LoadImage(String),
    ClearToStandby,
    SetPlaying(bool),
    TogglePlayPause,
    Seek(f64),
    SetLoop(bool),
    SetResolution(u32, u32),
    SetFps(u32),
    SetVirtualCam(bool),
    StopWorker,
}

pub struct VideoStreamEngine {
    state: Arc<Mutex<VideoEngineState>>,
    latest_frame: Arc<Mutex<Option<VideoFramePacket>>>,
    cmd_tx: Sender<VideoCommand>,
    running: Arc<AtomicBool>,
}

impl VideoStreamEngine {
    pub fn new(target_w: u32, target_h: u32, target_fps: u32) -> Self {
        let initial_state = VideoEngineState {
            target_w,
            target_h,
            target_fps,
            ..Default::default()
        };
        let state = Arc::new(Mutex::new(initial_state));
        let latest_frame = Arc::new(Mutex::new(None));
        let (cmd_tx, cmd_rx) = unbounded();
        let running = Arc::new(AtomicBool::new(true));

        let state_clone = Arc::clone(&state);
        let frame_clone = Arc::clone(&latest_frame);
        let running_clone = Arc::clone(&running);

        thread::spawn(move || {
            Self::worker_loop(state_clone, frame_clone, cmd_rx, running_clone);
        });

        Self {
            state,
            latest_frame,
            cmd_tx,
            running,
        }
    }

    pub fn state(&self) -> VideoEngineState {
        self.state.lock().clone()
    }

    pub fn take_latest_frame(&self) -> Option<VideoFramePacket> {
        self.latest_frame.lock().clone()
    }

    pub fn clear_error(&self) {
        self.state.lock().last_error = None;
    }

    pub fn load_local_video(&self, path: impl Into<String>) {
        let _ = self.cmd_tx.send(VideoCommand::LoadLocalVideo(path.into()));
    }

    pub fn load_youtube_video(&self, video_url: String, duration: f64, title: String) {
        let _ = self.cmd_tx.send(VideoCommand::LoadYouTube {
            video_url,
            duration,
            title,
        });
    }

    pub fn load_image(&self, path: impl Into<String>) {
        let _ = self.cmd_tx.send(VideoCommand::LoadImage(path.into()));
    }

    pub fn clear_source(&self) {
        let _ = self.cmd_tx.send(VideoCommand::ClearToStandby);
    }

    pub fn play(&self) {
        let _ = self.cmd_tx.send(VideoCommand::SetPlaying(true));
    }

    pub fn pause(&self) {
        let _ = self.cmd_tx.send(VideoCommand::SetPlaying(false));
    }

    pub fn toggle_play_pause(&self) {
        let _ = self.cmd_tx.send(VideoCommand::TogglePlayPause);
    }

    pub fn seek(&self, seconds: f64) {
        let _ = self.cmd_tx.send(VideoCommand::Seek(seconds.max(0.0)));
    }

    pub fn set_loop(&self, enabled: bool) {
        let _ = self.cmd_tx.send(VideoCommand::SetLoop(enabled));
    }

    pub fn set_output_resolution(&self, width: u32, height: u32) {
        let _ = self.cmd_tx.send(VideoCommand::SetResolution(width, height));
    }

    pub fn set_target_fps(&self, fps: u32) {
        let _ = self.cmd_tx.send(VideoCommand::SetFps(fps));
    }

    pub fn set_virtual_cam(&self, enabled: bool) {
        let _ = self.cmd_tx.send(VideoCommand::SetVirtualCam(enabled));
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = self.cmd_tx.send(VideoCommand::StopWorker);
    }

    fn spawn_ffmpeg_decoder(
        source: &str,
        start_sec: f64,
        out_w: u32,
        out_h: u32,
        fps: u32,
    ) -> Result<Child, String> {
        let ffmpeg_bin = resolve_binary_path("ffmpeg").ok_or_else(|| {
            "FFmpeg non trovato! Aprilo da '📦 Installer & Dipendenze' per installarlo con 1 click."
                .to_string()
        })?;

        let mut cmd = Command::new(ffmpeg_bin);
        cmd.args(["-loglevel", "quiet"]);
        if start_sec > 0.1 {
            cmd.args(["-ss", &format!("{:.3}", start_sec)]);
        }
        cmd.args(["-i", source]);

        // Letterbox / Pillarbox filter directly inside FFmpeg for maximum SIMD speed
        let vf = format!(
            "fps={},scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2:black",
            fps, out_w, out_h, out_w, out_h
        );
        cmd.args([
            "-vf",
            &vf,
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ]);
        cmd.stdout(Stdio::piped()).stderr(Stdio::null());

        cmd.spawn()
            .map_err(|e| format!("Impossibile avviare FFmpeg decoder: {}", e))
    }

    fn kill_decoder(child_opt: &mut Option<Child>) {
        if let Some(mut c) = child_opt.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    fn worker_loop(
        state: Arc<Mutex<VideoEngineState>>,
        latest_frame: Arc<Mutex<Option<VideoFramePacket>>>,
        cmd_rx: Receiver<VideoCommand>,
        running: Arc<AtomicBool>,
    ) {
        let mut standby_tick: u64 = 0;
        let mut active_source_path: Option<String> = None;
        let mut static_image_raw: Option<(u32, u32, Vec<u8>)> = None;
        let mut cached_scaled_image: Option<Vec<u8>> = None;
        let mut last_frame_buf: Option<Vec<u8>> = None;
        let mut ffmpeg_child: Option<Child> = None;

        while running.load(Ordering::SeqCst) {
            let loop_start = Instant::now();
            let mut need_restart_decoder = false;

            // Process pending commands
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    VideoCommand::LoadLocalVideo(path) => {
                        Self::kill_decoder(&mut ffmpeg_child);
                        let meta = get_video_metadata(&path);
                        let title = Path::new(&path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Video Locale")
                            .to_string();

                        active_source_path = Some(path);
                        static_image_raw = None;
                        cached_scaled_image = None;

                        let mut st = state.lock();
                        st.source_type = SourceType::LocalVideo;
                        st.source_title = title;
                        st.duration_sec = meta.duration;
                        st.current_pos_sec = 0.0;
                        st.is_playing = true;
                        need_restart_decoder = true;
                    }
                    VideoCommand::LoadYouTube {
                        video_url,
                        duration,
                        title,
                    } => {
                        Self::kill_decoder(&mut ffmpeg_child);
                        active_source_path = Some(video_url);
                        static_image_raw = None;
                        cached_scaled_image = None;

                        let mut st = state.lock();
                        st.source_type = SourceType::YouTube;
                        st.source_title = title;
                        st.duration_sec = duration;
                        st.current_pos_sec = 0.0;
                        st.is_playing = true;
                        need_restart_decoder = true;
                    }
                    VideoCommand::LoadImage(path) => {
                        Self::kill_decoder(&mut ffmpeg_child);
                        active_source_path = Some(path.clone());
                        match image::open(&path) {
                            Ok(img) => {
                                let rgb = img.to_rgb8();
                                let (iw, ih) = rgb.dimensions();
                                let (tw, th) = {
                                    let st = state.lock();
                                    (st.target_w, st.target_h)
                                };
                                let raw = rgb.into_raw();
                                let scaled = letterbox_frame(&raw, iw, ih, tw, th);
                                static_image_raw = Some((iw, ih, raw));
                                cached_scaled_image = Some(scaled);

                                let title = Path::new(&path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("Immagine")
                                    .to_string();
                                let mut st = state.lock();
                                st.source_type = SourceType::Image;
                                st.source_title = title;
                                st.duration_sec = 0.0;
                                st.current_pos_sec = 0.0;
                                st.is_playing = true;
                            }
                            Err(e) => {
                                let mut st = state.lock();
                                st.last_error =
                                    Some(format!("Impossibile caricare l'immagine: {}", e));
                                st.source_type = SourceType::Standby;
                            }
                        }
                    }
                    VideoCommand::ClearToStandby => {
                        Self::kill_decoder(&mut ffmpeg_child);
                        active_source_path = None;
                        static_image_raw = None;
                        cached_scaled_image = None;
                        last_frame_buf = None;

                        let mut st = state.lock();
                        st.source_type = SourceType::Standby;
                        st.source_title = "Standby".to_string();
                        st.duration_sec = 0.0;
                        st.current_pos_sec = 0.0;
                        st.is_playing = false;
                    }
                    VideoCommand::SetPlaying(play) => {
                        state.lock().is_playing = play;
                    }
                    VideoCommand::TogglePlayPause => {
                        let mut st = state.lock();
                        st.is_playing = !st.is_playing;
                    }
                    VideoCommand::Seek(sec) => {
                        let mut st = state.lock();
                        st.current_pos_sec = sec;
                        if matches!(st.source_type, SourceType::LocalVideo | SourceType::YouTube) {
                            need_restart_decoder = true;
                        }
                    }
                    VideoCommand::SetLoop(lp) => {
                        state.lock().loop_enabled = lp;
                    }
                    VideoCommand::SetResolution(w, h) => {
                        {
                            let mut st = state.lock();
                            st.target_w = w;
                            st.target_h = h;
                        }
                        if let Some((iw, ih, ref raw)) = static_image_raw {
                            cached_scaled_image = Some(letterbox_frame(raw, iw, ih, w, h));
                        }
                        last_frame_buf = None;
                        if active_source_path.is_some() {
                            need_restart_decoder = true;
                        }
                    }
                    VideoCommand::SetFps(fps) => {
                        state.lock().target_fps = fps.max(1);
                        if active_source_path.is_some() {
                            need_restart_decoder = true;
                        }
                    }
                    VideoCommand::SetVirtualCam(enable) => {
                        let mut st = state.lock();
                        if enable {
                            let driver_status = check_virtual_cam_driver_installed();
                            if driver_status.is_installed() {
                                st.vcam_active = true;
                                st.vcam_status_msg = "Attiva: DirectShow VirtualCam".to_string();
                            } else {
                                st.vcam_active = false;
                                st.vcam_status_msg =
                                    "Driver non disponibile (Usa Installer Dipendenze)".to_string();
                                st.last_error = Some(
                                    "Driver Webcam Virtuale non rilevato. Clicca su '📦 Installer & Dipendenze' per registrarlo automaticamente con 1 click."
                                        .to_string(),
                                );
                            }
                        } else {
                            st.vcam_active = false;
                            st.vcam_status_msg = "Webcam Virtuale Disattivata".to_string();
                        }
                    }
                    VideoCommand::StopWorker => {
                        Self::kill_decoder(&mut ffmpeg_child);
                        return;
                    }
                }
            }

            let (source_type, is_playing, loop_enabled, target_w, target_h, target_fps, cur_pos) = {
                let st = state.lock();
                (
                    st.source_type,
                    st.is_playing,
                    st.loop_enabled,
                    st.target_w,
                    st.target_h,
                    st.target_fps.max(1),
                    st.current_pos_sec,
                )
            };

            if need_restart_decoder
                && matches!(source_type, SourceType::LocalVideo | SourceType::YouTube)
            {
                Self::kill_decoder(&mut ffmpeg_child);
                if let Some(ref src) = active_source_path {
                    match Self::spawn_ffmpeg_decoder(src, cur_pos, target_w, target_h, target_fps) {
                        Ok(child) => {
                            ffmpeg_child = Some(child);
                        }
                        Err(e) => {
                            let mut st = state.lock();
                            st.last_error = Some(e);
                            st.source_type = SourceType::Standby;
                            st.is_playing = false;
                        }
                    }
                }
            }

            let expected_bytes = (target_w as usize) * (target_h as usize) * 3;
            let frame_rgb: Vec<u8> = match source_type {
                SourceType::Standby => {
                    standby_tick = standby_tick.wrapping_add(1);
                    generate_standby_frame(target_w, target_h, standby_tick)
                }
                SourceType::Image => {
                    if let Some(ref img_buf) = cached_scaled_image {
                        if img_buf.len() == expected_bytes {
                            img_buf.clone()
                        } else {
                            standby_tick = standby_tick.wrapping_add(1);
                            generate_standby_frame(target_w, target_h, standby_tick)
                        }
                    } else {
                        standby_tick = standby_tick.wrapping_add(1);
                        generate_standby_frame(target_w, target_h, standby_tick)
                    }
                }
                SourceType::LocalVideo | SourceType::YouTube => {
                    if is_playing {
                        let mut read_ok = false;
                        let mut buf = vec![0u8; expected_bytes];

                        if let Some(ref mut child) = ffmpeg_child {
                            if let Some(ref mut stdout) = child.stdout {
                                if stdout.read_exact(&mut buf).is_ok() {
                                    read_ok = true;
                                }
                            }
                        }

                        if read_ok {
                            {
                                let mut st = state.lock();
                                st.current_pos_sec += 1.0 / (target_fps as f64);
                                if st.duration_sec > 0.0 && st.current_pos_sec > st.duration_sec {
                                    st.current_pos_sec = st.duration_sec;
                                }
                            }
                            last_frame_buf = Some(buf.clone());
                            buf
                        } else {
                            // Reached EOF
                            Self::kill_decoder(&mut ffmpeg_child);
                            if loop_enabled {
                                {
                                    let mut st = state.lock();
                                    st.current_pos_sec = 0.0;
                                }
                                if let Some(ref src) = active_source_path {
                                    if let Ok(c) = Self::spawn_ffmpeg_decoder(
                                        src, 0.0, target_w, target_h, target_fps,
                                    ) {
                                        ffmpeg_child = Some(c);
                                    }
                                }
                            } else {
                                state.lock().is_playing = false;
                            }

                            last_frame_buf.clone().unwrap_or_else(|| {
                                generate_standby_frame(target_w, target_h, standby_tick)
                            })
                        }
                    } else {
                        // Paused: keep displaying last decoded frame
                        last_frame_buf.clone().unwrap_or_else(|| {
                            standby_tick = standby_tick.wrapping_add(1);
                            generate_standby_frame(target_w, target_h, standby_tick)
                        })
                    }
                }
            };

            *latest_frame.lock() = Some(VideoFramePacket {
                width: target_w,
                height: target_h,
                rgb_data: frame_rgb,
            });

            // High-precision frame pacing
            let target_frame_dur = Duration::from_secs_f64(1.0 / (target_fps as f64));
            let elapsed = loop_start.elapsed();
            if let Some(rem) = target_frame_dur.checked_sub(elapsed) {
                if rem > Duration::from_millis(1) {
                    thread::sleep(rem);
                }
            }
        }

        Self::kill_decoder(&mut ffmpeg_child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_letterbox_aspect_ratios() {
        let target_w = 1280;
        let target_h = 720;

        // Case 1: Identical 16:9 frame
        let frame_16_9 = vec![100u8; (1280 * 720 * 3) as usize];
        let out_16_9 = letterbox_frame(&frame_16_9, 1280, 720, target_w, target_h);
        assert_eq!(out_16_9.len(), (1280 * 720 * 3) as usize);
        let center_idx = ((360 * 1280 + 640) * 3) as usize;
        assert_eq!(out_16_9[center_idx], 100);

        // Case 2: 4:3 input (1024x768 -> pillarbox black bars on left/right)
        let frame_4_3 = vec![200u8; (1024 * 768 * 3) as usize];
        let out_4_3 = letterbox_frame(&frame_4_3, 1024, 768, target_w, target_h);
        assert_eq!(out_4_3.len(), (1280 * 720 * 3) as usize);
        let left_edge_idx = ((360 * 1280 + 10) * 3) as usize;
        assert_eq!(out_4_3[left_edge_idx], 0); // Black pillarbox bar
        assert_eq!(out_4_3[center_idx], 200); // Center content

        // Case 3: Vertical 9:16 input (1080x1920)
        let frame_vert = vec![150u8; (1080 * 1920 * 3) as usize];
        let out_vert = letterbox_frame(&frame_vert, 1080, 1920, target_w, target_h);
        assert_eq!(out_vert.len(), (1280 * 720 * 3) as usize);
        assert_eq!(out_vert[left_edge_idx], 0);
        assert_eq!(out_vert[center_idx], 150);
    }

    #[test]
    fn test_standby_frame_generation() {
        let frame = generate_standby_frame(640, 360, 10);
        assert_eq!(frame.len(), 640 * 360 * 3);
    }
}

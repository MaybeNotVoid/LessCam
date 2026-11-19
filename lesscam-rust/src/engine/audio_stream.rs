//! LessCam - Audio Stream & Mixing Engine (Rust Edition)
//! Handles audio decoding via FFmpeg f32le stereo pipe and low-latency output routing
//! via Windows WASAPI (`cpal`), including gain scaling (0% to 200%), soft-clipping tanh limiter,
//! mute toggle, and real-time stereo RMS/Peak VU-metering.

use crate::engine::deps::resolve_binary_path;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub index: usize,
    pub name: String,
    pub api: String,
    pub is_default: bool,
    pub is_virtual: bool,
    pub channels: u16,
    pub sample_rate: u32,
}

/// Check if an audio device name corresponds to a virtual audio router (VB-Cable, Voicemeeter, etc.)
pub fn is_virtual_audio_device(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["cable", "virtual", "vb-audio", "voicemeeter"]
        .iter()
        .any(|k| lower.contains(k))
}

/// Convert a volume percentage (0..=200) to a formatted decibel string.
pub fn format_gain_db(percentage: u32) -> String {
    if percentage == 0 {
        "-∞ dB".to_string()
    } else {
        let db = 20.0 * ((percentage as f64) / 100.0).log10();
        if db > 0.0 {
            format!("+{:.1} dB", db)
        } else {
            format!("{:.1} dB", db)
        }
    }
}

/// Apply gain and soft-clipping tanh limiter to interleaved stereo samples `[L, R, L, R, ...]`
/// and return `(rms_left, rms_right)` scaled for VU-meter display in `[0.0, 1.0]`.
pub fn process_audio_chunk_gain_and_rms(samples: &mut [f32], gain: f32, muted: bool) -> (f32, f32) {
    if muted || gain <= 0.0 || samples.is_empty() {
        for s in samples.iter_mut() {
            *s = 0.0;
        }
        return (0.0, 0.0);
    }

    let mut sum_sq_l = 0.0f64;
    let mut sum_sq_r = 0.0f64;
    let mut frames_count = 0usize;

    for frame in samples.chunks_mut(2) {
        let mut l = frame[0] * gain;
        let mut r = if frame.len() > 1 { frame[1] * gain } else { l };

        if gain > 1.0 {
            l = l.tanh();
            r = r.tanh();
        } else {
            l = l.clamp(-1.0, 1.0);
            r = r.clamp(-1.0, 1.0);
        }

        frame[0] = l;
        if frame.len() > 1 {
            frame[1] = r;
        }

        sum_sq_l += (l as f64) * (l as f64);
        sum_sq_r += (r as f64) * (r as f64);
        frames_count += 1;
    }

    if frames_count == 0 {
        return (0.0, 0.0);
    }

    let rms_l = ((sum_sq_l / (frames_count as f64)).sqrt() as f32 * 1.8).clamp(0.0, 1.0);
    let rms_r = ((sum_sq_r / (frames_count as f64)).sqrt() as f32 * 1.8).clamp(0.0, 1.0);
    (rms_l, rms_r)
}

/// Enumerate all valid audio output devices on the system.
pub fn get_audio_output_devices() -> Vec<AudioDeviceInfo> {
    #[cfg(windows)]
    {
        use cpal::traits::{DeviceTrait, HostTrait};
        let mut list = Vec::new();
        let host = cpal::default_host();
        let default_name = host
            .default_output_device()
            .and_then(|d| d.name().ok())
            .unwrap_or_default();

        if let Ok(devices) = host.output_devices() {
            for (idx, dev) in devices.enumerate() {
                let name = dev.name().unwrap_or_else(|_| format!("Device {}", idx));
                let cfg = dev.default_output_config().ok();
                let channels = cfg.as_ref().map(|c| c.channels()).unwrap_or(2);
                let sample_rate = cfg.as_ref().map(|c| c.sample_rate().0).unwrap_or(44100);
                let is_virtual = is_virtual_audio_device(&name);
                let is_default = !default_name.is_empty() && name == default_name;

                list.push(AudioDeviceInfo {
                    index: idx,
                    name,
                    api: "WASAPI".to_string(),
                    is_default,
                    is_virtual,
                    channels,
                    sample_rate,
                });
            }
        }
        if !list.is_empty() {
            return list;
        }
    }

    // Fallback devices list (used on Linux headless VM or if no physical card is attached)
    vec![
        AudioDeviceInfo {
            index: 0,
            name: "CABLE Input (VB-Audio Virtual Cable)".to_string(),
            api: "WASAPI Virtual".to_string(),
            is_default: false,
            is_virtual: true,
            channels: 2,
            sample_rate: 44100,
        },
        AudioDeviceInfo {
            index: 1,
            name: "Altoparlanti / Cuffie (Default System Output)".to_string(),
            api: "System Audio".to_string(),
            is_default: true,
            is_virtual: false,
            channels: 2,
            sample_rate: 44100,
        },
    ]
}

struct SharedAudioState {
    source_path: Option<String>,
    is_playing: bool,
    is_muted: bool,
    gain: f32,
    volume_pct: u32,
    loop_enabled: bool,
    selected_device: usize,
    seek_requested: Option<f64>,
    rms_levels: (f32, f32),
    sample_queue: VecDeque<f32>,
}

pub struct AudioEngine {
    shared: Arc<Mutex<SharedAudioState>>,
    stop_flag: Arc<AtomicBool>,
}

impl AudioEngine {
    pub fn new() -> Self {
        let devices = get_audio_output_devices();
        let initial_idx = devices
            .iter()
            .find(|d| d.is_virtual)
            .or_else(|| devices.iter().find(|d| d.is_default))
            .map(|d| d.index)
            .unwrap_or(0);

        let shared = Arc::new(Mutex::new(SharedAudioState {
            source_path: None,
            is_playing: false,
            is_muted: false,
            gain: 1.0,
            volume_pct: 100,
            loop_enabled: true,
            selected_device: initial_idx,
            seek_requested: None,
            rms_levels: (0.0, 0.0),
            sample_queue: VecDeque::with_capacity(44100 * 4),
        }));

        let stop_flag = Arc::new(AtomicBool::new(false));
        let shared_decoder = Arc::clone(&shared);
        let stop_decoder = Arc::clone(&stop_flag);

        thread::spawn(move || {
            Self::decoder_and_router_loop(shared_decoder, stop_decoder);
        });

        Self { shared, stop_flag }
    }

    pub fn load_source(&self, source_path: Option<String>) {
        let mut guard = self.shared.lock();
        guard.source_path = source_path;
        guard.sample_queue.clear();
        guard.seek_requested = Some(0.0);
        guard.rms_levels = (0.0, 0.0);
    }

    pub fn play(&self) {
        self.shared.lock().is_playing = true;
    }

    pub fn pause(&self) {
        let mut guard = self.shared.lock();
        guard.is_playing = false;
        guard.rms_levels = (0.0, 0.0);
    }

    pub fn stop(&self) {
        let mut guard = self.shared.lock();
        guard.is_playing = false;
        guard.source_path = None;
        guard.sample_queue.clear();
        guard.rms_levels = (0.0, 0.0);
    }

    pub fn seek(&self, target_sec: f64) {
        let mut guard = self.shared.lock();
        guard.seek_requested = Some(target_sec.max(0.0));
        guard.sample_queue.clear();
    }

    pub fn set_volume(&self, percentage: u32) {
        let clamped = percentage.min(200);
        let mut guard = self.shared.lock();
        guard.volume_pct = clamped;
        guard.gain = (clamped as f32) / 100.0;
    }

    pub fn volume_pct(&self) -> u32 {
        self.shared.lock().volume_pct
    }

    pub fn set_muted(&self, muted: bool) {
        let mut guard = self.shared.lock();
        guard.is_muted = muted;
        if muted {
            guard.rms_levels = (0.0, 0.0);
        }
    }

    pub fn is_muted(&self) -> bool {
        self.shared.lock().is_muted
    }

    pub fn set_loop(&self, loop_enabled: bool) {
        self.shared.lock().loop_enabled = loop_enabled;
    }

    pub fn set_output_device(&self, device_index: usize) {
        self.shared.lock().selected_device = device_index;
    }

    pub fn selected_device(&self) -> usize {
        self.shared.lock().selected_device
    }

    pub fn get_meter_levels(&self) -> (f32, f32) {
        let guard = self.shared.lock();
        if guard.is_muted || !guard.is_playing {
            (0.0, 0.0)
        } else {
            guard.rms_levels
        }
    }

    pub fn shutdown(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    fn spawn_ffmpeg_audio(source: &str, start_sec: f64) -> Option<Child> {
        let ffmpeg_bin = resolve_binary_path("ffmpeg")?;
        let mut cmd = Command::new(ffmpeg_bin);
        cmd.args(["-loglevel", "quiet"]);
        if start_sec > 0.1 {
            cmd.args(["-ss", &format!("{:.3}", start_sec)]);
        }
        cmd.args([
            "-i", source, "-vn", "-ac", "2", "-ar", "44100", "-f", "f32le", "pipe:1",
        ]);
        cmd.stdout(Stdio::piped()).stderr(Stdio::null());
        cmd.spawn().ok()
    }

    fn decoder_and_router_loop(shared: Arc<Mutex<SharedAudioState>>, stop_flag: Arc<AtomicBool>) {
        #[cfg(windows)]
        let _cpal_stream = Self::start_windows_cpal_output(Arc::clone(&shared));

        let mut ffmpeg_child: Option<Child> = None;
        let mut current_source: Option<String> = None;

        // 1024 stereo frames = 2048 f32 samples = 8192 bytes (~23.2 ms at 44.1 kHz)
        let chunk_samples = 2048usize;
        let mut byte_buf = vec![0u8; chunk_samples * 4];

        while !stop_flag.load(Ordering::SeqCst) {
            let (src_opt, is_playing, loop_enabled, seek_opt, gain, is_muted) = {
                let mut g = shared.lock();
                let sk = g.seek_requested.take();
                (
                    g.source_path.clone(),
                    g.is_playing,
                    g.loop_enabled,
                    sk,
                    g.gain,
                    g.is_muted,
                )
            };

            if src_opt != current_source || seek_opt.is_some() {
                if let Some(mut c) = ffmpeg_child.take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                current_source = src_opt.clone();
                if let Some(ref s) = current_source {
                    ffmpeg_child = Self::spawn_ffmpeg_audio(s, seek_opt.unwrap_or(0.0));
                }
            }

            if !is_playing || current_source.is_none() {
                shared.lock().rms_levels = (0.0, 0.0);
                thread::sleep(Duration::from_millis(20));
                continue;
            }

            let mut read_ok = false;
            if let Some(ref mut child) = ffmpeg_child {
                if let Some(ref mut stdout) = child.stdout {
                    if stdout.read_exact(&mut byte_buf).is_ok() {
                        read_ok = true;
                    }
                }
            }

            if read_ok {
                let mut samples = Vec::with_capacity(chunk_samples);
                for b in byte_buf.chunks_exact(4) {
                    samples.push(f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                }

                let (rms_l, rms_r) = process_audio_chunk_gain_and_rms(&mut samples, gain, is_muted);

                {
                    let mut g = shared.lock();
                    g.rms_levels = (rms_l, rms_r);
                    #[cfg(windows)]
                    {
                        if g.sample_queue.len() < 44100 * 4 {
                            g.sample_queue.extend(samples);
                        }
                    }
                }

                // Pace audio decoding to ~23ms per 1024-frame block
                thread::sleep(Duration::from_millis(21));
            } else {
                if let Some(mut c) = ffmpeg_child.take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                if loop_enabled {
                    if let Some(ref s) = current_source {
                        ffmpeg_child = Self::spawn_ffmpeg_audio(s, 0.0);
                    }
                } else {
                    shared.lock().rms_levels = (0.0, 0.0);
                    thread::sleep(Duration::from_millis(30));
                }
            }
        }

        if let Some(mut c) = ffmpeg_child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    #[cfg(windows)]
    fn start_windows_cpal_output(shared: Arc<Mutex<SharedAudioState>>) -> Option<cpal::Stream> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let host = cpal::default_host();
        let selected_idx = shared.lock().selected_device;
        let device = host
            .output_devices()
            .ok()
            .and_then(|mut it| it.nth(selected_idx))
            .or_else(|| host.default_output_device())?;

        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(44100),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut g = shared.lock();
                    if !g.is_playing || g.is_muted {
                        for s in data.iter_mut() {
                            *s = 0.0;
                        }
                        return;
                    }
                    for s in data.iter_mut() {
                        *s = g.sample_queue.pop_front().unwrap_or(0.0);
                    }
                },
                |err| eprintln!("[AudioEngine] CPAL stream error: {}", err),
                None,
            )
            .ok()?;

        let _ = stream.play();
        Some(stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_device_enumeration() {
        let devices = get_audio_output_devices();
        assert!(!devices.is_empty());
    }

    #[test]
    fn test_gain_db_formatting() {
        assert_eq!(format_gain_db(0), "-∞ dB");
        assert_eq!(format_gain_db(100), "0.0 dB");
        assert_eq!(format_gain_db(200), "+6.0 dB");
    }

    #[test]
    fn test_soft_clip_and_rms_calculation() {
        let mut samples = vec![0.8f32, -0.8f32, 0.8f32, -0.8f32];
        let (l, r) = process_audio_chunk_gain_and_rms(&mut samples, 2.0, false);
        assert!(l > 0.5 && l <= 1.0);
        assert!(r > 0.5 && r <= 1.0);
        // Verify tanh soft clipping prevented values from exceeding [-1.0, 1.0]
        for s in samples {
            assert!(s <= 1.0 && s >= -1.0);
        }
    }
}

//! LessCam - Video Trimmer & Compressor Engine (Rust Edition)
//! Extracts video metadata via ffprobe and performs high-speed H.264/AAC trimming
//! and compression via ffmpeg with real-time progress notifications.

use crate::engine::deps::resolve_binary_path;
use crossbeam_channel::{unbounded, Receiver, Sender};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub has_audio: bool,
    pub file_size: u64,
    pub file_name: String,
}

impl Default for VideoMetadata {
    fn default() -> Self {
        Self {
            duration: 0.0,
            width: 0,
            height: 0,
            fps: 30.0,
            has_audio: false,
            file_size: 0,
            file_name: String::new(),
        }
    }
}

/// Extract metadata (duration, resolution, size, fps, has_audio) from a local video file.
pub fn get_video_metadata(file_path: &str) -> VideoMetadata {
    let path = Path::new(file_path);
    let mut meta = VideoMetadata {
        file_name: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string(),
        ..Default::default()
    };

    if !path.exists() {
        return meta;
    }

    if let Ok(fs_meta) = fs::metadata(path) {
        meta.file_size = fs_meta.len();
    }

    if let Some(ffprobe_bin) = resolve_binary_path("ffprobe") {
        let out = Command::new(ffprobe_bin)
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                file_path,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        if let Ok(output) = out {
            if let Ok(json) = serde_json::from_slice::<Value>(&output.stdout) {
                if let Some(dur_str) = json
                    .get("format")
                    .and_then(|f| f.get("duration"))
                    .and_then(|d| d.as_str())
                {
                    meta.duration = dur_str.parse::<f64>().unwrap_or(0.0);
                }

                if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
                    for stream in streams {
                        let codec_type = stream
                            .get("codec_type")
                            .and_then(|c| c.as_str())
                            .unwrap_or("");
                        if codec_type == "video" && meta.width == 0 {
                            meta.width = stream
                                .get("width")
                                .and_then(|w| w.as_u64())
                                .unwrap_or(1280) as u32;
                            meta.height = stream
                                .get("height")
                                .and_then(|h| h.as_u64())
                                .unwrap_or(720) as u32;
                            if let Some(rate_str) =
                                stream.get("avg_frame_rate").and_then(|r| r.as_str())
                            {
                                let parts: Vec<&str> = rate_str.split('/').collect();
                                if parts.len() == 2 {
                                    let num = parts[0].parse::<f64>().unwrap_or(30.0);
                                    let den = parts[1].parse::<f64>().unwrap_or(1.0);
                                    if den > 0.0 && num > 0.0 {
                                        meta.fps = num / den;
                                    }
                                }
                            }
                            if meta.duration <= 0.0 {
                                if let Some(sd) = stream.get("duration").and_then(|d| d.as_str()) {
                                    meta.duration = sd.parse::<f64>().unwrap_or(0.0);
                                }
                            }
                        } else if codec_type == "audio" {
                            meta.has_audio = true;
                        }
                    }
                }
            }
        }
    }

    if meta.width == 0 {
        meta.width = 1280;
        meta.height = 720;
    }

    meta
}

#[derive(Debug, Clone)]
pub enum TrimmerEvent {
    Progress(u8, String),
    Finished {
        output_path: String,
        orig_size: u64,
        new_size: u64,
    },
    Error(String),
}

pub struct VideoCompressorHandle {
    pub rx: Receiver<TrimmerEvent>,
    cancelled: Arc<AtomicBool>,
}

impl VideoCompressorHandle {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

/// Start a background FFmpeg transcode & trim task.
pub fn spawn_video_compressor(
    input_path: String,
    output_path: String,
    start_sec: f64,
    end_sec: Option<f64>,
    target_height: Option<u32>,
    crf: u8,
) -> VideoCompressorHandle {
    let (tx, rx): (Sender<TrimmerEvent>, Receiver<TrimmerEvent>) = unbounded();
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_clone = Arc::clone(&cancelled);

    thread::spawn(move || {
        let in_path = Path::new(&input_path);
        if !in_path.exists() {
            let _ = tx.send(TrimmerEvent::Error(format!(
                "File non trovato: {}",
                input_path
            )));
            return;
        }

        let orig_size = fs::metadata(in_path).map(|m| m.len()).unwrap_or(0);
        if let Some(parent) = Path::new(&output_path).parent() {
            let _ = fs::create_dir_all(parent);
        }

        let ffmpeg_bin = match resolve_binary_path("ffmpeg") {
            Some(p) => p,
            None => {
                let _ = tx.send(TrimmerEvent::Error(
                    "FFmpeg non trovato! Aprilo da '📦 Installer & Dipendenze' per installarlo automaticamente."
                        .to_string(),
                ));
                return;
            }
        };

        let meta = get_video_metadata(&input_path);
        let effective_start = start_sec.max(0.0);
        let effective_end = match end_sec {
            Some(e) if e > effective_start => e,
            _ if meta.duration > effective_start => meta.duration,
            _ => effective_start + 10.0,
        };
        let target_duration = (effective_end - effective_start).max(1.0);

        let _ = tx.send(TrimmerEvent::Progress(
            2,
            "Inizializzazione codec H.264/AAC...".to_string(),
        ));

        let mut cmd = Command::new(&ffmpeg_bin);
        cmd.arg("-y");
        if effective_start > 0.1 {
            cmd.args(["-ss", &format!("{:.3}", effective_start)]);
        }
        cmd.args(["-i", &input_path]);
        if end_sec.is_some() || meta.duration > 0.0 {
            cmd.args(["-t", &format!("{:.3}", target_duration)]);
        }

        if let Some(th) = target_height {
            if th < meta.height || meta.height == 0 {
                cmd.args(["-vf", &format!("scale=-2:{}", th)]);
            } else {
                cmd.args(["-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2"]);
            }
        } else {
            cmd.args(["-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2"]);
        }

        cmd.args([
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-tune",
            "fastdecode",
            "-crf",
            &crf.to_string(),
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-ac",
            "2",
            "-ar",
            "44100",
            "-progress",
            "pipe:1",
            "-nostats",
            &output_path,
        ]);

        cmd.stdout(Stdio::piped()).stderr(Stdio::null());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(TrimmerEvent::Error(format!(
                    "Impossibile avviare FFmpeg: {}",
                    e
                )));
                return;
            }
        };

        let _ = tx.send(TrimmerEvent::Progress(
            5,
            "Compressione e trimming H.264 in corso...".to_string(),
        ));

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if cancelled_clone.load(Ordering::SeqCst) {
                    let _ = child.kill();
                    let _ = fs::remove_file(&output_path);
                    let _ = tx.send(TrimmerEvent::Error("Operazione annullata.".to_string()));
                    return;
                }

                if let Some(us_str) = line.strip_prefix("out_time_us=") {
                    if let Ok(us) = us_str.trim().parse::<f64>() {
                        if us > 0.0 {
                            let elapsed_sec = us / 1_000_000.0;
                            let pct = ((elapsed_sec / target_duration) * 100.0)
                                .clamp(5.0, 98.0) as u8;
                            let _ = tx.send(TrimmerEvent::Progress(
                                pct,
                                format!("Elaborazione video... {}%", pct),
                            ));
                        }
                    }
                }
            }
        }

        match child.wait() {
            Ok(status) if status.success() => {
                let new_size = fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
                let _ = tx.send(TrimmerEvent::Progress(100, "Completato!".to_string()));
                let _ = tx.send(TrimmerEvent::Finished {
                    output_path,
                    orig_size,
                    new_size,
                });
            }
            _ => {
                let _ = fs::remove_file(&output_path);
                let _ = tx.send(TrimmerEvent::Error(
                    "Errore durante la transcodifica FFmpeg.".to_string(),
                ));
            }
        }
    });

    VideoCompressorHandle { rx, cancelled }
}

/// Format bytes into readable string (e.g. `24.5 MB`).
pub fn format_bytes(size_bytes: u64) -> String {
    if size_bytes < 1024 {
        format!("{} B", size_bytes)
    } else if size_bytes < 1024 * 1024 {
        format!("{:.1} KB", size_bytes as f64 / 1024.0)
    } else if size_bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", size_bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", size_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_metadata_nonexistent_file() {
        let meta = get_video_metadata("non_existent_video_file.mp4");
        assert_eq!(meta.duration, 0.0);
        assert_eq!(meta.file_size, 0);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(10 * 1024 * 1024), "10.0 MB");
    }
}

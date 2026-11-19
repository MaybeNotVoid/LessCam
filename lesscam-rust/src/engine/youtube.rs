//! LessCam - YouTube Stream Extraction Engine (Rust Edition)
//! Handles fast asynchronous extraction of direct video and audio stream URLs using yt-dlp.

use crate::engine::deps::resolve_binary_path;
use crossbeam_channel::{unbounded, Receiver, Sender};
use regex::Regex;
use serde_json::Value;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

#[derive(Debug, Clone)]
pub struct YouTubeStreamMetadata {
    pub title: String,
    pub duration: f64,
    pub video_url: String,
    pub audio_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub height: u32,
    pub is_live: bool,
}

#[derive(Debug, Clone)]
pub enum YouTubeExtractResult {
    Success(YouTubeStreamMetadata),
    Error(String),
}

/// Validate if the string looks like a supported YouTube URL.
pub fn is_valid_youtube_url(url: &str) -> bool {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return false;
    }

    let patterns = [
        r"(?i)^(https?://)?(www\.|m\.)?youtube\.com/watch\?v=[\w-]+",
        r"(?i)^(https?://)?(www\.)?youtu\.be/[\w-]+",
        r"(?i)^(https?://)?(www\.|m\.)?youtube\.com/shorts/[\w-]+",
        r"(?i)^(https?://)?(www\.|m\.)?youtube\.com/live/[\w-]+",
        r"(?i)^(https?://)?(www\.|m\.)?youtube\.com/embed/[\w-]+",
    ];

    patterns.iter().any(|pat| {
        Regex::new(pat)
            .map(|re| re.is_match(trimmed))
            .unwrap_or(false)
    })
}

pub struct YouTubeExtractorHandle {
    pub rx: Receiver<YouTubeExtractResult>,
    cancelled: Arc<AtomicBool>,
}

impl YouTubeExtractorHandle {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

/// Spawn a background thread that runs `yt-dlp -J` and selects optimal video/audio streams.
pub fn spawn_youtube_extractor(url: String, target_resolution: u32) -> YouTubeExtractorHandle {
    let (tx, rx): (Sender<YouTubeExtractResult>, Receiver<YouTubeExtractResult>) = unbounded();
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_clone = Arc::clone(&cancelled);

    thread::spawn(move || {
        let clean_url = url.trim().to_string();
        if !is_valid_youtube_url(&clean_url) {
            let _ = tx.send(YouTubeExtractResult::Error(
                "URL YouTube non valido o non supportato.".to_string(),
            ));
            return;
        }

        let ytdlp_bin = match resolve_binary_path("yt-dlp") {
            Some(p) => p,
            None => {
                let _ = tx.send(YouTubeExtractResult::Error(
                    "yt-dlp non trovato! Clicca su '📦 Installer & Dipendenze' in alto per installarlo con 1 click."
                        .to_string(),
                ));
                return;
            }
        };

        if cancelled_clone.load(Ordering::SeqCst) {
            return;
        }

        let output = Command::new(&ytdlp_bin)
            .args([
                "--dump-json",
                "--no-playlist",
                "--no-warnings",
                "--skip-download",
                &clean_url,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        if cancelled_clone.load(Ordering::SeqCst) {
            return;
        }

        match output {
            Ok(out) => {
                if !out.status.success() {
                    let err_str = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    let _ = tx.send(YouTubeExtractResult::Error(format!(
                        "Errore yt-dlp: {}",
                        if err_str.is_empty() {
                            "Impossibile estrarre lo stream YouTube."
                        } else {
                            &err_str
                        }
                    )));
                    return;
                }

                let json_str = String::from_utf8_lossy(&out.stdout);
                match parse_ytdlp_json(&json_str, target_resolution) {
                    Ok(meta) => {
                        if !cancelled_clone.load(Ordering::SeqCst) {
                            let _ = tx.send(YouTubeExtractResult::Success(meta));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(YouTubeExtractResult::Error(e));
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(YouTubeExtractResult::Error(format!(
                    "Errore avvio yt-dlp: {}",
                    e
                )));
            }
        }
    });

    YouTubeExtractorHandle { rx, cancelled }
}

/// Parse yt-dlp JSON output and score video/audio formats matching the target resolution.
pub fn parse_ytdlp_json(json_str: &str, target_resolution: u32) -> Result<YouTubeStreamMetadata, String> {
    let info: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON yt-dlp non valido: {}", e))?;

    let formats = info
        .get("formats")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Nessun formato streaming trovato per questo video.".to_string())?;

    #[derive(Clone)]
    struct VideoCandidate {
        score: i64,
        height: u32,
        url: String,
    }

    #[derive(Clone)]
    struct AudioCandidate {
        score: i64,
        url: String,
    }

    #[derive(Clone)]
    struct ProgressiveCandidate {
        height: u32,
        url: String,
    }

    let mut video_candidates = Vec::new();
    let mut audio_candidates = Vec::new();
    let mut progressive_candidates = Vec::new();

    for f in formats {
        let url = match f.get("url").and_then(|u| u.as_str()) {
            Some(u) if u.starts_with("http://") || u.starts_with("https://") => u.to_string(),
            _ => continue,
        };

        let vcodec = f.get("vcodec").and_then(|v| v.as_str()).unwrap_or("none");
        let acodec = f.get("acodec").and_then(|v| v.as_str()).unwrap_or("none");
        let height = f.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let fps = f.get("fps").and_then(|v| v.as_f64()).unwrap_or(30.0) as i64;
        let ext = f.get("ext").and_then(|v| v.as_str()).unwrap_or("");

        // 1. Progressive stream (both video and audio)
        if vcodec != "none" && acodec != "none" && height > 0 && height <= target_resolution {
            progressive_candidates.push(ProgressiveCandidate {
                height,
                url: url.clone(),
            });
        }

        // 2. Adaptive Video-only
        if vcodec != "none" && acodec == "none" && height > 0 && height <= target_resolution {
            let mut score = (height as i64) * 10 + fps;
            if vcodec.contains("avc") || ext == "mp4" {
                score += 1000;
            }
            video_candidates.push(VideoCandidate {
                score,
                height,
                url: url.clone(),
            });
        }

        // 3. Adaptive Audio-only
        if acodec != "none" && vcodec == "none" {
            let abr = f.get("abr").and_then(|v| v.as_f64()).unwrap_or(0.0) as i64;
            let mut score = abr;
            if ext == "m4a" || ext == "aac" {
                score += 500;
            }
            audio_candidates.push(AudioCandidate { score, url });
        }
    }

    video_candidates.sort_by(|a, b| b.score.cmp(&a.score));
    audio_candidates.sort_by(|a, b| b.score.cmp(&a.score));
    progressive_candidates.sort_by(|a, b| b.height.cmp(&a.height));

    let (selected_video_url, selected_audio_url, res_actual) =
        if !video_candidates.is_empty() && !audio_candidates.is_empty() {
            (
                video_candidates[0].url.clone(),
                Some(audio_candidates[0].url.clone()),
                video_candidates[0].height,
            )
        } else if !progressive_candidates.is_empty() {
            (
                progressive_candidates[0].url.clone(),
                Some(progressive_candidates[0].url.clone()),
                progressive_candidates[0].height,
            )
        } else if !video_candidates.is_empty() {
            (
                video_candidates[0].url.clone(),
                None,
                video_candidates[0].height,
            )
        } else {
            return Err("Nessuno stream video compatibile trovato.".to_string());
        };

    let title = info
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("YouTube Video")
        .to_string();
    let duration = info
        .get("duration")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let thumbnail_url = info
        .get("thumbnail")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let is_live = info
        .get("is_live")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(YouTubeStreamMetadata {
        title,
        duration,
        video_url: selected_video_url,
        audio_url: selected_audio_url,
        thumbnail_url,
        height: res_actual,
        is_live,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_youtube_url_validator() {
        let valid_urls = [
            "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
            "http://youtu.be/aqz-KE-bpKQ",
            "https://www.youtube.com/shorts/abcdef12345",
            "https://youtube.com/live/xyz12345",
            "https://www.youtube.com/embed/aqz-KE-bpKQ",
        ];
        let invalid_urls = [
            "https://www.google.com",
            "https://vimeo.com/12345",
            "",
            "not_a_url",
        ];

        for u in valid_urls {
            assert!(is_valid_youtube_url(u), "Expected valid YouTube URL: {}", u);
        }
        for u in invalid_urls {
            assert!(
                !is_valid_youtube_url(u),
                "Expected invalid YouTube URL: {}",
                u
            );
        }
    }

    #[test]
    fn test_parse_ytdlp_json_scoring() {
        let sample_json = r#"{
            "title": "Big Buck Bunny 60fps",
            "duration": 634.5,
            "is_live": false,
            "formats": [
                {
                    "url": "https://cdn.example.com/v1080.mp4",
                    "vcodec": "avc1.64002a",
                    "acodec": "none",
                    "height": 1080,
                    "fps": 60,
                    "ext": "mp4"
                },
                {
                    "url": "https://cdn.example.com/v720.mp4",
                    "vcodec": "avc1.4d401f",
                    "acodec": "none",
                    "height": 720,
                    "fps": 30,
                    "ext": "mp4"
                },
                {
                    "url": "https://cdn.example.com/audio.m4a",
                    "vcodec": "none",
                    "acodec": "mp4a.40.2",
                    "abr": 128.0,
                    "ext": "m4a"
                }
            ]
        }"#;

        let meta_720 = parse_ytdlp_json(sample_json, 720).expect("Should parse 720p");
        assert_eq!(meta_720.height, 720);
        assert_eq!(meta_720.video_url, "https://cdn.example.com/v720.mp4");
        assert_eq!(
            meta_720.audio_url.as_deref(),
            Some("https://cdn.example.com/audio.m4a")
        );
    }
}

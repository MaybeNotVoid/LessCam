//! LessCam - Video Trimming & H.264 Compression Modal (Rust Edition)
//! Allows users to trim start/end timestamps and compress local video files
//! before loading them into the streaming engine.

use crate::engine::video_trimmer::{
    format_bytes, get_video_metadata, spawn_video_compressor, TrimmerEvent, VideoCompressorHandle,
    VideoMetadata,
};
use crate::gui::theme::{
    inner_card, ACCENT_DANGER, ACCENT_PRIMARY, ACCENT_SUCCESS, TEXT_PRIMARY, TEXT_SECONDARY,
};
use egui::{Context, RichText, Window};
use std::env;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub enum TrimModalOutcome {
    LoadVideo {
        final_path: String,
        compression_stats: Option<(u64, u64)>, // (orig_size, new_size)
    },
    Cancelled,
}

pub struct VideoTrimModal {
    pub file_path: String,
    pub meta: VideoMetadata,
    pub enable_trim: bool,
    pub start_sec: f64,
    pub end_sec: f64,
    pub enable_compress: bool,
    pub res_choice_idx: usize, // 0 = 720p, 1 = 1080p, 2 = Original
    pub qual_choice_idx: usize, // 0 = Balanced (CRF 26), 1 = Ultra-light (CRF 30), 2 = High (CRF 22)
    pub progress_pct: u8,
    pub status_text: String,
    pub error_text: Option<String>,
    worker: Option<VideoCompressorHandle>,
}

impl VideoTrimModal {
    pub fn new(file_path: String) -> Self {
        let meta = get_video_metadata(&file_path);
        let dur = if meta.duration > 0.5 {
            meta.duration
        } else {
            60.0
        };

        Self {
            file_path,
            meta,
            enable_trim: false,
            start_sec: 0.0,
            end_sec: dur,
            enable_compress: true,
            res_choice_idx: 0,
            qual_choice_idx: 0,
            progress_pct: 0,
            status_text: String::new(),
            error_text: None,
            worker: None,
        }
    }

    pub fn show(&mut self, ctx: &Context) -> Option<TrimModalOutcome> {
        let mut outcome: Option<TrimModalOutcome> = None;

        // Poll background worker events
        if let Some(ref handle) = self.worker {
            while let Ok(ev) = handle.rx.try_recv() {
                match ev {
                    TrimmerEvent::Progress(pct, msg) => {
                        self.progress_pct = pct;
                        self.status_text = msg;
                    }
                    TrimmerEvent::Finished {
                        output_path,
                        orig_size,
                        new_size,
                    } => {
                        outcome = Some(TrimModalOutcome::LoadVideo {
                            final_path: output_path,
                            compression_stats: Some((orig_size, new_size)),
                        });
                    }
                    TrimmerEvent::Error(err) => {
                        self.error_text = Some(err);
                    }
                }
            }
        }

        if outcome.is_some() {
            self.worker = None;
            return outcome;
        }

        let is_processing = self.worker.is_some() && self.error_text.is_none();

        Window::new("✂️ Ottimizzazione, Trimming & Compressione Video")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([540.0, 420.0])
            .show(ctx, |ui| {
                // File summary card
                inner_card(ui, |ui| {
                    ui.label(
                        RichText::new(format!("📄 File: {}", self.meta.file_name))
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Durata: {:.1}s", self.meta.duration))
                                .color(TEXT_SECONDARY),
                        );
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "Risoluzione: {}x{}",
                                self.meta.width, self.meta.height
                            ))
                            .color(TEXT_SECONDARY),
                        );
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "Peso Originale: {}",
                                format_bytes(self.meta.file_size)
                            ))
                            .strong()
                            .color(ACCENT_PRIMARY),
                        );
                    });
                });

                ui.add_space(6.0);

                // 1. Trimming Section
                ui.add_enabled_ui(!is_processing, |ui| {
                    ui.checkbox(
                        &mut self.enable_trim,
                        RichText::new("Ritaglia il video (seleziona inizio e fine)").strong(),
                    );

                    if self.enable_trim {
                        let max_dur = self.meta.duration.max(10.0);
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label("Inizio (sec):");
                            ui.add(
                                egui::DragValue::new(&mut self.start_sec)
                                    .speed(0.5)
                                    .range(0.0..=(self.end_sec - 0.5).max(0.0)),
                            );
                            ui.add_space(12.0);
                            ui.label("Fine (sec):");
                            ui.add(
                                egui::DragValue::new(&mut self.end_sec)
                                    .speed(0.5)
                                    .range((self.start_sec + 0.5)..=max_dur),
                            );
                            let final_dur = (self.end_sec - self.start_sec).max(0.0);
                            ui.label(
                                RichText::new(format!("➔ {:.1}s", final_dur))
                                    .strong()
                                    .color(ACCENT_SUCCESS),
                            );
                        });
                    }

                    ui.add_space(6.0);

                    // 2. Compression Section
                    ui.checkbox(
                        &mut self.enable_compress,
                        RichText::new("Comprimi il video in H.264/AAC per ridurne il peso (Consigliato)")
                            .strong(),
                    );

                    if self.enable_compress {
                        let res_options = [
                            "720p HD (1280x720) - Ideale per streaming fluido",
                            "1080p Full HD (1920x1080) - Alta Definizione",
                            "Mantieni Risoluzione Originale",
                        ];
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label("Risoluzione Output:");
                            egui::ComboBox::from_id_salt("trim_res_combo")
                                .selected_text(res_options[self.res_choice_idx])
                                .width(320.0)
                                .show_ui(ui, |ui| {
                                    for (idx, label) in res_options.iter().enumerate() {
                                        ui.selectable_value(&mut self.res_choice_idx, idx, *label);
                                    }
                                });
                        });

                        let qual_options = [
                            "Bilanciata (CRF 26 - Ottima qualità, peso ridotto)",
                            "Massima Compressione (CRF 30 - File ultraleggero)",
                            "Alta Qualità (CRF 22 - Minima compressione)",
                        ];
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label("Qualità H.264:          ");
                            egui::ComboBox::from_id_salt("trim_qual_combo")
                                .selected_text(qual_options[self.qual_choice_idx])
                                .width(320.0)
                                .show_ui(ui, |ui| {
                                    for (idx, label) in qual_options.iter().enumerate() {
                                        ui.selectable_value(&mut self.qual_choice_idx, idx, *label);
                                    }
                                });
                        });
                    }
                });

                // 3. Progress or Error feedback
                if is_processing {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(&self.status_text)
                            .strong()
                            .color(ACCENT_PRIMARY),
                    );
                    ui.add(
                        egui::ProgressBar::new((self.progress_pct as f32) / 100.0)
                            .show_percentage()
                            .animate(true),
                    );
                }

                if let Some(ref err) = self.error_text {
                    ui.add_space(8.0);
                    ui.label(RichText::new(format!("❌ {}", err)).color(ACCENT_DANGER));
                }

                ui.add_space(14.0);
                ui.separator();

                // 4. Bottom Action Buttons
                ui.horizontal(|ui| {
                    if ui
                        .button(if is_processing {
                            "✖ Interrompi"
                        } else {
                            "Annulla"
                        })
                        .clicked()
                    {
                        if let Some(ref h) = self.worker {
                            h.cancel();
                        }
                        outcome = Some(TrimModalOutcome::Cancelled);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can_start = !is_processing;

                        if ui
                            .add_enabled(
                                can_start,
                                egui::Button::new(
                                    RichText::new("✨ Elabora & Carica")
                                        .strong()
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(ACCENT_PRIMARY),
                            )
                            .clicked()
                        {
                            if !self.enable_trim && !self.enable_compress {
                                outcome = Some(TrimModalOutcome::LoadVideo {
                                    final_path: self.file_path.clone(),
                                    compression_stats: None,
                                });
                            } else {
                                self.start_compression_job();
                            }
                        }

                        if ui
                            .add_enabled(
                                can_start,
                                egui::Button::new("▶ Carica Originale Diretto"),
                            )
                            .clicked()
                        {
                            outcome = Some(TrimModalOutcome::LoadVideo {
                                final_path: self.file_path.clone(),
                                compression_stats: None,
                            });
                        }
                    });
                });
            });

        outcome
    }

    fn start_compression_job(&mut self) {
        self.error_text = None;
        self.progress_pct = 1;
        self.status_text = "Avvio elaborazione FFmpeg...".to_string();

        let start_s = if self.enable_trim {
            self.start_sec
        } else {
            0.0
        };
        let end_s = if self.enable_trim {
            Some(self.end_sec)
        } else {
            None
        };
        let target_h = if self.enable_compress {
            match self.res_choice_idx {
                0 => Some(720),
                1 => Some(1080),
                _ => None,
            }
        } else {
            None
        };
        let crf = if self.enable_compress {
            match self.qual_choice_idx {
                0 => 26,
                1 => 30,
                _ => 22,
            }
        } else {
            23
        };

        let cache_dir = env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("cache")
            .join("trimmed");
        let _ = std::fs::create_dir_all(&cache_dir);

        let stem = Path::new(&self.file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("video");
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let out_path = cache_dir.join(format!("{}_opt_{}.mp4", stem, ts));

        self.worker = Some(spawn_video_compressor(
            self.file_path.clone(),
            out_path.to_string_lossy().to_string(),
            start_s,
            end_s,
            target_h,
            crf,
        ));
    }
}

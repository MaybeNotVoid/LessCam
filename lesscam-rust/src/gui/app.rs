//! LessCam - Main Application Window & Controller (Rust Edition)
//! Coordinates the GPU-accelerated UI, video streaming engine, audio mixer,
//! YouTube extractor, video trimmer modal, mouse automation bot, and dependency installer.

use crate::engine::{
    audio_stream::{format_gain_db, get_audio_output_devices, AudioDeviceInfo, AudioEngine},
    deps::DependencyManager,
    mouse_automation::{
        get_open_windows, MouseAutomationEngine, MouseBotSettings, TemplateTarget, WindowInfo,
    },
    video_stream::VideoStreamEngine,
    video_trimmer::format_bytes,
    youtube::{spawn_youtube_extractor, YouTubeExtractResult, YouTubeExtractorHandle},
};
use crate::gui::{
    deps_modal::DependencyInstallerModal,
    preview::VideoPreviewCanvas,
    theme::{
        apply_modern_studio_theme, card_header, inner_card, segmented_tabs, status_pill,
        studio_card, toggle_switch, ACCENT_DANGER, ACCENT_PRIMARY, ACCENT_SUCCESS, ACCENT_WARNING,
        BG_DEEP, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
    },
    trim_modal::{TrimModalOutcome, VideoTrimModal},
    vu_meter::StereoVuMeter,
};
use egui::{
    Color32, ColorImage, Context, Frame, Key, Margin, Modifiers, RichText, Rounding,
    ScrollArea, TextureHandle, TextureOptions, Ui, Vec2, Window,
};
use std::time::{Duration, Instant};

/// Format seconds into `MM:SS` or `HH:MM:SS`.
pub fn format_timestamp(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 || seconds >= 360000.0 {
        return "00:00".to_string();
    }
    let total_sec = seconds as u64;
    let hours = total_sec / 3600;
    let minutes = (total_sec % 3600) / 60;
    let secs = total_sec % 60;
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    }
}

struct ImageSlotUiState {
    target: Option<TemplateTarget>,
    thumb_tex: Option<TextureHandle>,
    scroll_enabled: bool,
}

impl Default for ImageSlotUiState {
    fn default() -> Self {
        Self {
            target: None,
            thumb_tex: None,
            scroll_enabled: true,
        }
    }
}

struct ToastBanner {
    title: String,
    message: String,
    color: Color32,
    created_at: Instant,
    duration: Duration,
}

pub struct LessCamApp {
    // Core Engines
    video_engine: VideoStreamEngine,
    audio_engine: AudioEngine,
    mouse_engine: MouseAutomationEngine,
    dep_manager: DependencyManager,
    yt_extractor: Option<YouTubeExtractorHandle>,

    // Preview & Meters
    preview_canvas: VideoPreviewCanvas,
    vu_meter: StereoVuMeter,

    // Modals & Popups
    trim_modal: Option<VideoTrimModal>,
    deps_modal: DependencyInstallerModal,
    show_prereq_guide: bool,
    toast: Option<ToastBanner>,

    // Source Panel State
    source_tab_idx: usize, // 0 = YouTube, 1 = Local Video, 2 = Static Image
    yt_url_input: String,
    yt_res_choice_idx: usize, // 0 = 1080p, 1 = 720p, 2 = 480p
    yt_status_text: String,
    yt_status_is_err: bool,
    local_video_label: String,
    local_image_label: String,
    scrub_value: f64,
    is_scrubbing: bool,
    loop_enabled: bool,

    // Right Column State
    right_tab_idx: usize, // 0 = Regia & Audio, 1 = Automazione Mouse

    // Camera Panel State
    vcam_switch: bool,
    cam_res_choice_idx: usize, // 0 = 720p, 1 = 1080p
    cam_fps_choice_idx: usize, // 0 = 30 FPS, 1 = 60 FPS

    // Audio Panel State
    audio_devices: Vec<AudioDeviceInfo>,
    selected_audio_dev_pos: usize,
    volume_pct: u32,
    is_muted: bool,

    // Mouse Bot Panel State
    open_windows: Vec<WindowInfo>,
    selected_win_pos: usize, // 0 = Full Desktop, 1.. = Window
    periodic_move_enabled: bool,
    move_interval_sec: u32,
    slots: [ImageSlotUiState; 4],
    precision_pct: u32,
    enable_post_scroll: bool,
    scroll_steps: i32,
    scroll_delay_sec: f64,
}

impl LessCamApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_modern_studio_theme(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let audio_devices = get_audio_output_devices();
        let audio_dev_names: Vec<String> = audio_devices.iter().map(|d| d.name.clone()).collect();
        let selected_audio_dev_pos = audio_devices
            .iter()
            .position(|d| d.is_virtual)
            .or_else(|| audio_devices.iter().position(|d| d.is_default))
            .unwrap_or(0);

        let video_engine = VideoStreamEngine::new(1280, 720, 30);
        let audio_engine = AudioEngine::new();
        if let Some(dev) = audio_devices.get(selected_audio_dev_pos) {
            audio_engine.set_output_device(dev.index);
        }

        let mouse_engine = MouseAutomationEngine::new();
        let dep_manager = DependencyManager::new(audio_dev_names);
        let open_windows = get_open_windows();

        Self {
            video_engine,
            audio_engine,
            mouse_engine,
            dep_manager,
            yt_extractor: None,
            preview_canvas: VideoPreviewCanvas::new(),
            vu_meter: StereoVuMeter::new(),
            trim_modal: None,
            deps_modal: DependencyInstallerModal::default(),
            show_prereq_guide: false,
            toast: None,
            source_tab_idx: 0,
            yt_url_input: String::new(),
            yt_res_choice_idx: 0,
            yt_status_text: "Nessun video YouTube caricato.".to_string(),
            yt_status_is_err: false,
            local_video_label: "Nessun file selezionato (MP4, MKV, AVI, MOV, WEBM)".to_string(),
            local_image_label: "Nessuna immagine selezionata (PNG, JPG, WEBP, BMP)".to_string(),
            scrub_value: 0.0,
            is_scrubbing: false,
            loop_enabled: true,
            right_tab_idx: 0,
            vcam_switch: false,
            cam_res_choice_idx: 0,
            cam_fps_choice_idx: 0,
            audio_devices,
            selected_audio_dev_pos,
            volume_pct: 100,
            is_muted: false,
            open_windows,
            selected_win_pos: 0,
            periodic_move_enabled: true,
            move_interval_sec: 10,
            slots: Default::default(),
            precision_pct: 85,
            enable_post_scroll: true,
            scroll_steps: 2,
            scroll_delay_sec: 0.20,
        }
    }

    fn push_toast(&mut self, title: impl Into<String>, message: impl Into<String>, color: Color32) {
        self.toast = Some(ToastBanner {
            title: title.into(),
            message: message.into(),
            color,
            created_at: Instant::now(),
            duration: Duration::from_millis(4500),
        });
    }

    fn sync_mouse_settings(&self) {
        let target_hwnd = if self.selected_win_pos == 0 {
            0
        } else {
            self.open_windows
                .get(self.selected_win_pos - 1)
                .map(|w| w.hwnd)
                .unwrap_or(0)
        };

        let mut templates = Vec::new();
        for slot in &self.slots {
            if let Some(ref t) = slot.target {
                let mut cloned = t.clone();
                cloned.scroll_enabled = slot.scroll_enabled;
                templates.push(cloned);
            }
        }

        self.mouse_engine.update_settings(MouseBotSettings {
            target_hwnd,
            move_interval_sec: self.move_interval_sec as f64,
            periodic_move_enabled: self.periodic_move_enabled,
            templates,
            enable_scroll: self.enable_post_scroll,
            scroll_steps: self.scroll_steps,
            scroll_delay_sec: self.scroll_delay_sec,
            threshold: (self.precision_pct as f32) / 100.0,
            cooldown_sec: 1.8,
        });
    }

    fn poll_background_engines(&mut self, ctx: &Context) {
        // 1. Poll latest video frame
        if let Some(packet) = self.video_engine.take_latest_frame() {
            self.preview_canvas.update_frame(ctx, &packet);
        }

        let vstate = self.video_engine.state();
        self.preview_canvas.is_live = vstate.vcam_active;
        self.preview_canvas.source_title = vstate.source_title.clone();
        self.vcam_switch = vstate.vcam_active;

        let res_name = if vstate.target_h == 1080 {
            "1080p"
        } else {
            "720p"
        };
        self.preview_canvas.resolution_text = format!("{} | {} FPS", res_name, vstate.target_fps);

        if !self.is_scrubbing && vstate.duration_sec > 0.0 {
            self.scrub_value = (vstate.current_pos_sec / vstate.duration_sec).clamp(0.0, 1.0);
        }

        if let Some(err) = vstate.last_error {
            self.push_toast("Avviso Motore Video", err, ACCENT_WARNING);
            self.video_engine.clear_error();
        }

        // 2. Poll YouTube Extractor if active
        let mut yt_done = false;
        if let Some(ref handle) = self.yt_extractor {
            if let Ok(res) = handle.rx.try_recv() {
                yt_done = true;
                self.preview_canvas.is_buffering = false;
                match res {
                    YouTubeExtractResult::Success(meta) => {
                        self.yt_status_text =
                            format!("Pronto: {} ({}p)", meta.title, meta.height);
                        self.yt_status_is_err = false;
                        self.video_engine.load_youtube_video(
                            meta.video_url,
                            meta.duration,
                            meta.title.clone(),
                        );
                        self.audio_engine.load_source(meta.audio_url);
                        self.audio_engine.play();
                        self.push_toast(
                            "Stream YouTube Caricato",
                            format!("Riproduzione di: {}", meta.title),
                            ACCENT_SUCCESS,
                        );
                    }
                    YouTubeExtractResult::Error(err) => {
                        self.yt_status_text = err.clone();
                        self.yt_status_is_err = true;
                        self.push_toast("Errore YouTube", err, ACCENT_DANGER);
                    }
                }
            }
        }
        if yt_done {
            self.yt_extractor = None;
        }

        // 3. Poll Audio VU-Meter levels
        let (l, r) = self.audio_engine.get_meter_levels();
        self.vu_meter.update_levels(l, r);

        // 4. Check in-app keyboard shortcuts (CTRL+ALT+A / CTRL+ALT+D)
        let ctrl_alt = Modifiers {
            alt: true,
            ctrl: true,
            shift: false,
            mac_cmd: false,
            command: false,
        };
        if ctx.input_mut(|i| i.consume_key(ctrl_alt, Key::A)) {
            self.sync_mouse_settings();
            self.mouse_engine.start_bot();
        }
        if ctx.input_mut(|i| i.consume_key(ctrl_alt, Key::D)) {
            self.mouse_engine.stop_bot();
        }
    }

    fn render_top_header(&mut self, ui: &mut Ui) {
        let dep_snap = self.dep_manager.snapshot();

        ui.horizontal(|ui| {
            ui.label(RichText::new("🎥").size(22.0));
            ui.label(
                RichText::new("LessCam")
                    .size(21.0)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            status_pill(ui, "RUST STUDIO 2.0", ACCENT_PRIMARY);

            ui.add_space(8.0);

            // Real-time external dependencies status pills
            Self::mini_dep_indicator(ui, "FFmpeg", dep_snap.ffmpeg.is_installed());
            Self::mini_dep_indicator(ui, "yt-dlp", dep_snap.ytdlp.is_installed());
            Self::mini_dep_indicator(ui, "VB-Cable", dep_snap.vbcable.is_installed());
            Self::mini_dep_indicator(ui, "VirtualCam", dep_snap.vcam.is_installed());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("❓ Guida OBS & Cable").clicked() {
                    self.show_prereq_guide = true;
                }

                let missing_any = !dep_snap.ffmpeg.is_installed()
                    || !dep_snap.ytdlp.is_installed()
                    || !dep_snap.vbcable.is_installed()
                    || !dep_snap.vcam.is_installed();

                let btn_color = if missing_any {
                    ACCENT_PRIMARY
                } else {
                    Color32::from_rgb(38, 42, 56)
                };
                let btn_label = if missing_any {
                    "📦 Installer & Programmi Esterni (Configura)"
                } else {
                    "📦 Programmi Esterni (✔ Pronti)"
                };

                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(btn_label).strong().color(Color32::WHITE),
                        )
                        .fill(btn_color),
                    )
                    .clicked()
                {
                    self.deps_modal.is_open = true;
                }
            });
        });
    }

    fn mini_dep_indicator(ui: &mut Ui, name: &str, ok: bool) {
        let (dot, color) = if ok {
            ("●", ACCENT_SUCCESS)
        } else {
            ("○", ACCENT_WARNING)
        };
        ui.label(
            RichText::new(format!("{} {}", dot, name))
                .size(11.5)
                .color(color),
        );
    }

    fn render_source_panel(&mut self, ui: &mut Ui) {
        let vstate = self.video_engine.state();

        studio_card(ui, |ui| {
            card_header(
                ui,
                "🎬",
                "Sorgente Multimediale",
                Some((&format!("In onda: {}", vstate.source_title), ACCENT_PRIMARY)),
            );

            ui.add_space(4.0);

            segmented_tabs(
                ui,
                "source_tabs",
                &[
                    ("▶", "YouTube Stream"),
                    ("📁", "Video Locale"),
                    ("🖼", "Immagine Statica"),
                ],
                &mut self.source_tab_idx,
            );

            ui.add_space(6.0);

            match self.source_tab_idx {
                0 => {
                    // YouTube Page
                    ui.horizontal(|ui| {
                        let res_labels = ["1080p (Full HD)", "720p (HD)", "480p (SD)"];
                        let res_values = [1080u32, 720, 480];

                        let input_w = (ui.available_width() - 255.0).max(160.0);
                        let resp = ui.add_sized(
                            [input_w, 30.0],
                            egui::TextEdit::singleline(&mut self.yt_url_input).hint_text(
                                "Incolla link YouTube (es. https://www.youtube.com/watch?v=...)",
                            ),
                        );

                        egui::ComboBox::from_id_salt("yt_res_combo")
                            .selected_text(res_labels[self.yt_res_choice_idx])
                            .width(120.0)
                            .show_ui(ui, |ui| {
                                for (i, lbl) in res_labels.iter().enumerate() {
                                    ui.selectable_value(&mut self.yt_res_choice_idx, i, *lbl);
                                }
                            });

                        let enter_pressed =
                            resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        let load_clicked = ui
                            .add(
                                egui::Button::new(
                                    RichText::new("▶ Carica & Play")
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(ACCENT_PRIMARY),
                            )
                            .clicked();

                        if (load_clicked || enter_pressed) && !self.yt_url_input.trim().is_empty() {
                            if let Some(ref h) = self.yt_extractor {
                                h.cancel();
                            }
                            let target_res = res_values[self.yt_res_choice_idx];
                            self.yt_status_text =
                                "Connessione ed estrazione stream diretto con yt-dlp...".to_string();
                            self.yt_status_is_err = false;
                            self.preview_canvas.is_buffering = true;
                            self.preview_canvas.buffering_text =
                                "Estrazione stream YouTube...".to_string();
                            self.yt_extractor = Some(spawn_youtube_extractor(
                                self.yt_url_input.clone(),
                                target_res,
                            ));
                        }
                    });

                    let st_color = if self.yt_status_is_err {
                        ACCENT_DANGER
                    } else if self.yt_status_text.starts_with("Pronto:") {
                        ACCENT_SUCCESS
                    } else {
                        TEXT_MUTED
                    };
                    ui.label(RichText::new(&self.yt_status_text).size(12.0).color(st_color));
                }
                1 => {
                    // Local Video Page
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("📂 Seleziona Video Locale...")
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(ACCENT_PRIMARY),
                            )
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter(
                                    "Video Files",
                                    &["mp4", "mkv", "avi", "mov", "webm", "flv"],
                                )
                                .pick_file()
                            {
                                let path_str = path.to_string_lossy().to_string();
                                self.trim_modal = Some(VideoTrimModal::new(path_str));
                            }
                        }

                        ui.label(
                            RichText::new(&self.local_video_label)
                                .size(12.5)
                                .color(TEXT_SECONDARY),
                        );
                    });
                }
                _ => {
                    // Static Image Page
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("🖼 Seleziona Immagine...")
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(ACCENT_PRIMARY),
                            )
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Immagini", &["png", "jpg", "jpeg", "webp", "bmp"])
                                .pick_file()
                            {
                                let path_str = path.to_string_lossy().to_string();
                                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                    self.local_image_label = name.to_string();
                                }
                                self.video_engine.load_image(path_str);
                                self.audio_engine.stop();
                            }
                        }

                        ui.label(
                            RichText::new(&self.local_image_label)
                                .size(12.5)
                                .color(TEXT_SECONDARY),
                        );
                    });
                }
            }

            ui.add_space(6.0);
            ui.separator();

            // Timeline Scrubber Row
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format_timestamp(vstate.current_pos_sec))
                        .monospace()
                        .size(12.0)
                        .color(TEXT_SECONDARY),
                );

                let avail_slider_w = (ui.available_width() - 62.0).max(80.0);
                ui.spacing_mut().slider_width = avail_slider_w;
                let slider_resp = ui.add(
                    egui::Slider::new(&mut self.scrub_value, 0.0..=1.0).show_value(false),
                );

                if slider_resp.drag_started() {
                    self.is_scrubbing = true;
                }
                if slider_resp.drag_stopped()
                    || (slider_resp.changed() && !slider_resp.dragged())
                {
                    self.is_scrubbing = false;
                    if vstate.duration_sec > 0.0 {
                        let target_sec = self.scrub_value * vstate.duration_sec;
                        self.video_engine.seek(target_sec);
                        self.audio_engine.seek(target_sec);
                    }
                }

                ui.label(
                    RichText::new(format_timestamp(vstate.duration_sec))
                        .monospace()
                        .size(12.0)
                        .color(TEXT_SECONDARY),
                );
            });

            // Transport Controls Row
            ui.horizontal(|ui| {
                let play_label = if vstate.is_playing {
                    "⏸ Pausa"
                } else {
                    "▶ Play"
                };
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(play_label).strong().color(Color32::WHITE),
                        )
                        .fill(ACCENT_PRIMARY)
                        .min_size(Vec2::new(96.0, 30.0)),
                    )
                    .clicked()
                {
                    let next_playing = !vstate.is_playing;
                    self.video_engine.toggle_play_pause();
                    if next_playing {
                        self.audio_engine.play();
                    } else {
                        self.audio_engine.pause();
                    }
                }

                if ui
                    .button("⏹ Standby")
                    .on_hover_text("Arresta riproduzione e torna a Standby")
                    .clicked()
                {
                    self.video_engine.clear_source();
                    self.audio_engine.stop();
                }

                let loop_text = if self.loop_enabled {
                    "🔁 Loop: ON"
                } else {
                    "🔁 Loop: OFF"
                };
                if ui
                    .selectable_label(self.loop_enabled, loop_text)
                    .on_hover_text("Riproduzione continua in loop")
                    .clicked()
                {
                    self.loop_enabled = !self.loop_enabled;
                    self.video_engine.set_loop(self.loop_enabled);
                    self.audio_engine.set_loop(self.loop_enabled);
                }
            });
        });
    }

    fn render_camera_panel(&mut self, ui: &mut Ui) {
        let vstate = self.video_engine.state();

        studio_card(ui, |ui| {
            let badge = if vstate.vcam_active {
                ("ATTIVA", ACCENT_SUCCESS)
            } else {
                ("SPENTA", TEXT_MUTED)
            };
            card_header(ui, "📡", "Webcam Virtuale (DirectShow)", Some(badge));

            ui.add_space(4.0);

            // Switch row
            ui.horizontal(|ui| {
                ui.label(RichText::new("Trasmissione Webcam:").color(TEXT_PRIMARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut sw = self.vcam_switch;
                    if toggle_switch(ui, &mut sw).changed() {
                        self.vcam_switch = sw;
                        self.video_engine.set_virtual_cam(sw);
                    }
                });
            });

            // Resolution dropdown
            let res_items = [
                ("1280x720 (720p HD)", (1280u32, 720u32)),
                ("1920x1080 (1080p Full HD)", (1920u32, 1080u32)),
            ];
            ui.horizontal(|ui| {
                ui.label("Risoluzione Output:");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let prev_idx = self.cam_res_choice_idx;
                    egui::ComboBox::from_id_salt("cam_res_combo")
                        .selected_text(res_items[self.cam_res_choice_idx].0)
                        .width(205.0)
                        .show_ui(ui, |ui| {
                            for (i, (label, _)) in res_items.iter().enumerate() {
                                ui.selectable_value(&mut self.cam_res_choice_idx, i, *label);
                            }
                        });
                    if self.cam_res_choice_idx != prev_idx {
                        let (w, h) = res_items[self.cam_res_choice_idx].1;
                        self.video_engine.set_output_resolution(w, h);
                    }
                });
            });

            // Framerate dropdown
            let fps_items = [("30 FPS", 30u32), ("60 FPS", 60u32)];
            ui.horizontal(|ui| {
                ui.label("Frequenza Frame:");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let prev_idx = self.cam_fps_choice_idx;
                    egui::ComboBox::from_id_salt("cam_fps_combo")
                        .selected_text(fps_items[self.cam_fps_choice_idx].0)
                        .width(205.0)
                        .show_ui(ui, |ui| {
                            for (i, (label, _)) in fps_items.iter().enumerate() {
                                ui.selectable_value(&mut self.cam_fps_choice_idx, i, *label);
                            }
                        });
                    if self.cam_fps_choice_idx != prev_idx {
                        self.video_engine
                            .set_target_fps(fps_items[self.cam_fps_choice_idx].1);
                    }
                });
            });

            // Driver status line
            ui.horizontal(|ui| {
                ui.label(RichText::new("Stato Driver:").size(12.0).color(TEXT_MUTED));
                let st_col = if vstate.vcam_active {
                    ACCENT_SUCCESS
                } else {
                    ACCENT_PRIMARY
                };
                ui.label(
                    RichText::new(&vstate.vcam_status_msg)
                        .size(12.0)
                        .strong()
                        .color(st_col),
                );
            });
        });
    }

    fn render_audio_panel(&mut self, ui: &mut Ui) {
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔊").size(17.0).color(ACCENT_PRIMARY));
                ui.label(
                    RichText::new("Mixer Audio & Routing")
                        .size(15.5)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("🔄 Aggiorna")
                        .on_hover_text("Aggiorna elenco periferiche audio")
                        .clicked()
                    {
                        self.audio_devices = get_audio_output_devices();
                        let names: Vec<String> =
                            self.audio_devices.iter().map(|d| d.name.clone()).collect();
                        self.dep_manager.refresh(names);
                    }
                });
            });

            ui.add_space(4.0);

            ui.label(
                RichText::new("Periferica di Output (Virtual Cable):")
                    .size(12.5)
                    .color(TEXT_SECONDARY),
            );

            let current_dev_label = self
                .audio_devices
                .get(self.selected_audio_dev_pos)
                .map(|d| {
                    let prefix = if d.is_virtual { "⭐ " } else { "" };
                    format!("{}{} ({})", prefix, d.name, d.api)
                })
                .unwrap_or_else(|| "Nessuna periferica".to_string());

            let prev_pos = self.selected_audio_dev_pos;
            egui::ComboBox::from_id_salt("audio_dev_combo")
                .selected_text(current_dev_label)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (pos, dev) in self.audio_devices.iter().enumerate() {
                        let prefix = if dev.is_virtual { "⭐ " } else { "" };
                        let label = format!("{}{} ({})", prefix, dev.name, dev.api);
                        ui.selectable_value(&mut self.selected_audio_dev_pos, pos, label);
                    }
                });

            if self.selected_audio_dev_pos != prev_pos {
                if let Some(dev) = self.audio_devices.get(self.selected_audio_dev_pos) {
                    self.audio_engine.set_output_device(dev.index);
                }
            }

            ui.add_space(4.0);

            // Volume & Gain Header
            ui.horizontal(|ui| {
                ui.label("Volume Master / Guadagno:");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.is_muted {
                        status_pill(ui, "MUTATO", ACCENT_DANGER);
                    } else {
                        let gain_str =
                            format!("{}% ({})", self.volume_pct, format_gain_db(self.volume_pct));
                        status_pill(ui, &gain_str, ACCENT_PRIMARY);
                    }
                });
            });

            // Mute + Slider Row
            ui.horizontal(|ui| {
                let mute_icon = if self.is_muted { "🔇" } else { "🔈" };
                if ui
                    .selectable_label(self.is_muted, mute_icon)
                    .on_hover_text("Muta / Riattiva Audio")
                    .clicked()
                {
                    self.is_muted = !self.is_muted;
                    self.audio_engine.set_muted(self.is_muted);
                }

                ui.spacing_mut().slider_width = (ui.available_width() - 10.0).max(80.0);
                if ui
                    .add(
                        egui::Slider::new(&mut self.volume_pct, 0..=200)
                            .show_value(false)
                            .trailing_fill(true),
                    )
                    .changed()
                {
                    self.audio_engine.set_volume(self.volume_pct);
                }
            });

            ui.add_space(2.0);
            ui.label(
                RichText::new("Livello Audio Output (Stereo VU-Meter + Peak Hold):")
                    .size(11.5)
                    .color(TEXT_MUTED),
            );
            self.vu_meter.ui(ui);
        });
    }

    fn render_mouse_panel(&mut self, ctx: &Context, ui: &mut Ui) {
        let bot_status = self.mouse_engine.status();

        studio_card(ui, |ui| {
            let badge = if bot_status.is_running {
                ("ATTIVO", ACCENT_SUCCESS)
            } else {
                ("INATTIVO", TEXT_MUTED)
            };
            card_header(ui, "🖱", "Automazione Mouse (Computer Vision)", Some(badge));

            ui.label(
                RichText::new("⌨ Scorciatoie: CTRL+ALT+A avvia | CTRL+ALT+D arresta")
                    .size(12.0)
                    .strong()
                    .color(ACCENT_PRIMARY),
            );

            // 1. Target Window Dropdown
            ui.horizontal(|ui| {
                ui.label("Finestra:");
                let selected_win_title = if self.selected_win_pos == 0 {
                    "🖥 Schermo Intero (Desktop Primario)".to_string()
                } else {
                    self.open_windows
                        .get(self.selected_win_pos - 1)
                        .map(|w| format!("🪟 {}", w.title))
                        .unwrap_or_else(|| "🖥 Schermo Intero".to_string())
                };

                let prev_win = self.selected_win_pos;
                egui::ComboBox::from_id_salt("mouse_win_combo")
                    .selected_text(selected_win_title)
                    .width((ui.available_width() - 38.0).max(120.0))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.selected_win_pos,
                            0,
                            "🖥 Schermo Intero (Desktop Primario)",
                        );
                        for (i, w) in self.open_windows.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.selected_win_pos,
                                i + 1,
                                format!("🪟 {}", w.title),
                            );
                        }
                    });

                if ui
                    .small_button("🔄")
                    .on_hover_text("Aggiorna finestre aperte")
                    .clicked()
                {
                    self.open_windows = get_open_windows();
                    if self.selected_win_pos > self.open_windows.len() {
                        self.selected_win_pos = 0;
                    }
                }

                if self.selected_win_pos != prev_win {
                    self.sync_mouse_settings();
                }
            });

            // 2. Periodic Anti-AFK Mouse Jiggler
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut self.periodic_move_enabled, "Muovi mouse ogni:")
                    .changed()
                {
                    self.sync_mouse_settings();
                }
                if ui
                    .add(
                        egui::DragValue::new(&mut self.move_interval_sec)
                            .range(1..=300)
                            .suffix(" sec"),
                    )
                    .changed()
                {
                    self.sync_mouse_settings();
                }
                ui.label(RichText::new("(Anti-AFK Jiggler)").size(11.5).color(TEXT_MUTED));
            });

            ui.add_space(2.0);
            ui.label(
                RichText::new("Immagini Target da Cliccare (Max 4 Slot):")
                    .size(12.5)
                    .strong()
                    .color(TEXT_PRIMARY),
            );

            // 3. 4 Target Image Slots
            let mut slots_changed = false;
            for idx in 0..4 {
                inner_card(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Thumbnail box
                        if let Some(ref tex) = self.slots[idx].thumb_tex {
                            ui.image((tex.id(), Vec2::new(28.0, 28.0)));
                        } else {
                            Frame::none()
                                .fill(BG_DEEP)
                                .rounding(Rounding::same(4.0))
                                .inner_margin(Margin::symmetric(6.0, 4.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!("#{}", idx + 1))
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                });
                        }

                        let slot_name = self.slots[idx]
                            .target
                            .as_ref()
                            .map(|t| t.name.as_str())
                            .unwrap_or("Vuoto");
                        let name_col = if self.slots[idx].target.is_some() {
                            TEXT_PRIMARY
                        } else {
                            TEXT_MUTED
                        };
                        ui.label(
                            RichText::new(format!("Slot {}: {}", idx + 1, slot_name))
                                .size(12.0)
                                .strong()
                                .color(name_col),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add_enabled(
                                    self.slots[idx].target.is_some(),
                                    egui::Button::new("🗑"),
                                )
                                .on_hover_text("Rimuovi target")
                                .clicked()
                            {
                                self.slots[idx].target = None;
                                self.slots[idx].thumb_tex = None;
                                slots_changed = true;
                            }

                            if ui
                                .button("📂")
                                .on_hover_text("Carica immagine target (PNG, JPG, WEBP)")
                                .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter(
                                        "Immagini Target",
                                        &["png", "jpg", "jpeg", "webp", "bmp"],
                                    )
                                    .pick_file()
                                {
                                    let path_str = path.to_string_lossy().to_string();
                                    match TemplateTarget::from_file(
                                        idx,
                                        &path_str,
                                        self.slots[idx].scroll_enabled,
                                    ) {
                                        Ok(tmpl) => {
                                            let cimg = ColorImage::from_rgb(
                                                [tmpl.width as usize, tmpl.height as usize],
                                                &tmpl.rgb_preview,
                                            );
                                            let tex = ctx.load_texture(
                                                format!("slot_thumb_{}", idx),
                                                cimg,
                                                TextureOptions::LINEAR,
                                            );
                                            self.slots[idx].thumb_tex = Some(tex);
                                            self.slots[idx].target = Some(tmpl);
                                            slots_changed = true;
                                        }
                                        Err(e) => {
                                            self.push_toast("Errore Slot Immagine", e, ACCENT_DANGER);
                                        }
                                    }
                                }
                            }

                            if ui
                                .checkbox(&mut self.slots[idx].scroll_enabled, "Scroll")
                                .on_hover_text("Esegui scroll down dopo il click su questo slot")
                                .changed()
                            {
                                slots_changed = true;
                            }
                        });
                    });
                });
            }
            if slots_changed {
                self.sync_mouse_settings();
            }

            // 4. Precision / Matching Tolerance Slider (50% - 99%)
            ui.horizontal(|ui| {
                ui.label("Precisione Matching:");
                let (desc, col) = if self.precision_pct >= 92 {
                    ("Massima", ACCENT_SUCCESS)
                } else if self.precision_pct >= 85 {
                    ("Alta (Consigliata)", ACCENT_PRIMARY)
                } else if self.precision_pct >= 70 {
                    ("Media", ACCENT_WARNING)
                } else {
                    ("Bassa", ACCENT_DANGER)
                };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    status_pill(ui, &format!("{}% - {}", self.precision_pct, desc), col);
                });
            });

            ui.spacing_mut().slider_width = (ui.available_width() - 10.0).max(80.0);
            if ui
                .add(
                    egui::Slider::new(&mut self.precision_pct, 50..=99)
                        .show_value(false)
                        .trailing_fill(true),
                )
                .changed()
            {
                self.sync_mouse_settings();
            }

            // 5. Post-Click Configurable Scroll Down Section
            inner_card(ui, |ui| {
                if ui
                    .checkbox(
                        &mut self.enable_post_scroll,
                        "Scroll verso il basso dopo ogni click",
                    )
                    .changed()
                {
                    self.sync_mouse_settings();
                }

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Intensità:").size(12.0).color(TEXT_SECONDARY));
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.scroll_steps)
                                .range(1..=20)
                                .suffix(" scatti"),
                        )
                        .changed()
                    {
                        self.sync_mouse_settings();
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new("Ritardo:").size(12.0).color(TEXT_SECONDARY));
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.scroll_delay_sec)
                                .speed(0.05)
                                .range(0.05..=3.0)
                                .suffix(" sec"),
                        )
                        .changed()
                    {
                        self.sync_mouse_settings();
                    }
                });
            });

            // 6. Two Dedicated Start / Stop Buttons
            ui.columns(2, |cols| {
                let start_btn = egui::Button::new(
                    RichText::new("▶ AVVIA BOT (CTRL+ALT+A)")
                        .size(12.5)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(ACCENT_PRIMARY)
                .min_size(Vec2::new(cols[0].available_width(), 34.0));

                if cols[0]
                    .add_enabled(!bot_status.is_running, start_btn)
                    .clicked()
                {
                    self.sync_mouse_settings();
                    self.mouse_engine.start_bot();
                }

                let stop_fill = if bot_status.is_running {
                    ACCENT_DANGER
                } else {
                    Color32::from_rgb(45, 48, 60)
                };
                let stop_btn = egui::Button::new(
                    RichText::new("⏹ ARRESTA (CTRL+ALT+D)")
                        .size(12.5)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(stop_fill)
                .min_size(Vec2::new(cols[1].available_width(), 34.0));

                if cols[1]
                    .add_enabled(bot_status.is_running, stop_btn)
                    .clicked()
                {
                    self.mouse_engine.stop_bot();
                }
            });

            // Status / Log Footer
            if let Some(ref err) = bot_status.last_error {
                ui.label(RichText::new(err).size(11.5).color(ACCENT_DANGER));
            } else if let Some(ref act) = bot_status.last_action {
                ui.label(
                    RichText::new(format!("✔ {}", act))
                        .size(11.5)
                        .strong()
                        .color(ACCENT_SUCCESS),
                );
            } else {
                ui.label(
                    RichText::new(&bot_status.status_message)
                        .size(11.5)
                        .color(TEXT_MUTED),
                );
            }
        });
    }

    fn render_modals(&mut self, ctx: &Context) {
        // 1. Video Trim & Compression Modal
        let mut trim_result: Option<TrimModalOutcome> = None;
        if let Some(ref mut modal) = self.trim_modal {
            trim_result = modal.show(ctx);
        }
        if let Some(outcome) = trim_result {
            self.trim_modal = None;
            if let TrimModalOutcome::LoadVideo {
                final_path,
                compression_stats,
            } = outcome
            {
                let name = std::path::Path::new(&final_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("video.mp4")
                    .to_string();
                self.local_video_label = name;
                self.video_engine.load_local_video(final_path.clone());
                self.audio_engine.load_source(Some(final_path));
                self.audio_engine.play();

                if let Some((orig, new_sz)) = compression_stats {
                    let saved_pct = if orig > 0 && new_sz < orig {
                        (((orig - new_sz) as f64 / (orig as f64)) * 100.0).round() as u64
                    } else {
                        0
                    };
                    self.push_toast(
                        "Video Ottimizzato in H.264",
                        format!(
                            "Peso ridotto: {} ➔ {} (-{}%)",
                            format_bytes(orig),
                            format_bytes(new_sz),
                            saved_pct
                        ),
                        ACCENT_SUCCESS,
                    );
                }
            }
        }

        // 2. External Programs & Drivers Installer Modal
        let audio_names: Vec<String> = self.audio_devices.iter().map(|d| d.name.clone()).collect();
        self.deps_modal.show(ctx, &self.dep_manager, &audio_names);

        // 3. Prerequisites Info Guide Window
        if self.show_prereq_guide {
            let mut open = self.show_prereq_guide;
            Window::new("📘 Guida Rapida Regia Virtuale - LessCam Rust")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .fixed_size([520.0, 310.0])
                .show(ctx, |ui| {
                    inner_card(ui, |ui| {
                        ui.label(
                            RichText::new("1. PROGRAMMI ESTERNI AUTOMATICI (1-CLICK)")
                                .strong()
                                .color(ACCENT_PRIMARY),
                        );
                        ui.label(
                            "Clicca sul pulsante '📦 Installer & Programmi Esterni' in alto a destra \
                            per scaricare e configurare automaticamente FFmpeg, yt-dlp, VB-Audio Virtual Cable \
                            e il driver DirectShow Virtual Camera.",
                        );
                    });

                    ui.add_space(6.0);

                    inner_card(ui, |ui| {
                        ui.label(
                            RichText::new("2. CONFIGURAZIONE SU ZOOM / DISCORD / MEET / TEAMS")
                                .strong()
                                .color(ACCENT_SUCCESS),
                        );
                        ui.label("• Videocamera: Seleziona 'Unity Video Capture' o 'OBS Virtual Camera'.");
                        ui.label("• Microfono: Seleziona 'CABLE Output (VB-Audio Virtual Cable)'.");
                        ui.label("• In LessCam: Imposta il Mixer Audio su '⭐ CABLE Input'.");
                    });

                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Ho capito").strong().color(Color32::WHITE),
                                )
                                .fill(ACCENT_PRIMARY),
                            )
                            .clicked()
                        {
                            self.show_prereq_guide = false;
                        }
                    });
                });
            if !open {
                self.show_prereq_guide = false;
            }
        }
    }
}

impl eframe::App for LessCamApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_background_engines(ctx);

        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(BG_DEEP)
                    .inner_margin(Margin::symmetric(18.0, 14.0)),
            )
            .show(ctx, |ui| {
                // 1. Header Bar
                self.render_top_header(ui);

                // Optional floating Toast Notification Banner
                let mut clear_toast = false;
                if let Some(ref t) = self.toast {
                    if t.created_at.elapsed() >= t.duration {
                        clear_toast = true;
                    } else {
                        ui.add_space(4.0);
                        Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(
                                t.color.r(),
                                t.color.g(),
                                t.color.b(),
                                35,
                            ))
                            .stroke(egui::Stroke::new(1.0_f32, t.color))
                            .rounding(Rounding::same(8.0))
                            .inner_margin(Margin::symmetric(12.0, 6.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&t.title).strong().color(t.color));
                                    ui.label(RichText::new("—").color(TEXT_MUTED));
                                    ui.label(RichText::new(&t.message).color(TEXT_PRIMARY));
                                });
                            });
                    }
                }
                if clear_toast {
                    self.toast = None;
                }

                ui.add_space(10.0);

                // 2. Two-Column Studio Layout
                let total_w = ui.available_width();
                let spacing = 16.0;
                let left_w = ((total_w - spacing) * 0.58).max(460.0);
                let right_w = (total_w - spacing - left_w).max(340.0);

                ui.horizontal_top(|ui| {
                    // LEFT COLUMN: GPU Preview Canvas + Source Switcher & Transport
                    ui.allocate_ui(Vec2::new(left_w, ui.available_height()), |ui| {
                        ui.vertical(|ui| {
                            self.preview_canvas.ui(ui, 315.0);
                            ui.add_space(10.0);
                            self.render_source_panel(ui);
                        });
                    });

                    ui.add_space(spacing);

                    // RIGHT COLUMN: Segmented Studio / Mouse Automation Tabs
                    ui.allocate_ui(Vec2::new(right_w, ui.available_height()), |ui| {
                        ui.vertical(|ui| {
                            segmented_tabs(
                                ui,
                                "right_col_tabs",
                                &[("🎛", "Regia & Audio"), ("🖱", "Automazione Mouse")],
                                &mut self.right_tab_idx,
                            );
                            ui.add_space(8.0);

                            ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if self.right_tab_idx == 0 {
                                        self.render_camera_panel(ui);
                                        ui.add_space(10.0);
                                        self.render_audio_panel(ui);
                                    } else {
                                        self.render_mouse_panel(ctx, ui);
                                    }
                                });
                        });
                    });
                });
            });

        self.render_modals(ctx);

        // Request continuous 60 FPS repaint for smooth video preview and VU-meter ballistics
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(ref h) = self.yt_extractor {
            h.cancel();
        }
        self.mouse_engine.shutdown();
        self.audio_engine.shutdown();
        self.video_engine.shutdown();
    }
}

//! LessCam - External Programs & Drivers Installer Modal (Rust Edition)
//! Provides a 1-click GUI installer and status dashboard for:
//! - yt-dlp
//! - FFmpeg & FFprobe
//! - VB-Audio Virtual Cable
//! - DirectShow Virtual Camera Driver (UnityCapture / OBS VirtualCam)

use crate::engine::deps::{DependencyManager, ExternalProgram, ProgramStatus};
use crate::gui::theme::{
    inner_card, status_pill, ACCENT_DANGER, ACCENT_PRIMARY, ACCENT_SUCCESS, ACCENT_WARNING,
    BG_INPUT, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use egui::{Color32, Context, Frame, Margin, RichText, Rounding, ScrollArea, Window};

pub struct DependencyInstallerModal {
    pub is_open: bool,
}

impl Default for DependencyInstallerModal {
    fn default() -> Self {
        Self { is_open: false }
    }
}

impl DependencyInstallerModal {
    pub fn show(
        &mut self,
        ctx: &Context,
        dep_mgr: &DependencyManager,
        audio_device_names: &[String],
    ) {
        if !self.is_open {
            return;
        }

        let snap = dep_mgr.snapshot();
        let mut open_flag = self.is_open;

        Window::new("📦 Installer & Gestore Programmi Esterni Necessari")
            .open(&mut open_flag)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([660.0, 520.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Verifica e installa automaticamente con 1 click tutti i programmi e driver esterni richiesti da LessCam:",
                    )
                    .color(TEXT_SECONDARY),
                );

                ui.add_space(6.0);

                // Top global actions bar
                ui.horizontal(|ui| {
                    let all_installed = snap.ytdlp.is_installed()
                        && snap.ffmpeg.is_installed()
                        && snap.vbcable.is_installed()
                        && snap.vcam.is_installed();

                    let install_all_btn = egui::Button::new(
                        RichText::new(if all_installed {
                            "✅ Tutti i Programmi sono Installati"
                        } else if snap.is_busy {
                            "⏳ Installazione Automatica in corso..."
                        } else {
                            "🚀 Installa Tutti i Programmi Necessari (1-Click)"
                        })
                        .strong()
                        .color(Color32::WHITE),
                    )
                    .fill(if all_installed {
                        ACCENT_SUCCESS
                    } else {
                        ACCENT_PRIMARY
                    });

                    if ui
                        .add_enabled(!snap.is_busy && !all_installed, install_all_btn)
                        .clicked()
                    {
                        dep_mgr.install_all();
                    }

                    if ui
                        .add_enabled(
                            !snap.is_busy,
                            egui::Button::new("🔄 Ricontrolla Stato Sistema"),
                        )
                        .clicked()
                    {
                        dep_mgr.refresh(audio_device_names.to_vec());
                    }
                });

                ui.add_space(6.0);

                // Program rows
                Self::render_program_row(
                    ui,
                    dep_mgr,
                    ExternalProgram::YtDlp,
                    &snap.ytdlp,
                    snap.is_busy,
                );
                Self::render_program_row(
                    ui,
                    dep_mgr,
                    ExternalProgram::Ffmpeg,
                    &snap.ffmpeg,
                    snap.is_busy,
                );
                Self::render_program_row(
                    ui,
                    dep_mgr,
                    ExternalProgram::VbAudioCable,
                    &snap.vbcable,
                    snap.is_busy,
                );
                Self::render_program_row(
                    ui,
                    dep_mgr,
                    ExternalProgram::VirtualCamDriver,
                    &snap.vcam,
                    snap.is_busy,
                );

                ui.add_space(6.0);
                ui.label(
                    RichText::new("Console Log Installer:")
                        .size(12.0)
                        .strong()
                        .color(TEXT_MUTED),
                );

                Frame::none()
                    .fill(BG_INPUT)
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(10.0))
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .max_height(110.0)
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                for line in &snap.log_lines {
                                    ui.label(
                                        RichText::new(line)
                                            .monospace()
                                            .size(11.5)
                                            .color(TEXT_SECONDARY),
                                    );
                                }
                            });
                    });
            });

        self.is_open = open_flag;
    }

    fn render_program_row(
        ui: &mut egui::Ui,
        dep_mgr: &DependencyManager,
        prog: ExternalProgram,
        status: &ProgramStatus,
        is_busy: bool,
    ) {
        inner_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(430.0);
                    ui.label(
                        RichText::new(prog.display_name())
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.label(
                        RichText::new(prog.description())
                            .size(11.5)
                            .color(TEXT_MUTED),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match status {
                        ProgramStatus::Installed(detail) => {
                            status_pill(ui, "✔ INSTALLATO", ACCENT_SUCCESS);
                            if ui
                                .small_button("Reinstalla")
                                .on_hover_text(detail)
                                .clicked()
                                && !is_busy
                            {
                                dep_mgr.install_program(prog);
                            }
                        }
                        ProgramStatus::Missing => {
                            if ui
                                .add_enabled(
                                    !is_busy,
                                    egui::Button::new(
                                        RichText::new("⬇ Installa Ora")
                                            .strong()
                                            .color(Color32::WHITE),
                                    )
                                    .fill(ACCENT_PRIMARY),
                                )
                                .clicked()
                            {
                                dep_mgr.install_program(prog);
                            }
                            status_pill(ui, "MANCANTE", ACCENT_WARNING);
                        }
                        ProgramStatus::Installing(msg) => {
                            status_pill(ui, msg, ACCENT_PRIMARY);
                        }
                        ProgramStatus::Failed(err) => {
                            if ui
                                .add_enabled(
                                    !is_busy,
                                    egui::Button::new(
                                        RichText::new("🔄 Riprova")
                                            .strong()
                                            .color(Color32::WHITE),
                                    )
                                    .fill(ACCENT_DANGER),
                                )
                                .on_hover_text(err)
                                .clicked()
                            {
                                dep_mgr.install_program(prog);
                            }
                            status_pill(ui, "ERRORE", ACCENT_DANGER);
                        }
                    }
                });
            });
        });
    }
}

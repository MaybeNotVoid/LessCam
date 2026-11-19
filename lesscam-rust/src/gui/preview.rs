//! LessCam - GPU Video Preview Canvas (Rust Edition)
//! Renders incoming RGB video frames as hardware-accelerated GPU textures with aspect-ratio
//! preservation, rounded studio borders, pulsing LIVE indicator, resolution HUD, and buffering overlay.

use crate::engine::video_stream::VideoFramePacket;
use crate::gui::theme::{
    ACCENT_DANGER, ACCENT_PRIMARY, BG_DEEP, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY,
};
use egui::{
    Color32, ColorImage, Context, FontId, Pos2, Rect, Rounding, Sense, Stroke, TextureHandle,
    TextureOptions, Ui, Vec2,
};

pub struct VideoPreviewCanvas {
    texture: Option<TextureHandle>,
    pub is_live: bool,
    pub resolution_text: String,
    pub is_buffering: bool,
    pub buffering_text: String,
    pub source_title: String,
}

impl Default for VideoPreviewCanvas {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoPreviewCanvas {
    pub fn new() -> Self {
        Self {
            texture: None,
            is_live: false,
            resolution_text: "720p | 30 FPS".to_string(),
            is_buffering: false,
            buffering_text: "Caricamento stream...".to_string(),
            source_title: "Standby".to_string(),
        }
    }

    /// Upload a new RGB frame packet to the GPU texture.
    pub fn update_frame(&mut self, ctx: &Context, packet: &VideoFramePacket) {
        let expected_len = (packet.width as usize) * (packet.height as usize) * 3;
        if packet.rgb_data.len() != expected_len || packet.width == 0 || packet.height == 0 {
            return;
        }

        let color_img = ColorImage::from_rgb(
            [packet.width as usize, packet.height as usize],
            &packet.rgb_data,
        );

        match &mut self.texture {
            Some(tex) => {
                tex.set(color_img, TextureOptions::LINEAR);
            }
            None => {
                self.texture =
                    Some(ctx.load_texture("lesscam_preview_tex", color_img, TextureOptions::LINEAR));
            }
        }
    }

    pub fn ui(&self, ui: &mut Ui, min_height: f32) {
        let avail_w = ui.available_width();
        let desired_h = (avail_w * (9.0 / 16.0)).max(min_height);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, desired_h), Sense::hover());

        if !ui.is_rect_visible(rect) {
            return;
        }

        let painter = ui.painter();

        // Outer screen surface
        painter.rect_filled(rect, Rounding::same(12.0), BG_DEEP);

        // Render video texture preserving aspect ratio inside canvas
        if let Some(ref tex) = self.texture {
            let tex_size = tex.size_vec2();
            if tex_size.x > 0.0 && tex_size.y > 0.0 {
                let scale = (rect.width() / tex_size.x).min(rect.height() / tex_size.y);
                let draw_w = tex_size.x * scale;
                let draw_h = tex_size.y * scale;
                let draw_x = rect.left() + (rect.width() - draw_w) * 0.5;
                let draw_y = rect.top() + (rect.height() - draw_h) * 0.5;
                let target_rect =
                    Rect::from_min_size(Pos2::new(draw_x, draw_y), Vec2::new(draw_w, draw_h));

                painter.image(
                    tex.id(),
                    target_rect,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        }

        // If in Standby mode, draw crisp vector typography overlay on top of the animated standby shader
        if self.source_title == "Standby" && !self.is_buffering {
            let center = rect.center();
            painter.text(
                Pos2::new(center.x, center.y - 26.0),
                egui::Align2::CENTER_CENTER,
                "LessCam Studio",
                FontId::proportional(28.0),
                TEXT_PRIMARY,
            );
            painter.text(
                Pos2::new(center.x, center.y + 24.0),
                egui::Align2::CENTER_CENTER,
                "Nessuna sorgente attiva • Carica YouTube, Video Locale o Immagine",
                FontId::proportional(13.5),
                TEXT_MUTED,
            );
        }

        // Border stroke
        painter.rect_stroke(rect, Rounding::same(12.0), Stroke::new(1.5_f32, BORDER_SUBTLE));

        // Top-Left Badge: LIVE / OFFLINE
        let badge_pos = Pos2::new(rect.left() + 14.0, rect.top() + 14.0);
        let (badge_label, badge_bg, badge_fg) = if self.is_live {
            (
                "● LIVE ON AIR",
                Color32::from_rgba_unmultiplied(
                    ACCENT_DANGER.r(),
                    ACCENT_DANGER.g(),
                    ACCENT_DANGER.b(),
                    225,
                ),
                Color32::WHITE,
            )
        } else {
            (
                "○ STANDBY CAM",
                Color32::from_rgba_unmultiplied(28, 30, 40, 215),
                Color32::from_rgb(190, 195, 210),
            )
        };

        let badge_w = if self.is_live { 108.0 } else { 114.0 };
        let badge_rect = Rect::from_min_size(badge_pos, Vec2::new(badge_w, 25.0));
        painter.rect_filled(badge_rect, Rounding::same(6.0), badge_bg);
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            badge_label,
            FontId::proportional(11.5),
            badge_fg,
        );

        // Top-Right Badge: Resolution & Framerate
        let res_w = 108.0;
        let res_rect = Rect::from_min_size(
            Pos2::new(rect.right() - res_w - 14.0, rect.top() + 14.0),
            Vec2::new(res_w, 25.0),
        );
        painter.rect_filled(
            res_rect,
            Rounding::same(6.0),
            Color32::from_rgba_unmultiplied(20, 22, 30, 215),
        );
        painter.rect_stroke(
            res_rect,
            Rounding::same(6.0),
            Stroke::new(1.0_f32, BORDER_SUBTLE),
        );
        painter.text(
            res_rect.center(),
            egui::Align2::CENTER_CENTER,
            &self.resolution_text,
            FontId::proportional(11.5),
            Color32::from_rgb(215, 220, 235),
        );

        // Center Buffering Overlay
        if self.is_buffering {
            let ov_w = 290.0;
            let ov_h = 50.0;
            let ov_rect = Rect::from_center_size(rect.center(), Vec2::new(ov_w, ov_h));
            painter.rect_filled(
                ov_rect,
                Rounding::same(10.0),
                Color32::from_rgba_unmultiplied(12, 14, 20, 235),
            );
            painter.rect_stroke(
                ov_rect,
                Rounding::same(10.0),
                Stroke::new(1.5_f32, ACCENT_PRIMARY),
            );
            painter.text(
                ov_rect.center(),
                egui::Align2::CENTER_CENTER,
                &format!("⏳ {}", self.buffering_text),
                FontId::proportional(13.5),
                Color32::WHITE,
            );
        }
    }
}

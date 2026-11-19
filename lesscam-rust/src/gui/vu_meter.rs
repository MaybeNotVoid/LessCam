//! LessCam - Stereo VU-Meter GPU Widget (Rust Edition)
//! Renders a dual-channel (Left/Right) real-time audio level meter with ballistic decay,
//! segmented LED/gradient color bars (Emerald -> Amber -> Crimson), and Peak-Hold marker.

use crate::gui::theme::{ACCENT_DANGER, ACCENT_SUCCESS, ACCENT_WARNING, BG_INPUT, TEXT_SECONDARY};
use egui::{Color32, FontId, Pos2, Rect, Rounding, Sense, Ui, Vec2};

pub struct StereoVuMeter {
    level_l: f32,
    level_r: f32,
    peak_l: f32,
    peak_r: f32,
}

impl Default for StereoVuMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl StereoVuMeter {
    pub fn new() -> Self {
        Self {
            level_l: 0.0,
            level_r: 0.0,
            peak_l: 0.0,
            peak_r: 0.0,
        }
    }

    /// Update instantaneous RMS levels `[0.0, 1.0]` and apply smooth ballistic decay.
    pub fn update_levels(&mut self, left: f32, right: f32) {
        let l = left.clamp(0.0, 1.0);
        let r = right.clamp(0.0, 1.0);

        // Ballistic decay step
        let decay_rate = 0.035;
        let peak_decay = 0.012;

        self.level_l = (self.level_l - decay_rate).max(0.0);
        self.level_r = (self.level_r - decay_rate).max(0.0);
        self.peak_l = (self.peak_l - peak_decay).max(0.0);
        self.peak_r = (self.peak_r - peak_decay).max(0.0);

        // Fast attack
        if l > self.level_l {
            self.level_l = l;
        }
        if r > self.level_r {
            self.level_r = r;
        }
        if l > self.peak_l {
            self.peak_l = l;
        }
        if r > self.peak_r {
            self.peak_r = r;
        }
    }

    pub fn ui(&self, ui: &mut Ui) {
        let desired_size = Vec2::new(ui.available_width(), 44.0);
        let (rect, _) = ui.allocate_exact_size(desired_size, Sense::hover());

        if !ui.is_rect_visible(rect) {
            return;
        }

        let painter = ui.painter();
        let label_w = 22.0;
        let bar_x = rect.left() + label_w;
        let bar_w = (rect.width() - label_w - 4.0).max(40.0);
        let bar_h = 11.0;

        // Channel L
        let y_l = rect.top() + 5.0;
        painter.text(
            Pos2::new(rect.left() + 4.0, y_l + bar_h * 0.5),
            egui::Align2::LEFT_CENTER,
            "L",
            FontId::proportional(11.5),
            TEXT_SECONDARY,
        );
        Self::draw_channel_bar(painter, bar_x, y_l, bar_w, bar_h, self.level_l, self.peak_l);

        // Channel R
        let y_r = rect.top() + 24.0;
        painter.text(
            Pos2::new(rect.left() + 4.0, y_r + bar_h * 0.5),
            egui::Align2::LEFT_CENTER,
            "R",
            FontId::proportional(11.5),
            TEXT_SECONDARY,
        );
        Self::draw_channel_bar(painter, bar_x, y_r, bar_w, bar_h, self.level_r, self.peak_r);
    }

    fn draw_channel_bar(
        painter: &egui::Painter,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        level: f32,
        peak: f32,
    ) {
        let track_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
        painter.rect_filled(track_rect, Rounding::same(4.0), BG_INPUT);

        // Draw segmented LED bar for crisp studio mixer look
        let num_segments = 28usize;
        let gap = 2.0f32;
        let seg_w = ((w - gap * (num_segments as f32 - 1.0)) / (num_segments as f32)).max(2.0);
        let active_segments = ((level * num_segments as f32).ceil() as usize).min(num_segments);

        for i in 0..active_segments {
            let frac = (i as f32) / (num_segments as f32);
            let seg_color = if frac < 0.68 {
                ACCENT_SUCCESS
            } else if frac < 0.86 {
                ACCENT_WARNING
            } else {
                ACCENT_DANGER
            };

            let sx = x + (i as f32) * (seg_w + gap);
            let seg_rect = Rect::from_min_size(Pos2::new(sx, y + 1.0), Vec2::new(seg_w, h - 2.0));
            painter.rect_filled(seg_rect, Rounding::same(2.0), seg_color);
        }

        // Peak-hold marker line
        if peak > 0.03 {
            let px = (x + w * peak).min(x + w - 2.5);
            let peak_rect = Rect::from_min_size(Pos2::new(px, y), Vec2::new(2.5, h));
            let peak_color = if peak > 0.86 {
                ACCENT_DANGER
            } else {
                Color32::from_rgb(250, 204, 21)
            };
            painter.rect_filled(peak_rect, Rounding::same(1.0), peak_color);
        }
    }
}

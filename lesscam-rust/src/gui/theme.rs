//! LessCam - Obsidian Studio Modern Theme & Custom UI Components
//! Provides a layered glassmorphic dark palette, sleek rounded cards, animated toggle switches,
//! status pills, and modern Windows 11 Fluent-inspired typography and spacing.

use egui::{
    Color32, Context, FontFamily, FontId, Frame, Margin, Response, RichText, Rounding, Sense,
    Stroke, TextStyle, Ui, Vec2, Visuals,
};

pub const BG_DEEP: Color32 = Color32::from_rgb(12, 13, 18);
pub const BG_CARD: Color32 = Color32::from_rgb(21, 23, 31);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(30, 33, 44);
pub const BG_INPUT: Color32 = Color32::from_rgb(16, 18, 25);

pub const BORDER_SUBTLE: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 20);
pub const BORDER_ACCENT: Color32 = Color32::from_rgb(0, 155, 255);

pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(0, 145, 245);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(30, 168, 255);
pub const ACCENT_SUCCESS: Color32 = Color32::from_rgb(16, 185, 129);
pub const ACCENT_WARNING: Color32 = Color32::from_rgb(245, 158, 11);
pub const ACCENT_DANGER: Color32 = Color32::from_rgb(239, 68, 68);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(243, 244, 248);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(162, 168, 186);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(112, 118, 136);

/// Configure global `egui` style and Obsidian Studio Dark visuals.
pub fn apply_modern_studio_theme(ctx: &Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = Visuals::dark();

    visuals.panel_fill = BG_DEEP;
    visuals.window_fill = BG_CARD;
    visuals.extreme_bg_color = BG_INPUT;
    visuals.faint_bg_color = BG_ELEVATED;

    visuals.window_rounding = Rounding::same(12.0);
    visuals.menu_rounding = Rounding::same(8.0);

    visuals.selection.bg_fill = ACCENT_PRIMARY;
    visuals.selection.stroke = Stroke::new(1.5_f32, Color32::WHITE);

    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.noninteractive.rounding = Rounding::same(8.0);

    visuals.widgets.inactive.bg_fill = BG_ELEVATED;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.inactive.rounding = Rounding::same(8.0);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 44, 58);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.2_f32, ACCENT_PRIMARY);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.hovered.rounding = Rounding::same(8.0);

    visuals.widgets.active.bg_fill = ACCENT_PRIMARY;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, ACCENT_HOVER);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.rounding = Rounding::same(8.0);

    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(10.0, 9.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.spacing.window_margin = Margin::same(16.0);

    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(19.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.5, FontFamily::Proportional),
    );

    ctx.set_style(style);
}

/// Render a sleek Studio Card container with subtle top-lit border and rounded corners.
pub fn studio_card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::none()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
        .rounding(Rounding::same(12.0))
        .inner_margin(Margin::symmetric(16.0, 14.0))
        .show(ui, add_contents)
        .inner
}

/// Render an inner elevated panel inside a card.
pub fn inner_card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::none()
        .fill(BG_ELEVATED)
        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(12.0, 10.0))
        .show(ui, add_contents)
        .inner
}

/// Draw a modern section header with icon prefix and optional right-side status badge.
pub fn card_header(ui: &mut Ui, icon: &str, title: &str, badge: Option<(&str, Color32)>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(17.0).color(ACCENT_HOVER));
        ui.label(
            RichText::new(title)
                .size(15.5)
                .strong()
                .color(TEXT_PRIMARY),
        );
        if let Some((badge_text, badge_color)) = badge {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                status_pill(ui, badge_text, badge_color);
            });
        }
    });
}

/// Render a modern rounded status pill.
pub fn status_pill(ui: &mut Ui, text: &str, color: Color32) {
    let bg = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 32);
    let border = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 110);
    Frame::none()
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, border))
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::symmetric(9.0, 3.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.5).strong().color(color));
        });
}

/// Modern animated iOS/Fluent-style toggle switch widget.
pub fn toggle_switch(ui: &mut Ui, on: &mut bool) -> Response {
    let desired_size = Vec2::new(46.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let bg_color = Color32::from_rgb(
            (45.0 + (ACCENT_PRIMARY.r() as f32 - 45.0) * how_on) as u8,
            (50.0 + (ACCENT_PRIMARY.g() as f32 - 50.0) * how_on) as u8,
            (65.0 + (ACCENT_PRIMARY.b() as f32 - 65.0) * how_on) as u8,
        );

        let radius = 0.5 * rect.height();
        ui.painter()
            .rect_filled(rect, Rounding::same(radius), bg_color);
        ui.painter()
            .rect_stroke(rect, Rounding::same(radius), Stroke::new(1.0_f32, BORDER_SUBTLE));

        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter()
            .circle_filled(center, radius - 3.5, Color32::WHITE);
    }

    response
}

/// Render a modern segmented tab selector bar.
pub fn segmented_tabs(
    ui: &mut Ui,
    id_salt: &str,
    items: &[(&str, &str)],
    current_index: &mut usize,
) {
    let _ = id_salt;
    Frame::none()
        .fill(BG_INPUT)
        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
        .rounding(Rounding::same(9.0))
        .inner_margin(Margin::same(3.0))
        .show(ui, |ui| {
            ui.columns(items.len(), |cols| {
                for (idx, (icon, label)) in items.iter().enumerate() {
                    let selected = *current_index == idx;
                    let fill = if selected {
                        ACCENT_PRIMARY
                    } else {
                        Color32::TRANSPARENT
                    };
                    let fg = if selected {
                        Color32::WHITE
                    } else {
                        TEXT_SECONDARY
                    };

                    let btn = egui::Button::new(
                        RichText::new(format!("{} {}", icon, label))
                            .size(13.0)
                            .strong()
                            .color(fg),
                    )
                    .fill(fill)
                    .stroke(Stroke::NONE)
                    .rounding(Rounding::same(7.0))
                    .min_size(Vec2::new(cols[idx].available_width(), 29.0));

                    if cols[idx].add(btn).clicked() {
                        *current_index = idx;
                    }
                }
            });
        });
}

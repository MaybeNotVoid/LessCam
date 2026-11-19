#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! LessCam - Modern Virtual Camera, Audio Router & Computer Vision Automation Studio
//! High-performance native Rust edition with GPU-accelerated GUI (`eframe` / `egui`).

pub mod engine;
pub mod gui;

use eframe::egui;
use gui::app::LessCamApp;

fn main() -> eframe::Result<()> {
    // Support `--check-deps` CLI flag for automated installer verification
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--check-deps") {
        let ff = engine::deps::resolve_binary_path("ffmpeg");
        let yt = engine::deps::resolve_binary_path("yt-dlp");
        println!("LessCam Rust v2.0.0 - Verifica Programmi Esterni:");
        println!("  FFmpeg : {:?}", ff);
        println!("  yt-dlp : {:?}", yt);
        return Ok(());
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LessCam Studio 2.0 (Rust Edition) - Virtual Camera & Audio Router")
            .with_inner_size([1280.0, 810.0])
            .with_min_inner_size([1060.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "LessCam Studio",
        native_options,
        Box::new(|cc| Ok(Box::new(LessCamApp::new(cc)))),
    )
}

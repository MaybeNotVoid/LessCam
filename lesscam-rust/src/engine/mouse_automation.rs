//! LessCam - Computer Vision Mouse Automation Engine (Rust Edition)
//! Provides window enumeration, screen capture, parallel Normalized Cross-Correlation (NCC)
//! template matching via `rayon` (up to 4 image slots), precision tolerance threshold (50%-99%),
//! automated click + configurable post-click scroll down, periodic Anti-AFK mouse jiggling,
//! corner failsafe, and global hotkeys (CTRL+ALT+A / CTRL+ALT+D).

use parking_lot::Mutex;
use rand::Rng;
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub rect: (i32, i32, i32, i32), // (left, top, right, bottom)
}

#[derive(Debug, Clone)]
pub struct TemplateTarget {
    pub slot_index: usize,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub gray_data: Vec<f32>,
    pub rgb_preview: Vec<u8>,
    pub scroll_enabled: bool,
}

impl TemplateTarget {
    /// Load a template image from disk, precomputing normalized grayscale values for fast NCC matching.
    pub fn from_file(slot_index: usize, path: &str, scroll_enabled: bool) -> Result<Self, String> {
        let img = image::open(path).map_err(|e| format!("Impossibile leggere immagine: {}", e))?;
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        if w < 4 || h < 4 {
            return Err("Immagine target troppo piccola (minimo 4x4 pixel).".to_string());
        }

        let gray = img.to_luma8();
        let gray_data: Vec<f32> = gray.as_raw().iter().map(|&p| p as f32).collect();
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("target.png")
            .to_string();

        Ok(Self {
            slot_index,
            name,
            width: w,
            height: h,
            gray_data,
            rgb_preview: rgb.into_raw(),
            scroll_enabled,
        })
    }
}

/// Fast multi-threaded Normalized Cross-Correlation (`TM_CCOEFF_NORMED` equivalent) in pure Rust.
/// Returns `(best_score, best_x, best_y)` where `best_score` is in `[-1.0, 1.0]`.
pub fn match_template_ncc(
    screen_gray: &[f32],
    scr_w: u32,
    scr_h: u32,
    tmpl_gray: &[f32],
    tw: u32,
    th: u32,
    step: u32,
) -> (f32, u32, u32) {
    if tw == 0 || th == 0 || tw > scr_w || th > scr_h {
        return (0.0, 0, 0);
    }

    let n = (tw * th) as f32;
    let tmpl_mean: f32 = tmpl_gray.iter().copied().sum::<f32>() / n;
    let mut tmpl_zero_mean = Vec::with_capacity(tmpl_gray.len());
    let mut tmpl_var_sum = 0.0f32;

    for &val in tmpl_gray {
        let diff = val - tmpl_mean;
        tmpl_zero_mean.push(diff);
        tmpl_var_sum += diff * diff;
    }

    if tmpl_var_sum <= 1e-5 {
        return (0.0, 0, 0);
    }

    let max_y = scr_h - th;
    let max_x = scr_w - tw;
    let stride = step.max(1);

    let y_positions: Vec<u32> = (0..=max_y).step_by(stride as usize).collect();

    let best_per_row: Option<(f32, u32, u32)> = y_positions
        .par_iter()
        .map(|&y| {
            let mut row_best_score = -1.0f32;
            let mut row_best_x = 0u32;

            let mut x = 0u32;
            while x <= max_x {
                // Compute window mean
                let mut sum_s = 0.0f32;
                for ty in 0..th {
                    let row_start = ((y + ty) * scr_w + x) as usize;
                    let slice = &screen_gray[row_start..row_start + (tw as usize)];
                    for &sv in slice {
                        sum_s += sv;
                    }
                }
                let mean_s = sum_s / n;

                let mut num = 0.0f32;
                let mut var_s = 0.0f32;
                for ty in 0..th {
                    let scr_row = ((y + ty) * scr_w + x) as usize;
                    let tmpl_row = (ty * tw) as usize;
                    for tx in 0..(tw as usize) {
                        let ds = screen_gray[scr_row + tx] - mean_s;
                        let dt = tmpl_zero_mean[tmpl_row + tx];
                        num += ds * dt;
                        var_s += ds * ds;
                    }
                }

                let denom = (var_s * tmpl_var_sum).sqrt();
                let score = if denom > 1e-5 { num / denom } else { 0.0 };

                if score > row_best_score {
                    row_best_score = score;
                    row_best_x = x;
                }
                x += stride;
            }

            (row_best_score, row_best_x, y)
        })
        .reduce_with(|a, b| if a.0 >= b.0 { a } else { b });

    let (coarse_score, coarse_x, coarse_y) = best_per_row.unwrap_or((0.0, 0, 0));

    // Fine 1-pixel refinement around coarse maximum if stride > 1
    if stride > 1 && coarse_score > 0.45 {
        let refine_y_min = coarse_y.saturating_sub(stride);
        let refine_y_max = (coarse_y + stride).min(max_y);
        let refine_x_min = coarse_x.saturating_sub(stride);
        let refine_x_max = (coarse_x + stride).min(max_x);

        let mut best_score = coarse_score;
        let mut best_x = coarse_x;
        let mut best_y = coarse_y;

        for y in refine_y_min..=refine_y_max {
            for x in refine_x_min..=refine_x_max {
                let mut sum_s = 0.0f32;
                for ty in 0..th {
                    let row_start = ((y + ty) * scr_w + x) as usize;
                    for &sv in &screen_gray[row_start..row_start + (tw as usize)] {
                        sum_s += sv;
                    }
                }
                let mean_s = sum_s / n;
                let mut num = 0.0f32;
                let mut var_s = 0.0f32;
                for ty in 0..th {
                    let scr_row = ((y + ty) * scr_w + x) as usize;
                    let tmpl_row = (ty * tw) as usize;
                    for tx in 0..(tw as usize) {
                        let ds = screen_gray[scr_row + tx] - mean_s;
                        let dt = tmpl_zero_mean[tmpl_row + tx];
                        num += ds * dt;
                        var_s += ds * ds;
                    }
                }
                let denom = (var_s * tmpl_var_sum).sqrt();
                let score = if denom > 1e-5 { num / denom } else { 0.0 };
                if score > best_score {
                    best_score = score;
                    best_x = x;
                    best_y = y;
                }
            }
        }
        return (best_score, best_x, best_y);
    }

    (coarse_score, coarse_x, coarse_y)
}

/// Enumerate open desktop windows.
pub fn get_open_windows() -> Vec<WindowInfo> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowRect, GetWindowTextW, IsIconic, IsWindowVisible,
        };

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let list = &mut *(lparam as *mut Vec<WindowInfo>);
            if IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0 {
                let mut buf = [0u16; 256];
                let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
                if len > 0 {
                    let title = String::from_utf16_lossy(&buf[..len as usize])
                        .trim()
                        .to_string();
                    if !title.is_empty()
                        && title != "Program Manager"
                        && title != "Settings"
                        && title != "Default IME"
                        && title != "MSCTFIME UI"
                    {
                        let mut rect = RECT {
                            left: 0,
                            top: 0,
                            right: 0,
                            bottom: 0,
                        };
                        if GetWindowRect(hwnd, &mut rect) != 0 {
                            let w = rect.right - rect.left;
                            let h = rect.bottom - rect.top;
                            if w > 80 && h > 80 {
                                list.push(WindowInfo {
                                    hwnd: hwnd as isize,
                                    title,
                                    rect: (rect.left, rect.top, rect.right, rect.bottom),
                                });
                            }
                        }
                    }
                }
            }
            1
        }

        let mut windows: Vec<WindowInfo> = Vec::new();
        unsafe {
            EnumWindows(Some(enum_proc), &mut windows as *mut _ as isize);
        }
        windows.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        return windows;
    }

    #[cfg(not(windows))]
    {
        vec![
            WindowInfo {
                hwnd: 101,
                title: "Discord - Canale Vocale & Video".to_string(),
                rect: (0, 0, 1280, 720),
            },
            WindowInfo {
                hwnd: 102,
                title: "Google Chrome - Studio Browser".to_string(),
                rect: (100, 100, 1400, 900),
            },
        ]
    }
}

#[derive(Debug, Clone)]
pub struct MouseBotSettings {
    pub target_hwnd: isize,
    pub move_interval_sec: f64,
    pub periodic_move_enabled: bool,
    pub templates: Vec<TemplateTarget>,
    pub enable_scroll: bool,
    pub scroll_steps: i32,
    pub scroll_delay_sec: f64,
    pub threshold: f32, // 0.50 ..= 0.99
    pub cooldown_sec: f64,
}

impl Default for MouseBotSettings {
    fn default() -> Self {
        Self {
            target_hwnd: 0,
            move_interval_sec: 10.0,
            periodic_move_enabled: true,
            templates: Vec::new(),
            enable_scroll: true,
            scroll_steps: 2,
            scroll_delay_sec: 0.20,
            threshold: 0.85,
            cooldown_sec: 1.8,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MouseBotStatus {
    pub is_running: bool,
    pub status_message: String,
    pub last_action: Option<String>,
    pub last_matched_slot: Option<(usize, String)>,
    pub last_error: Option<String>,
}

impl Default for MouseBotStatus {
    fn default() -> Self {
        Self {
            is_running: false,
            status_message:
                "Pronto per l'avvio. Failsafe attivo (angolo schermo per stop d'emergenza)."
                    .to_string(),
            last_action: None,
            last_matched_slot: None,
            last_error: None,
        }
    }
}

pub struct MouseAutomationEngine {
    settings: Arc<Mutex<MouseBotSettings>>,
    status: Arc<Mutex<MouseBotStatus>>,
    worker_running: Arc<AtomicBool>,
    shutdown_flag: Arc<AtomicBool>,
}

impl MouseAutomationEngine {
    pub fn new() -> Self {
        let settings = Arc::new(Mutex::new(MouseBotSettings::default()));
        let status = Arc::new(Mutex::new(MouseBotStatus::default()));
        let worker_running = Arc::new(AtomicBool::new(false));
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        let eng = Self {
            settings,
            status,
            worker_running,
            shutdown_flag,
        };
        eng.spawn_global_hotkey_listener();
        eng
    }

    pub fn update_settings(&self, new_settings: MouseBotSettings) {
        *self.settings.lock() = new_settings;
    }

    pub fn settings(&self) -> MouseBotSettings {
        self.settings.lock().clone()
    }

    pub fn status(&self) -> MouseBotStatus {
        self.status.lock().clone()
    }

    pub fn is_running(&self) -> bool {
        self.worker_running.load(Ordering::SeqCst)
    }

    pub fn start_bot(&self) {
        if self.worker_running.swap(true, Ordering::SeqCst) {
            return;
        }

        {
            let mut st = self.status.lock();
            st.is_running = true;
            st.last_error = None;
            st.status_message =
                "Automazione Mouse Avviata (CTRL+ALT+D per arrestare)".to_string();
        }

        let settings_clone = Arc::clone(&self.settings);
        let status_clone = Arc::clone(&self.status);
        let running_clone = Arc::clone(&self.worker_running);

        thread::spawn(move || {
            Self::bot_worker_loop(settings_clone, status_clone, running_clone);
        });
    }

    pub fn stop_bot(&self) {
        self.worker_running.store(false, Ordering::SeqCst);
        let mut st = self.status.lock();
        st.is_running = false;
        st.status_message = "Automazione Mouse Disattivata.".to_string();
    }

    pub fn shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
        self.stop_bot();
    }

    fn spawn_global_hotkey_listener(&self) {
        #[cfg(windows)]
        {
            let settings_clone = Arc::clone(&self.settings);
            let status_clone = Arc::clone(&self.status);
            let running_clone = Arc::clone(&self.worker_running);
            let shutdown_clone = Arc::clone(&self.shutdown_flag);

            thread::spawn(move || {
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
                const VK_CONTROL: i32 = 0x11;
                const VK_MENU: i32 = 0x12; // ALT
                const VK_A: i32 = 0x41;
                const VK_D: i32 = 0x44;

                let mut prev_a = false;
                let mut prev_d = false;

                while !shutdown_clone.load(Ordering::SeqCst) {
                    unsafe {
                        let ctrl = (GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000) != 0;
                        let alt = (GetAsyncKeyState(VK_MENU) as u16 & 0x8000) != 0;
                        let key_a = (GetAsyncKeyState(VK_A) as u16 & 0x8000) != 0;
                        let key_d = (GetAsyncKeyState(VK_D) as u16 & 0x8000) != 0;

                        if ctrl && alt && key_a && !prev_a {
                            if !running_clone.swap(true, Ordering::SeqCst) {
                                {
                                    let mut st = status_clone.lock();
                                    st.is_running = true;
                                    st.last_error = None;
                                    st.status_message =
                                        "Automazione Mouse Avviata via Hotkey (CTRL+ALT+A)"
                                            .to_string();
                                }
                                let s_c = Arc::clone(&settings_clone);
                                let st_c = Arc::clone(&status_clone);
                                let r_c = Arc::clone(&running_clone);
                                thread::spawn(move || {
                                    Self::bot_worker_loop(s_c, st_c, r_c);
                                });
                            }
                        }

                        if ctrl && alt && key_d && !prev_d {
                            if running_clone.swap(false, Ordering::SeqCst) {
                                let mut st = status_clone.lock();
                                st.is_running = false;
                                st.status_message =
                                    "Automazione Mouse Arrestata via Hotkey (CTRL+ALT+D)"
                                        .to_string();
                            }
                        }

                        prev_a = ctrl && alt && key_a;
                        prev_d = ctrl && alt && key_d;
                    }
                    thread::sleep(Duration::from_millis(35));
                }
            });
        }
    }

    fn bot_worker_loop(
        settings: Arc<Mutex<MouseBotSettings>>,
        status: Arc<Mutex<MouseBotStatus>>,
        running: Arc<AtomicBool>,
    ) {
        let mut last_move_time = Instant::now();
        let mut last_click_time: Option<Instant> = None;

        while running.load(Ordering::SeqCst) {
            // Check corner failsafe on Windows
            if Self::is_cursor_in_failsafe_corner() {
                running.store(false, Ordering::SeqCst);
                let mut st = status.lock();
                st.is_running = false;
                st.last_error = Some(
                    "FailSafe attivato: cursore spostato nell'angolo dello schermo. Bot arrestato."
                        .to_string(),
                );
                st.status_message = "Bot arrestato da FailSafe d'emergenza.".to_string();
                break;
            }

            let cfg = settings.lock().clone();
            let bbox = match Self::resolve_target_bbox(cfg.target_hwnd) {
                Some(b) => b,
                None => {
                    status.lock().status_message =
                        "Finestra target non visibile o minimizzata.".to_string();
                    thread::sleep(Duration::from_millis(800));
                    continue;
                }
            };

            let (left, top, width, height) = bbox;
            let mut image_found_this_cycle = false;

            let cooldown_ready = last_click_time
                .map(|t| t.elapsed().as_secs_f64() >= cfg.cooldown_sec)
                .unwrap_or(true);

            // 1. Template Matching across up to 4 loaded target slots
            if !cfg.templates.is_empty() && cooldown_ready {
                if let Some((scr_gray, scr_w, scr_h)) =
                    Self::capture_region_grayscale(left, top, width, height)
                {
                    for tmpl in &cfg.templates {
                        if !running.load(Ordering::SeqCst) {
                            break;
                        }
                        if tmpl.width > scr_w || tmpl.height > scr_h {
                            continue;
                        }

                        let (score, match_x, match_y) = match_template_ncc(
                            &scr_gray,
                            scr_w,
                            scr_h,
                            &tmpl.gray_data,
                            tmpl.width,
                            tmpl.height,
                            2,
                        );

                        if score >= cfg.threshold {
                            image_found_this_cycle = true;
                            let target_x = left + (match_x as i32) + ((tmpl.width as i32) / 2);
                            let target_y = top + (match_y as i32) + ((tmpl.height as i32) / 2);

                            Self::bring_window_to_front(cfg.target_hwnd);
                            Self::move_cursor_and_click(target_x, target_y);

                            let mut did_scroll = false;
                            if cfg.enable_scroll && tmpl.scroll_enabled {
                                thread::sleep(Duration::from_secs_f64(
                                    cfg.scroll_delay_sec.max(0.05),
                                ));
                                Self::scroll_mouse_wheel(-cfg.scroll_steps.abs());
                                did_scroll = true;
                            }

                            let mut action_msg = format!(
                                "Cliccato [{}] in ({}, {}) [{:.0}%]",
                                tmpl.name,
                                target_x,
                                target_y,
                                score * 100.0
                            );
                            if did_scroll {
                                action_msg.push_str(&format!(
                                    " + Scroll Down ({} scatti)",
                                    cfg.scroll_steps.abs()
                                ));
                            }

                            {
                                let mut st = status.lock();
                                st.status_message = format!(
                                    "Rilevato target: {} ({:.0}%)",
                                    tmpl.name,
                                    score * 100.0
                                );
                                st.last_action = Some(action_msg);
                                st.last_matched_slot = Some((tmpl.slot_index, tmpl.name.clone()));
                            }

                            let now = Instant::now();
                            last_click_time = Some(now);
                            last_move_time = now;
                            break;
                        }
                    }
                }
            }

            // 2. Periodic Anti-AFK Mouse Jiggler
            if !image_found_this_cycle
                && cfg.periodic_move_enabled
                && last_move_time.elapsed().as_secs_f64() >= cfg.move_interval_sec.max(1.0)
            {
                let mut rng = rand::thread_rng();
                let win_cx = left + (width / 2);
                let win_cy = top + (height / 2);
                let offset_x = rng.gen_range(-35..=35);
                let offset_y = rng.gen_range(-35..=35);
                let dest_x = (win_cx + offset_x).clamp(left + 10, left + width - 10);
                let dest_y = (win_cy + offset_y).clamp(top + 10, top + height - 10);

                Self::move_cursor_only(dest_x, dest_y);
                last_move_time = Instant::now();

                status.lock().status_message =
                    format!("Movimento periodico Anti-AFK eseguito a ({}, {})", dest_x, dest_y);
            }

            thread::sleep(Duration::from_millis(150));
        }

        let mut st = status.lock();
        st.is_running = false;
    }

    fn is_cursor_in_failsafe_corner() -> bool {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::Foundation::POINT;
            use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
            let mut pt = POINT { x: 100, y: 100 };
            if GetCursorPos(&mut pt) != 0 {
                return pt.x <= 2 && pt.y <= 2;
            }
        }
        false
    }

    fn resolve_target_bbox(target_hwnd: isize) -> Option<(i32, i32, i32, i32)> {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::Foundation::{HWND, RECT};
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                GetSystemMetrics, GetWindowRect, IsIconic, IsWindow, SM_CXSCREEN, SM_CYSCREEN,
            };

            if target_hwnd != 0 {
                let hwnd = target_hwnd as HWND;
                if IsWindow(hwnd) == 0 || IsIconic(hwnd) != 0 {
                    return None;
                }
                let mut r = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                if GetWindowRect(hwnd, &mut r) == 0 {
                    return None;
                }
                let w = r.right - r.left;
                let h = r.bottom - r.top;
                if w <= 10 || h <= 10 {
                    return None;
                }
                return Some((r.left, r.top, w, h));
            } else {
                let w = GetSystemMetrics(SM_CXSCREEN).max(640);
                let h = GetSystemMetrics(SM_CYSCREEN).max(480);
                return Some((0, 0, w, h));
            }
        }

        #[cfg(not(windows))]
        {
            let _ = target_hwnd;
            Some((0, 0, 1280, 720))
        }
    }

    fn capture_region_grayscale(
        left: i32,
        top: i32,
        width: i32,
        height: i32,
    ) -> Option<(Vec<f32>, u32, u32)> {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::Graphics::Gdi::{
                BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
                GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
                DIB_RGB_COLORS, SRCCOPY,
            };

            let w = width.max(1);
            let h = height.max(1);
            let hdc_screen = GetDC(std::ptr::null_mut());
            if hdc_screen.is_null() {
                return None;
            }
            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
            if hbm.is_null() {
                DeleteDC(hdc_mem);
                ReleaseDC(std::ptr::null_mut(), hdc_screen);
                return None;
            }

            let old_obj = SelectObject(hdc_mem, hbm);
            let _ = BitBlt(hdc_mem, 0, 0, w, h, hdc_screen, left, top, SRCCOPY);

            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [std::mem::zeroed()],
            };

            let mut bgra = vec![0u8; (w as usize) * (h as usize) * 4];
            let lines = GetDIBits(
                hdc_mem,
                hbm,
                0,
                h as u32,
                bgra.as_mut_ptr() as *mut _,
                &mut bmi,
                DIB_RGB_COLORS,
            );

            SelectObject(hdc_mem, old_obj);
            DeleteObject(hbm);
            DeleteDC(hdc_mem);
            ReleaseDC(std::ptr::null_mut(), hdc_screen);

            if lines == 0 {
                return None;
            }

            let mut gray = Vec::with_capacity((w as usize) * (h as usize));
            for px in bgra.chunks_exact(4) {
                let b = px[0] as f32;
                let g = px[1] as f32;
                let r = px[2] as f32;
                gray.push(0.299 * r + 0.587 * g + 0.114 * b);
            }

            return Some((gray, w as u32, h as u32));
        }

        #[cfg(not(windows))]
        {
            let _ = (left, top, width, height);
            None
        }
    }

    fn bring_window_to_front(target_hwnd: isize) {
        #[cfg(windows)]
        if target_hwnd != 0 {
            unsafe {
                use windows_sys::Win32::Foundation::HWND;
                use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
                SetForegroundWindow(target_hwnd as HWND);
            }
        }
        #[cfg(not(windows))]
        let _ = target_hwnd;
    }

    fn move_cursor_only(x: i32, y: i32) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::SetCursorPos;
            SetCursorPos(x, y);
        }
        #[cfg(not(windows))]
        let _ = (x, y);
    }

    fn move_cursor_and_click(x: i32, y: i32) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
                mouse_event, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
            };
            use windows_sys::Win32::UI::WindowsAndMessaging::SetCursorPos;

            SetCursorPos(x, y);
            thread::sleep(Duration::from_millis(30));
            mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
            thread::sleep(Duration::from_millis(25));
            mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
        }
        #[cfg(not(windows))]
        let _ = (x, y);
    }

    fn scroll_mouse_wheel(steps: i32) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
                mouse_event, MOUSEEVENTF_WHEEL,
            };
            let wheel_delta = steps * 120;
            mouse_event(MOUSEEVENTF_WHEEL, 0, 0, wheel_delta as u32, 0);
        }
        #[cfg(not(windows))]
        let _ = steps;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_enumeration_and_defaults() {
        let wins = get_open_windows();
        assert!(!wins.is_empty());

        let cfg = MouseBotSettings::default();
        assert_eq!(cfg.target_hwnd, 0);
        assert_eq!(cfg.scroll_steps, 2);
        assert!((cfg.threshold - 0.85).abs() < 1e-4);
    }

    #[test]
    fn test_ncc_template_matching_exact_detection() {
        let scr_w = 64u32;
        let scr_h = 64u32;
        let mut screen = vec![20.0f32; (scr_w * scr_h) as usize];

        let tw = 10u32;
        let th = 10u32;
        let mut tmpl = vec![0.0f32; (tw * th) as usize];

        // Create a recognizable gradient pattern for the target template
        for y in 0..th {
            for x in 0..tw {
                let val = 50.0 + (x as f32) * 12.0 + (y as f32) * 8.0;
                tmpl[(y * tw + x) as usize] = val;
            }
        }

        // Embed template inside screen at (24, 18)
        let embed_x = 24u32;
        let embed_y = 18u32;
        for y in 0..th {
            for x in 0..tw {
                screen[((embed_y + y) * scr_w + (embed_x + x)) as usize] =
                    tmpl[(y * tw + x) as usize];
            }
        }

        let (score, found_x, found_y) =
            match_template_ncc(&screen, scr_w, scr_h, &tmpl, tw, th, 2);
        assert!(score > 0.99, "Expected NCC score > 0.99, got {}", score);
        assert_eq!(found_x, embed_x);
        assert_eq!(found_y, embed_y);
    }
}

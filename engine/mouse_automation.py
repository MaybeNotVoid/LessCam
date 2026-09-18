"""
LessCam - Mouse Automation Engine
Provides window capture, periodic mouse jiggling, OpenCV multi-template matching (up to 4 targets),
automated click execution, post-click configurable mouse scrolling, and global hotkeys.
"""

import time
import random
from typing import List, Tuple, Dict, Any, Optional

import cv2
import numpy as np
import mss
import pyautogui
import win32gui
from PyQt6.QtCore import QObject, QThread, pyqtSignal

# Enable fail-safe: moving mouse to extreme screen corner raises exception and aborts
pyautogui.FAILSAFE = True
pyautogui.PAUSE = 0.05


def get_open_windows() -> List[Dict[str, Any]]:
    """
    Enumerate visible application windows on Windows desktop.
    Returns list of {'hwnd': int, 'title': str, 'rect': tuple}
    """
    windows = []

    def enum_callback(hwnd, _):
        if win32gui.IsWindowVisible(hwnd) and not win32gui.IsIconic(hwnd):
            title = win32gui.GetWindowText(hwnd).strip()
            if title and title not in ('Program Manager', 'Settings', 'Default IME', 'MSCTFIME UI'):
                rect = win32gui.GetWindowRect(hwnd)
                w = rect[2] - rect[0]
                h = rect[3] - rect[1]
                if w > 80 and h > 80:
                    windows.append({
                        'hwnd': hwnd,
                        'title': title,
                        'rect': rect
                    })
        return True

    try:
        win32gui.EnumWindows(enum_callback, None)
    except Exception as exc:
        print(f"[MouseAutomation] EnumWindows error: {exc}")

    windows.sort(key=lambda x: x['title'].lower())
    return windows


class HotkeyListener(QObject):
    """
    Global hotkey listener using low-level Windows keyboard hook.
    CTRL+ALT+A: Start Mouse Bot
    CTRL+ALT+D: Stop Mouse Bot
    """
    start_requested = pyqtSignal()
    stop_requested = pyqtSignal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._registered = False
        self.register_hotkeys()

    def register_hotkeys(self):
        try:
            import keyboard
            keyboard.add_hotkey('ctrl+alt+a', lambda: self.start_requested.emit(), suppress=False)
            keyboard.add_hotkey('ctrl+alt+d', lambda: self.stop_requested.emit(), suppress=False)
            self._registered = True
        except Exception as exc:
            print(f"[HotkeyListener] Error registering hotkeys: {exc}")

    def cleanup(self):
        if self._registered:
            try:
                import keyboard
                keyboard.remove_hotkey('ctrl+alt+a')
                keyboard.remove_hotkey('ctrl+alt+d')
            except Exception:
                pass
            self._registered = False


class MouseAutomationWorker(QThread):
    """
    Background worker that monitors the target window, executes periodic mouse moves,
    scans for up to 4 image templates, clicks them, and applies a post-click scroll down.
    """
    status_changed = pyqtSignal(str)
    action_performed = pyqtSignal(str)
    target_matched = pyqtSignal(int, str)  # (slot_index, image_name)
    error_occurred = pyqtSignal(str)
    stopped = pyqtSignal()

    def __init__(
        self,
        target_hwnd: int = 0,
        move_interval_sec: float = 10.0,
        periodic_move_enabled: bool = True,
        templates: Optional[List[Any]] = None,
        enable_scroll: bool = True,
        scroll_amount: int = -200,
        scroll_delay_sec: float = 0.2,
        threshold: float = 0.80,
        cooldown_sec: float = 1.8,
        parent=None
    ):
        super().__init__(parent)
        self.target_hwnd = target_hwnd
        self.move_interval_sec = max(1.0, float(move_interval_sec))
        self.periodic_move_enabled = periodic_move_enabled
        self.templates = templates or []
        self.enable_scroll = enable_scroll
        self.scroll_amount = scroll_amount if scroll_amount < 0 else -abs(scroll_amount)
        self.scroll_delay_sec = max(0.05, float(scroll_delay_sec))
        self.threshold = threshold
        self.cooldown_sec = cooldown_sec

        self._is_running = True

    def stop(self):
        """Stop worker loop gracefully."""
        self._is_running = False

    def update_settings(
        self,
        target_hwnd: int,
        move_interval_sec: float,
        periodic_move_enabled: bool,
        templates: List[Any],
        enable_scroll: bool,
        scroll_amount: int,
        scroll_delay_sec: float = 0.2,
        threshold: float = 0.85
    ):
        """Update runtime settings on-the-fly."""
        self.target_hwnd = target_hwnd
        self.move_interval_sec = max(1.0, float(move_interval_sec))
        self.periodic_move_enabled = periodic_move_enabled
        self.templates = templates
        self.enable_scroll = enable_scroll
        self.scroll_amount = scroll_amount if scroll_amount < 0 else -abs(scroll_amount)
        self.scroll_delay_sec = max(0.05, float(scroll_delay_sec))
        self.threshold = max(0.50, min(0.99, float(threshold)))

    def run(self):
        self.status_changed.emit("Automazione Mouse Avviata (CTRL+ALT+D per arrestare)")
        last_move_time = time.time()
        last_click_time = 0.0

        with mss.mss() as sct:
            while self._is_running:
                try:
                    # 1. Determine bounding box
                    bbox = None
                    if self.target_hwnd != 0:
                        if not win32gui.IsWindow(self.target_hwnd) or win32gui.IsIconic(self.target_hwnd):
                            self.status_changed.emit("Finestra target non visibile o minimizzata.")
                            time.sleep(1.0)
                            continue
                        rect = win32gui.GetWindowRect(self.target_hwnd)
                        l, t, r, b = rect
                        w, h = r - l, b - t
                        if w <= 10 or h <= 10:
                            time.sleep(0.5)
                            continue
                        bbox = {'left': l, 'top': t, 'width': w, 'height': h}
                    else:
                        # Full primary screen
                        bbox = sct.monitors[1]

                    # 2. Capture target region
                    sct_img = sct.grab(bbox)
                    screen_bgr = np.array(sct_img)[:, :, :3]
                    scr_h, scr_w = screen_bgr.shape[:2]

                    current_time = time.time()
                    image_found_this_cycle = False

                    # 3. Check for loaded template images (up to 4)
                    if self.templates and (current_time - last_click_time >= self.cooldown_sec):
                        for tmpl_entry in self.templates:
                            if not self._is_running:
                                break

                            # Parse template tuple
                            if len(tmpl_entry) >= 4:
                                slot_idx, tmpl_bgr, tmpl_name, slot_scroll = tmpl_entry[:4]
                            else:
                                slot_idx, tmpl_bgr, tmpl_name = tmpl_entry[:3]
                                slot_scroll = True

                            if tmpl_bgr is None:
                                continue

                            th, tw = tmpl_bgr.shape[:2]
                            if th > scr_h or tw > scr_w:
                                continue

                            # OpenCV Template Matching
                            res = cv2.matchTemplate(screen_bgr, tmpl_bgr, cv2.TM_CCOEFF_NORMED)
                            min_val, max_val, min_loc, max_loc = cv2.minMaxLoc(res)

                            if max_val >= self.threshold:
                                image_found_this_cycle = True
                                target_x = bbox['left'] + max_loc[0] + (tw // 2)
                                target_y = bbox['top'] + max_loc[1] + (th // 2)

                                # Optional: Bring window to foreground
                                if self.target_hwnd != 0:
                                    try:
                                        win32gui.SetForegroundWindow(self.target_hwnd)
                                    except Exception:
                                        pass

                                # Move mouse smoothly and click
                                self.status_changed.emit(f"Rilevato target: {tmpl_name} ({max_val:.2f})")
                                pyautogui.moveTo(target_x, target_y, duration=0.2)
                                pyautogui.click()

                                # Post-click configurable scroll down
                                did_scroll = False
                                if self.enable_scroll and slot_scroll:
                                    time.sleep(self.scroll_delay_sec)
                                    pyautogui.scroll(self.scroll_amount)
                                    did_scroll = True

                                msg = f"Cliccato [{tmpl_name}] in ({target_x}, {target_y})"
                                if did_scroll:
                                    scatti = abs(self.scroll_amount) // 100
                                    msg += f" + Scroll Down ({scatti} scatti)"

                                self.action_performed.emit(msg)
                                self.target_matched.emit(slot_idx, tmpl_name)
                                last_click_time = time.time()
                                last_move_time = time.time()
                                break  # Handled one match per cycle

                    # 4. Periodic Mouse Move (Anti-AFK / Jiggler)
                    if not image_found_this_cycle and self.periodic_move_enabled:
                        if (current_time - last_move_time) >= self.move_interval_sec:
                            win_cx = bbox['left'] + (bbox['width'] // 2)
                            win_cy = bbox['top'] + (bbox['height'] // 2)
                            offset_x = random.randint(-35, 35)
                            offset_y = random.randint(-35, 35)
                            dest_x = max(bbox['left'] + 10, min(bbox['left'] + bbox['width'] - 10, win_cx + offset_x))
                            dest_y = max(bbox['top'] + 10, min(bbox['top'] + bbox['height'] - 10, win_cy + offset_y))

                            pyautogui.moveTo(dest_x, dest_y, duration=0.25)
                            last_move_time = time.time()
                            self.status_changed.emit(f"Movimento periodico eseguito a ({dest_x}, {dest_y})")

                    time.sleep(0.15)

                except pyautogui.FailSafeException:
                    self.error_occurred.emit("FailSafe attivato: cursore spostato nell'angolo dello schermo. Bot arrestato.")
                    break
                except Exception as exc:
                    self.error_occurred.emit(f"Errore automazione: {exc}")
                    time.sleep(1.0)

        self.status_changed.emit("Automazione Mouse Disattivata.")
        self.stopped.emit()

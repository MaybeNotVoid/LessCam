"""
LessCam - Video Streaming & Virtual Camera Engine
Handles video capture (OpenCV), image playback, aspect-ratio scaling (letterbox/pillarbox),
high-precision framerate pacing, and broadcasting to PyVirtualCam.
"""

import time
import enum
import threading
from typing import Optional, Tuple

import cv2
import numpy as np
import pyvirtualcam
from PyQt6.QtCore import QThread, pyqtSignal
from PyQt6.QtGui import QImage


class SourceType(enum.Enum):
    STANDBY = 0
    LOCAL_VIDEO = 1
    YOUTUBE = 2
    IMAGE = 3


def letterbox_frame(frame: np.ndarray, target_w: int, target_h: int) -> np.ndarray:
    """
    Scale frame to fit within (target_w, target_h) preserving original aspect ratio.
    Fills remaining space with black borders (letterbox/pillarbox).
    """
    in_h, in_w = frame.shape[:2]
    if in_h == 0 or in_w == 0:
        return np.zeros((target_h, target_w, 3), dtype=np.uint8)

    scale = min(target_w / in_w, target_h / in_h)
    new_w = max(1, int(round(in_w * scale)))
    new_h = max(1, int(round(in_h * scale)))

    resized = cv2.resize(frame, (new_w, new_h), interpolation=cv2.INTER_LINEAR)
    canvas = np.zeros((target_h, target_w, 3), dtype=np.uint8)

    x_offset = (target_w - new_w) // 2
    y_offset = (target_h - new_h) // 2

    canvas[y_offset:y_offset + new_h, x_offset:x_offset + new_w] = resized
    return canvas


class VideoStreamWorker(QThread):
    """
    Dedicated QThread worker for capturing video frames, letterboxing,
    sending to Virtual Camera, and emitting preview frames to the GUI.
    """
    frame_ready = pyqtSignal(QImage)
    position_changed = pyqtSignal(float, float)  # (current_sec, total_sec)
    playback_state_changed = pyqtSignal(bool)    # True = playing, False = paused
    virtual_cam_status_changed = pyqtSignal(bool, str) # (active, message/device)
    source_loaded = pyqtSignal(str, float)       # (title, duration_sec)
    error_occurred = pyqtSignal(str)

    def __init__(self, target_w: int = 1280, target_h: int = 720, target_fps: int = 30):
        super().__init__()
        self.target_w = target_w
        self.target_h = target_h
        self.target_fps = target_fps

        # Stream & source state
        self._source_type = SourceType.STANDBY
        self._source_path: Optional[str] = None
        self._source_title = "Standby"
        self._is_playing = False
        self._loop = True
        self._seek_requested: Optional[float] = None
        self._duration: float = 0.0
        self._current_pos: float = 0.0
        self._total_frames = 0
        self._fps = float(target_fps)

        # Video capture & image objects
        self._cap: Optional[cv2.VideoCapture] = None
        self._cached_image_bgr: Optional[np.ndarray] = None
        self._lock = threading.Lock()

        # PyVirtualCam
        self._vcam_enabled = False
        self._vcam: Optional[pyvirtualcam.Camera] = None
        self._vcam_lock = threading.Lock()
        self._vcam_error_reported = False

        # Thread lifecycle
        self._is_running = True

    def run(self):
        """Main rendering & streaming loop."""
        standby_tick = 0

        while self._is_running:
            loop_start = time.perf_counter()
            frame_bgr = None

            with self._lock:
                source_type = self._source_type
                is_playing = self._is_playing
                seek_sec = self._seek_requested
                self._seek_requested = None

            # Handle seeking
            if seek_sec is not None and self._cap is not None and self._cap.isOpened():
                try:
                    self._cap.set(cv2.CAP_PROP_POS_MSEC, seek_sec * 1000.0)
                    self._current_pos = seek_sec
                except Exception as e:
                    print(f"[VideoStream] Seek error: {e}")

            # 1. STANDBY SOURCE
            if source_type == SourceType.STANDBY:
                standby_tick += 1
                frame_bgr = self._generate_standby_frame(standby_tick)

            # 2. STATIC IMAGE
            elif source_type == SourceType.IMAGE:
                if self._cached_image_bgr is not None:
                    frame_bgr = self._cached_image_bgr.copy()
                else:
                    frame_bgr = self._generate_standby_frame(standby_tick)

            # 3. VIDEO (LOCAL OR YOUTUBE)
            elif source_type in (SourceType.LOCAL_VIDEO, SourceType.YOUTUBE):
                if self._cap is not None and self._cap.isOpened():
                    if is_playing:
                        ret, frame = self._cap.read()
                        if ret:
                            frame_bgr = frame
                            # Update timestamp
                            pos_msec = self._cap.get(cv2.CAP_PROP_POS_MSEC)
                            self._current_pos = pos_msec / 1000.0 if pos_msec > 0 else (self._current_pos + 1.0 / self.target_fps)
                            self.position_changed.emit(self._current_pos, self._duration)
                        else:
                            # Reached EOF
                            if self._loop:
                                self._cap.set(cv2.CAP_PROP_POS_FRAMES, 0)
                                self._current_pos = 0.0
                                ret2, frame2 = self._cap.read()
                                if ret2:
                                    frame_bgr = frame2
                            else:
                                self._is_playing = False
                                self.playback_state_changed.emit(False)
                    else:
                        # Paused: keep showing last frame or current frame without advancing
                        ret, frame = self._cap.read()
                        if ret:
                            frame_bgr = frame
                            # Rewind 1 frame so we stay on current position
                            current_frame = int(self._cap.get(cv2.CAP_PROP_POS_FRAMES))
                            if current_frame > 1:
                                self._cap.set(cv2.CAP_PROP_POS_FRAMES, current_frame - 1)
                        else:
                            frame_bgr = self._generate_standby_frame(standby_tick)
                else:
                    frame_bgr = self._generate_standby_frame(standby_tick)

            if frame_bgr is None:
                frame_bgr = self._generate_standby_frame(standby_tick)

            # Aspect Ratio Preservation (Letterbox / Pillarbox)
            output_bgr = letterbox_frame(frame_bgr, self.target_w, self.target_h)

            # Broadcast to PyVirtualCam
            self._send_to_virtual_cam(output_bgr)

            # Emit QImage for GUI Preview
            # Convert BGR to RGB for Qt rendering
            rgb_frame = cv2.cvtColor(output_bgr, cv2.COLOR_BGR2RGB)
            h, w, ch = rgb_frame.shape
            bytes_per_line = ch * w
            qimg = QImage(rgb_frame.data, w, h, bytes_per_line, QImage.Format.Format_RGB888).copy()
            self.frame_ready.emit(qimg)

            # Frame Pacing: sleep to maintain precise FPS
            target_frame_time = 1.0 / self.target_fps
            elapsed = time.perf_counter() - loop_start
            sleep_time = target_frame_time - elapsed
            if sleep_time > 0.001:
                time.sleep(sleep_time)

        # Cleanup on thread exit
        self._close_virtual_cam()
        self._release_capture()

    def _generate_standby_frame(self, tick: int) -> np.ndarray:
        """Create a sleek dark standby frame with Fluent styling."""
        frame = np.zeros((self.target_h, self.target_w, 3), dtype=np.uint8)
        frame[:] = (20, 20, 22)  # Fluent Dark background

        # Subtle glowing accent line
        line_y = int(self.target_h * 0.52)
        cx = self.target_w // 2
        pulse = 0.5 + 0.5 * np.sin(tick * 0.05)
        accent_b = int(220 * pulse)
        accent_g = int(120 * pulse)
        accent_r = int(0 * pulse)
        cv2.line(frame, (cx - 150, line_y), (cx + 150, line_y), (accent_b, accent_g, accent_r), 2, cv2.LINE_AA)

        # Text
        text_title = "LessCam"
        text_sub = "Nessuna sorgente attiva - Carica YouTube, Video o Immagine"

        font = cv2.FONT_HERSHEY_DUPLEX
        cv2.putText(frame, text_title, (cx - 110, line_y - 30), font, 1.6, (245, 245, 245), 2, cv2.LINE_AA)
        cv2.putText(frame, text_sub, (cx - 240, line_y + 40), cv2.FONT_HERSHEY_SIMPLEX, 0.55, (160, 160, 160), 1, cv2.LINE_AA)
        return frame

    # ==================== Source Loading Methods ====================

    def load_local_video(self, file_path: str):
        """Open a local video file for playback."""
        with self._lock:
            self._release_capture()
            self._source_path = file_path
            self._source_type = SourceType.LOCAL_VIDEO
            self._source_title = file_path.split("/")[-1].split("\\")[-1]

            self._cap = cv2.VideoCapture(file_path)
            if not self._cap.isOpened():
                self.error_occurred.emit(f"Impossibile aprire il file video: {file_path}")
                self._source_type = SourceType.STANDBY
                return

            self._fps = self._cap.get(cv2.CAP_PROP_FPS) or float(self.target_fps)
            self._total_frames = int(self._cap.get(cv2.CAP_PROP_FRAME_COUNT))
            self._duration = (self._total_frames / self._fps) if self._fps > 0 else 0.0
            self._current_pos = 0.0
            self._is_playing = True

        self.source_loaded.emit(self._source_title, self._duration)
        self.playback_state_changed.emit(True)

    def load_youtube_video(self, video_url: str, duration: float, title: str):
        """Open an extracted YouTube video stream URL."""
        with self._lock:
            self._release_capture()
            self._source_path = video_url
            self._source_type = SourceType.YOUTUBE
            self._source_title = title

            self._cap = cv2.VideoCapture(video_url)
            if not self._cap.isOpened():
                self.error_occurred.emit("Impossibile connettersi allo stream video YouTube.")
                self._source_type = SourceType.STANDBY
                return

            self._fps = self._cap.get(cv2.CAP_PROP_FPS) or float(self.target_fps)
            self._duration = duration
            self._current_pos = 0.0
            self._is_playing = True

        self.source_loaded.emit(title, duration)
        self.playback_state_changed.emit(True)

    def load_image(self, image_path: str):
        """Load a static image as continuous video frame."""
        with self._lock:
            self._release_capture()
            self._source_path = image_path
            self._source_type = SourceType.IMAGE
            self._source_title = image_path.split("/")[-1].split("\\")[-1]

            img = cv2.imread(image_path)
            if img is None:
                self.error_occurred.emit(f"Impossibile caricare l'immagine: {image_path}")
                self._source_type = SourceType.STANDBY
                return

            self._cached_image_bgr = img
            self._duration = 0.0
            self._current_pos = 0.0
            self._is_playing = True

        self.source_loaded.emit(self._source_title, 0.0)
        self.playback_state_changed.emit(True)

    def clear_source(self):
        """Return to Standby mode."""
        with self._lock:
            self._release_capture()
            self._source_type = SourceType.STANDBY
            self._source_title = "Standby"
            self._duration = 0.0
            self._current_pos = 0.0
            self._is_playing = False

        self.source_loaded.emit("Standby", 0.0)
        self.playback_state_changed.emit(False)

    def _release_capture(self):
        """Internal cleanup of active cv2 capture."""
        if self._cap is not None:
            try:
                self._cap.release()
            except Exception:
                pass
            self._cap = None
        self._cached_image_bgr = None

    # ==================== Playback Controls ====================

    def play(self):
        with self._lock:
            self._is_playing = True
        self.playback_state_changed.emit(True)

    def pause(self):
        with self._lock:
            self._is_playing = False
        self.playback_state_changed.emit(False)

    def toggle_play_pause(self):
        with self._lock:
            self._is_playing = not self._is_playing
            new_state = self._is_playing
        self.playback_state_changed.emit(new_state)

    def seek(self, target_sec: float):
        with self._lock:
            self._seek_requested = max(0.0, target_sec)

    def set_loop(self, loop: bool):
        with self._lock:
            self._loop = loop

    def set_output_resolution(self, width: int, height: int):
        """Update virtual cam and canvas output resolution."""
        with self._lock:
            self.target_w = width
            self.target_h = height

        # Reinitialize virtual camera if active
        if self._vcam_enabled:
            self._restart_virtual_cam()

    def set_target_fps(self, fps: int):
        """Update target FPS."""
        with self._lock:
            self.target_fps = fps

        if self._vcam_enabled:
            self._restart_virtual_cam()

    # ==================== PyVirtualCam Management ====================

    def start_virtual_cam(self):
        """Enable broadcasting to PyVirtualCam."""
        with self._vcam_lock:
            self._vcam_enabled = True
            self._restart_virtual_cam()

    def stop_virtual_cam(self):
        """Disable broadcasting."""
        with self._vcam_lock:
            self._vcam_enabled = False
            self._close_virtual_cam()
            self.virtual_cam_status_changed.emit(False, "Webcam Virtuale Disattivata")

    def _restart_virtual_cam(self):
        """Safely recreate PyVirtualCam instance."""
        self._close_virtual_cam()
        try:
            # Initialize with BGR pixel format matching OpenCV output
            self._vcam = pyvirtualcam.Camera(
                width=self.target_w,
                height=self.target_h,
                fps=self.target_fps,
                fmt=pyvirtualcam.PixelFormat.BGR
            )
            device_name = self._vcam.device
            self.virtual_cam_status_changed.emit(True, f"Attiva: {device_name}")
            self._vcam_error_reported = False
        except Exception as exc:
            self._vcam = None
            self._vcam_enabled = False
            err_msg = str(exc)
            if not self._vcam_error_reported:
                self.error_occurred.emit(
                    f"Driver Webcam Virtuale non trovato o occupato.\n"
                    f"Installa OBS Studio (e avvia 'Avvia Fotocamera Virtuale') oppure Unity Capture per abilitare la trasmissione.\n"
                    f"Dettaglio: {err_msg}"
                )
                self._vcam_error_reported = True
            self.virtual_cam_status_changed.emit(False, "Driver non disponibile (Installa OBS VirtualCam)")

    def _send_to_virtual_cam(self, frame_bgr: np.ndarray):
        """Send output frame to PyVirtualCam if running."""
        with self._vcam_lock:
            if self._vcam is not None and self._vcam_enabled:
                try:
                    self._vcam.send(frame_bgr)
                except Exception as exc:
                    print(f"[VideoStream] Error sending frame to vcam: {exc}")

    def _close_virtual_cam(self):
        """Safely release the virtual camera device."""
        if self._vcam is not None:
            try:
                self._vcam.close()
            except Exception:
                pass
            self._vcam = None

    def stop_worker(self):
        """Gracefully stop thread."""
        self._is_running = False
        self.wait(2000)

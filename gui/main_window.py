"""
LessCam - Main Application Window
Modern Windows 11 Fluent Design window integrating the video pipeline, audio routing,
multi-source switcher, preview canvas, and driver diagnostics.
"""

import sys
from typing import Optional

from PyQt6.QtCore import Qt, QTimer
from PyQt6.QtWidgets import (
    QMainWindow, QWidget, QHBoxLayout, QVBoxLayout,
    QSplitter, QScrollArea, QDialog, QLabel, QStackedWidget
)
from PyQt6.QtGui import QIcon, QFont, QCloseEvent

from qfluentwidgets import (
    setTheme, Theme, TitleLabel, SubtitleLabel, CaptionLabel,
    PrimaryPushButton, PushButton, ToolButton, FluentIcon,
    InfoBar, InfoBarPosition, IconWidget, MessageBox, SegmentedWidget
)

from engine.video_stream import VideoStreamWorker
from engine.audio_stream import AudioEngine, get_audio_output_devices
from engine.youtube import YouTubeExtractorThread
from gui.preview_widget import VideoPreviewWidget
from gui.source_panel import SourcePanel
from gui.camera_panel import CameraPanel
from gui.audio_panel import AudioPanel
from gui.mouse_panel import MousePanel


class PrerequisitesDialog(MessageBox):
    """Informational dialog explaining how to set up OBS VirtualCam and VB-Audio Cable."""
    def __init__(self, parent=None):
        super().__init__(
            title="Guida Prerequisiti - LessCam",
            content=(
                "Per sfruttare appieno LessCam come regia virtuale in altre applicazioni:\n\n"
                "1. WEBCAM VIRTUALE (OBS Studio):\n"
                "   - Installa OBS Studio gratuito da https://obsproject.com\n"
                "   - Avvia OBS almeno una volta e clicca su 'Avvia fotocamera virtuale' "
                "per registrare il driver DirectShow nel sistema Windows.\n"
                "   - LessCam utilizzerà automaticamente questo driver per trasmettere il flusso video a Zoom, Meet, Teams e Discord!\n\n"
                "2. AUDIO VIRTUALE (VB-Audio Cable):\n"
                "   - Scarica il driver gratuito VB-CABLE da https://vb-audio.com/Cable/\n"
                "   - Estrai ed esegui 'VBCABLE_Setup_x64.exe' come Amministratore.\n"
                "   - In LessCam, seleziona 'CABLE Input' nel Mixer Audio.\n"
                "   - In Discord/Meet/Zoom, imposta il tuo microfono su 'CABLE Output' per far ascoltare l'audio ai tuoi interlocutori!"
            ),
            parent=parent
        )
        self.yesButton.setText("Ho capito")
        self.cancelButton.hide()


class LessCamMainWindow(QMainWindow):
    """
    Main LessCam Window coordinating GUI panels and background streaming engines.
    """
    def __init__(self):
        super().__init__()
        self.setWindowTitle("LessCam - Virtual Camera & Audio Studio")
        self.setMinimumSize(1080, 720)
        self.resize(1280, 800)

        # Apply dark mode Fluent design
        setTheme(Theme.DARK)
        self.setStyleSheet("background-color: #121214;")

        # Background Engines
        self.video_worker = VideoStreamWorker(target_w=1280, target_h=720, target_fps=30)
        self.audio_engine = AudioEngine()
        self.yt_extractor: Optional[YouTubeExtractorThread] = None

        # VU-Meter poll timer (33ms = 30 FPS)
        self.meter_timer = QTimer(self)
        self.meter_timer.setInterval(33)
        self.meter_timer.timeout.connect(self._poll_audio_meter)
        self.meter_timer.start()

        # Build UI
        self._init_ui()

        # Connect signals
        self._connect_signals()

        # Populate audio devices
        self._refresh_audio_devices()

        # Start video worker thread
        self.video_worker.start()

    def _init_ui(self):
        central_widget = QWidget(self)
        self.setCentralWidget(central_widget)

        root_layout = QVBoxLayout(central_widget)
        root_layout.setContentsMargins(18, 14, 18, 14)
        root_layout.setSpacing(14)

        # 1. Top Header Bar
        header_bar = QHBoxLayout()
        header_bar.setSpacing(12)

        icon_title = IconWidget(FluentIcon.CAMERA, self)
        icon_title.setFixedSize(26, 26)
        header_bar.addWidget(icon_title)

        title_label = TitleLabel("LessCam", self)
        title_label.setFont(QFont("Segoe UI", 18, QFont.Weight.Bold))
        header_bar.addWidget(title_label)

        sub_label = CaptionLabel("Virtual Camera & Audio Studio", self)
        sub_label.setStyleSheet("color: #0078D4; font-weight: 500;")
        header_bar.addWidget(sub_label)

        header_bar.addStretch()

        btn_prereq = PushButton(FluentIcon.QUESTION, "Guida Prerequisiti (OBS & Cable)", self)
        btn_prereq.clicked.connect(self._show_prerequisites)
        header_bar.addWidget(btn_prereq)

        root_layout.addLayout(header_bar)

        # 2. Main Content Splitter / Layout (Two Columns)
        content_layout = QHBoxLayout()
        content_layout.setSpacing(16)

        # LEFT COLUMN: Preview Canvas + Source Controls & Timeline
        left_col = QVBoxLayout()
        left_col.setSpacing(14)

        self.preview_widget = VideoPreviewWidget(self)
        self.preview_widget.setMinimumHeight(380)
        left_col.addWidget(self.preview_widget, stretch=3)

        self.source_panel = SourcePanel(self)
        left_col.addWidget(self.source_panel, stretch=2)

        content_layout.addLayout(left_col, stretch=6)

        # RIGHT COLUMN: Segmented tabs for Regia (Cam & Audio) and Mouse Bot
        right_col = QVBoxLayout()
        right_col.setSpacing(10)

        self.right_segmented = SegmentedWidget(self)
        self.right_segmented.addItem("studio", "Regia & Audio", onClick=lambda: self.right_stack.setCurrentIndex(0))
        self.right_segmented.addItem("mouse", "Automazione Mouse", onClick=lambda: self.right_stack.setCurrentIndex(1))
        right_col.addWidget(self.right_segmented)

        self.right_stack = QStackedWidget(self)

        # Page 0: Regia & Audio
        studio_page = QWidget()
        studio_layout = QVBoxLayout(studio_page)
        studio_layout.setContentsMargins(0, 4, 0, 0)
        studio_layout.setSpacing(12)

        self.camera_panel = CameraPanel(self)
        studio_layout.addWidget(self.camera_panel)

        self.audio_panel = AudioPanel(self)
        studio_layout.addWidget(self.audio_panel)
        studio_layout.addStretch()
        self.right_stack.addWidget(studio_page)

        # Page 1: Automazione Mouse
        mouse_page = QWidget()
        mouse_layout = QVBoxLayout(mouse_page)
        mouse_layout.setContentsMargins(0, 4, 0, 0)
        mouse_layout.setSpacing(12)

        self.mouse_panel = MousePanel(self)
        mouse_layout.addWidget(self.mouse_panel)
        mouse_layout.addStretch()
        self.right_stack.addWidget(mouse_page)

        right_col.addWidget(self.right_stack)
        self.right_segmented.setCurrentItem("studio")
        self.right_stack.setCurrentIndex(0)

        content_layout.addLayout(right_col, stretch=4)

        root_layout.addLayout(content_layout)

    def _connect_signals(self):
        # Video Worker -> GUI
        self.video_worker.frame_ready.connect(self.preview_widget.update_frame)
        self.video_worker.position_changed.connect(self.source_panel.update_position)
        self.video_worker.playback_state_changed.connect(self._on_playback_state_changed)
        self.video_worker.virtual_cam_status_changed.connect(self._on_vcam_status_changed)
        self.video_worker.source_loaded.connect(self.source_panel.set_source_info)
        self.video_worker.error_occurred.connect(self._show_warning_bar)

        # Source Panel -> Video & Audio
        self.source_panel.youtube_requested.connect(self._on_youtube_requested)
        self.source_panel.local_video_requested.connect(self._on_local_video_requested)
        self.source_panel.compression_completed.connect(self._on_compression_completed)
        self.source_panel.image_requested.connect(self._on_image_requested)
        self.source_panel.standby_requested.connect(self._on_standby_requested)
        self.source_panel.play_pause_toggled.connect(self._on_play_pause_toggled)
        self.source_panel.seek_requested.connect(self._on_seek_requested)
        self.source_panel.loop_toggled.connect(self._on_loop_toggled)

        # Camera Panel -> Video Worker
        self.camera_panel.vcam_toggled.connect(self._on_vcam_toggled)
        self.camera_panel.resolution_changed.connect(self._on_res_changed)
        self.camera_panel.fps_changed.connect(self._on_fps_changed)

        # Audio Panel -> Audio Engine
        self.audio_panel.device_selected.connect(self.audio_engine.set_output_device)
        self.audio_panel.volume_changed.connect(self.audio_engine.set_volume)
        self.audio_panel.mute_toggled.connect(self.audio_engine.set_muted)
        self.audio_panel.refresh_requested.connect(self._refresh_audio_devices)
        self.audio_engine.error_occurred.connect(self._show_warning_bar)

    # ==================== Source Handling ====================

    def _on_youtube_requested(self, url: str, res: int):
        self.preview_widget.set_buffering(True, "Estrazione stream YouTube...")

        if self.yt_extractor and self.yt_extractor.isRunning():
            self.yt_extractor.cancel()
            self.yt_extractor.wait()

        self.yt_extractor = YouTubeExtractorThread(url, target_resolution=res, parent=self)
        self.yt_extractor.success.connect(self._on_youtube_success)
        self.yt_extractor.error.connect(self._on_youtube_error)
        self.yt_extractor.start()

    def _on_youtube_success(self, meta: dict):
        self.preview_widget.set_buffering(False)
        title = meta.get('title', 'YouTube Video')
        duration = meta.get('duration', 0.0)
        video_url = meta.get('video_url')
        audio_url = meta.get('audio_url')

        self.source_panel.set_youtube_status(f"Pronto: {title} ({meta.get('height')}p)")
        self.video_worker.load_youtube_video(video_url, duration, title)

        if audio_url:
            self.audio_engine.load_source(audio_url, duration)
            self.audio_engine.play()

        InfoBar.success(
            title="Stream YouTube Caricato",
            content=f"Riproduzione di: {title[:45]}...",
            orient=Qt.Orientation.Horizontal,
            isClosable=True,
            position=InfoBarPosition.TOP_RIGHT,
            duration=3500,
            parent=self
        )

    def _on_youtube_error(self, err_msg: str):
        self.preview_widget.set_buffering(False)
        self.source_panel.set_youtube_status(err_msg, is_error=True)
        InfoBar.error(
            title="Errore YouTube",
            content=err_msg,
            orient=Qt.Orientation.Horizontal,
            isClosable=True,
            position=InfoBarPosition.TOP_RIGHT,
            duration=6000,
            parent=self
        )

    def _on_local_video_requested(self, path: str):
        self.video_worker.load_local_video(path)
        # Load audio simultaneously
        self.audio_engine.load_source(path, self.video_worker._duration)
        self.audio_engine.play()

    def _on_compression_completed(self, orig_size: int, new_size: int, pct_saved: int):
        orig_mb = orig_size / (1024 * 1024)
        new_mb = new_size / (1024 * 1024)
        InfoBar.success(
            title="Video Ottimizzato con Successo",
            content=f"Dimensione ridotta: {orig_mb:.1f} MB ➔ {new_mb:.1f} MB (-{pct_saved}%)",
            orient=Qt.Orientation.Horizontal,
            isClosable=True,
            position=InfoBarPosition.TOP_RIGHT,
            duration=4500,
            parent=self
        )

    def _on_image_requested(self, path: str):
        self.video_worker.load_image(path)
        self.audio_engine.stop()

    def _on_standby_requested(self):
        self.video_worker.clear_source()
        self.audio_engine.stop()

    # ==================== Playback & Sync ====================

    def _on_play_pause_toggled(self):
        self.video_worker.toggle_play_pause()

    def _on_playback_state_changed(self, is_playing: bool):
        self.source_panel.set_playback_state(is_playing)
        if is_playing:
            self.audio_engine.play()
        else:
            self.audio_engine.pause()

    def _on_seek_requested(self, seconds: float):
        self.video_worker.seek(seconds)
        self.audio_engine.seek(seconds)

    def _on_loop_toggled(self, loop: bool):
        self.video_worker.set_loop(loop)
        self.audio_engine.set_loop(loop)

    # ==================== Virtual Camera & Audio ====================

    def _on_vcam_toggled(self, checked: bool):
        if checked:
            self.video_worker.start_virtual_cam()
        else:
            self.video_worker.stop_virtual_cam()

    def _on_res_changed(self, w: int, h: int):
        self.video_worker.set_output_resolution(w, h)
        fps = self.video_worker.target_fps
        res_name = "1080p" if h == 1080 else "720p"
        self.preview_widget.set_resolution_info(f"{res_name} | {fps} FPS")

    def _on_fps_changed(self, fps: int):
        self.video_worker.set_target_fps(fps)
        h = self.video_worker.target_h
        res_name = "1080p" if h == 1080 else "720p"
        self.preview_widget.set_resolution_info(f"{res_name} | {fps} FPS")

    def _on_vcam_status_changed(self, is_active: bool, message: str):
        self.camera_panel.set_status(is_active, message)
        self.preview_widget.set_live_status(is_active)

    def _poll_audio_meter(self):
        """Poll real-time Left/Right audio levels for VU-Meter."""
        l, r = self.audio_engine.get_meter_levels()
        self.audio_panel.update_vu_levels(l, r)

    def _refresh_audio_devices(self):
        devices = get_audio_output_devices()
        current_dev = self.audio_engine._selected_device or -1
        self.audio_panel.populate_devices(devices, current_dev)

    def _show_warning_bar(self, message: str):
        InfoBar.warning(
            title="Avviso LessCam",
            content=message,
            orient=Qt.Orientation.Horizontal,
            isClosable=True,
            position=InfoBarPosition.TOP_RIGHT,
            duration=7000,
            parent=self
        )

    def _show_prerequisites(self):
        dialog = PrerequisitesDialog(self)
        dialog.exec()

    def closeEvent(self, event: QCloseEvent):
        """Clean shutdown of background threads, mouse bot, and audio streams."""
        self.meter_timer.stop()
        if self.yt_extractor and self.yt_extractor.isRunning():
            self.yt_extractor.cancel()
            self.yt_extractor.wait(1000)

        self.mouse_panel.stop_bot()
        self.mouse_panel.hotkeys.cleanup()
        self.audio_engine.stop()
        self.video_worker.stop_virtual_cam()
        self.video_worker.stop_worker()
        super().closeEvent(event)

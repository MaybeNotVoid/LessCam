"""
LessCam - Source Switcher & Playback Control Panel
Fluent panel for switching between YouTube streams, local video files, and static images,
with timeline scrubbing, continuous loop, and playback controls.
"""

import os
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QFileDialog, QStackedWidget, QDialog
)
from qfluentwidgets import (
    SimpleCardWidget, SegmentedWidget, LineEdit, PrimaryPushButton, PushButton,
    ToolButton, Slider, CaptionLabel, SubtitleLabel, BodyLabel, FluentIcon,
    IconWidget, ComboBox
)

from gui.trim_dialog import VideoTrimDialog


def format_timestamp(seconds: float) -> str:
    """Format seconds into HH:MM:SS or MM:SS."""
    if seconds <= 0 or not (seconds < 360000):
        return "00:00"
    total_sec = int(seconds)
    hours = total_sec // 3600
    minutes = (total_sec % 3600) // 60
    secs = total_sec % 60
    if hours > 0:
        return f"{hours:02d}:{minutes:02d}:{secs:02d}"
    return f"{minutes:02d}:{secs:02d}"


class SourcePanel(SimpleCardWidget):
    """
    Source selector and media transport bar.
    """
    youtube_requested = pyqtSignal(str, int)  # (url, resolution)
    local_video_requested = pyqtSignal(str)   # path
    compression_completed = pyqtSignal(int, int, int) # (orig_size, new_size, pct_saved)
    image_requested = pyqtSignal(str)         # path
    standby_requested = pyqtSignal()

    play_pause_toggled = pyqtSignal()
    stop_requested = pyqtSignal()
    seek_requested = pyqtSignal(float)        # target in seconds
    loop_toggled = pyqtSignal(bool)           # loop enabled

    def __init__(self, parent=None):
        super().__init__(parent)
        self._total_duration = 0.0
        self._is_user_scrubbing = False
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(18, 16, 18, 16)
        layout.setSpacing(14)

        # Header
        header_layout = QHBoxLayout()
        header_layout.setSpacing(10)

        icon_widget = IconWidget(FluentIcon.VIDEO, self)
        icon_widget.setFixedSize(22, 22)
        header_layout.addWidget(icon_widget)

        title = SubtitleLabel("Sorgente Multimediale", self)
        header_layout.addWidget(title)
        header_layout.addStretch()

        layout.addLayout(header_layout)

        # Segmented Source Selector
        self.segmented = SegmentedWidget(self)
        self.segmented.addItem("youtube", "YouTube", onClick=lambda: self.stack.setCurrentIndex(0))
        self.segmented.addItem("video", "Video Locale", onClick=lambda: self.stack.setCurrentIndex(1))
        self.segmented.addItem("image", "Immagine", onClick=lambda: self.stack.setCurrentIndex(2))
        layout.addWidget(self.segmented)

        # Stacked Pages for Source Config
        self.stack = QStackedWidget(self)

        # 1. YouTube Page
        yt_page = QWidget()
        yt_layout = QVBoxLayout(yt_page)
        yt_layout.setContentsMargins(0, 8, 0, 0)
        yt_layout.setSpacing(10)

        yt_row = QHBoxLayout()
        yt_row.setSpacing(8)

        self.input_yt_url = LineEdit(self)
        self.input_yt_url.setPlaceholderText("Incolla link YouTube (es. https://www.youtube.com/watch?v=...)")
        self.input_yt_url.setClearButtonEnabled(True)
        self.input_yt_url.returnPressed.connect(self._on_load_youtube)

        self.combo_yt_res = ComboBox(self)
        self.combo_yt_res.addItem("1080p (Full HD)", userData=1080)
        self.combo_yt_res.addItem("720p (HD)", userData=720)
        self.combo_yt_res.addItem("480p (SD)", userData=480)
        self.combo_yt_res.setCurrentIndex(0)
        self.combo_yt_res.setFixedWidth(140)

        self.btn_load_yt = PrimaryPushButton(FluentIcon.PLAY, "Carica & Play", self)
        self.btn_load_yt.clicked.connect(self._on_load_youtube)

        yt_row.addWidget(self.input_yt_url, stretch=1)
        yt_row.addWidget(self.combo_yt_res)
        yt_row.addWidget(self.btn_load_yt)
        yt_layout.addLayout(yt_row)

        self.lbl_yt_info = CaptionLabel("Nessun video YouTube caricato.", self)
        self.lbl_yt_info.setStyleSheet("color: #888888;")
        yt_layout.addWidget(self.lbl_yt_info)
        self.stack.addWidget(yt_page)

        # 2. Local Video Page
        vid_page = QWidget()
        vid_layout = QVBoxLayout(vid_page)
        vid_layout.setContentsMargins(0, 8, 0, 0)
        vid_layout.setSpacing(10)

        vid_row = QHBoxLayout()
        self.btn_browse_vid = PushButton(FluentIcon.FOLDER, "Seleziona Video Locale...", self)
        self.btn_browse_vid.clicked.connect(self._on_browse_video)

        self.lbl_vid_path = BodyLabel("Nessun file selezionato (Supportati: MP4, MKV, AVI, MOV, WEBM)", self)
        self.lbl_vid_path.setStyleSheet("color: #aaaaaa;")

        vid_row.addWidget(self.btn_browse_vid)
        vid_row.addWidget(self.lbl_vid_path, stretch=1)
        vid_layout.addLayout(vid_row)
        self.stack.addWidget(vid_page)

        # 3. Static Image Page
        img_page = QWidget()
        img_layout = QVBoxLayout(img_page)
        img_layout.setContentsMargins(0, 8, 0, 0)
        img_layout.setSpacing(10)

        img_row = QHBoxLayout()
        self.btn_browse_img = PushButton(FluentIcon.PHOTO, "Seleziona Immagine...", self)
        self.btn_browse_img.clicked.connect(self._on_browse_image)

        self.lbl_img_path = BodyLabel("Nessuna immagine selezionata (Supportati: PNG, JPG, WEBP, BMP)", self)
        self.lbl_img_path.setStyleSheet("color: #aaaaaa;")

        img_row.addWidget(self.btn_browse_img)
        img_row.addWidget(self.lbl_img_path, stretch=1)
        img_layout.addLayout(img_row)
        self.stack.addWidget(img_page)

        layout.addWidget(self.stack)

        # Transport & Timeline Scrubber
        timeline_box = QVBoxLayout()
        timeline_box.setSpacing(6)

        # Timeline Slider with Labels
        time_row = QHBoxLayout()
        self.lbl_current_time = CaptionLabel("00:00", self)
        self.lbl_current_time.setFixedWidth(50)

        self.slider_timeline = Slider(Qt.Orientation.Horizontal, self)
        self.slider_timeline.setRange(0, 1000)
        self.slider_timeline.setValue(0)
        self.slider_timeline.sliderPressed.connect(self._on_slider_pressed)
        self.slider_timeline.sliderReleased.connect(self._on_slider_released)

        self.lbl_total_time = CaptionLabel("00:00", self)
        self.lbl_total_time.setFixedWidth(50)
        self.lbl_total_time.setAlignment(Qt.AlignmentFlag.AlignRight)

        time_row.addWidget(self.lbl_current_time)
        time_row.addWidget(self.slider_timeline, stretch=1)
        time_row.addWidget(self.lbl_total_time)
        timeline_box.addLayout(time_row)

        # Transport Buttons
        btn_row = QHBoxLayout()
        btn_row.setSpacing(10)

        self.btn_play_pause = PrimaryPushButton(FluentIcon.PAUSE, "Pausa", self)
        self.btn_play_pause.clicked.connect(self.play_pause_toggled.emit)

        self.btn_stop = ToolButton(FluentIcon.CANCEL, self)
        self.btn_stop.setToolTip("Arresta e passa a Standby")
        self.btn_stop.clicked.connect(self.standby_requested.emit)

        self.btn_loop = ToolButton(FluentIcon.SYNC, self)
        self.btn_loop.setCheckable(True)
        self.btn_loop.setChecked(True)
        self.btn_loop.setToolTip("Riproduzione continua in loop (Attivo)")
        self.btn_loop.clicked.connect(self._on_loop_clicked)

        btn_row.addWidget(self.btn_play_pause)
        btn_row.addWidget(self.btn_stop)
        btn_row.addWidget(self.btn_loop)
        btn_row.addStretch()

        self.lbl_source_title = CaptionLabel("LessCam pronto", self)
        self.lbl_source_title.setStyleSheet("color: #0078D4; font-weight: bold;")
        btn_row.addWidget(self.lbl_source_title)

        timeline_box.addLayout(btn_row)
        layout.addLayout(timeline_box)

        # Set default tab
        self.segmented.setCurrentItem("youtube")
        self.stack.setCurrentIndex(0)

    def _on_load_youtube(self):
        url = self.input_yt_url.text().strip()
        if not url:
            return
        res = self.combo_yt_res.currentData() or 1080
        self.lbl_yt_info.setText("Connessione ed estrazione stream in corso...")
        self.lbl_yt_info.setStyleSheet("color: #0078D4;")
        self.youtube_requested.emit(url, res)

    def _on_browse_video(self):
        file_path, _ = QFileDialog.getOpenFileName(
            self,
            "Seleziona File Video",
            "",
            "Video Files (*.mp4 *.mkv *.avi *.mov *.webm *.flv);;Tutti i File (*.*)"
        )
        if file_path:
            dialog = VideoTrimDialog(file_path, parent=self)
            if dialog.exec() == QDialog.DialogCode.Accepted:
                final_path = dialog.result_file_path
                name = os.path.basename(final_path)
                self.lbl_vid_path.setText(name)
                self.local_video_requested.emit(final_path)
                if dialog.was_compressed:
                    saved_pct = int(round((1.0 - (dialog.new_size / max(1, dialog.orig_size))) * 100))
                    self.compression_completed.emit(dialog.orig_size, dialog.new_size, saved_pct)

    def _on_browse_image(self):
        file_path, _ = QFileDialog.getOpenFileName(
            self,
            "Seleziona Immagine",
            "",
            "Immagini (*.png *.jpg *.jpeg *.webp *.bmp);;Tutti i File (*.*)"
        )
        if file_path:
            name = file_path.split("/")[-1].split("\\")[-1]
            self.lbl_img_path.setText(name)
            self.image_requested.emit(file_path)

    def _on_slider_pressed(self):
        self._is_user_scrubbing = True

    def _on_slider_released(self):
        self._is_user_scrubbing = False
        if self._total_duration > 0:
            fraction = self.slider_timeline.value() / 1000.0
            target_sec = fraction * self._total_duration
            self.seek_requested.emit(target_sec)

    def _on_loop_clicked(self):
        is_loop = self.btn_loop.isChecked()
        tooltip = "Riproduzione continua in loop (Attivo)" if is_loop else "Riproduzione continua in loop (Disattivo)"
        self.btn_loop.setToolTip(tooltip)
        self.loop_toggled.emit(is_loop)

    def update_position(self, current_sec: float, total_sec: float):
        """Update timeline slider and time stamps from video worker."""
        self._total_duration = total_sec
        self.lbl_current_time.setText(format_timestamp(current_sec))
        self.lbl_total_time.setText(format_timestamp(total_sec))

        if not self._is_user_scrubbing and total_sec > 0:
            val = int((current_sec / total_sec) * 1000)
            self.slider_timeline.blockSignals(True)
            self.slider_timeline.setValue(min(1000, max(0, val)))
            self.slider_timeline.blockSignals(False)

    def set_playback_state(self, is_playing: bool):
        """Update Play/Pause button appearance."""
        if is_playing:
            self.btn_play_pause.setIcon(FluentIcon.PAUSE)
            self.btn_play_pause.setText("Pausa")
        else:
            self.btn_play_pause.setIcon(FluentIcon.PLAY)
            self.btn_play_pause.setText("Play")

    def set_source_info(self, title: str, duration: float):
        """Display loaded title and duration."""
        self._total_duration = duration
        self.lbl_source_title.setText(f"In riproduzione: {title}")
        self.lbl_total_time.setText(format_timestamp(duration))
        if duration <= 0:
            self.slider_timeline.setValue(0)
            self.lbl_current_time.setText("00:00")

    def set_youtube_status(self, text: str, is_error: bool = False):
        """Update status line below YouTube URL."""
        self.lbl_yt_info.setText(text)
        if is_error:
            self.lbl_yt_info.setStyleSheet("color: #E81123;")
        else:
            self.lbl_yt_info.setStyleSheet("color: #107C41;")

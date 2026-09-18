"""
LessCam - Video Trimming & Compression Dialog
Modern Fluent UI dialog allowing users to trim start/end times and compress local videos
for minimal file size and optimal streaming performance.
"""

import os
import time
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QDialog, QVBoxLayout, QHBoxLayout, QWidget, QFrame
)
from qfluentwidgets import (
    TitleLabel, SubtitleLabel, BodyLabel, CaptionLabel,
    CheckBox, SwitchButton, ComboBox, PrimaryPushButton, PushButton,
    ProgressBar, FluentIcon, IconWidget, DoubleSpinBox, SpinBox
)

from engine.video_trimmer import get_video_metadata, VideoCompressorWorker


def format_bytes(size_bytes: int) -> str:
    """Format bytes into readable string (e.g. 24.5 MB)."""
    if size_bytes < 1024:
        return f"{size_bytes} B"
    elif size_bytes < 1024 * 1024:
        return f"{size_bytes / 1024:.1f} KB"
    elif size_bytes < 1024 * 1024 * 1024:
        return f"{size_bytes / (1024 * 1024):.1f} MB"
    return f"{size_bytes / (1024 * 1024 * 1024):.2f} GB"


def format_time_hms(seconds: float) -> str:
    """Format seconds into HH:MM:SS."""
    sec_int = int(seconds)
    h = sec_int // 3600
    m = (sec_int % 3600) // 60
    s = sec_int % 60
    if h > 0:
        return f"{h:02d}:{m:02d}:{s:02d}"
    return f"{m:02d}:{s:02d}"


class VideoTrimDialog(QDialog):
    """
    Fluent dialog for video trimming and compression.
    """
    def __init__(self, file_path: str, parent=None):
        super().__init__(parent)
        self.file_path = file_path
        self.result_file_path = file_path
        self.was_compressed = False
        self.orig_size = 0
        self.new_size = 0

        self.meta = get_video_metadata(file_path)
        self.duration = self.meta.get('duration', 0.0)

        self.worker: VideoCompressorWorker = None

        self.setWindowTitle("Ottimizzazione Video Locale - LessCam")
        self.setMinimumWidth(560)
        self.setStyleSheet("background-color: #1e1e22; color: #f0f0f0;")

        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(24, 20, 24, 20)
        layout.setSpacing(16)

        # Header with Icon
        header = QHBoxLayout()
        header.setSpacing(12)

        icon = IconWidget(FluentIcon.EDIT, self)
        icon.setFixedSize(26, 26)
        header.addWidget(icon)

        title = SubtitleLabel("Trimming e Compressione Video", self)
        header.addWidget(title)
        header.addStretch()
        layout.addLayout(header)

        # File Details Card
        card = QFrame(self)
        card.setStyleSheet("background-color: #26262b; border-radius: 8px; padding: 10px;")
        card_layout = QVBoxLayout(card)
        card_layout.setSpacing(6)

        lbl_name = BodyLabel(f"File: {self.meta['file_name']}", card)
        lbl_name.setStyleSheet("font-weight: bold;")
        card_layout.addWidget(lbl_name)

        h_info = QHBoxLayout()
        lbl_dur = CaptionLabel(f"Durata: {format_time_hms(self.duration)}", card)
        lbl_res = CaptionLabel(f"Risoluzione: {self.meta['width']}x{self.meta['height']}", card)
        lbl_size = CaptionLabel(f"Peso originale: {format_bytes(self.meta['file_size'])}", card)
        lbl_size.setStyleSheet("color: #0078D4; font-weight: bold;")

        h_info.addWidget(lbl_dur)
        h_info.addSpacing(16)
        h_info.addWidget(lbl_res)
        h_info.addSpacing(16)
        h_info.addWidget(lbl_size)
        h_info.addStretch()
        card_layout.addLayout(h_info)

        layout.addWidget(card)

        # 1. Trimming Option Section
        self.chk_trim = CheckBox("Ritaglia il video (seleziona inizio e fine)", self)
        self.chk_trim.stateChanged.connect(self._toggle_trim_controls)
        layout.addWidget(self.chk_trim)

        self.trim_container = QWidget(self)
        trim_layout = QHBoxLayout(self.trim_container)
        trim_layout.setContentsMargins(20, 0, 0, 0)
        trim_layout.setSpacing(12)

        # Start Time
        lbl_start = BodyLabel("Inizio (sec):", self.trim_container)
        self.spin_start = DoubleSpinBox(self.trim_container)
        self.spin_start.setRange(0.0, max(0.0, self.duration - 0.5))
        self.spin_start.setSingleStep(1.0)
        self.spin_start.setValue(0.0)
        self.spin_start.valueChanged.connect(self._update_trim_labels)

        # End Time
        lbl_end = BodyLabel("Fine (sec):", self.trim_container)
        self.spin_end = DoubleSpinBox(self.trim_container)
        self.spin_end.setRange(0.5, max(1.0, self.duration))
        self.spin_end.setSingleStep(1.0)
        self.spin_end.setValue(self.duration)
        self.spin_end.valueChanged.connect(self._update_trim_labels)

        self.lbl_trim_dur = CaptionLabel(f"Durata finale: {format_time_hms(self.duration)}", self.trim_container)
        self.lbl_trim_dur.setStyleSheet("color: #107C41; font-weight: bold;")

        trim_layout.addWidget(lbl_start)
        trim_layout.addWidget(self.spin_start)
        trim_layout.addSpacing(10)
        trim_layout.addWidget(lbl_end)
        trim_layout.addWidget(self.spin_end)
        trim_layout.addSpacing(10)
        trim_layout.addWidget(self.lbl_trim_dur)
        trim_layout.addStretch()

        self.trim_container.setEnabled(False)
        layout.addWidget(self.trim_container)

        # 2. Compression Option Section
        self.chk_compress = CheckBox("Comprimi il video per ridurne il peso (Consigliato)", self)
        self.chk_compress.setChecked(True)
        self.chk_compress.stateChanged.connect(self._toggle_compress_controls)
        layout.addWidget(self.chk_compress)

        self.compress_container = QWidget(self)
        comp_layout = QVBoxLayout(self.compress_container)
        comp_layout.setContentsMargins(20, 0, 0, 0)
        comp_layout.setSpacing(10)

        # Resolution dropdown
        row_res = QHBoxLayout()
        lbl_comp_res = BodyLabel("Risoluzione di output:", self.compress_container)
        self.combo_res = ComboBox(self.compress_container)
        self.combo_res.addItem("720p HD (1280x720) - Ideale per streaming fluido", userData=720)
        self.combo_res.addItem("1080p Full HD (1920x1080) - Alta Definizione", userData=1080)
        self.combo_res.addItem("Mantieni Risoluzione Originale", userData=None)
        self.combo_res.setCurrentIndex(0)

        row_res.addWidget(lbl_comp_res)
        row_res.addWidget(self.combo_res, stretch=1)
        comp_layout.addLayout(row_res)

        # Quality dropdown
        row_qual = QHBoxLayout()
        lbl_comp_qual = BodyLabel("Livello di compressione:", self.compress_container)
        self.combo_qual = ComboBox(self.compress_container)
        self.combo_qual.addItem("Bilanciata (Consigliato - Ottima qualità, peso ridotto)", userData=26)
        self.combo_qual.addItem("Massima Compressione (File ultraleggero)", userData=30)
        self.combo_qual.addItem("Alta Qualità (Minima compressione)", userData=22)
        self.combo_qual.setCurrentIndex(0)

        row_qual.addWidget(lbl_comp_qual)
        row_qual.addWidget(self.combo_qual, stretch=1)
        comp_layout.addLayout(row_qual)

        layout.addWidget(self.compress_container)

        # 3. Progress Section (hidden initially)
        self.progress_container = QWidget(self)
        prog_layout = QVBoxLayout(self.progress_container)
        prog_layout.setContentsMargins(0, 8, 0, 0)
        prog_layout.setSpacing(6)

        self.lbl_progress = CaptionLabel("Pronto per l'elaborazione...", self.progress_container)
        self.lbl_progress.setStyleSheet("color: #0078D4; font-weight: bold;")
        self.progress_bar = ProgressBar(self.progress_container)
        self.progress_bar.setValue(0)

        prog_layout.addWidget(self.lbl_progress)
        prog_layout.addWidget(self.progress_bar)
        self.progress_container.hide()
        layout.addWidget(self.progress_container)

        # 4. Action Buttons
        btn_layout = QHBoxLayout()
        btn_layout.setSpacing(10)

        self.btn_cancel = PushButton("Annulla", self)
        self.btn_cancel.clicked.connect(self.reject)

        self.btn_direct = PushButton(FluentIcon.PLAY, "Carica Originale Diretto", self)
        self.btn_direct.clicked.connect(self._on_direct_load)

        self.btn_process = PrimaryPushButton(FluentIcon.COMPLETED, "Elabora & Carica", self)
        self.btn_process.clicked.connect(self._on_start_processing)

        btn_layout.addWidget(self.btn_cancel)
        btn_layout.addStretch()
        btn_layout.addWidget(self.btn_direct)
        btn_layout.addWidget(self.btn_process)

        layout.addLayout(btn_layout)

    def _toggle_trim_controls(self, state):
        self.trim_container.setEnabled(bool(state))

    def _toggle_compress_controls(self, state):
        self.compress_container.setEnabled(bool(state))

    def _update_trim_labels(self):
        start = self.spin_start.value()
        end = self.spin_end.value()
        dur = max(0.0, end - start)
        self.lbl_trim_dur.setText(f"Durata finale: {format_time_hms(dur)}")

    def _on_direct_load(self):
        """User chose to bypass trimming and compression."""
        self.result_file_path = self.file_path
        self.was_compressed = False
        self.accept()

    def _on_start_processing(self):
        """Start trimming / compression worker."""
        do_trim = self.chk_trim.isChecked()
        do_compress = self.chk_compress.isChecked()

        # If neither is checked, load direct
        if not do_trim and not do_compress:
            self._on_direct_load()
            return

        start_s = self.spin_start.value() if do_trim else 0.0
        end_s = self.spin_end.value() if do_trim else self.duration
        target_h = self.combo_res.currentData() if do_compress else None
        crf = self.combo_qual.currentData() if do_compress else 23

        # Prepare output cache path
        cache_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "cache", "trimmed")
        os.makedirs(cache_dir, exist_ok=True)
        base_name = os.path.splitext(os.path.basename(self.file_path))[0]
        out_name = f"{base_name}_opt_{int(time.time())}.mp4"
        out_path = os.path.abspath(os.path.join(cache_dir, out_name))

        # Update UI state
        self.progress_container.show()
        self.btn_process.setEnabled(False)
        self.btn_direct.setEnabled(False)
        self.btn_cancel.setText("Interrompi")

        self.worker = VideoCompressorWorker(
            input_path=self.file_path,
            output_path=out_path,
            start_sec=start_s,
            end_sec=end_s,
            target_height=target_h,
            crf=crf,
            parent=self
        )
        self.worker.progress.connect(self.progress_bar.setValue)
        self.worker.status_changed.connect(self.lbl_progress.setText)
        self.worker.finished.connect(self._on_worker_finished)
        self.worker.error.connect(self._on_worker_error)
        self.worker.start()

    def _on_worker_finished(self, output_path: str, orig_size: int, new_size: int):
        self.result_file_path = output_path
        self.was_compressed = True
        self.orig_size = orig_size
        self.new_size = new_size
        self.accept()

    def _on_worker_error(self, err_msg: str):
        self.lbl_progress.setText(f"Errore: {err_msg}")
        self.lbl_progress.setStyleSheet("color: #E81123;")
        self.btn_process.setEnabled(True)
        self.btn_direct.setEnabled(True)
        self.btn_cancel.setText("Annulla")

    def reject(self):
        if self.worker and self.worker.isRunning():
            self.worker.cancel()
            self.worker.wait(1000)
        super().reject()

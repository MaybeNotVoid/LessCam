"""
LessCam - Virtual Camera Control Panel
Fluent card widget managing virtual webcam broadcasting, resolution, framerate, and driver status.
"""

from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel
from qfluentwidgets import (
    SimpleCardWidget, TitleLabel, SubtitleLabel, BodyLabel, CaptionLabel,
    SwitchButton, ComboBox, FluentIcon, IconWidget, InfoBadge, InfoBadgePosition
)


class CameraPanel(SimpleCardWidget):
    """
    Card providing virtual camera activation, format configuration, and status monitoring.
    """
    vcam_toggled = pyqtSignal(bool)           # True = start, False = stop
    resolution_changed = pyqtSignal(int, int) # (width, height)
    fps_changed = pyqtSignal(int)             # 30 or 60

    def __init__(self, parent=None):
        super().__init__(parent)
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(18, 16, 18, 16)
        layout.setSpacing(14)

        # Header with icon and title
        header_layout = QHBoxLayout()
        header_layout.setSpacing(10)

        icon_widget = IconWidget(FluentIcon.CAMERA, self)
        icon_widget.setFixedSize(22, 22)
        header_layout.addWidget(icon_widget)

        title = SubtitleLabel("Webcam Virtuale", self)
        header_layout.addWidget(title)
        header_layout.addStretch()

        layout.addLayout(header_layout)

        # Switch: Start / Stop broadcast
        switch_row = QHBoxLayout()
        switch_label = BodyLabel("Trasmissione Webcam:", self)
        self.switch_btn = SwitchButton(self)
        self.switch_btn.setOnText("ATTIVA")
        self.switch_btn.setOffText("SPENTA")
        self.switch_btn.checkedChanged.connect(self._on_switch_changed)

        switch_row.addWidget(switch_label)
        switch_row.addStretch()
        switch_row.addWidget(self.switch_btn)
        layout.addLayout(switch_row)

        # Resolution Dropdown
        res_row = QHBoxLayout()
        res_label = BodyLabel("Risoluzione Output:", self)
        self.combo_res = ComboBox(self)
        self.combo_res.addItem("1280x720 (720p HD)", userData=(1280, 720))
        self.combo_res.addItem("1920x1080 (1080p Full HD)", userData=(1920, 1080))
        self.combo_res.setCurrentIndex(0)
        self.combo_res.currentIndexChanged.connect(self._on_res_changed)

        res_row.addWidget(res_label)
        res_row.addStretch()
        res_row.addWidget(self.combo_res)
        layout.addLayout(res_row)

        # Framerate Dropdown
        fps_row = QHBoxLayout()
        fps_label = BodyLabel("Frequenza Frame:", self)
        self.combo_fps = ComboBox(self)
        self.combo_fps.addItem("30 FPS", userData=30)
        self.combo_fps.addItem("60 FPS", userData=60)
        self.combo_fps.setCurrentIndex(0)
        self.combo_fps.currentIndexChanged.connect(self._on_fps_changed)

        fps_row.addWidget(fps_label)
        fps_row.addStretch()
        fps_row.addWidget(self.combo_fps)
        layout.addLayout(fps_row)

        # Status badge / info
        status_row = QHBoxLayout()
        status_title = CaptionLabel("Stato Driver:", self)
        status_title.setStyleSheet("color: #888888;")
        self.status_label = CaptionLabel("In attesa...", self)
        self.status_label.setStyleSheet("color: #0078D4; font-weight: bold;")

        status_row.addWidget(status_title)
        status_row.addWidget(self.status_label)
        status_row.addStretch()
        layout.addLayout(status_row)

    def _on_switch_changed(self, checked: bool):
        self.vcam_toggled.emit(checked)

    def _on_res_changed(self, index: int):
        data = self.combo_res.itemData(index)
        if data:
            w, h = data
            self.resolution_changed.emit(w, h)

    def _on_fps_changed(self, index: int):
        data = self.combo_fps.itemData(index)
        if data:
            self.fps_changed.emit(int(data))

    def set_status(self, is_active: bool, text: str):
        """Update driver status text and color."""
        self.status_label.setText(text)
        if is_active:
            self.status_label.setStyleSheet("color: #107C41; font-weight: bold;")
        else:
            self.status_label.setStyleSheet("color: #888888;")
            self.switch_btn.blockSignals(True)
            self.switch_btn.setChecked(False)
            self.switch_btn.blockSignals(False)

    def set_error_state(self, message: str):
        """Display error badge and reset switch."""
        self.switch_btn.blockSignals(True)
        self.switch_btn.setChecked(False)
        self.switch_btn.blockSignals(False)
        self.status_label.setText("Driver non rilevato")
        self.status_label.setStyleSheet("color: #E81123; font-weight: bold;")

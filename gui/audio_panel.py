"""
LessCam - Audio Mixer & Routing Panel
Fluent card widget providing audio device selection (including Virtual Audio Cable),
volume control (gain 0% to 200%), decibel readout, mute toggle, and real-time VU-meter.
"""

import math
from typing import List, Dict, Any
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout
from qfluentwidgets import (
    SimpleCardWidget, SubtitleLabel, BodyLabel, CaptionLabel,
    ComboBox, Slider, ToolButton, PushButton, FluentIcon, IconWidget
)

from gui.vu_meter import VUMeterWidget


class AudioPanel(SimpleCardWidget):
    """
    Audio mixer card for routing audio, adjusting gain, and monitoring levels.
    """
    device_selected = pyqtSignal(int)      # device index
    volume_changed = pyqtSignal(int)       # 0 - 200%
    mute_toggled = pyqtSignal(bool)        # True = muted
    refresh_requested = pyqtSignal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self._is_muted = False
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(18, 16, 18, 16)
        layout.setSpacing(14)

        # Header with icon and title
        header_layout = QHBoxLayout()
        header_layout.setSpacing(10)

        icon_widget = IconWidget(FluentIcon.VOLUME, self)
        icon_widget.setFixedSize(22, 22)
        header_layout.addWidget(icon_widget)

        title = SubtitleLabel("Mixer Audio & Routing", self)
        header_layout.addWidget(title)
        header_layout.addStretch()

        # Refresh devices button
        self.btn_refresh = ToolButton(FluentIcon.SYNC, self)
        self.btn_refresh.setToolTip("Aggiorna periferiche audio")
        self.btn_refresh.clicked.connect(self.refresh_requested.emit)
        header_layout.addWidget(self.btn_refresh)

        layout.addLayout(header_layout)

        # Output Device Selection Row
        dev_label = BodyLabel("Periferica di Output (Virtual Cable):", self)
        layout.addWidget(dev_label)

        self.combo_devices = ComboBox(self)
        self.combo_devices.currentIndexChanged.connect(self._on_device_changed)
        layout.addWidget(self.combo_devices)

        # Volume & Gain Row
        vol_header = QHBoxLayout()
        vol_label = BodyLabel("Volume Master / Guadagno:", self)
        self.gain_label = CaptionLabel("100% (0.0 dB)", self)
        self.gain_label.setStyleSheet("color: #0078D4; font-weight: bold;")

        vol_header.addWidget(vol_label)
        vol_header.addStretch()
        vol_header.addWidget(self.gain_label)
        layout.addLayout(vol_header)

        # Slider and Mute button row
        slider_row = QHBoxLayout()
        slider_row.setSpacing(10)

        self.btn_mute = ToolButton(FluentIcon.MUTE, self)
        self.btn_mute.setCheckable(True)
        self.btn_mute.setToolTip("Muta / Riattiva Audio")
        self.btn_mute.clicked.connect(self._on_mute_clicked)
        slider_row.addWidget(self.btn_mute)

        self.slider_volume = Slider(Qt.Orientation.Horizontal, self)
        self.slider_volume.setRange(0, 200)
        self.slider_volume.setValue(100)
        self.slider_volume.valueChanged.connect(self._on_volume_changed)
        slider_row.addWidget(self.slider_volume)

        layout.addLayout(slider_row)

        # VU-Meter Section
        meter_label = CaptionLabel("Livello Audio Output (VU-Meter):", self)
        meter_label.setStyleSheet("color: #888888;")
        layout.addWidget(meter_label)

        self.vu_meter = VUMeterWidget(self)
        layout.addWidget(self.vu_meter)

    def populate_devices(self, devices: List[Dict[str, Any]], current_index: int = -1):
        """Populate output devices dropdown and auto-select best candidate."""
        self.combo_devices.blockSignals(True)
        self.combo_devices.clear()

        select_item_idx = 0
        for i, dev in enumerate(devices):
            prefix = "⭐ " if dev['is_virtual'] else ""
            display_name = f"{prefix}{dev['name']} ({dev['api']})"
            self.combo_devices.addItem(display_name, userData=dev['index'])

            if current_index != -1 and dev['index'] == current_index:
                select_item_idx = i
            elif current_index == -1 and dev['is_virtual']:
                select_item_idx = i

        if self.combo_devices.count() > 0:
            self.combo_devices.setCurrentIndex(select_item_idx)

        self.combo_devices.blockSignals(False)

    def _on_device_changed(self, index: int):
        dev_idx = self.combo_devices.itemData(index)
        if dev_idx is not None:
            self.device_selected.emit(int(dev_idx))

    def _on_volume_changed(self, value: int):
        # Calculate dB
        if value <= 0:
            db_text = "-∞ dB"
        else:
            db_val = 20.0 * math.log10(value / 100.0)
            sign = "+" if db_val > 0 else ""
            db_text = f"{sign}{db_val:.1f} dB"

        self.gain_label.setText(f"{value}% ({db_text})")
        self.volume_changed.emit(value)

    def _on_mute_clicked(self):
        self._is_muted = self.btn_mute.isChecked()
        if self._is_muted:
            self.gain_label.setText("MUTATO")
            self.gain_label.setStyleSheet("color: #E81123; font-weight: bold;")
        else:
            self._on_volume_changed(self.slider_volume.value())
            self.gain_label.setStyleSheet("color: #0078D4; font-weight: bold;")
        self.mute_toggled.emit(self._is_muted)

    def update_vu_levels(self, left: float, right: float):
        """Update live VU-meter bars."""
        if self._is_muted:
            self.vu_meter.set_levels(0.0, 0.0)
        else:
            self.vu_meter.set_levels(left, right)

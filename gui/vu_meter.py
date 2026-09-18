"""
LessCam - Stereo VU-Meter Widget
Renders a real-time dual-channel (Left/Right) audio level meter with peak-hold,
smooth decay, and color segmentation (Normal -> Warning -> Clip).
"""

from PyQt6.QtCore import Qt, QTimer, QRectF
from PyQt6.QtWidgets import QWidget
from PyQt6.QtGui import QPainter, QColor, QFont, QPainterPath, QLinearGradient


class VUMeterWidget(QWidget):
    """
    Dual-channel audio VU-meter with smooth animations and peak hold.
    """
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setFixedHeight(48)
        self.setMinimumWidth(180)

        self._level_l = 0.0
        self._level_r = 0.0
        self._peak_l = 0.0
        self._peak_r = 0.0

        # Animation timer (30 FPS)
        self._decay_timer = QTimer(self)
        self._decay_timer.setInterval(33)
        self._decay_timer.timeout.connect(self._decay_step)
        self._decay_timer.start()

    def set_levels(self, left: float, right: float):
        """Update instantaneous levels [0.0, 1.0]."""
        left = max(0.0, min(1.0, left))
        right = max(0.0, min(1.0, right))

        # Fast attack
        if left > self._level_l:
            self._level_l = left
        if right > self._level_r:
            self._level_r = right

        # Peak hold
        if left > self._peak_l:
            self._peak_l = left
        if right > self._peak_r:
            self._peak_r = right

        self.update()

    def _decay_step(self):
        """Smooth exponential decay for meters and peaks."""
        decay_rate = 0.04
        peak_decay = 0.015

        self._level_l = max(0.0, self._level_l - decay_rate)
        self._level_r = max(0.0, self._level_r - decay_rate)
        self._peak_l = max(0.0, self._peak_l - peak_decay)
        self._peak_r = max(0.0, self._peak_r - peak_decay)

        self.update()

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        w = float(self.width())
        bar_x = 24.0
        bar_w = w - bar_x - 12.0
        bar_h = 10.0
        radius = 4.0

        painter.setFont(QFont("Segoe UI", 8, QFont.Weight.DemiBold))

        # Channel L
        painter.setPen(QColor(160, 160, 165))
        painter.drawText(QRectF(0, 6, 20, 14), Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter, "L")
        self._draw_bar(painter, bar_x, 8.0, bar_w, bar_h, self._level_l, self._peak_l, radius)

        # Channel R
        painter.drawText(QRectF(0, 26, 20, 14), Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter, "R")
        self._draw_bar(painter, bar_x, 28.0, bar_w, bar_h, self._level_r, self._peak_r, radius)

    def _draw_bar(self, painter: QPainter, x: float, y: float, w: float, h: float,
                  level: float, peak: float, radius: float):
        # Background slot
        bg_path = QPainterPath()
        bg_rect = QRectF(x, y, w, h)
        bg_path.addRoundedRect(bg_rect, radius, radius)
        painter.fillPath(bg_path, QColor(32, 32, 36))

        # Filled meter level
        fill_w = max(0.0, min(w, w * level))
        if fill_w > 1.0:
            fill_path = QPainterPath()
            fill_rect = QRectF(x, y, fill_w, h)
            fill_path.addRoundedRect(fill_rect, radius, radius)

            # Gradient: Green (safe) -> Yellow (warning) -> Red (clip)
            gradient = QLinearGradient(x, y, x + w, y)
            gradient.setColorAt(0.0, QColor(16, 185, 129))   # Green
            gradient.setColorAt(0.70, QColor(245, 158, 11))  # Amber/Yellow
            gradient.setColorAt(0.90, QColor(239, 68, 68))   # Red
            painter.fillPath(fill_path, gradient)

        # Peak indicator line
        peak_x = x + (w * peak)
        if peak_x > x + 2.0:
            peak_w = 2.5
            peak_rect = QRectF(min(peak_x - peak_w, x + w - peak_w), y, peak_w, h)
            peak_color = QColor(239, 68, 68) if peak > 0.88 else QColor(250, 204, 21)
            painter.fillRect(peak_rect, peak_color)

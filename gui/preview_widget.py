"""
LessCam - Video Preview Widget
Modern, high-performance video canvas with aspect ratio preservation, rounded corners,
and Windows 11 Fluent style badges (LIVE status, resolution, buffering indicator).
"""

from PyQt6.QtCore import Qt, QRectF
from PyQt6.QtWidgets import QWidget
from PyQt6.QtGui import QPainter, QColor, QImage, QPainterPath, QFont, QPen


class VideoPreviewWidget(QWidget):
    """
    Renders incoming video frames preserving aspect ratio with smooth scaling
    and modern UI overlays.
    """
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setMinimumSize(480, 270)
        self.setAttribute(Qt.WidgetAttribute.WA_OpaquePaintEvent, False)

        self._current_image: QImage = None
        self._is_live = False
        self._resolution_text = "1080p | 30 FPS"
        self._is_buffering = False
        self._buffering_text = "Caricamento in corso..."

    def update_frame(self, image: QImage):
        """Receive a new frame from VideoStreamWorker and schedule repaint."""
        self._current_image = image
        self.update()

    def set_live_status(self, is_live: bool):
        """Toggle the LIVE badge."""
        self._is_live = is_live
        self.update()

    def set_resolution_info(self, text: str):
        """Update resolution/FPS overlay."""
        self._resolution_text = text
        self.update()

    def set_buffering(self, buffering: bool, text: str = "Caricamento in corso..."):
        """Show or hide buffering indicator."""
        self._is_buffering = buffering
        self._buffering_text = text
        self.update()

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        painter.setRenderHint(QPainter.RenderHint.SmoothPixmapTransform)

        w, h = self.width(), self.height()
        radius = 12.0

        # Create rounded card clipping path
        path = QPainterPath()
        rect = QRectF(0, 0, w, h)
        path.addRoundedRect(rect, radius, radius)
        painter.setClipPath(path)

        # Background fill (Fluent dark surface)
        painter.fillRect(rect, QColor(18, 18, 20))

        # Render video frame preserving aspect ratio
        if self._current_image and not self._current_image.isNull():
            img_w = self._current_image.width()
            img_h = self._current_image.height()

            scale = min(w / img_w, h / img_h)
            draw_w = img_w * scale
            draw_h = img_h * scale
            draw_x = (w - draw_w) / 2.0
            draw_y = (h - draw_h) / 2.0

            target_rect = QRectF(draw_x, draw_y, draw_w, draw_h)
            painter.drawImage(target_rect, self._current_image)
        else:
            # Placeholder text if no image
            painter.setPen(QColor(120, 120, 130))
            font = QFont("Segoe UI", 12)
            painter.setFont(font)
            painter.drawText(rect, Qt.AlignmentFlag.AlignCenter, "LessCam - Nessun Segnale Video")

        # Subtle card border
        painter.setPen(QPen(QColor(255, 255, 255, 25), 1.5))
        painter.drawRoundedRect(rect.adjusted(0.5, 0.5, -0.5, -0.5), radius, radius)

        # Overlays: Live Badge (Top-Left)
        badge_y = 16.0
        badge_x = 16.0
        badge_h = 24.0

        if self._is_live:
            badge_text = "• LIVE"
            bg_color = QColor(220, 53, 69, 210)  # Red accent
            text_color = QColor(255, 255, 255)
        else:
            badge_text = "OFFLINE"
            bg_color = QColor(50, 50, 55, 190)
            text_color = QColor(190, 190, 195)

        font_badge = QFont("Segoe UI", 9, QFont.Weight.Bold)
        painter.setFont(font_badge)
        text_w = painter.fontMetrics().horizontalAdvance(badge_text) + 20
        badge_rect = QRectF(badge_x, badge_y, text_w, badge_h)

        b_path = QPainterPath()
        b_path.addRoundedRect(badge_rect, 6.0, 6.0)
        painter.fillPath(b_path, bg_color)
        painter.setPen(text_color)
        painter.drawText(badge_rect, Qt.AlignmentFlag.AlignCenter, badge_text)

        # Resolution / FPS Badge (Top-Right)
        if self._resolution_text:
            font_res = QFont("Segoe UI", 9)
            painter.setFont(font_res)
            res_w = painter.fontMetrics().horizontalAdvance(self._resolution_text) + 18
            res_x = w - res_w - 16.0
            res_rect = QRectF(res_x, badge_y, res_w, badge_h)

            r_path = QPainterPath()
            r_path.addRoundedRect(res_rect, 6.0, 6.0)
            painter.fillPath(r_path, QColor(30, 30, 35, 190))
            painter.setPen(QColor(210, 210, 215))
            painter.drawText(res_rect, Qt.AlignmentFlag.AlignCenter, self._resolution_text)

        # Buffering Overlay
        if self._is_buffering:
            overlay_w = 260.0
            overlay_h = 44.0
            buf_rect = QRectF((w - overlay_w) / 2.0, (h - overlay_h) / 2.0, overlay_w, overlay_h)
            buf_path = QPainterPath()
            buf_path.addRoundedRect(buf_rect, 10.0, 10.0)
            painter.fillPath(buf_path, QColor(15, 15, 20, 220))
            painter.setPen(QPen(QColor(0, 120, 215, 180), 1.5))
            painter.drawPath(buf_path)

            painter.setPen(QColor(255, 255, 255))
            painter.setFont(QFont("Segoe UI", 10, QFont.Weight.DemiBold))
            painter.drawText(buf_rect, Qt.AlignmentFlag.AlignCenter, self._buffering_text)

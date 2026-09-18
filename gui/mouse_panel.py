"""
LessCam - Mouse Automation Panel
Fluent UI panel allowing target window selection, periodic mouse jiggling with fully visible interval,
loading up to 4 image targets with per-slot scroll toggles, precision adjustment slider (matching tolerance),
post-click configurable scroll down, two dedicated Start/Stop buttons without icon ghosting,
and global keyboard shortcuts (CTRL+ALT+A to activate, CTRL+ALT+D to deactivate).
"""

import os
from typing import List, Tuple, Optional, Any

import cv2
import numpy as np
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QGridLayout, QFileDialog, QFrame
)
from PyQt6.QtGui import QPixmap, QImage, QKeySequence, QShortcut
from qfluentwidgets import (
    SimpleCardWidget, SubtitleLabel, BodyLabel, CaptionLabel,
    ComboBox, ToolButton, PrimaryPushButton, PushButton, CheckBox,
    SpinBox, DoubleSpinBox, Slider, FluentIcon, IconWidget, InfoBadge
)

from engine.mouse_automation import get_open_windows, MouseAutomationWorker, HotkeyListener


class ImageSlotWidget(QFrame):
    """Widget representing one of the 4 template image targets with per-slot scroll toggle."""
    image_changed = pyqtSignal(int)  # slot_index

    def __init__(self, slot_index: int, parent=None):
        super().__init__(parent)
        self.slot_index = slot_index
        self.file_path: Optional[str] = None
        self.cv_image: Optional[np.ndarray] = None
        self.image_name = "Nessuna immagine"

        self.setStyleSheet("background-color: #242428; border-radius: 6px; padding: 4px;")
        self._init_ui()

    def _init_ui(self):
        layout = QHBoxLayout(self)
        layout.setContentsMargins(8, 6, 8, 6)
        layout.setSpacing(8)

        # Thumbnail Label
        self.lbl_thumb = CaptionLabel(self)
        self.lbl_thumb.setFixedSize(38, 38)
        self.lbl_thumb.setStyleSheet("background-color: #1a1a1c; border-radius: 4px; border: 1px dashed #444;")
        self.lbl_thumb.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.lbl_thumb.setText(f"#{self.slot_index + 1}")
        layout.addWidget(self.lbl_thumb)

        # Name label
        self.lbl_name = CaptionLabel(f"Slot {self.slot_index + 1}: Vuoto", self)
        self.lbl_name.setStyleSheet("color: #888888;")
        layout.addWidget(self.lbl_name, stretch=1)

        # Per-slot Scroll Down Checkbox
        self.chk_slot_scroll = CheckBox("Scroll", self)
        self.chk_slot_scroll.setChecked(True)
        self.chk_slot_scroll.setToolTip("Esegui scroll down dopo aver cliccato questo target")
        self.chk_slot_scroll.stateChanged.connect(lambda _: self.image_changed.emit(self.slot_index))
        layout.addWidget(self.chk_slot_scroll)

        # Load Button
        self.btn_load = ToolButton(FluentIcon.FOLDER, self)
        self.btn_load.setToolTip("Carica immagine target (PNG, JPG)")
        self.btn_load.clicked.connect(self._on_load_clicked)
        layout.addWidget(self.btn_load)

        # Remove Button
        self.btn_clear = ToolButton(FluentIcon.DELETE, self)
        self.btn_clear.setToolTip("Rimuovi target")
        self.btn_clear.setEnabled(False)
        self.btn_clear.clicked.connect(self._on_clear_clicked)
        layout.addWidget(self.btn_clear)

    def _on_load_clicked(self):
        file_path, _ = QFileDialog.getOpenFileName(
            self,
            f"Seleziona Target per Slot {self.slot_index + 1}",
            "",
            "Immagini Target (*.png *.jpg *.jpeg *.bmp *.webp);;Tutti i File (*.*)"
        )
        if file_path:
            img = cv2.imread(file_path)
            if img is not None:
                self.file_path = file_path
                self.cv_image = img
                self.image_name = os.path.basename(file_path)
                self.lbl_name.setText(self.image_name[:18])
                self.lbl_name.setStyleSheet("color: #f0f0f0; font-weight: bold;")
                self.btn_clear.setEnabled(True)

                # Thumbnail Pixmap
                rgb = cv2.cvtColor(img, cv2.COLOR_BGR2RGB)
                h, w, ch = rgb.shape
                qimg = QImage(rgb.data, w, h, ch * w, QImage.Format.Format_RGB888)
                pixmap = QPixmap.fromImage(qimg).scaled(38, 38, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation)
                self.lbl_thumb.setPixmap(pixmap)

                self.image_changed.emit(self.slot_index)

    def _on_clear_clicked(self):
        self.file_path = None
        self.cv_image = None
        self.image_name = "Nessuna immagine"
        self.lbl_name.setText(f"Slot {self.slot_index + 1}: Vuoto")
        self.lbl_name.setStyleSheet("color: #888888;")
        self.lbl_thumb.clear()
        self.lbl_thumb.setText(f"#{self.slot_index + 1}")
        self.btn_clear.setEnabled(False)
        self.image_changed.emit(self.slot_index)


class MousePanel(SimpleCardWidget):
    """
    Panel providing full controls for Window Targeting, Periodic Jiggling,
    Template Detection, Precision Tolerance Slider, Post-Click Scroll Down,
    Two Dedicated Action Buttons, and Global Hotkeys.
    """
    def __init__(self, parent=None):
        super().__init__(parent)
        self.worker: Optional[MouseAutomationWorker] = None

        # Initialize Global Hotkeys (CTRL+ALT+A / CTRL+ALT+D)
        self.hotkeys = HotkeyListener(self)
        self.hotkeys.start_requested.connect(self.start_bot)
        self.hotkeys.stop_requested.connect(self.stop_bot)

        # In-App Qt Shortcuts
        self.sc_start = QShortcut(QKeySequence("Ctrl+Alt+A"), self)
        self.sc_start.activated.connect(self.start_bot)
        self.sc_stop = QShortcut(QKeySequence("Ctrl+Alt+D"), self)
        self.sc_stop.activated.connect(self.stop_bot)

        self._init_ui()
        self.refresh_windows()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(18, 14, 18, 14)
        layout.setSpacing(12)

        # Header
        header = QHBoxLayout()
        header.setSpacing(10)

        icon = IconWidget(FluentIcon.APPLICATION, self)
        icon.setFixedSize(22, 22)
        header.addWidget(icon)

        title = SubtitleLabel("Automazione Mouse", self)
        header.addWidget(title)
        header.addStretch()

        self.badge_status = CaptionLabel("INATTIVO", self)
        self.badge_status.setStyleSheet("color: #888888; font-weight: bold; padding: 2px 8px; border-radius: 4px; background: #26262a;")
        header.addWidget(self.badge_status)
        layout.addLayout(header)

        # Shortcut Reminder Banner
        shortcut_box = QHBoxLayout()
        shortcut_box.setSpacing(6)
        lbl_sc_hint = CaptionLabel("⌨️ Scorciatoie: CTRL+ALT+A per attivare | CTRL+ALT+D per disattivare", self)
        lbl_sc_hint.setStyleSheet("color: #0078D4; font-weight: 500;")
        shortcut_box.addWidget(lbl_sc_hint)
        shortcut_box.addStretch()
        layout.addLayout(shortcut_box)

        # 1. Target Window Selection
        win_box = QVBoxLayout()
        win_box.setSpacing(4)
        win_label = BodyLabel("Finestra Target:", self)
        win_box.addWidget(win_label)

        win_row = QHBoxLayout()
        win_row.setSpacing(8)

        self.combo_windows = ComboBox(self)
        win_row.addWidget(self.combo_windows, stretch=1)

        self.btn_refresh_wins = ToolButton(FluentIcon.SYNC, self)
        self.btn_refresh_wins.setToolTip("Aggiorna elenco finestre aperte")
        self.btn_refresh_wins.clicked.connect(self.refresh_windows)
        win_row.addWidget(self.btn_refresh_wins)

        win_box.addLayout(win_row)
        layout.addLayout(win_box)

        # 2. Periodic Mouse Jiggler Section (FIXED VISIBILITY)
        jiggle_box = QHBoxLayout()
        jiggle_box.setSpacing(10)

        self.chk_periodic = CheckBox("Muovi mouse ogni:", self)
        self.chk_periodic.setChecked(True)
        jiggle_box.addWidget(self.chk_periodic)

        # Sized properly to 135px with visible text and suffix
        self.spin_interval = SpinBox(self)
        self.spin_interval.setRange(1, 300)
        self.spin_interval.setValue(10)
        self.spin_interval.setSuffix(" sec")
        self.spin_interval.setFixedWidth(135)
        self.spin_interval.setStyleSheet("font-size: 13px; font-weight: bold;")
        self.spin_interval.valueChanged.connect(self._on_settings_changed)
        jiggle_box.addWidget(self.spin_interval)

        lbl_sec = BodyLabel("(Anti-AFK)", self)
        lbl_sec.setStyleSheet("color: #888888;")
        jiggle_box.addWidget(lbl_sec)
        jiggle_box.addStretch()
        layout.addLayout(jiggle_box)

        # 3. Target Images (Max 4 Slots)
        layout.addSpacing(2)
        lbl_templates = BodyLabel("Immagini Target da Cliccare (Max 4):", self)
        layout.addWidget(lbl_templates)

        self.slots: List[ImageSlotWidget] = []
        slots_grid = QVBoxLayout()
        slots_grid.setSpacing(6)

        for i in range(4):
            slot = ImageSlotWidget(i, self)
            slot.image_changed.connect(self._on_template_updated)
            self.slots.append(slot)
            slots_grid.addWidget(slot)

        layout.addLayout(slots_grid)

        # 4. Precision / Matching Tolerance Slider Section
        layout.addSpacing(4)
        prec_box = QVBoxLayout()
        prec_box.setSpacing(4)

        prec_header = QHBoxLayout()
        lbl_prec_title = BodyLabel("Precisione Riconoscimento (Soglia Matching):", self)
        self.lbl_prec_val = CaptionLabel("85% (Alta precisione)", self)
        self.lbl_prec_val.setStyleSheet("color: #0078D4; font-weight: bold;")
        prec_header.addWidget(lbl_prec_title)
        prec_header.addStretch()
        prec_header.addWidget(self.lbl_prec_val)
        prec_box.addLayout(prec_header)

        self.slider_precision = Slider(Qt.Orientation.Horizontal, self)
        self.slider_precision.setRange(50, 99)
        self.slider_precision.setValue(85)
        self.slider_precision.valueChanged.connect(self._on_precision_changed)
        prec_box.addWidget(self.slider_precision)

        lbl_prec_desc = CaptionLabel("Aumenta la barra (es. 90-95%) se il bot scambia elementi simili per il bersaglio.", self)
        lbl_prec_desc.setStyleSheet("color: #888888;")
        prec_box.addWidget(lbl_prec_desc)

        layout.addLayout(prec_box)

        # 5. Configurable Post-Click Scroll Down Section
        layout.addSpacing(4)
        scroll_card = QFrame(self)
        scroll_card.setStyleSheet("background-color: #242428; border-radius: 6px; padding: 6px;")
        scroll_card_layout = QVBoxLayout(scroll_card)
        scroll_card_layout.setSpacing(8)

        scroll_header = QHBoxLayout()
        self.chk_scroll = CheckBox("Scroll verso il basso dopo ogni click", scroll_card)
        self.chk_scroll.setChecked(True)
        self.chk_scroll.stateChanged.connect(self._on_settings_changed)
        scroll_header.addWidget(self.chk_scroll)
        scroll_header.addStretch()
        scroll_card_layout.addLayout(scroll_header)

        # Configurable Scroll Parameters
        scroll_params_row = QHBoxLayout()
        scroll_params_row.setSpacing(12)

        lbl_steps = CaptionLabel("Intensità scroll:", scroll_card)
        scroll_params_row.addWidget(lbl_steps)

        self.spin_scroll_steps = SpinBox(scroll_card)
        self.spin_scroll_steps.setRange(1, 20)
        self.spin_scroll_steps.setValue(2)
        self.spin_scroll_steps.setSuffix(" scatti")
        self.spin_scroll_steps.setFixedWidth(120)
        self.spin_scroll_steps.valueChanged.connect(self._on_settings_changed)
        scroll_params_row.addWidget(self.spin_scroll_steps)

        lbl_delay = CaptionLabel("Ritardo post-click:", scroll_card)
        scroll_params_row.addWidget(lbl_delay)

        self.spin_scroll_delay = DoubleSpinBox(scroll_card)
        self.spin_scroll_delay.setRange(0.05, 3.0)
        self.spin_scroll_delay.setSingleStep(0.05)
        self.spin_scroll_delay.setValue(0.20)
        self.spin_scroll_delay.setSuffix(" sec")
        self.spin_scroll_delay.setFixedWidth(120)
        self.spin_scroll_delay.valueChanged.connect(self._on_settings_changed)
        scroll_params_row.addWidget(self.spin_scroll_delay)

        scroll_params_row.addStretch()
        scroll_card_layout.addLayout(scroll_params_row)

        layout.addWidget(scroll_card)

        # 6. Two Dedicated Start / Stop Buttons (NO ICON OVERLAPPING)
        layout.addSpacing(4)
        btn_row = QHBoxLayout()
        btn_row.setSpacing(10)

        self.btn_start = PrimaryPushButton(FluentIcon.PLAY, "AVVIA BOT (CTRL+ALT+A)", self)
        self.btn_start.setFixedHeight(38)
        self.btn_start.clicked.connect(self.start_bot)

        self.btn_stop = PushButton(FluentIcon.PAUSE, "ARRESTA (CTRL+ALT+D)", self)
        self.btn_stop.setFixedHeight(38)
        self.btn_stop.setEnabled(False)
        self.btn_stop.clicked.connect(self.stop_bot)

        btn_row.addWidget(self.btn_start, stretch=1)
        btn_row.addWidget(self.btn_stop, stretch=1)
        layout.addLayout(btn_row)

        # Action Log / Status Feedback
        self.lbl_log = CaptionLabel("Pronto per l'avvio. Failsafe attivo (angolo schermo per stop d'emergenza).", self)
        self.lbl_log.setStyleSheet("color: #888888;")
        layout.addWidget(self.lbl_log)

    def refresh_windows(self):
        """Populate the target window dropdown."""
        self.combo_windows.blockSignals(True)
        self.combo_windows.clear()

        # Full screen option
        self.combo_windows.addItem("🖥️ Schermo Intero (Desktop Primario)", userData=0)

        # Enumerate Windows
        open_wins = get_open_windows()
        for win in open_wins:
            title_display = win['title']
            if len(title_display) > 36:
                title_display = title_display[:34] + "..."
            self.combo_windows.addItem(f"🪟 {title_display}", userData=win['hwnd'])

        self.combo_windows.blockSignals(False)

    def _get_active_templates(self) -> List[Tuple[int, np.ndarray, str, bool]]:
        """Collect all currently loaded templates with per-slot scroll settings."""
        active = []
        for slot in self.slots:
            if slot.cv_image is not None:
                active.append((
                    slot.slot_index,
                    slot.cv_image,
                    slot.image_name,
                    slot.chk_slot_scroll.isChecked()
                ))
        return active

    def _on_template_updated(self, slot_index: int):
        self._on_settings_changed()

    def _on_precision_changed(self, val: int):
        """Update precision label and forward threshold to running worker."""
        if val >= 92:
            desc = "Massima (Quasi identica)"
            color = "#107C41"
        elif val >= 85:
            desc = "Alta (Consigliata per evitare falsi positivi)"
            color = "#0078D4"
        elif val >= 70:
            desc = "Media (Standard)"
            color = "#D83B01"
        else:
            desc = "Bassa (Tolleranza molto alta)"
            color = "#E81123"

        self.lbl_prec_val.setText(f"{val}% ({desc})")
        self.lbl_prec_val.setStyleSheet(f"color: {color}; font-weight: bold;")
        self._on_settings_changed()

    def _on_settings_changed(self):
        """If worker is running, push updated settings dynamically."""
        if self.worker and self.worker.isRunning():
            hwnd = self.combo_windows.currentData() or 0
            interval = float(self.spin_interval.value())
            periodic = self.chk_periodic.isChecked()
            templates = self._get_active_templates()
            do_scroll = self.chk_scroll.isChecked()
            scroll_amt = -abs(self.spin_scroll_steps.value() * 100)
            delay = float(self.spin_scroll_delay.value())
            thresh = float(self.slider_precision.value()) / 100.0

            self.worker.update_settings(
                target_hwnd=hwnd,
                move_interval_sec=interval,
                periodic_move_enabled=periodic,
                templates=templates,
                enable_scroll=do_scroll,
                scroll_amount=scroll_amt,
                scroll_delay_sec=delay,
                threshold=thresh
            )

    def start_bot(self):
        if self.worker and self.worker.isRunning():
            return

        hwnd = self.combo_windows.currentData() or 0
        interval = float(self.spin_interval.value())
        periodic = self.chk_periodic.isChecked()
        templates = self._get_active_templates()
        do_scroll = self.chk_scroll.isChecked()
        scroll_amt = -abs(self.spin_scroll_steps.value() * 100)
        delay = float(self.spin_scroll_delay.value())
        thresh = float(self.slider_precision.value()) / 100.0

        self.worker = MouseAutomationWorker(
            target_hwnd=hwnd,
            move_interval_sec=interval,
            periodic_move_enabled=periodic,
            templates=templates,
            enable_scroll=do_scroll,
            scroll_amount=scroll_amt,
            scroll_delay_sec=delay,
            threshold=thresh,
            parent=self
        )
        self.worker.status_changed.connect(self._on_status_changed)
        self.worker.action_performed.connect(self._on_action_performed)
        self.worker.error_occurred.connect(self._on_error_occurred)
        self.worker.stopped.connect(self._on_worker_stopped)
        self.worker.start()

        self.btn_start.setEnabled(False)
        self.btn_stop.setEnabled(True)
        self.btn_stop.setStyleSheet("background-color: #C42B1C; color: white; font-weight: bold;")
        self.badge_status.setText("ATTIVO")
        self.badge_status.setStyleSheet("color: #107C41; font-weight: bold; padding: 2px 8px; border-radius: 4px; background: #1b3824;")

    def stop_bot(self):
        if self.worker:
            self.worker.stop()
            self.worker.wait(1500)
            self.worker = None
        self._on_worker_stopped()

    def _on_worker_stopped(self):
        self.btn_start.setEnabled(True)
        self.btn_stop.setEnabled(False)
        self.btn_stop.setStyleSheet("")
        self.badge_status.setText("INATTIVO")
        self.badge_status.setStyleSheet("color: #888888; font-weight: bold; padding: 2px 8px; border-radius: 4px; background: #26262a;")

    def _on_status_changed(self, msg: str):
        self.lbl_log.setText(msg)

    def _on_action_performed(self, action: str):
        self.lbl_log.setText(f"Azione: {action}")
        self.lbl_log.setStyleSheet("color: #0078D4; font-weight: bold;")

    def _on_error_occurred(self, err: str):
        self.lbl_log.setText(f"Errore: {err}")
        self.lbl_log.setStyleSheet("color: #E81123;")
        self.stop_bot()

    def closeEvent(self, event):
        self.hotkeys.cleanup()
        super().closeEvent(event)

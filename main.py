"""
LessCam - Modern Virtual Camera Studio
Entry point of the application.
"""

import sys
import os

from PyQt6.QtCore import Qt
from PyQt6.QtWidgets import QApplication
from PyQt6.QtGui import QFont

from gui.main_window import LessCamMainWindow


def main():
    # Enable High DPI scaling and crisp font rendering on Windows
    QApplication.setHighDpiScaleFactorRoundingPolicy(
        Qt.HighDpiScaleFactorRoundingPolicy.PassThrough
    )

    app = QApplication(sys.argv)
    app.setApplicationName("LessCam")
    app.setOrganizationName("LessCamStudio")

    # Set default modern Windows typography
    font = QFont("Segoe UI Variable Display", 10)
    font.setStyleStrategy(QFont.StyleStrategy.PreferAntialias)
    app.setFont(font)

    window = LessCamMainWindow()
    window.show()

    sys.exit(app.exec())


if __name__ == "__main__":
    main()

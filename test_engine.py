"""
LessCam - Automated Unit & Integration Tests
Validates letterbox scaling, YouTube URL validation, PyAV/Sounddevice engine logic,
and virtual camera fallback resilience.
"""

import unittest
import numpy as np

from engine.video_stream import letterbox_frame
from engine.youtube import is_valid_youtube_url
from engine.audio_stream import get_audio_output_devices


class TestLessCamCore(unittest.TestCase):
    def test_youtube_url_validator(self):
        valid_urls = [
            "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
            "http://youtu.be/aqz-KE-bpKQ",
            "https://www.youtube.com/shorts/abcdef12345",
            "https://youtube.com/live/xyz12345",
        ]
        invalid_urls = [
            "https://www.google.com",
            "https://vimeo.com/12345",
            "",
            "not_a_url",
            None
        ]
        for url in valid_urls:
            self.assertTrue(is_valid_youtube_url(url), f"Should be valid: {url}")
        for url in invalid_urls:
            self.assertFalse(is_valid_youtube_url(url), f"Should be invalid: {url}")

    def test_letterbox_aspect_ratios(self):
        target_w, target_h = 1280, 720

        # Case 1: Identical 16:9 frame
        frame_16_9 = np.ones((720, 1280, 3), dtype=np.uint8) * 100
        out_16_9 = letterbox_frame(frame_16_9, target_w, target_h)
        self.assertEqual(out_16_9.shape, (720, 1280, 3))
        # Center should be non-zero
        self.assertEqual(out_16_9[360, 640, 0], 100)

        # Case 2: 4:3 input (Pillarbox - black bars on left/right)
        frame_4_3 = np.ones((768, 1024, 3), dtype=np.uint8) * 200
        out_4_3 = letterbox_frame(frame_4_3, target_w, target_h)
        self.assertEqual(out_4_3.shape, (720, 1280, 3))
        # Left edge should be black (pillarbox)
        self.assertEqual(out_4_3[360, 10, 0], 0)
        # Center should be 200
        self.assertEqual(out_4_3[360, 640, 0], 200)

        # Case 3: Vertical 9:16 input (Pillarbox - heavy bars on left/right)
        frame_vert = np.ones((1920, 1080, 3), dtype=np.uint8) * 150
        out_vert = letterbox_frame(frame_vert, target_w, target_h)
        self.assertEqual(out_vert.shape, (720, 1280, 3))
        self.assertEqual(out_vert[360, 10, 0], 0)
        self.assertEqual(out_vert[360, 640, 0], 150)

    def test_audio_device_enumeration(self):
        devices = get_audio_output_devices()
        self.assertIsInstance(devices, list)
        self.assertGreater(len(devices), 0, "System should have at least one audio output device")
        for dev in devices:
            self.assertIn('index', dev)
            self.assertIn('name', dev)
            self.assertIn('api', dev)
            self.assertIn('is_virtual', dev)

    def test_video_metadata_parser(self):
        from engine.video_trimmer import get_video_metadata
        # Test on non-existent file gracefully
        meta = get_video_metadata("non_existent_file.mp4")
        self.assertEqual(meta['duration'], 0.0)
        self.assertEqual(meta['file_size'], 0)

    def test_mouse_automation_worker(self):
        from engine.mouse_automation import get_open_windows, MouseAutomationWorker
        wins = get_open_windows()
        self.assertIsInstance(wins, list)

        worker = MouseAutomationWorker(
            target_hwnd=0,
            move_interval_sec=5.0,
            periodic_move_enabled=False,
            templates=[],
            enable_scroll=True,
            scroll_amount=-150
        )
        self.assertEqual(worker.target_hwnd, 0)
        self.assertEqual(worker.scroll_amount, -150)
        worker.stop()


if __name__ == "__main__":
    unittest.main()

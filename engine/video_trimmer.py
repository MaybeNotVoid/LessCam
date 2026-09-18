"""
LessCam - Video Trimmer & Compressor Engine
Uses PyAV (FFmpeg) to accurately trim video/audio and transcode using H.264 (libx264)
and AAC with configurable CRF and resolution scaling for lightweight file sizes.
"""

import os
import time
from typing import Optional

import av
from PyQt6.QtCore import QThread, pyqtSignal


def get_video_metadata(file_path: str) -> dict:
    """Extract metadata (duration, resolution, size, fps, has_audio) from video file."""
    meta = {
        'duration': 0.0,
        'width': 0,
        'height': 0,
        'fps': 30.0,
        'has_audio': False,
        'file_size': 0,
        'file_name': os.path.basename(file_path)
    }
    try:
        meta['file_size'] = os.path.getsize(file_path)
        container = av.open(file_path)
        v_stream = next((s for s in container.streams if s.type == 'video'), None)
        a_stream = next((s for s in container.streams if s.type == 'audio'), None)

        if v_stream:
            meta['width'] = v_stream.width
            meta['height'] = v_stream.height
            if v_stream.duration and v_stream.time_base:
                meta['duration'] = float(v_stream.duration * v_stream.time_base)
            elif container.duration:
                meta['duration'] = float(container.duration / av.time.AV_TIME_BASE)
            if v_stream.average_rate:
                meta['fps'] = float(v_stream.average_rate)

        if a_stream:
            meta['has_audio'] = True
            if meta['duration'] == 0.0 and a_stream.duration and a_stream.time_base:
                meta['duration'] = float(a_stream.duration * a_stream.time_base)

        container.close()
    except Exception as exc:
        print(f"[VideoTrimmer] Error reading metadata: {exc}")
    return meta


class VideoCompressorWorker(QThread):
    """
    Dedicated background worker for trimming and compressing a video file.
    """
    progress = pyqtSignal(int)                         # 0 to 100
    status_changed = pyqtSignal(str)                   # Status description
    finished = pyqtSignal(str, int, int)               # (output_path, orig_size, new_size)
    error = pyqtSignal(str)                            # Error message

    def __init__(
        self,
        input_path: str,
        output_path: str,
        start_sec: float = 0.0,
        end_sec: Optional[float] = None,
        target_height: Optional[int] = 720,
        crf: int = 26,
        preset: str = 'veryfast',
        parent=None
    ):
        super().__init__(parent)
        self.input_path = input_path
        self.output_path = output_path
        self.start_sec = max(0.0, start_sec)
        self.end_sec = end_sec
        self.target_height = target_height
        self.crf = crf
        self.preset = preset
        self._is_cancelled = False

    def cancel(self):
        self._is_cancelled = True

    def run(self):
        if not os.path.exists(self.input_path):
            self.error.emit(f"File non trovato: {self.input_path}")
            return

        orig_size = os.path.getsize(self.input_path)
        os.makedirs(os.path.dirname(os.path.abspath(self.output_path)), exist_ok=True)

        try:
            self.status_changed.emit("Inizializzazione codec...")
            input_container = av.open(self.input_path)
            output_container = av.open(self.output_path, mode='w')

            v_in = next((s for s in input_container.streams if s.type == 'video'), None)
            a_in = next((s for s in input_container.streams if s.type == 'audio'), None)

            if not v_in:
                self.error.emit("Nessun flusso video trovato nel file selezionato.")
                input_container.close()
                output_container.close()
                return

            # Compute actual duration
            total_file_duration = 0.0
            if v_in.duration and v_in.time_base:
                total_file_duration = float(v_in.duration * v_in.time_base)
            elif input_container.duration:
                total_file_duration = float(input_container.duration / av.time.AV_TIME_BASE)

            effective_end = self.end_sec if (self.end_sec and self.end_sec > self.start_sec) else total_file_duration
            if effective_end <= self.start_sec:
                effective_end = self.start_sec + 1.0

            target_duration = effective_end - self.start_sec

            # Configure Output Video Stream
            v_rate = v_in.average_rate if v_in.average_rate else 30
            v_out = output_container.add_stream('libx264', rate=v_rate)

            # Determine dimensions
            out_w = v_in.width
            out_h = v_in.height
            if self.target_height and self.target_height < v_in.height:
                scale = self.target_height / float(v_in.height)
                out_w = int(round(v_in.width * scale / 2.0) * 2)
                out_h = int(round(self.target_height / 2.0) * 2)
            else:
                out_w = int(round(v_in.width / 2.0) * 2)
                out_h = int(round(v_in.height / 2.0) * 2)

            v_out.width = out_w
            v_out.height = out_h
            v_out.pix_fmt = 'yuv420p'
            v_out.options = {
                'crf': str(self.crf),
                'preset': self.preset,
                'tune': 'fastdecode'
            }

            # Configure Output Audio Stream (if source has audio)
            a_out = None
            if a_in:
                a_out = output_container.add_stream('aac', rate=44100)
                a_out.layout = 'stereo'
                a_out.bit_rate = 128000  # 128 kbps AAC stereo

            # Seek to start if trimmed
            if self.start_sec > 0.5:
                pts = int(self.start_sec / float(v_in.time_base))
                input_container.seek(pts, stream=v_in, any_frame=False)

            self.status_changed.emit("Compressione e trimming in corso...")

            # Transcode Video and Audio
            resampler = av.AudioResampler(format='fltp', layout='stereo', rate=44100) if a_in else None

            last_progress = 0
            for packet in input_container.demux(v_in, a_in if a_in else ()):
                if self._is_cancelled:
                    break

                for frame in packet.decode():
                    if self._is_cancelled:
                        break

                    # Video Frame Processing
                    if isinstance(frame, av.VideoFrame):
                        t = float(frame.pts * frame.time_base) if frame.pts is not None else 0.0
                        if t < self.start_sec:
                            continue
                        if t > effective_end:
                            break

                        # Update progress
                        if target_duration > 0:
                            pct = int(min(98, max(1, ((t - self.start_sec) / target_duration) * 100)))
                            if pct != last_progress:
                                last_progress = pct
                                self.progress.emit(pct)

                        # Reformat frame if dimensions changed
                        if frame.width != out_w or frame.height != out_h or frame.format.name != 'yuv420p':
                            frame = frame.reformat(width=out_w, height=out_h, format='yuv420p')

                        frame.pts = None
                        for out_packet in v_out.encode(frame):
                            output_container.mux(out_packet)

                    # Audio Frame Processing
                    elif isinstance(frame, av.AudioFrame) and a_out:
                        t = float(frame.pts * frame.time_base) if frame.pts is not None else 0.0
                        if t < self.start_sec:
                            continue
                        if t > effective_end:
                            continue

                        if resampler:
                            resampled_frames = resampler.resample(frame)
                            for rf in resampled_frames:
                                rf.pts = None
                                for out_packet in a_out.encode(rf):
                                    output_container.mux(out_packet)

            # Flush encoders & resampler
            if not self._is_cancelled:
                for out_packet in v_out.encode():
                    output_container.mux(out_packet)
                if a_out and resampler:
                    for rf in resampler.resample(None):
                        rf.pts = None
                        for out_packet in a_out.encode(rf):
                            output_container.mux(out_packet)
                if a_out:
                    for out_packet in a_out.encode():
                        output_container.mux(out_packet)

            input_container.close()
            output_container.close()

            if self._is_cancelled:
                if os.path.exists(self.output_path):
                    try:
                        os.remove(self.output_path)
                    except Exception:
                        pass
                self.error.emit("Operazione annullata.")
                return

            new_size = os.path.getsize(self.output_path)
            self.progress.emit(100)
            self.status_changed.emit("Completato!")
            self.finished.emit(self.output_path, orig_size, new_size)

        except Exception as exc:
            if os.path.exists(self.output_path):
                try:
                    os.remove(self.output_path)
                except Exception:
                    pass
            self.error.emit(f"Errore durante l'elaborazione video: {exc}")

"""
LessCam - Audio Stream & Mixing Engine
Handles high-fidelity audio decoding via PyAV and low-latency output routing via SoundDevice.
Includes gain scaling (0% to 200%), soft clipping limiter, mute toggle, and real-time VU-meter RMS metering.
"""

import threading
import collections
import time
import math
from typing import List, Dict, Any, Optional, Tuple

import numpy as np
import sounddevice as sd
import av
from PyQt6.QtCore import QObject, pyqtSignal


def get_audio_output_devices() -> List[Dict[str, Any]]:
    """
    Enumerate all valid audio output devices on the Windows system.
    Detects if VB-Audio Virtual Cable or other virtual devices are available.
    """
    devices_list = []
    try:
        devices = sd.query_devices()
        hostapis = sd.query_hostapis()
        default_output = sd.default.device[1]

        for idx, dev in enumerate(devices):
            if dev.get('max_output_channels', 0) > 0:
                api_index = dev.get('hostapi', 0)
                api_name = hostapis[api_index]['name'] if api_index < len(hostapis) else 'Unknown'
                name = dev.get('name', f'Device {idx}')

                is_virtual = any(k in name.lower() for k in ['cable', 'virtual', 'vb-audio', 'voicemeeter'])
                is_default = (idx == default_output)

                devices_list.append({
                    'index': idx,
                    'name': name,
                    'api': api_name,
                    'is_default': is_default,
                    'is_virtual': is_virtual,
                    'channels': dev.get('max_output_channels', 2),
                    'samplerate': dev.get('default_samplerate', 44100)
                })
    except Exception as e:
        print(f"[AudioEngine] Error enumerating devices: {e}")
    return devices_list


class AudioEngine(QObject):
    """
    Coordinates audio decoding from local media or streaming URLs,
    routing to the chosen sound device with volume/gain control,
    and provides real-time RMS/peak levels for GUI VU-meters.
    """
    error_occurred = pyqtSignal(str)
    position_changed = pyqtSignal(float)  # current audio time in seconds

    def __init__(self, sample_rate: int = 44100, channels: int = 2):
        super().__init__()
        self.sample_rate = sample_rate
        self.channels = channels

        # Audio state
        self._source_path: Optional[str] = None
        self._is_playing = False
        self._is_muted = False
        self._gain = 1.0  # 1.0 = 100%, 2.0 = 200%
        self._loop = True
        self._seek_requested: Optional[float] = None
        self._current_time: float = 0.0
        self._duration: float = 0.0

        # Hardware audio output
        self._selected_device: Optional[int] = None
        self._stream: Optional[sd.OutputStream] = None
        self._stream_lock = threading.Lock()

        # Buffer for decoded audio samples (float32 numpy chunks)
        self._buffer = collections.deque()
        self._buffer_lock = threading.Lock()
        self._max_buffer_samples = sample_rate * 3  # Buffer up to 3 seconds of audio
        self._buffered_samples = 0

        # RMS VU-meter levels (Left, Right) in range [0.0, 1.0]
        self._rms_levels: Tuple[float, float] = (0.0, 0.0)
        self._peak_levels: Tuple[float, float] = (0.0, 0.0)
        self._meter_lock = threading.Lock()

        # Decoder thread control
        self._decoder_thread: Optional[threading.Thread] = None
        self._stop_event = threading.Event()

        # Initialize default device
        self._find_initial_output_device()

    def _find_initial_output_device(self):
        """Prefer VB-Audio Cable if installed, otherwise use Windows default."""
        devices = get_audio_output_devices()
        virtual_dev = next((d for d in devices if d['is_virtual']), None)
        if virtual_dev:
            self._selected_device = virtual_dev['index']
        else:
            default_dev = next((d for d in devices if d['is_default']), None)
            if default_dev:
                self._selected_device = default_dev['index']
            elif devices:
                self._selected_device = devices[0]['index']

    def set_output_device(self, device_index: int):
        """Switch audio output to another device on-the-fly."""
        with self._stream_lock:
            self._selected_device = device_index
            if self._stream and self._stream.active:
                self._restart_stream()

    def _restart_stream(self):
        """Internal helper to reopen the sounddevice stream."""
        if self._stream:
            try:
                self._stream.stop()
                self._stream.close()
            except Exception:
                pass
            self._stream = None

        try:
            self._stream = sd.OutputStream(
                samplerate=self.sample_rate,
                channels=self.channels,
                dtype='float32',
                device=self._selected_device,
                blocksize=1024,
                callback=self._audio_callback
            )
            self._stream.start()
        except Exception as exc:
            self.error_occurred.emit(f"Errore periferica audio: {exc}")

    def load_source(self, source_path: Optional[str], duration: float = 0.0):
        """Load a new audio source (file path or HTTP URL)."""
        self.stop()
        self._source_path = source_path
        self._duration = duration
        self._current_time = 0.0

        if not source_path:
            return

        self._stop_event.clear()
        self._decoder_thread = threading.Thread(target=self._decoder_worker, daemon=True)
        self._decoder_thread.start()

        # Open audio stream if not open
        with self._stream_lock:
            if self._stream is None or not self._stream.active:
                self._restart_stream()

    def play(self):
        """Resume audio playback."""
        self._is_playing = True
        with self._stream_lock:
            if self._stream and not self._stream.active:
                try:
                    self._stream.start()
                except Exception:
                    self._restart_stream()

    def pause(self):
        """Pause audio playback."""
        self._is_playing = False

    def stop(self):
        """Stop playback and clear all audio buffers."""
        self._is_playing = False
        self._stop_event.set()
        if self._decoder_thread and self._decoder_thread.is_alive():
            self._decoder_thread.join(timeout=1.0)
        self._decoder_thread = None

        with self._buffer_lock:
            self._buffer.clear()
            self._buffered_samples = 0

        with self._meter_lock:
            self._rms_levels = (0.0, 0.0)
            self._peak_levels = (0.0, 0.0)

    def seek(self, target_sec: float):
        """Request a seek to a specific position in seconds."""
        self._seek_requested = max(0.0, target_sec)
        with self._buffer_lock:
            self._buffer.clear()
            self._buffered_samples = 0

    def set_volume(self, percentage: int):
        """
        Set volume in range 0% - 200%.
        percentage 100 -> gain = 1.0 (0 dB).
        percentage 200 -> gain = 2.0 (+6 dB).
        """
        clamped = max(0, min(200, percentage))
        self._gain = clamped / 100.0

    def set_muted(self, muted: bool):
        """Mute or unmute audio output."""
        self._is_muted = muted

    def set_loop(self, loop: bool):
        """Toggle continuous loop."""
        self._loop = loop

    def get_meter_levels(self) -> Tuple[float, float]:
        """
        Get current RMS levels [0.0, 1.0] for Left and Right channels.
        Used by GUI VU-meter at high refresh rate (e.g. 30 FPS).
        """
        with self._meter_lock:
            return self._rms_levels

    def get_current_time(self) -> float:
        """Returns the current playback timestamp in seconds."""
        return self._current_time

    # ==================== Internal Audio Callback ====================

    def _audio_callback(self, outdata: np.ndarray, frames: int, time_info, status):
        """Real-time sounddevice output callback pulling decoded samples."""
        if status:
            pass  # Overflow or underflow handling

        if not self._is_playing or self._is_muted or self._gain == 0.0:
            outdata.fill(0.0)
            with self._meter_lock:
                self._rms_levels = (0.0, 0.0)
                self._peak_levels = (0.0, 0.0)
            return

        needed = frames
        collected_chunks = []
        samples_taken = 0

        with self._buffer_lock:
            while self._buffer and samples_taken < needed:
                chunk = self._buffer.popleft()
                chunk_len = len(chunk)

                if samples_taken + chunk_len <= needed:
                    collected_chunks.append(chunk)
                    samples_taken += chunk_len
                    self._buffered_samples -= chunk_len
                else:
                    split_idx = needed - samples_taken
                    collected_chunks.append(chunk[:split_idx])
                    self._buffer.appendleft(chunk[split_idx:])
                    self._buffered_samples -= split_idx
                    samples_taken += split_idx
                    break

        if samples_taken > 0:
            audio_data = np.vstack(collected_chunks)
            if samples_taken < needed:
                # Pad with silence if buffer underflow
                pad = np.zeros((needed - samples_taken, self.channels), dtype=np.float32)
                audio_data = np.vstack([audio_data, pad])
        else:
            audio_data = np.zeros((needed, self.channels), dtype=np.float32)

        # Apply gain
        if self._gain != 1.0:
            audio_data = audio_data * self._gain
            # Soft limiting when boosted above 1.0 to avoid harsh digital square clipping
            if self._gain > 1.0:
                audio_data = np.tanh(audio_data)
            else:
                np.clip(audio_data, -1.0, 1.0, out=audio_data)
        else:
            np.clip(audio_data, -1.0, 1.0, out=audio_data)

        outdata[:] = audio_data

        # Calculate RMS & Peak for VU Meter
        if samples_taken > 0:
            left = audio_data[:, 0]
            right = audio_data[:, 1] if self.channels > 1 else left
            rms_l = float(np.sqrt(np.mean(left ** 2)))
            rms_r = float(np.sqrt(np.mean(right ** 2)))
            with self._meter_lock:
                self._rms_levels = (min(1.0, rms_l * 1.8), min(1.0, rms_r * 1.8))
        else:
            with self._meter_lock:
                self._rms_levels = (0.0, 0.0)

        # Update clock
        self._current_time += (samples_taken / self.sample_rate)

    # ==================== Audio Decoder Worker ====================

    def _decoder_worker(self):
        """
        Background worker that opens container with PyAV, decodes audio packets,
        resamples to 44.1kHz stereo float32, and pushes to circular buffer.
        """
        while not self._stop_event.is_set():
            if not self._source_path:
                time.sleep(0.05)
                continue

            try:
                container = av.open(self._source_path)
                audio_streams = [s for s in container.streams if s.type == 'audio']
                if not audio_streams:
                    container.close()
                    time.sleep(0.1)
                    return

                stream = audio_streams[0]
                resampler = av.AudioResampler(format='flt', layout='stereo', rate=self.sample_rate)

                for frame in container.decode(stream):
                    if self._stop_event.is_set():
                        break

                    # Handle seek request
                    if self._seek_requested is not None:
                        target_pts = int(self._seek_requested / float(stream.time_base))
                        container.seek(target_pts, stream=stream, any_frame=False)
                        self._current_time = self._seek_requested
                        self._seek_requested = None
                        continue

                    # Buffer throttling: wait if buffer has enough data
                    while self._buffered_samples >= self._max_buffer_samples and not self._stop_event.is_set():
                        if self._seek_requested is not None:
                            break
                        time.sleep(0.02)

                    # Resample to float32 stereo
                    resampled_frames = resampler.resample(frame)
                    if resampled_frames:
                        for rf in resampled_frames:
                            # rf.to_ndarray() has shape (channels, samples)
                            arr = rf.to_ndarray()
                            # Transpose to (samples, channels)
                            arr = arr.T.astype(np.float32)
                            with self._buffer_lock:
                                self._buffer.append(arr)
                                self._buffered_samples += len(arr)

                container.close()

                # If reached EOF and loop is enabled, restart loop
                if self._loop and not self._stop_event.is_set():
                    self._current_time = 0.0
                    continue
                else:
                    break

            except Exception as e:
                if not self._stop_event.is_set():
                    print(f"[AudioEngine] Decoder error: {e}")
                time.sleep(0.5)
                break

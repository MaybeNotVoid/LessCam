"""
LessCam - YouTube Stream Extraction Engine
Handles fast asynchronous extraction of direct video and audio stream URLs using yt-dlp.
"""

import re
from typing import Optional, Dict, Any
from PyQt6.QtCore import QThread, pyqtSignal
import yt_dlp


def is_valid_youtube_url(url: str) -> bool:
    """Validate if the string looks like a supported YouTube URL."""
    if not url or not isinstance(url, str):
        return False
    url = url.strip()
    patterns = [
        r'(https?://)?(www\.)?(youtube\.com/watch\?v=[\w-]+)',
        r'(https?://)?(www\.)?(youtu\.be/[\w-]+)',
        r'(https?://)?(www\.)?(youtube\.com/shorts/[\w-]+)',
        r'(https?://)?(www\.)?(youtube\.com/live/[\w-]+)',
        r'(https?://)?(www\.)?(youtube\.com/embed/[\w-]+)',
    ]
    return any(re.search(pat, url) for pat in patterns)


class YouTubeExtractorThread(QThread):
    """
    QThread worker to extract direct media URLs from YouTube without blocking the GUI.
    """
    success = pyqtSignal(dict)  # Emits metadata dict
    error = pyqtSignal(str)     # Emits error message

    def __init__(self, url: str, target_resolution: int = 1080, parent=None):
        super().__init__(parent)
        self.url = url.strip()
        self.target_resolution = target_resolution
        self._is_cancelled = False

    def cancel(self):
        self._is_cancelled = True

    def run(self):
        if not is_valid_youtube_url(self.url):
            self.error.emit("URL YouTube non valido o non supportato.")
            return

        ydl_opts = {
            'quiet': True,
            'no_warnings': True,
            'extract_flat': False,
            'skip_download': True,
            'youtube_include_dash_manifest': False,
            'youtube_include_hls_manifest': False,
        }

        try:
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                if self._is_cancelled:
                    return
                info = ydl.extract_info(self.url, download=False)
                if not info:
                    self.error.emit("Impossibile estrarre informazioni dal video YouTube.")
                    return

                formats = info.get('formats', [])
                if not formats:
                    self.error.emit("Nessun formato streaming trovato per questo video.")
                    return

                # Filter valid video formats (height <= target_resolution)
                video_candidates = []
                audio_candidates = []
                progressive_candidates = []

                for f in formats:
                    url = f.get('url')
                    if not url or not url.startswith(('http://', 'https://')):
                        continue

                    vcodec = f.get('vcodec', 'none')
                    acodec = f.get('acodec', 'none')
                    height = f.get('height') or 0
                    fps = f.get('fps') or 30
                    ext = f.get('ext', '')

                    # Combined progressive (video + audio in one container)
                    if vcodec != 'none' and acodec != 'none' and height > 0:
                        if height <= self.target_resolution:
                            progressive_candidates.append({
                                'format_id': f.get('format_id'),
                                'height': height,
                                'fps': fps,
                                'ext': ext,
                                'url': url,
                                'vcodec': vcodec,
                                'acodec': acodec,
                            })

                    # Adaptive Video-only
                    if vcodec != 'none' and acodec == 'none' and height > 0:
                        if height <= self.target_resolution:
                            # Prefer h264/mp4 for smooth OpenCV decoding
                            score = height * 10 + (fps or 0)
                            if 'avc' in vcodec or ext == 'mp4':
                                score += 1000
                            video_candidates.append({
                                'score': score,
                                'height': height,
                                'fps': fps,
                                'url': url,
                                'ext': ext,
                                'vcodec': vcodec,
                            })

                    # Adaptive Audio-only
                    if acodec != 'none' and vcodec == 'none':
                        abr = f.get('abr') or 0
                        score = abr
                        if ext in ('m4a', 'aac'):
                            score += 500  # PyAV and AAC are highly reliable
                        audio_candidates.append({
                            'score': score,
                            'abr': abr,
                            'url': url,
                            'ext': ext,
                            'acodec': acodec,
                        })

                # Select best video and audio streams
                selected_video_url = None
                selected_audio_url = None
                res_actual = self.target_resolution

                if video_candidates and audio_candidates:
                    # Sort candidates by score descending
                    video_candidates.sort(key=lambda x: x['score'], reverse=True)
                    audio_candidates.sort(key=lambda x: x['score'], reverse=True)

                    selected_video_url = video_candidates[0]['url']
                    res_actual = video_candidates[0]['height']
                    selected_audio_url = audio_candidates[0]['url']
                elif progressive_candidates:
                    progressive_candidates.sort(key=lambda x: x['height'], reverse=True)
                    best_prog = progressive_candidates[0]
                    selected_video_url = best_prog['url']
                    selected_audio_url = best_prog['url']
                    res_actual = best_prog['height']
                elif video_candidates:
                    video_candidates.sort(key=lambda x: x['score'], reverse=True)
                    selected_video_url = video_candidates[0]['url']
                    res_actual = video_candidates[0]['height']
                else:
                    self.error.emit("Nessuno stream video compatibile trovato.")
                    return

                result = {
                    'title': info.get('title', 'YouTube Video'),
                    'duration': float(info.get('duration') or 0.0),
                    'video_url': selected_video_url,
                    'audio_url': selected_audio_url,
                    'thumbnail_url': info.get('thumbnail'),
                    'height': res_actual,
                    'is_live': info.get('is_live', False),
                }

                if not self._is_cancelled:
                    self.success.emit(result)

        except Exception as exc:
            if not self._is_cancelled:
                self.error.emit(f"Errore durante l'estrazione YouTube: {str(exc)}")

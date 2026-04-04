"""Beat and onset detection using librosa (WSAA-01).

Provides beat tracking, onset detection, tempo estimation, and downbeat
detection for audio files. Results are returned as BeatAnalysisResult
Pydantic models compatible with the Vidodo asset-IR schema.
"""

from __future__ import annotations

from pathlib import Path

from .models.analysis_result import (
    AnalysisStatus,
    BeatAnalysisResult,
    BeatInfo,
    OnsetInfo,
    TempoEstimate,
)


def analyze_beats(
    audio_path: str | Path,
    asset_id: str = "",
    sr: int = 22050,
) -> BeatAnalysisResult:
    """Run beat and onset detection on an audio file.

    Args:
        audio_path: Path to audio file (WAV, FLAC, MP3, etc.).
        asset_id: Asset identifier for the result.
        sr: Target sample rate for analysis.

    Returns:
        BeatAnalysisResult with detected beats, onsets, tempo, and downbeats.
    """
    try:
        import librosa
        import numpy as np
    except ImportError as e:
        return BeatAnalysisResult(
            asset_id=asset_id,
            source_path=str(audio_path),
            duration_sec=0.0,
            sample_rate=sr,
            tempo=TempoEstimate(bpm=120.0, confidence=0.0),
            status=AnalysisStatus.ERROR,
            error_message=f"Missing dependency: {e}",
        )

    audio_path = Path(audio_path)
    if not audio_path.exists():
        return BeatAnalysisResult(
            asset_id=asset_id,
            source_path=str(audio_path),
            duration_sec=0.0,
            sample_rate=sr,
            tempo=TempoEstimate(bpm=120.0, confidence=0.0),
            status=AnalysisStatus.ERROR,
            error_message=f"File not found: {audio_path}",
        )

    y, sr_actual = librosa.load(str(audio_path), sr=sr)
    duration = librosa.get_duration(y=y, sr=sr_actual)

    # Tempo estimation
    tempo_val, beat_frames = librosa.beat.beat_track(y=y, sr=sr_actual)
    if isinstance(tempo_val, np.ndarray):
        tempo_val = float(tempo_val[0])
    beat_times = librosa.frames_to_time(beat_frames, sr=sr_actual)

    beats = [
        BeatInfo(time_sec=float(t), confidence=0.8, beat_number=i)
        for i, t in enumerate(beat_times)
    ]

    # Onset detection
    onset_frames = librosa.onset.onset_detect(y=y, sr=sr_actual)
    onset_times = librosa.frames_to_time(onset_frames, sr=sr_actual)
    onset_env = librosa.onset.onset_strength(y=y, sr=sr_actual)

    onsets = []
    for i, t in enumerate(onset_times):
        strength = float(onset_env[onset_frames[i]]) if i < len(onset_frames) and onset_frames[i] < len(onset_env) else 0.0
        onsets.append(OnsetInfo(time_sec=float(t), strength=strength))

    # Downbeat estimation (using beat phase)
    downbeats = [float(beat_times[i]) for i in range(0, len(beat_times), 4)]

    return BeatAnalysisResult(
        asset_id=asset_id,
        source_path=str(audio_path),
        duration_sec=float(duration),
        sample_rate=sr_actual,
        tempo=TempoEstimate(bpm=float(tempo_val), confidence=0.8),
        beats=beats,
        onsets=onsets,
        downbeats=downbeats,
        time_signature_estimate=[4, 4],
        status=AnalysisStatus.SUCCESS,
    )

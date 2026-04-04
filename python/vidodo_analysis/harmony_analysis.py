"""Harmony, key, and chord analysis using essentia (WSAA-02).

Provides key detection, chord recognition, and scale analysis for audio files.
Results are returned as HarmonyAnalysisResult Pydantic models compatible
with the Vidodo asset-IR schema.
"""

from __future__ import annotations

from pathlib import Path

from .models.analysis_result import (
    AnalysisStatus,
    ChordEvent,
    HarmonyAnalysisResult,
    KeyEstimate,
)


def analyze_harmony(
    audio_path: str | Path,
    asset_id: str = "",
    sr: int = 44100,
) -> HarmonyAnalysisResult:
    """Run harmony and key analysis on an audio file.

    Args:
        audio_path: Path to audio file (WAV, FLAC, MP3, etc.).
        asset_id: Asset identifier for the result.
        sr: Target sample rate for analysis.

    Returns:
        HarmonyAnalysisResult with detected key, chords, and scale.
    """
    try:
        import essentia
        import essentia.standard as es
    except ImportError as e:
        return HarmonyAnalysisResult(
            asset_id=asset_id,
            source_path=str(audio_path),
            duration_sec=0.0,
            key=KeyEstimate(key="unknown", confidence=0.0),
            status=AnalysisStatus.ERROR,
            error_message=f"Missing dependency: {e}",
        )

    audio_path = Path(audio_path)
    if not audio_path.exists():
        return HarmonyAnalysisResult(
            asset_id=asset_id,
            source_path=str(audio_path),
            duration_sec=0.0,
            key=KeyEstimate(key="unknown", confidence=0.0),
            status=AnalysisStatus.ERROR,
            error_message=f"File not found: {audio_path}",
        )

    # Load audio
    loader = es.MonoLoader(filename=str(audio_path), sampleRate=sr)
    audio = loader()
    duration = len(audio) / sr

    # Key detection
    key_extractor = es.KeyExtractor()
    key, scale, key_strength = key_extractor(audio)
    key_result = KeyEstimate(
        key=f"{key} {scale}",
        confidence=float(key_strength),
    )

    # Chord detection using HPCP + chord templates
    chords = _detect_chords(audio, sr)

    return HarmonyAnalysisResult(
        asset_id=asset_id,
        source_path=str(audio_path),
        duration_sec=float(duration),
        key=key_result,
        chords=chords,
        scale=scale,
        status=AnalysisStatus.SUCCESS,
    )


def _detect_chords(audio, sr: int, hop_size: int = 2048) -> list[ChordEvent]:
    """Detect chords using essentia's ChordsDetection algorithm."""
    try:
        import essentia.standard as es
    except ImportError:
        return []

    frame_size = 4096
    w = es.Windowing(type="blackmanharris62")
    spectrum = es.Spectrum()
    spectral_peaks = es.SpectralPeaks(
        orderBy="magnitude",
        magnitudeThreshold=0.0001,
        maxPeaks=60,
        minFrequency=20,
        maxFrequency=5000,
    )
    hpcp = es.HPCP(size=36, referenceFrequency=440)
    chords_detect = es.ChordsDetection(hopSize=hop_size)

    hpcp_frames = []
    for frame in es.FrameGenerator(audio, frameSize=frame_size, hopSize=hop_size):
        windowed = w(frame)
        spec = spectrum(windowed)
        freqs, mags = spectral_peaks(spec)
        hpcp_frame = hpcp(freqs, mags)
        hpcp_frames.append(hpcp_frame)

    if not hpcp_frames:
        return []

    import numpy as np

    hpcp_array = np.array(hpcp_frames)
    chord_labels, chord_strengths = chords_detect(hpcp_array)

    chords = []
    frame_duration = hop_size / sr
    for i, (label, strength) in enumerate(zip(chord_labels, chord_strengths)):
        if label and label != "N":
            chords.append(
                ChordEvent(
                    time_sec=float(i * frame_duration),
                    duration_sec=float(frame_duration),
                    label=str(label),
                    confidence=float(strength),
                )
            )

    return chords

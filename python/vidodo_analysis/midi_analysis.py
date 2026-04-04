"""MIDI symbolic parsing using music21 (WSAA-03).

Provides MIDI file parsing, note extraction, tempo/key/time-signature detection.
Results are returned as MidiAnalysisResult Pydantic models compatible
with the Vidodo asset-IR schema.
"""

from __future__ import annotations

from pathlib import Path

from .models.analysis_result import (
    AnalysisStatus,
    MidiAnalysisResult,
    MidiNote,
    MidiTrackInfo,
    TempoEstimate,
)


def parse_midi(
    midi_path: str | Path,
    asset_id: str = "",
) -> MidiAnalysisResult:
    """Parse a MIDI file and extract musical structure.

    Args:
        midi_path: Path to MIDI file (.mid, .midi).
        asset_id: Asset identifier for the result.

    Returns:
        MidiAnalysisResult with tracks, notes, tempo, key, and time signatures.
    """
    try:
        from music21 import converter, meter, key, tempo as m21_tempo
    except ImportError as e:
        return MidiAnalysisResult(
            asset_id=asset_id,
            source_path=str(midi_path),
            duration_sec=0.0,
            ticks_per_beat=480,
            status=AnalysisStatus.ERROR,
            error_message=f"Missing dependency: {e}",
        )

    midi_path = Path(midi_path)
    if not midi_path.exists():
        return MidiAnalysisResult(
            asset_id=asset_id,
            source_path=str(midi_path),
            duration_sec=0.0,
            ticks_per_beat=480,
            status=AnalysisStatus.ERROR,
            error_message=f"File not found: {midi_path}",
        )

    score = converter.parse(str(midi_path))

    # Duration
    duration_sec = float(score.duration.quarterLength * 60.0 / 120.0)  # rough estimate

    # Tempo changes
    tempo_changes = []
    for mm in score.flatten().getElementsByClass(m21_tempo.MetronomeMark):
        tempo_changes.append(
            TempoEstimate(bpm=float(mm.number), confidence=1.0)
        )
    if not tempo_changes:
        tempo_changes.append(TempoEstimate(bpm=120.0, confidence=0.5))

    # Time signatures
    time_sigs = []
    for ts in score.flatten().getElementsByClass(meter.TimeSignature):
        time_sigs.append([ts.numerator, ts.denominator])
    if not time_sigs:
        time_sigs.append([4, 4])

    # Key signatures
    key_sigs = []
    for ks in score.flatten().getElementsByClass(key.KeySignature):
        mode = getattr(ks, "mode", "major")
        key_sigs.append(f"{ks.asKey(mode).tonic.name} {mode}")

    # Tracks
    tracks = []
    total_notes = 0
    for i, part in enumerate(score.parts):
        track_notes = []
        for n in part.flatten().notes:
            if hasattr(n, "pitch"):
                track_notes.append(
                    MidiNote(
                        pitch=n.pitch.midi,
                        velocity=getattr(n, "volume", None) and int(n.volume.velocity or 64) or 64,
                        start_sec=float(n.offset * 60.0 / (tempo_changes[0].bpm if tempo_changes else 120.0)),
                        duration_sec=float(n.duration.quarterLength * 60.0 / (tempo_changes[0].bpm if tempo_changes else 120.0)),
                        channel=0,
                    )
                )
            elif hasattr(n, "pitches"):
                for p in n.pitches:
                    track_notes.append(
                        MidiNote(
                            pitch=p.midi,
                            velocity=64,
                            start_sec=float(n.offset * 60.0 / (tempo_changes[0].bpm if tempo_changes else 120.0)),
                            duration_sec=float(n.duration.quarterLength * 60.0 / (tempo_changes[0].bpm if tempo_changes else 120.0)),
                            channel=0,
                        )
                    )

        instrument_name = None
        if part.partName:
            instrument_name = part.partName

        total_notes += len(track_notes)
        tracks.append(
            MidiTrackInfo(
                track_index=i,
                name=instrument_name,
                instrument=instrument_name,
                note_count=len(track_notes),
                notes=track_notes,
            )
        )

    return MidiAnalysisResult(
        asset_id=asset_id,
        source_path=str(midi_path),
        duration_sec=duration_sec,
        ticks_per_beat=480,
        tempo_changes=tempo_changes,
        time_signatures=time_sigs,
        key_signatures=key_sigs,
        tracks=tracks,
        total_notes=total_notes,
        status=AnalysisStatus.SUCCESS,
    )

"""Tests for Pydantic analysis result models (no external deps needed)."""

import json
import pytest

from vidodo_analysis.models.analysis_result import (
    AnalysisStatus,
    BeatAnalysisResult,
    BeatInfo,
    ChordEvent,
    HarmonyAnalysisResult,
    KeyEstimate,
    MidiAnalysisResult,
    MidiNote,
    MidiTrackInfo,
    OnsetInfo,
    TempoEstimate,
)


class TestBeatAnalysisResult:
    def test_minimal_roundtrip(self):
        result = BeatAnalysisResult(
            asset_id="test-001",
            source_path="/tmp/test.wav",
            duration_sec=180.0,
            sample_rate=22050,
            tempo=TempoEstimate(bpm=128.0, confidence=0.9),
        )
        data = json.loads(result.model_dump_json())
        decoded = BeatAnalysisResult.model_validate(data)
        assert decoded.asset_id == "test-001"
        assert decoded.tempo.bpm == 128.0

    def test_with_beats_and_onsets(self):
        result = BeatAnalysisResult(
            asset_id="test-002",
            source_path="/tmp/test.wav",
            duration_sec=60.0,
            sample_rate=44100,
            tempo=TempoEstimate(bpm=120.0, confidence=0.85),
            beats=[
                BeatInfo(time_sec=0.5, confidence=0.9, beat_number=0),
                BeatInfo(time_sec=1.0, confidence=0.88, beat_number=1),
            ],
            onsets=[OnsetInfo(time_sec=0.5, strength=0.7)],
            downbeats=[0.5],
            time_signature_estimate=[4, 4],
        )
        assert len(result.beats) == 2
        assert len(result.onsets) == 1
        assert result.status == AnalysisStatus.SUCCESS

    def test_error_status(self):
        result = BeatAnalysisResult(
            asset_id="err",
            source_path="missing.wav",
            duration_sec=0.0,
            sample_rate=22050,
            tempo=TempoEstimate(bpm=120.0, confidence=0.0),
            status=AnalysisStatus.ERROR,
            error_message="file not found",
        )
        assert result.status == AnalysisStatus.ERROR
        assert "not found" in result.error_message


class TestHarmonyAnalysisResult:
    def test_roundtrip(self):
        result = HarmonyAnalysisResult(
            asset_id="h-001",
            source_path="/tmp/test.wav",
            duration_sec=120.0,
            key=KeyEstimate(key="C major", confidence=0.92),
            chords=[
                ChordEvent(time_sec=0.0, duration_sec=2.0, label="Cmaj", confidence=0.8),
                ChordEvent(time_sec=2.0, duration_sec=2.0, label="Am", confidence=0.75),
            ],
            scale="major",
        )
        data = json.loads(result.model_dump_json())
        decoded = HarmonyAnalysisResult.model_validate(data)
        assert decoded.key.key == "C major"
        assert len(decoded.chords) == 2


class TestMidiAnalysisResult:
    def test_roundtrip(self):
        result = MidiAnalysisResult(
            asset_id="m-001",
            source_path="/tmp/test.mid",
            duration_sec=90.0,
            ticks_per_beat=480,
            tempo_changes=[TempoEstimate(bpm=120.0, confidence=1.0)],
            time_signatures=[[4, 4]],
            key_signatures=["C major"],
            tracks=[
                MidiTrackInfo(
                    track_index=0,
                    name="Piano",
                    instrument="Piano",
                    note_count=2,
                    notes=[
                        MidiNote(pitch=60, velocity=80, start_sec=0.0, duration_sec=0.5),
                        MidiNote(pitch=64, velocity=70, start_sec=0.5, duration_sec=0.5),
                    ],
                )
            ],
            total_notes=2,
        )
        data = json.loads(result.model_dump_json())
        decoded = MidiAnalysisResult.model_validate(data)
        assert decoded.total_notes == 2
        assert decoded.tracks[0].notes[0].pitch == 60

    def test_empty_midi(self):
        result = MidiAnalysisResult(
            asset_id="empty",
            source_path="/tmp/empty.mid",
            duration_sec=0.0,
            ticks_per_beat=480,
        )
        assert result.total_notes == 0
        assert len(result.tracks) == 0


class TestAnalyzerGracefulDegradation:
    """Test that analyzers return error results when deps are missing."""

    def test_beat_analyzer_missing_file(self):
        from vidodo_analysis.audio_analysis import analyze_beats

        result = analyze_beats("/nonexistent/file.wav", asset_id="test")
        assert result.status == AnalysisStatus.ERROR

    def test_harmony_analyzer_missing_file(self):
        from vidodo_analysis.harmony_analysis import analyze_harmony

        result = analyze_harmony("/nonexistent/file.wav", asset_id="test")
        assert result.status == AnalysisStatus.ERROR

    def test_midi_analyzer_missing_file(self):
        from vidodo_analysis.midi_analysis import parse_midi

        result = parse_midi("/nonexistent/file.mid", asset_id="test")
        assert result.status == AnalysisStatus.ERROR

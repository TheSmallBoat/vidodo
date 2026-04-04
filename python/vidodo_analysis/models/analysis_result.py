"""Pydantic models for analysis results.

These models match the JSON schema defined in schemas/ir/asset-ir.v0.json
and are used as the shared exchange format between Python analysis modules
and the Rust asset pipeline.
"""

from __future__ import annotations

from enum import Enum
from typing import Optional

from pydantic import BaseModel, Field


class AnalysisStatus(str, Enum):
    """Status of an analysis run."""

    SUCCESS = "success"
    PARTIAL = "partial"
    ERROR = "error"


class BeatInfo(BaseModel):
    """A single detected beat with timestamp and confidence."""

    time_sec: float = Field(..., description="Beat time in seconds")
    confidence: float = Field(0.0, ge=0.0, le=1.0, description="Detection confidence")
    beat_number: int = Field(0, ge=0, description="Sequential beat index")


class OnsetInfo(BaseModel):
    """A single detected onset event."""

    time_sec: float = Field(..., description="Onset time in seconds")
    strength: float = Field(0.0, ge=0.0, description="Onset strength")


class TempoEstimate(BaseModel):
    """Estimated tempo with confidence."""

    bpm: float = Field(..., gt=0.0, description="Estimated tempo in BPM")
    confidence: float = Field(0.0, ge=0.0, le=1.0)


class BeatAnalysisResult(BaseModel):
    """Result of beat/onset detection (WSAA-01)."""

    asset_id: str
    source_path: str
    duration_sec: float
    sample_rate: int
    tempo: TempoEstimate
    beats: list[BeatInfo] = Field(default_factory=list)
    onsets: list[OnsetInfo] = Field(default_factory=list)
    downbeats: list[float] = Field(default_factory=list, description="Downbeat times in seconds")
    time_signature_estimate: Optional[list[int]] = Field(None, description="e.g. [4, 4]")
    status: AnalysisStatus = AnalysisStatus.SUCCESS
    error_message: Optional[str] = None


class ChordEvent(BaseModel):
    """A chord detected at a specific time."""

    time_sec: float
    duration_sec: float
    label: str = Field(..., description="Chord label, e.g. 'Cmaj', 'Am7'")
    confidence: float = Field(0.0, ge=0.0, le=1.0)


class KeyEstimate(BaseModel):
    """Estimated musical key."""

    key: str = Field(..., description="Key name, e.g. 'C major', 'A minor'")
    confidence: float = Field(0.0, ge=0.0, le=1.0)


class HarmonyAnalysisResult(BaseModel):
    """Result of harmony/key/chord analysis (WSAA-02)."""

    asset_id: str
    source_path: str
    duration_sec: float
    key: KeyEstimate
    chords: list[ChordEvent] = Field(default_factory=list)
    scale: Optional[str] = Field(None, description="Detected scale, e.g. 'major', 'minor'")
    status: AnalysisStatus = AnalysisStatus.SUCCESS
    error_message: Optional[str] = None


class MidiNote(BaseModel):
    """A single MIDI note event."""

    pitch: int = Field(..., ge=0, le=127)
    velocity: int = Field(..., ge=0, le=127)
    start_sec: float
    duration_sec: float
    channel: int = Field(0, ge=0, le=15)


class MidiTrackInfo(BaseModel):
    """Summary of a single MIDI track."""

    track_index: int
    name: Optional[str] = None
    instrument: Optional[str] = None
    note_count: int = 0
    notes: list[MidiNote] = Field(default_factory=list)


class MidiAnalysisResult(BaseModel):
    """Result of MIDI symbolic parsing (WSAA-03)."""

    asset_id: str
    source_path: str
    duration_sec: float
    ticks_per_beat: int
    tempo_changes: list[TempoEstimate] = Field(default_factory=list)
    time_signatures: list[list[int]] = Field(default_factory=list, description="e.g. [[4,4], [3,4]]")
    key_signatures: list[str] = Field(default_factory=list)
    tracks: list[MidiTrackInfo] = Field(default_factory=list)
    total_notes: int = 0
    status: AnalysisStatus = AnalysisStatus.SUCCESS
    error_message: Optional[str] = None

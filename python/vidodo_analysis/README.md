# Vidodo Analysis

Python analysis modules for audio feature extraction, harmony analysis, and MIDI parsing.

## Modules

- `audio_analysis.py` — Beat & onset detection (librosa)
- `harmony_analysis.py` — Harmony, key & chord analysis (essentia)
- `midi_analysis.py` — MIDI symbolic parsing (music21)
- `models/analysis_result.py` — Shared Pydantic result models

## Requirements

```
librosa>=0.10
essentia>=2.1b6
music21>=9.1
pydantic>=2.0
numpy>=1.24
```

## Usage

```python
from vidodo_analysis.audio_analysis import analyze_beats
from vidodo_analysis.harmony_analysis import analyze_harmony
from vidodo_analysis.midi_analysis import parse_midi

result = analyze_beats("path/to/audio.wav")
```

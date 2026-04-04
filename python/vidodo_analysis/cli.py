"""Click CLI entry point for vidodo_analysis.

Subcommands:
  beat-detect     Run beat/onset detection (librosa)
  harmony-detect  Run harmony/key/chord analysis (essentia)
  midi-parse      Parse a MIDI file (music21)

Each subcommand reads an input file and writes JSON to stdout.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import click

from .models.analysis_result import AnalysisStatus


@click.group()
def cli():
    """Vidodo audio analysis CLI."""


@cli.command("beat-detect")
@click.argument("audio_path", type=click.Path(exists=True))
@click.option("--asset-id", default="", help="Asset identifier for the result.")
@click.option("--sr", default=22050, type=int, help="Target sample rate.")
def beat_detect(audio_path: str, asset_id: str, sr: int):
    """Run beat and onset detection on an audio file."""
    from .audio_analysis import analyze_beats

    result = analyze_beats(audio_path, asset_id=asset_id, sr=sr)
    click.echo(result.model_dump_json(indent=2))
    if result.status == AnalysisStatus.ERROR:
        sys.exit(1)


@cli.command("harmony-detect")
@click.argument("audio_path", type=click.Path(exists=True))
@click.option("--asset-id", default="", help="Asset identifier for the result.")
@click.option("--sr", default=44100, type=int, help="Target sample rate.")
def harmony_detect(audio_path: str, asset_id: str, sr: int):
    """Run harmony and key analysis on an audio file."""
    from .harmony_analysis import analyze_harmony

    result = analyze_harmony(audio_path, asset_id=asset_id, sr=sr)
    click.echo(result.model_dump_json(indent=2))
    if result.status == AnalysisStatus.ERROR:
        sys.exit(1)


@cli.command("midi-parse")
@click.argument("midi_path", type=click.Path(exists=True))
@click.option("--asset-id", default="", help="Asset identifier for the result.")
def midi_parse(midi_path: str, asset_id: str):
    """Parse a MIDI file and extract musical structure."""
    from .midi_analysis import parse_midi

    result = parse_midi(midi_path, asset_id=asset_id)
    click.echo(result.model_dump_json(indent=2))
    if result.status == AnalysisStatus.ERROR:
        sys.exit(1)


if __name__ == "__main__":
    cli()

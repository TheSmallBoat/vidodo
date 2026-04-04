//! Python analysis subprocess bridge.
//!
//! Runs `python -m vidodo_analysis <subcommand>` as a child process,
//! captures JSON stdout, and deserializes into Rust result types.

use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// Result from the Python beat-detection CLI.
#[derive(Debug, Clone, Deserialize)]
pub struct BeatDetectResult {
    pub asset_id: String,
    pub source_path: String,
    pub duration_sec: f64,
    pub sample_rate: u32,
    pub tempo: TempoEstimate,
    pub beats: Vec<BeatInfo>,
    pub status: String,
    #[serde(default)]
    pub error_message: Option<String>,
}

/// Result from the Python harmony-detection CLI.
#[derive(Debug, Clone, Deserialize)]
pub struct HarmonyDetectResult {
    pub asset_id: String,
    pub source_path: String,
    pub duration_sec: f64,
    pub key: KeyEstimate,
    pub status: String,
    #[serde(default)]
    pub error_message: Option<String>,
}

/// Result from the Python MIDI-parse CLI.
#[derive(Debug, Clone, Deserialize)]
pub struct MidiParseResult {
    pub asset_id: String,
    pub source_path: String,
    pub duration_sec: f64,
    pub ticks_per_beat: u32,
    pub total_notes: u32,
    pub status: String,
    #[serde(default)]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TempoEstimate {
    pub bpm: f64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BeatInfo {
    pub time_sec: f64,
    pub confidence: f64,
    pub beat_number: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyEstimate {
    pub key: String,
    pub confidence: f64,
}

/// Error from the Python bridge.
#[derive(Debug)]
pub enum BridgeError {
    /// Python not found or subprocess failed to start.
    PythonNotAvailable(String),
    /// Subprocess exited with non-zero code.
    SubprocessFailed { code: Option<i32>, stderr: String },
    /// JSON parsing failed.
    ParseError(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::PythonNotAvailable(msg) => write!(f, "Python not available: {msg}"),
            BridgeError::SubprocessFailed { code, stderr } => {
                write!(f, "subprocess failed (code={code:?}): {stderr}")
            }
            BridgeError::ParseError(msg) => write!(f, "JSON parse error: {msg}"),
        }
    }
}

/// Runs Python analysis CLI subcommands as child processes.
pub struct PythonAnalysisRunner {
    /// Python executable (e.g., "python3", "python").
    python_cmd: String,
    /// Path to the vidodo_analysis package root (for PYTHONPATH).
    package_root: Option<String>,
}

impl PythonAnalysisRunner {
    pub fn new() -> Self {
        Self { python_cmd: String::from("python3"), package_root: None }
    }

    /// Set a custom Python command.
    pub fn with_python_cmd(mut self, cmd: impl Into<String>) -> Self {
        self.python_cmd = cmd.into();
        self
    }

    /// Set the path containing the vidodo_analysis package.
    pub fn with_package_root(mut self, root: impl Into<String>) -> Self {
        self.package_root = Some(root.into());
        self
    }

    /// Run beat detection on an audio file.
    pub fn beat_detect(
        &self,
        audio_path: &Path,
        asset_id: &str,
    ) -> Result<BeatDetectResult, BridgeError> {
        let stdout = self.run_subcommand(
            "beat-detect",
            &[audio_path.to_string_lossy().as_ref(), "--asset-id", asset_id],
        )?;
        serde_json::from_str(&stdout).map_err(|e| BridgeError::ParseError(e.to_string()))
    }

    /// Run harmony analysis on an audio file.
    pub fn harmony_detect(
        &self,
        audio_path: &Path,
        asset_id: &str,
    ) -> Result<HarmonyDetectResult, BridgeError> {
        let stdout = self.run_subcommand(
            "harmony-detect",
            &[audio_path.to_string_lossy().as_ref(), "--asset-id", asset_id],
        )?;
        serde_json::from_str(&stdout).map_err(|e| BridgeError::ParseError(e.to_string()))
    }

    /// Parse a MIDI file.
    pub fn midi_parse(
        &self,
        midi_path: &Path,
        asset_id: &str,
    ) -> Result<MidiParseResult, BridgeError> {
        let stdout = self.run_subcommand(
            "midi-parse",
            &[midi_path.to_string_lossy().as_ref(), "--asset-id", asset_id],
        )?;
        serde_json::from_str(&stdout).map_err(|e| BridgeError::ParseError(e.to_string()))
    }

    /// Execute a vidodo_analysis subcommand and capture stdout.
    fn run_subcommand(&self, subcmd: &str, args: &[&str]) -> Result<String, BridgeError> {
        let mut cmd = Command::new(&self.python_cmd);
        cmd.arg("-m").arg("vidodo_analysis").arg(subcmd);
        cmd.args(args);

        // Set PYTHONPATH if package root is configured
        if let Some(ref root) = self.package_root {
            cmd.env("PYTHONPATH", root);
        }

        let output = cmd.output().map_err(|e| BridgeError::PythonNotAvailable(e.to_string()))?;

        if !output.status.success() {
            return Err(BridgeError::SubprocessFailed {
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl Default for PythonAnalysisRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod bridge_tests {
    use super::*;

    #[test]
    fn python_not_available_returns_clear_error() {
        let runner = PythonAnalysisRunner::new().with_python_cmd("__nonexistent_python_99__");
        let result = runner.beat_detect(Path::new("test.wav"), "test-id");
        assert!(result.is_err());
        if let Err(BridgeError::PythonNotAvailable(msg)) = result {
            assert!(!msg.is_empty());
        } else {
            panic!("expected PythonNotAvailable error");
        }
    }

    #[test]
    fn parse_beat_detect_json() {
        let json = r#"{
            "asset_id": "bass-loop",
            "source_path": "/tmp/bass.wav",
            "duration_sec": 8.0,
            "sample_rate": 22050,
            "tempo": {"bpm": 128.0, "confidence": 0.9},
            "beats": [{"time_sec": 0.5, "confidence": 0.8, "beat_number": 0}],
            "onsets": [],
            "downbeats": [],
            "time_signature_estimate": [4, 4],
            "status": "success",
            "error_message": null
        }"#;
        let result: BeatDetectResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.asset_id, "bass-loop");
        assert!((result.tempo.bpm - 128.0).abs() < f64::EPSILON);
        assert_eq!(result.beats.len(), 1);
    }

    #[test]
    fn parse_harmony_detect_json() {
        let json = r#"{
            "asset_id": "pad-1",
            "source_path": "/tmp/pad.wav",
            "duration_sec": 60.0,
            "key": {"key": "C major", "confidence": 0.85},
            "chords": [],
            "scale": "major",
            "status": "success",
            "error_message": null
        }"#;
        let result: HarmonyDetectResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.key.key, "C major");
    }

    #[test]
    fn parse_midi_parse_json() {
        let json = r#"{
            "asset_id": "melody-1",
            "source_path": "/tmp/melody.mid",
            "duration_sec": 30.0,
            "ticks_per_beat": 480,
            "tempo_changes": [{"bpm": 120.0, "confidence": 1.0}],
            "time_signatures": [[4, 4]],
            "key_signatures": ["C major"],
            "tracks": [],
            "total_notes": 42,
            "status": "success",
            "error_message": null
        }"#;
        let result: MidiParseResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.total_notes, 42);
    }
}

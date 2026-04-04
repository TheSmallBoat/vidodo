//! IR → OSC command translator.
//!
//! Translates Vidodo `ExecutablePayload::Audio` actions into sequences of
//! scsynth OSC messages. Supports three audio paths:
//!
//! - **asset_playback**: `/b_allocRead` + `/s_new` (buffer load + synth launch)
//! - **synth_render**: `/s_new` + `/n_set` (node create + param set)
//! - **stop**: `/n_free` + `/b_free` (node free + buffer free)

use crate::node_mapping::NodeMapping;
use crate::osc::{OscMessage, ScynthCmd};

/// The audio operation type inferred from the IR `op` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioPath {
    /// Play a buffer-backed asset: allocate buffer, load file, launch synth.
    AssetPlayback,
    /// Create a synth node and set parameters (no buffer).
    SynthRender,
    /// Stop / free a running node.
    Stop,
    /// Forward to an external adapter (MIDI instruments, etc.).
    DeferToAdapter,
    /// Unknown operation.
    Unknown(String),
}

impl AudioPath {
    /// Classify the IR `op` string into an audio path.
    pub fn from_op(op: &str) -> Self {
        match op {
            "play" | "play_asset" | "asset_playback" | "loop" => AudioPath::AssetPlayback,
            "synth" | "synth_render" | "render" | "gen" => AudioPath::SynthRender,
            "stop" | "free" | "kill" => AudioPath::Stop,
            "midi" | "midi_to_instrument" | "adapter" => AudioPath::DeferToAdapter,
            other => AudioPath::Unknown(other.to_string()),
        }
    }
}

/// A translated command sequence ready for dispatch to scsynth.
#[derive(Debug, Clone)]
pub struct CommandSequence {
    pub action_id: String,
    pub path: AudioPath,
    pub messages: Vec<OscMessage>,
}

/// Translates IR audio payloads into OSC command sequences.
pub struct CommandTranslator {
    mapping: NodeMapping,
    /// Default SynthDef name for asset playback.
    default_playback_def: String,
}

impl CommandTranslator {
    pub fn new() -> Self {
        Self { mapping: NodeMapping::new(), default_playback_def: String::from("vidodo_playbuf") }
    }

    /// Translate an `ExecutablePayload::Audio` into OSC commands.
    ///
    /// # Arguments
    /// - `layer_id`: the action/layer identifier
    /// - `op`: the operation string from IR
    /// - `target_asset_id`: optional asset file path for playback
    /// - `gain_db`: optional gain in dB
    /// - `duration_beats`: optional duration
    pub fn translate(
        &mut self,
        layer_id: &str,
        op: &str,
        target_asset_id: Option<&str>,
        gain_db: Option<f64>,
        duration_beats: Option<u32>,
    ) -> CommandSequence {
        let path = AudioPath::from_op(op);
        let messages = match &path {
            AudioPath::AssetPlayback => {
                self.translate_asset_playback(layer_id, target_asset_id, gain_db)
            }
            AudioPath::SynthRender => self.translate_synth_render(layer_id, gain_db),
            AudioPath::Stop => self.translate_stop(layer_id),
            AudioPath::DeferToAdapter => {
                // No OSC commands — adapter handles it externally
                vec![]
            }
            AudioPath::Unknown(_) => vec![],
        };

        let _ = duration_beats; // reserved for future envelope duration control

        CommandSequence { action_id: layer_id.to_string(), path, messages }
    }

    /// Asset playback: allocate buffer + load file + launch playbuf synth.
    fn translate_asset_playback(
        &mut self,
        layer_id: &str,
        target_asset_id: Option<&str>,
        gain_db: Option<f64>,
    ) -> Vec<OscMessage> {
        let node_id = self.mapping.allocate(layer_id);
        let buf_num = self.mapping.allocate_buffer(node_id);
        let asset_path = target_asset_id.unwrap_or("unknown.wav");

        let mut cmds = vec![
            // 1. Allocate and read the buffer
            ScynthCmd::buffer_alloc_read(buf_num, asset_path),
            // 2. Launch a playbuf synth on node
            ScynthCmd::synth_new(&self.default_playback_def, node_id, 0, 1),
        ];

        // 3. Set buffer number on the synth
        cmds.push(ScynthCmd::node_set(node_id, "bufnum", buf_num as f32));

        // 4. Set gain if provided
        if let Some(db) = gain_db {
            // Convert dB to linear amplitude
            let amp = 10.0_f64.powf(db / 20.0) as f32;
            cmds.push(ScynthCmd::node_set(node_id, "amp", amp));
        }

        cmds
    }

    /// Synth render: create synth node + set parameters.
    fn translate_synth_render(&mut self, layer_id: &str, gain_db: Option<f64>) -> Vec<OscMessage> {
        let node_id = self.mapping.allocate(layer_id);
        let def_name = format!("vidodo_{layer_id}");

        let mut cmds = vec![
            // 1. Create the synth node
            ScynthCmd::synth_new(&def_name, node_id, 0, 1),
        ];

        // 2. Set gain if provided
        if let Some(db) = gain_db {
            let amp = 10.0_f64.powf(db / 20.0) as f32;
            cmds.push(ScynthCmd::node_set(node_id, "amp", amp));
        }

        cmds
    }

    /// Stop: free the node and its buffer (if any).
    fn translate_stop(&mut self, layer_id: &str) -> Vec<OscMessage> {
        let mut cmds = Vec::new();

        if let Some(node_id) = self.mapping.remove(layer_id) {
            cmds.push(ScynthCmd::node_free(node_id));

            if let Some(buf_num) = self.mapping.remove_buffer(node_id) {
                cmds.push(ScynthCmd::buffer_free(buf_num));
            }
        }

        cmds
    }

    /// Access the node mapping.
    pub fn mapping(&self) -> &NodeMapping {
        &self.mapping
    }

    /// Mutable access to the node mapping.
    pub fn mapping_mut(&mut self) -> &mut NodeMapping {
        &mut self.mapping
    }
}

impl Default for CommandTranslator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod translator_tests {
    use super::*;
    use crate::osc::OscArg;

    #[test]
    fn asset_playback_generates_alloc_read_and_synth_new() {
        let mut t = CommandTranslator::new();
        let seq = t.translate("bass-layer", "play", Some("/audio/bass.wav"), None, None);

        assert_eq!(seq.path, AudioPath::AssetPlayback);
        assert!(seq.messages.len() >= 3); // b_allocRead + s_new + n_set(bufnum)

        assert_eq!(seq.messages[0].address, "/b_allocRead");
        assert_eq!(seq.messages[1].address, "/s_new");
        assert_eq!(seq.messages[2].address, "/n_set");

        // Verify buffer path is in the args
        if let OscArg::String(ref path) = seq.messages[0].args[1] {
            assert_eq!(path, "/audio/bass.wav");
        } else {
            panic!("expected string arg for buffer path");
        }
    }

    #[test]
    fn asset_playback_with_gain() {
        let mut t = CommandTranslator::new();
        let seq = t.translate("pad", "play", Some("pad.wav"), Some(-6.0), None);

        // Should have 4 messages: b_allocRead + s_new + n_set(bufnum) + n_set(amp)
        assert_eq!(seq.messages.len(), 4);
        assert_eq!(seq.messages[3].address, "/n_set");

        // Check amp parameter name
        if let OscArg::String(ref param) = seq.messages[3].args[1] {
            assert_eq!(param, "amp");
        } else {
            panic!("expected string arg for param name");
        }
    }

    #[test]
    fn synth_render_generates_synth_new_and_node_set() {
        let mut t = CommandTranslator::new();
        let seq = t.translate("drone", "synth", None, Some(-3.0), None);

        assert_eq!(seq.path, AudioPath::SynthRender);
        assert_eq!(seq.messages.len(), 2); // s_new + n_set(amp)

        assert_eq!(seq.messages[0].address, "/s_new");
        assert_eq!(seq.messages[1].address, "/n_set");

        // Verify def name includes layer_id
        if let OscArg::String(ref def) = seq.messages[0].args[0] {
            assert!(def.contains("drone"));
        } else {
            panic!("expected string arg for def name");
        }
    }

    #[test]
    fn stop_frees_node_and_buffer() {
        let mut t = CommandTranslator::new();

        // First play something to register the node
        t.translate("layer-1", "play", Some("test.wav"), None, None);
        assert_eq!(t.mapping().active_count(), 1);

        // Then stop it
        let seq = t.translate("layer-1", "stop", None, None, None);

        assert_eq!(seq.path, AudioPath::Stop);
        assert!(!seq.messages.is_empty()); // n_free + maybe b_free
        assert_eq!(seq.messages[0].address, "/n_free");
        if seq.messages.len() > 1 {
            assert_eq!(seq.messages[1].address, "/b_free");
        }
        assert_eq!(t.mapping().active_count(), 0);
    }

    #[test]
    fn defer_to_adapter_produces_no_osc() {
        let mut t = CommandTranslator::new();
        let seq = t.translate("midi-piano", "midi_to_instrument", None, None, None);
        assert_eq!(seq.path, AudioPath::DeferToAdapter);
        assert!(seq.messages.is_empty());
    }

    #[test]
    fn mapping_tracks_active_nodes() {
        let mut t = CommandTranslator::new();
        t.translate("a", "play", Some("a.wav"), None, None);
        t.translate("b", "synth", None, None, None);
        assert_eq!(t.mapping().active_count(), 2);
        assert!(t.mapping().lookup("a").is_some());
        assert!(t.mapping().lookup("b").is_some());
    }
}

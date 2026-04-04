//! OSC (Open Sound Control) message types and serialization for scsynth communication.
//!
//! This module defines the OSC message format used to communicate with SuperCollider's
//! scsynth audio server. Messages are serialized to bytes for UDP transport.

use serde::{Deserialize, Serialize};

/// OSC argument types supported by scsynth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OscArg {
    Int(i32),
    Float(f32),
    String(String),
    Blob(Vec<u8>),
}

/// A single OSC message with address pattern and arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OscMessage {
    pub address: String,
    pub args: Vec<OscArg>,
}

/// An OSC bundle containing multiple messages with a timetag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OscBundle {
    pub timetag: u64,
    pub messages: Vec<OscMessage>,
}

/// Top-level OSC packet: either a single message or a bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OscPacket {
    Message(OscMessage),
    Bundle(OscBundle),
}

impl OscMessage {
    pub fn new(address: impl Into<String>, args: Vec<OscArg>) -> Self {
        Self { address: address.into(), args }
    }
}

// ── OSC binary serialization ───────────────────────────────────────────

/// Pad length to next 4-byte boundary.
fn pad4(len: usize) -> usize {
    (len + 3) & !3
}

fn write_osc_string(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
}

fn write_osc_blob(buf: &mut Vec<u8>, data: &[u8]) {
    buf.extend_from_slice(&(data.len() as i32).to_be_bytes());
    buf.extend_from_slice(data);
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
}

impl OscMessage {
    /// Serialize this message to OSC binary wire format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Address pattern
        write_osc_string(&mut buf, &self.address);

        // Type tag string
        let mut typetag = String::from(",");
        for arg in &self.args {
            match arg {
                OscArg::Int(_) => typetag.push('i'),
                OscArg::Float(_) => typetag.push('f'),
                OscArg::String(_) => typetag.push('s'),
                OscArg::Blob(_) => typetag.push('b'),
            }
        }
        write_osc_string(&mut buf, &typetag);

        // Arguments
        for arg in &self.args {
            match arg {
                OscArg::Int(v) => buf.extend_from_slice(&v.to_be_bytes()),
                OscArg::Float(v) => buf.extend_from_slice(&v.to_be_bytes()),
                OscArg::String(v) => write_osc_string(&mut buf, v),
                OscArg::Blob(v) => write_osc_blob(&mut buf, v),
            }
        }

        buf
    }

    /// Deserialize an OSC message from binary wire format.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let mut pos = 0;

        let address = read_osc_string(data, &mut pos)?;
        let typetag = read_osc_string(data, &mut pos)?;

        if !typetag.starts_with(',') {
            return Err("invalid type tag: missing comma prefix".into());
        }

        let mut args = Vec::new();
        for ch in typetag[1..].chars() {
            match ch {
                'i' => {
                    if pos + 4 > data.len() {
                        return Err("truncated int arg".into());
                    }
                    let v = i32::from_be_bytes([
                        data[pos],
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                    ]);
                    pos += 4;
                    args.push(OscArg::Int(v));
                }
                'f' => {
                    if pos + 4 > data.len() {
                        return Err("truncated float arg".into());
                    }
                    let v = f32::from_be_bytes([
                        data[pos],
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                    ]);
                    pos += 4;
                    args.push(OscArg::Float(v));
                }
                's' => {
                    let s = read_osc_string(data, &mut pos)?;
                    args.push(OscArg::String(s));
                }
                'b' => {
                    if pos + 4 > data.len() {
                        return Err("truncated blob size".into());
                    }
                    let blen = i32::from_be_bytes([
                        data[pos],
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                    ]) as usize;
                    pos += 4;
                    if pos + blen > data.len() {
                        return Err("truncated blob data".into());
                    }
                    args.push(OscArg::Blob(data[pos..pos + blen].to_vec()));
                    pos += pad4(blen);
                }
                _ => return Err(format!("unsupported OSC type tag: {ch}")),
            }
        }

        Ok(Self { address, args })
    }
}

fn read_osc_string(data: &[u8], pos: &mut usize) -> Result<String, String> {
    let start = *pos;
    while *pos < data.len() && data[*pos] != 0 {
        *pos += 1;
    }
    if *pos >= data.len() {
        return Err("unterminated OSC string".into());
    }
    let s = String::from_utf8(data[start..*pos].to_vec())
        .map_err(|e| format!("invalid UTF-8 in OSC string: {e}"))?;
    *pos += 1; // skip null
    *pos = pad4(*pos); // align to 4 bytes
    Ok(s)
}

// ── Well-known scsynth commands ────────────────────────────────────────

/// Factory methods for common scsynth OSC commands.
pub struct ScynthCmd;

impl ScynthCmd {
    /// Query server status.
    pub fn status() -> OscMessage {
        OscMessage::new("/status", vec![])
    }

    /// Quit the server.
    pub fn quit() -> OscMessage {
        OscMessage::new("/quit", vec![])
    }

    /// Notify: register to receive replies from the server.
    pub fn notify(on: bool) -> OscMessage {
        OscMessage::new("/notify", vec![OscArg::Int(i32::from(on))])
    }

    /// Allocate and read a sound file into a buffer.
    pub fn buffer_alloc_read(buf_num: i32, path: &str) -> OscMessage {
        OscMessage::new(
            "/b_allocRead",
            vec![OscArg::Int(buf_num), OscArg::String(path.to_string())],
        )
    }

    /// Free a buffer.
    pub fn buffer_free(buf_num: i32) -> OscMessage {
        OscMessage::new("/b_free", vec![OscArg::Int(buf_num)])
    }

    /// Create a new synth node.
    pub fn synth_new(def_name: &str, node_id: i32, add_action: i32, target_id: i32) -> OscMessage {
        OscMessage::new(
            "/s_new",
            vec![
                OscArg::String(def_name.to_string()),
                OscArg::Int(node_id),
                OscArg::Int(add_action),
                OscArg::Int(target_id),
            ],
        )
    }

    /// Set a synth node control parameter.
    pub fn node_set(node_id: i32, param: &str, value: f32) -> OscMessage {
        OscMessage::new(
            "/n_set",
            vec![OscArg::Int(node_id), OscArg::String(param.to_string()), OscArg::Float(value)],
        )
    }

    /// Free a synth node.
    pub fn node_free(node_id: i32) -> OscMessage {
        OscMessage::new("/n_free", vec![OscArg::Int(node_id)])
    }

    /// Load a SynthDef file.
    pub fn synthdef_load(path: &str) -> OscMessage {
        OscMessage::new("/d_load", vec![OscArg::String(path.to_string())])
    }
}

#[cfg(test)]
mod osc_tests {
    use super::*;

    #[test]
    fn roundtrip_status_command() {
        let msg = ScynthCmd::status();
        let bytes = msg.to_bytes();
        let decoded = OscMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.address, "/status");
        assert!(decoded.args.is_empty());
    }

    #[test]
    fn roundtrip_buffer_alloc_read() {
        let msg = ScynthCmd::buffer_alloc_read(42, "/tmp/test.wav");
        let bytes = msg.to_bytes();
        let decoded = OscMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.address, "/b_allocRead");
        assert_eq!(decoded.args.len(), 2);
        assert_eq!(decoded.args[0], OscArg::Int(42));
        assert_eq!(decoded.args[1], OscArg::String("/tmp/test.wav".to_string()));
    }

    #[test]
    fn roundtrip_synth_new() {
        let msg = ScynthCmd::synth_new("default", 1001, 0, 1);
        let bytes = msg.to_bytes();
        let decoded = OscMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.address, "/s_new");
        assert_eq!(decoded.args.len(), 4);
        assert_eq!(decoded.args[0], OscArg::String("default".to_string()));
        assert_eq!(decoded.args[1], OscArg::Int(1001));
    }

    #[test]
    fn roundtrip_node_set_float() {
        let msg = ScynthCmd::node_set(1001, "freq", 440.0);
        let bytes = msg.to_bytes();
        let decoded = OscMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.address, "/n_set");
        assert_eq!(decoded.args[0], OscArg::Int(1001));
        assert_eq!(decoded.args[1], OscArg::String("freq".to_string()));
        assert_eq!(decoded.args[2], OscArg::Float(440.0));
    }

    #[test]
    fn roundtrip_blob_argument() {
        let msg = OscMessage::new("/test", vec![OscArg::Blob(vec![1, 2, 3, 4, 5])]);
        let bytes = msg.to_bytes();
        let decoded = OscMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.args[0], OscArg::Blob(vec![1, 2, 3, 4, 5]));
    }

    #[test]
    fn invalid_typetag_returns_error() {
        // Craft bytes with no comma in type tag
        let mut buf = Vec::new();
        write_osc_string(&mut buf, "/test");
        write_osc_string(&mut buf, "i"); // missing comma prefix
        assert!(OscMessage::from_bytes(&buf).is_err());
    }
}

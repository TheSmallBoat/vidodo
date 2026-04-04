//! scsynth server status query types.
//!
//! Parses the `/status.reply` OSC message returned by scsynth and provides
//! a structured representation of the server's runtime state.

use serde::{Deserialize, Serialize};

use crate::osc::{OscArg, OscMessage};

/// Parsed scsynth server status from `/status.reply`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    /// Number of unit generators running.
    pub num_ugens: i32,
    /// Number of synth nodes.
    pub num_synths: i32,
    /// Number of groups.
    pub num_groups: i32,
    /// Number of loaded SynthDefs.
    pub num_synthdefs: i32,
    /// Average CPU usage (percent).
    pub avg_cpu: f32,
    /// Peak CPU usage (percent).
    pub peak_cpu: f32,
    /// Nominal sample rate.
    pub nominal_sample_rate: f32,
    /// Actual sample rate.
    pub actual_sample_rate: f32,
}

/// Status query result: either a valid status or an error/timeout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryResult {
    Ok(ServerStatus),
    Timeout,
    Error(String),
}

impl ServerStatus {
    /// Parse a `/status.reply` OSC message into a `ServerStatus`.
    ///
    /// Expected arg layout from scsynth (after the unused first int):
    ///   [unused:i, num_ugens:i, num_synths:i, num_groups:i, num_synthdefs:i,
    ///    avg_cpu:f, peak_cpu:f, nominal_sr:f, actual_sr:f]
    pub fn from_status_reply(msg: &OscMessage) -> Result<Self, String> {
        if msg.address != "/status.reply" {
            return Err(format!("expected /status.reply, got {}", msg.address));
        }

        if msg.args.len() < 9 {
            return Err(format!("expected 9 args in /status.reply, got {}", msg.args.len()));
        }

        fn extract_int(arg: &OscArg, name: &str) -> Result<i32, String> {
            match arg {
                OscArg::Int(v) => Ok(*v),
                _ => Err(format!("expected int for {name}, got {arg:?}")),
            }
        }

        fn extract_float(arg: &OscArg, name: &str) -> Result<f32, String> {
            match arg {
                OscArg::Float(v) => Ok(*v),
                _ => Err(format!("expected float for {name}, got {arg:?}")),
            }
        }

        Ok(Self {
            num_ugens: extract_int(&msg.args[1], "num_ugens")?,
            num_synths: extract_int(&msg.args[2], "num_synths")?,
            num_groups: extract_int(&msg.args[3], "num_groups")?,
            num_synthdefs: extract_int(&msg.args[4], "num_synthdefs")?,
            avg_cpu: extract_float(&msg.args[5], "avg_cpu")?,
            peak_cpu: extract_float(&msg.args[6], "peak_cpu")?,
            nominal_sample_rate: extract_float(&msg.args[7], "nominal_sr")?,
            actual_sample_rate: extract_float(&msg.args[8], "actual_sr")?,
        })
    }
}

#[cfg(test)]
mod query_tests {
    use super::*;

    fn make_status_reply() -> OscMessage {
        OscMessage {
            address: "/status.reply".to_string(),
            args: vec![
                OscArg::Int(1),         // unused
                OscArg::Int(5),         // num_ugens
                OscArg::Int(2),         // num_synths
                OscArg::Int(3),         // num_groups
                OscArg::Int(10),        // num_synthdefs
                OscArg::Float(12.5),    // avg_cpu
                OscArg::Float(25.0),    // peak_cpu
                OscArg::Float(44100.0), // nominal_sr
                OscArg::Float(44099.8), // actual_sr
            ],
        }
    }

    #[test]
    fn parse_valid_status_reply() {
        let msg = make_status_reply();
        let status = ServerStatus::from_status_reply(&msg).unwrap();
        assert_eq!(status.num_ugens, 5);
        assert_eq!(status.num_synths, 2);
        assert_eq!(status.num_groups, 3);
        assert_eq!(status.num_synthdefs, 10);
        assert!((status.avg_cpu - 12.5).abs() < f32::EPSILON);
        assert!((status.peak_cpu - 25.0).abs() < f32::EPSILON);
        assert!((status.nominal_sample_rate - 44100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn wrong_address_returns_error() {
        let msg = OscMessage::new("/wrong", vec![]);
        assert!(ServerStatus::from_status_reply(&msg).is_err());
    }

    #[test]
    fn too_few_args_returns_error() {
        let msg = OscMessage {
            address: "/status.reply".to_string(),
            args: vec![OscArg::Int(1), OscArg::Int(2)],
        };
        assert!(ServerStatus::from_status_reply(&msg).is_err());
    }

    #[test]
    fn wrong_arg_type_returns_error() {
        let mut msg = make_status_reply();
        msg.args[1] = OscArg::Float(5.0); // should be Int
        assert!(ServerStatus::from_status_reply(&msg).is_err());
    }
}

#[cfg(test)]
mod ipc_tests {
    use crate::messages::*;

    #[test]
    fn action_message_roundtrip() {
        let env = MessageEnvelope {
            correlation_id: "corr-001".into(),
            message_id: String::from("msg-001"),
            parent_id: None,
            timestamp_ms: 1000,
            payload: RuntimeMessage::Action { payload_json: r#"{"op":"play"}"#.into() },
        };
        let json = serde_json::to_string(&env).unwrap();
        let decoded: MessageEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.correlation_id, "corr-001");
        assert!(matches!(decoded.payload, RuntimeMessage::Action { .. }));
    }

    #[test]
    fn transport_commands_roundtrip() {
        for cmd in [
            TransportCommand::Play,
            TransportCommand::Pause,
            TransportCommand::Stop,
            TransportCommand::Seek { beat: 42 },
        ] {
            let msg = RuntimeMessage::Transport { command: cmd };
            let json = serde_json::to_string(&msg).unwrap();
            let decoded: RuntimeMessage = serde_json::from_str(&json).unwrap();
            if let RuntimeMessage::Transport { command } = decoded {
                assert_eq!(command, cmd);
            } else {
                panic!("expected Transport");
            }
        }
    }

    #[test]
    fn patch_message_roundtrip() {
        let msg = RuntimeMessage::Patch {
            patch_id: "patch-7".into(),
            patch_json: r#"{"gain_db":-3.0}"#.into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: RuntimeMessage = serde_json::from_str(&json).unwrap();
        if let RuntimeMessage::Patch { patch_id, .. } = decoded {
            assert_eq!(patch_id, "patch-7");
        } else {
            panic!("expected Patch");
        }
    }

    #[test]
    fn terminate_roundtrip() {
        let msg = RuntimeMessage::Terminate { reason: "shutdown requested".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("shutdown requested"));
    }

    #[test]
    fn execution_ack_roundtrip() {
        let ack = RuntimeAck::ExecutionAck {
            correlation_id: "corr-002".into(),
            success: true,
            wall_ms: 15,
            detail: Some("audio dispatched".into()),
        };
        let json = serde_json::to_string(&ack).unwrap();
        let decoded: RuntimeAck = serde_json::from_str(&json).unwrap();
        if let RuntimeAck::ExecutionAck { success, wall_ms, .. } = decoded {
            assert!(success);
            assert_eq!(wall_ms, 15);
        } else {
            panic!("expected ExecutionAck");
        }
    }

    #[test]
    fn health_status_roundtrip() {
        let ack = RuntimeAck::HealthStatus {
            correlation_id: "hc-1".into(),
            process_id: "audio-runtime".into(),
            cpu_percent: 42.5,
            memory_mb: 128.0,
            status: ProcessHealth::Healthy,
        };
        let json = serde_json::to_string(&ack).unwrap();
        let decoded: RuntimeAck = serde_json::from_str(&json).unwrap();
        if let RuntimeAck::HealthStatus { status, .. } = decoded {
            assert_eq!(status, ProcessHealth::Healthy);
        } else {
            panic!("expected HealthStatus");
        }
    }

    #[test]
    fn patch_ack_roundtrip() {
        let ack = RuntimeAck::PatchAck {
            correlation_id: "patch-ack-1".into(),
            patch_id: "patch-7".into(),
            success: true,
            rollback_available: true,
        };
        let json = serde_json::to_string(&ack).unwrap();
        let decoded: RuntimeAck = serde_json::from_str(&json).unwrap();
        if let RuntimeAck::PatchAck { rollback_available, .. } = decoded {
            assert!(rollback_available);
        } else {
            panic!("expected PatchAck");
        }
    }

    #[test]
    fn error_ack_roundtrip() {
        let ack = RuntimeAck::Error {
            correlation_id: "err-1".into(),
            code: "TIMEOUT".into(),
            message: "backend did not respond".into(),
        };
        let json = serde_json::to_string(&ack).unwrap();
        assert!(json.contains("TIMEOUT"));
    }
}

//! Integration-level tests for the audio-bridge crate.

#[cfg(test)]
mod integration {
    use crate::osc::{OscArg, OscMessage, ScynthCmd};
    use crate::process_manager::{ProcessStatus, ScynthConfig, ScynthProcessManager};
    use crate::server_query::ServerStatus;

    #[test]
    fn osc_roundtrip_all_arg_types() {
        let msg = OscMessage::new(
            "/test/all",
            vec![
                OscArg::Int(42),
                OscArg::Float(1.234),
                OscArg::String("hello".to_string()),
                OscArg::Blob(vec![0xDE, 0xAD]),
            ],
        );
        let bytes = msg.to_bytes();
        let decoded = OscMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.address, "/test/all");
        assert_eq!(decoded.args.len(), 4);
        assert_eq!(decoded.args[0], OscArg::Int(42));
        assert_eq!(decoded.args[2], OscArg::String("hello".to_string()));
    }

    #[test]
    fn scynth_commands_are_well_formed() {
        // Verify all factory commands produce valid OSC
        let commands = vec![
            ScynthCmd::status(),
            ScynthCmd::quit(),
            ScynthCmd::notify(true),
            ScynthCmd::buffer_alloc_read(1, "/tmp/test.wav"),
            ScynthCmd::buffer_free(1),
            ScynthCmd::synth_new("default", 1001, 0, 1),
            ScynthCmd::node_set(1001, "freq", 440.0),
            ScynthCmd::node_free(1001),
            ScynthCmd::synthdef_load("/tmp/test.scsyndef"),
        ];
        for cmd in commands {
            let bytes = cmd.to_bytes();
            assert!(!bytes.is_empty(), "empty bytes for {}", cmd.address);
            let decoded = OscMessage::from_bytes(&bytes).unwrap();
            assert_eq!(decoded.address, cmd.address);
            assert_eq!(decoded.args.len(), cmd.args.len());
        }
    }

    #[test]
    fn process_manager_lifecycle() {
        let mut mgr = ScynthProcessManager::new(ScynthConfig::default());
        assert_eq!(mgr.status(), ProcessStatus::NotStarted);
        assert!(!mgr.check_alive());
        let _ = mgr.shutdown();
        assert_eq!(mgr.status(), ProcessStatus::Stopped);
    }

    #[test]
    fn server_status_serde_roundtrip() {
        let status = ServerStatus {
            num_ugens: 10,
            num_synths: 3,
            num_groups: 2,
            num_synthdefs: 15,
            avg_cpu: 8.5,
            peak_cpu: 22.0,
            nominal_sample_rate: 48000.0,
            actual_sample_rate: 47999.5,
        };
        let json = serde_json::to_string(&status).unwrap();
        let decoded: ServerStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.num_ugens, 10);
        assert!((decoded.avg_cpu - 8.5).abs() < f32::EPSILON);
    }
}

use std::collections::BTreeMap;
use std::path::Path;

use vidodo_ir::{AnalysisAdapter, AnalysisResult};

/// Example third-party audio analyzer adapter.
///
/// This stub implementation returns deterministic analysis results for any
/// asset. It serves as a reference for how external analyzer plugins will
/// integrate with the adapter registry.
pub struct ExampleAudioAnalyzer {
    analyzer_id: String,
}

impl ExampleAudioAnalyzer {
    pub fn new(analyzer_id: &str) -> Self {
        Self { analyzer_id: analyzer_id.to_string() }
    }
}

impl AnalysisAdapter for ExampleAudioAnalyzer {
    fn analyzer_id(&self) -> &str {
        &self.analyzer_id
    }

    fn ready(&self) -> bool {
        true
    }

    fn analyze(&self, asset_id: &str, _path: &Path) -> Result<AnalysisResult, String> {
        // Stub: returns deterministic metrics regardless of actual file content.
        let mut metrics = BTreeMap::new();
        metrics.insert(String::from("estimated_bpm"), serde_json::json!(128.0));
        metrics.insert(String::from("rms_db"), serde_json::json!(-12.5));
        metrics.insert(String::from("peak_db"), serde_json::json!(-3.0));
        metrics.insert(String::from("spectral_centroid_hz"), serde_json::json!(440.0));

        Ok(AnalysisResult {
            analyzer_id: self.analyzer_id.clone(),
            asset_id: asset_id.to_string(),
            status: String::from("complete"),
            metrics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn analyzer_reports_ready() {
        let analyzer = ExampleAudioAnalyzer::new("example-audio-analyzer");
        assert!(analyzer.ready());
        assert_eq!(analyzer.analyzer_id(), "example-audio-analyzer");
    }

    #[test]
    fn analyze_returns_structured_result() {
        let analyzer = ExampleAudioAnalyzer::new("example-audio-analyzer");
        let result =
            analyzer.analyze("audio.loop.pad-a", &PathBuf::from("/tmp/pad-a.wav")).unwrap();
        assert_eq!(result.status, "complete");
        assert_eq!(result.asset_id, "audio.loop.pad-a");
        assert!(result.metrics.contains_key("estimated_bpm"));
        assert!(result.metrics.contains_key("rms_db"));
    }

    #[test]
    fn analyzer_result_is_serializable() {
        let analyzer = ExampleAudioAnalyzer::new("example-audio-analyzer");
        let result =
            analyzer.analyze("audio.loop.pad-b", &PathBuf::from("/tmp/pad-b.wav")).unwrap();
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("example-audio-analyzer"));
        let back: AnalysisResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.analyzer_id, "example-audio-analyzer");
    }

    #[test]
    fn registry_can_load_and_query_analyzer() {
        use crate::AdapterRegistry;
        use vidodo_ir::{AdapterPluginManifest, HealthContract};

        let manifest = AdapterPluginManifest {
            plugin_id: String::from("example-audio-analyzer"),
            plugin_kind: String::from("audio_analyzer"),
            backend_kind: String::from("analysis"),
            version: String::from("0.1.0"),
            capabilities: vec![String::from("beat_track"), String::from("spectral_analysis")],
            target_topology_types: Vec::new(),
            health_contract: Some(HealthContract {
                reports_ack: false,
                reports_status: true,
                supports_degrade_mode: false,
            }),
            status: Some(String::from("ready")),
        };

        let mut registry = AdapterRegistry::new();
        registry.register(manifest).unwrap();

        let adapter = registry.lookup("example-audio-analyzer").unwrap();
        assert_eq!(adapter.plugin_kind, "audio_analyzer");
        assert!(adapter.capabilities.contains(&String::from("beat_track")));
    }
}

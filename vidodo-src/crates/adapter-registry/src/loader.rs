use vidodo_ir::{AdapterPluginManifest, BackendAdapter, BackendDescription, Diagnostic};
use vidodo_scheduler::audio_backend::AudioReferenceBackend;
use vidodo_scheduler::lighting_backend::LightingReferenceBackend;
use vidodo_scheduler::null_backend::NullBackendAdapter;
use vidodo_scheduler::visual_backend::VisualReferenceBackend;

/// Result of loading a single adapter plugin.
pub struct LoadedAdapter {
    pub manifest: AdapterPluginManifest,
    pub adapter: Box<dyn BackendAdapter>,
}

impl std::fmt::Debug for LoadedAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedAdapter")
            .field("plugin_id", &self.manifest.plugin_id)
            .field("backend_kind", &self.manifest.backend_kind)
            .finish()
    }
}

/// Load adapter plugins from a list of manifests.
///
/// Each manifest is matched by `plugin_kind` to a concrete adapter
/// implementation. Currently only `"null"` is supported, which maps to
/// [`NullBackendAdapter`].
///
/// Returns a `Vec<LoadedAdapter>` of successfully instantiated adapters.
/// Returns `Err` with a structured diagnostic if any manifest references an
/// unknown `plugin_kind`.
pub fn load_adapters(
    manifests: &[AdapterPluginManifest],
) -> Result<Vec<LoadedAdapter>, Box<Diagnostic>> {
    let mut loaded = Vec::with_capacity(manifests.len());
    for manifest in manifests {
        let adapter = instantiate(manifest)?;
        loaded.push(LoadedAdapter { manifest: manifest.clone(), adapter });
    }
    Ok(loaded)
}

/// Instantiate a single adapter from its manifest.
fn instantiate(
    manifest: &AdapterPluginManifest,
) -> Result<Box<dyn BackendAdapter>, Box<Diagnostic>> {
    match manifest.plugin_kind.as_str() {
        "null" | "null_backend" => {
            Ok(Box::new(NullBackendAdapter::new(&manifest.plugin_id, &manifest.backend_kind)))
        }
        "audio_output" => Ok(Box::new(AudioReferenceBackend::new(&manifest.plugin_id))),
        "visual_output" => Ok(Box::new(VisualReferenceBackend::new(&manifest.plugin_id))),
        "lighting_output" => Ok(Box::new(LightingReferenceBackend::new(&manifest.plugin_id))),
        unknown => Err(Box::new(Diagnostic::error(
            "LDR-001",
            format!("unknown plugin_kind '{}' for plugin '{}'", unknown, manifest.plugin_id),
        ))),
    }
}

/// Perform readiness checks on a set of loaded adapters.
///
/// Calls `describe_backend()` on each and returns only those whose status
/// is not `"error"`. The rejected adapters' descriptions are collected
/// in the `rejected` output.
pub fn readiness_check(
    adapters: &[LoadedAdapter],
) -> (Vec<&LoadedAdapter>, Vec<BackendDescription>) {
    let mut ready = Vec::new();
    let mut rejected = Vec::new();
    for loaded in adapters {
        let desc = loaded.adapter.describe_backend();
        if desc.status == "error" {
            rejected.push(desc);
        } else {
            ready.push(loaded);
        }
    }
    (ready, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::{AdapterPluginManifest, HealthContract};

    fn manifest(id: &str, kind: &str, backend: &str) -> AdapterPluginManifest {
        AdapterPluginManifest {
            plugin_id: id.to_string(),
            plugin_kind: kind.to_string(),
            backend_kind: backend.to_string(),
            version: String::from("1.0.0"),
            capabilities: vec![String::from("play")],
            target_topology_types: Vec::new(),
            health_contract: Some(HealthContract {
                reports_ack: true,
                reports_status: true,
                supports_degrade_mode: false,
            }),
            status: Some(String::from("ready")),
        }
    }

    #[test]
    fn load_single_null_adapter() {
        let manifests = vec![manifest("audio-null", "null", "fake_audio_backend")];
        let loaded = load_adapters(&manifests).unwrap();
        assert_eq!(loaded.len(), 1);
        let desc = loaded[0].adapter.describe_backend();
        assert_eq!(desc.plugin_id, "audio-null");
        assert_eq!(desc.backend_kind, "fake_audio_backend");
    }

    #[test]
    fn load_multiple_adapters() {
        let manifests = vec![
            manifest("audio-1", "audio_output", "fake_audio"),
            manifest("visual-1", "visual_output", "fake_visual"),
            manifest("lighting-1", "lighting_output", "fake_lighting"),
        ];
        let loaded = load_adapters(&manifests).unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].adapter.describe_backend().plugin_id, "audio-1");
        assert_eq!(loaded[1].adapter.describe_backend().plugin_id, "visual-1");
        assert_eq!(loaded[2].adapter.describe_backend().plugin_id, "lighting-1");
    }

    #[test]
    fn unknown_plugin_kind_returns_error() {
        let manifests = vec![manifest("bad", "quantum_backend", "fake")];
        let result = load_adapters(&manifests);
        assert!(result.is_err());
        let diag = result.unwrap_err();
        assert_eq!(diag.code, "LDR-001");
        assert!(diag.message.contains("quantum_backend"));
    }

    #[test]
    fn readiness_check_filters_errored_adapters() {
        let manifests = vec![manifest("a1", "null", "audio"), manifest("a2", "null", "visual")];
        let loaded = load_adapters(&manifests).unwrap();
        let (ready, rejected) = readiness_check(&loaded);
        // NullBackendAdapter always starts as "idle", never "error"
        assert_eq!(ready.len(), 2);
        assert!(rejected.is_empty());
    }

    #[test]
    fn reference_backends_report_correct_backend_kind() {
        let manifests = vec![
            manifest("a-ref", "audio_output", "audio"),
            manifest("v-ref", "visual_output", "visual"),
            manifest("l-ref", "lighting_output", "lighting"),
        ];
        let loaded = load_adapters(&manifests).unwrap();
        assert_eq!(loaded[0].adapter.describe_backend().backend_kind, "audio");
        assert_eq!(loaded[1].adapter.describe_backend().backend_kind, "visual");
        assert_eq!(loaded[2].adapter.describe_backend().backend_kind, "lighting");
    }
}

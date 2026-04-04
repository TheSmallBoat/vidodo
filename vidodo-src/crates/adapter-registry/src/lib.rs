use std::collections::BTreeMap;

use vidodo_ir::{AdapterPluginManifest, BackendHealthSnapshot, HealthContract};

pub mod example_audio_analyzer;
pub mod example_visual_executor;
pub mod loader;
pub mod persistence;

/// In-memory registry of adapter plugins.
///
/// Supports registration, lookup by plugin_id, filtering by backend_kind,
/// and health-status aggregation.
#[derive(Debug, Default)]
pub struct AdapterRegistry {
    plugins: BTreeMap<String, AdapterPluginManifest>,
}

/// Aggregated health summary across registered adapters.
#[derive(Debug, Clone, PartialEq)]
pub struct HealthSummary {
    pub total: usize,
    pub healthy: usize,
    pub degraded: usize,
    pub offline: usize,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an adapter plugin. Returns `Err` if a plugin with the same
    /// `plugin_id` is already registered.
    pub fn register(&mut self, manifest: AdapterPluginManifest) -> Result<(), String> {
        if self.plugins.contains_key(&manifest.plugin_id) {
            return Err(format!(
                "ADP-001: adapter plugin '{}' is already registered",
                manifest.plugin_id
            ));
        }
        self.plugins.insert(manifest.plugin_id.clone(), manifest);
        Ok(())
    }

    /// Look up a single adapter by plugin_id.
    pub fn lookup(&self, plugin_id: &str) -> Result<&AdapterPluginManifest, String> {
        self.plugins
            .get(plugin_id)
            .ok_or_else(|| format!("ADP-002: unknown plugin_id '{plugin_id}'"))
    }

    /// List all registered adapters.
    pub fn list(&self) -> Vec<&AdapterPluginManifest> {
        self.plugins.values().collect()
    }

    /// List adapters filtered by `backend_kind`.
    pub fn list_by_backend(&self, backend_kind: &str) -> Vec<&AdapterPluginManifest> {
        self.plugins.values().filter(|manifest| manifest.backend_kind == backend_kind).collect()
    }

    /// Compute an aggregate health summary from a set of health snapshots,
    /// scoped to adapters that are currently registered.
    pub fn health_summary(&self, snapshots: &[BackendHealthSnapshot]) -> HealthSummary {
        let mut summary =
            HealthSummary { total: self.plugins.len(), healthy: 0, degraded: 0, offline: 0 };

        // Track which plugins have a snapshot.
        let mut seen = std::collections::BTreeSet::new();

        for snapshot in snapshots {
            if !self.plugins.contains_key(&snapshot.plugin_ref) {
                continue;
            }
            seen.insert(snapshot.plugin_ref.clone());
            match snapshot.status.as_str() {
                "healthy" | "ok" => summary.healthy += 1,
                "degraded" => summary.degraded += 1,
                "offline" | "error" => summary.offline += 1,
                _ => summary.offline += 1,
            }
        }

        // Plugins without any snapshot are counted as offline.
        let unreported = self.plugins.len() - seen.len();
        summary.offline += unreported;

        summary
    }

    /// Return the health contract for a plugin, if declared.
    pub fn health_contract(&self, plugin_id: &str) -> Option<&HealthContract> {
        self.plugins.get(plugin_id).and_then(|m| m.health_contract.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest(id: &str, backend: &str) -> AdapterPluginManifest {
        AdapterPluginManifest {
            plugin_id: id.to_string(),
            plugin_kind: String::from("audio_output"),
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
    fn register_and_lookup() {
        let mut registry = AdapterRegistry::new();
        registry.register(sample_manifest("audio-1", "fake_audio_backend")).unwrap();
        let adapter = registry.lookup("audio-1").unwrap();
        assert_eq!(adapter.backend_kind, "fake_audio_backend");
    }

    #[test]
    fn duplicate_registration_returns_error() {
        let mut registry = AdapterRegistry::new();
        registry.register(sample_manifest("dup", "fake")).unwrap();
        let result = registry.register(sample_manifest("dup", "fake"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ADP-001"));
    }

    #[test]
    fn unknown_plugin_returns_error() {
        let registry = AdapterRegistry::new();
        let result = registry.lookup("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ADP-002"));
    }

    #[test]
    fn list_and_filter_by_backend() {
        let mut registry = AdapterRegistry::new();
        registry.register(sample_manifest("a1", "fake_audio")).unwrap();
        registry.register(sample_manifest("v1", "fake_visual")).unwrap();
        registry.register(sample_manifest("a2", "fake_audio")).unwrap();

        assert_eq!(registry.list().len(), 3);
        assert_eq!(registry.list_by_backend("fake_audio").len(), 2);
        assert_eq!(registry.list_by_backend("fake_visual").len(), 1);
        assert!(registry.list_by_backend("unknown").is_empty());
    }

    #[test]
    fn health_summary_aggregates_snapshots() {
        let mut registry = AdapterRegistry::new();
        registry.register(sample_manifest("p1", "fake")).unwrap();
        registry.register(sample_manifest("p2", "fake")).unwrap();
        registry.register(sample_manifest("p3", "fake")).unwrap();

        let snapshots = vec![
            BackendHealthSnapshot {
                backend_ref: String::from("b1"),
                plugin_ref: String::from("p1"),
                status: String::from("healthy"),
                timestamp: String::from("2026-01-01T00:00:00Z"),
                latency_ms: None,
                error_count: None,
                last_ack_lag_ms: None,
                degrade_reason: None,
            },
            BackendHealthSnapshot {
                backend_ref: String::from("b2"),
                plugin_ref: String::from("p2"),
                status: String::from("degraded"),
                timestamp: String::from("2026-01-01T00:00:00Z"),
                latency_ms: Some(150.0),
                error_count: Some(3),
                last_ack_lag_ms: None,
                degrade_reason: Some(String::from("high latency")),
            },
            // p3 has no snapshot → counted as offline
        ];

        let summary = registry.health_summary(&snapshots);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.healthy, 1);
        assert_eq!(summary.degraded, 1);
        assert_eq!(summary.offline, 1); // p3 unreported
    }
}

use std::fs;
use std::path::Path;

use rusqlite::{Connection, params};
use vidodo_ir::{AdapterPluginManifest, BackendHealthSnapshot};

use crate::HealthSummary;

/// Persistent adapter registry backed by SQLite.
///
/// Stores [`AdapterPluginManifest`] records so adapter registrations
/// survive process restarts.
pub struct PersistentAdapterRegistry {
    conn: Connection,
}

impl PersistentAdapterRegistry {
    /// Open (or create) the SQLite database at `path` and initialize schema.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("ADP-P01: failed to create directory: {e}"))?;
        }
        let conn =
            Connection::open(path).map_err(|e| format!("ADP-P02: failed to open database: {e}"))?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS adapter_plugins (
                plugin_id TEXT PRIMARY KEY,
                plugin_kind TEXT NOT NULL,
                backend_kind TEXT NOT NULL,
                version TEXT NOT NULL,
                status TEXT,
                manifest_json TEXT NOT NULL
            );
            ",
        )
        .map_err(|e| format!("ADP-P03: failed to initialize schema: {e}"))?;
        Ok(Self { conn })
    }

    /// Register an adapter plugin, persisting it to SQLite.
    /// Returns `Err` if `plugin_id` already exists.
    pub fn register(&self, manifest: &AdapterPluginManifest) -> Result<(), String> {
        let existing: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM adapter_plugins WHERE plugin_id = ?1)",
                params![manifest.plugin_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("ADP-P04: query failed: {e}"))?;

        if existing {
            return Err(format!(
                "ADP-001: adapter plugin '{}' is already registered",
                manifest.plugin_id
            ));
        }

        let json = serde_json::to_string(manifest)
            .map_err(|e| format!("ADP-P05: serialization failed: {e}"))?;

        self.conn
            .execute(
                "INSERT INTO adapter_plugins (plugin_id, plugin_kind, backend_kind, version, status, manifest_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    manifest.plugin_id,
                    manifest.plugin_kind,
                    manifest.backend_kind,
                    manifest.version,
                    manifest.status,
                    json,
                ],
            )
            .map_err(|e| format!("ADP-P06: insert failed: {e}"))?;

        Ok(())
    }

    /// Look up a single adapter by plugin_id.
    pub fn lookup(&self, plugin_id: &str) -> Result<AdapterPluginManifest, String> {
        let json: String = self
            .conn
            .query_row(
                "SELECT manifest_json FROM adapter_plugins WHERE plugin_id = ?1",
                params![plugin_id],
                |row| row.get(0),
            )
            .map_err(|_| format!("ADP-002: unknown plugin_id '{plugin_id}'"))?;
        serde_json::from_str(&json).map_err(|e| format!("ADP-P07: deserialization failed: {e}"))
    }

    /// List all registered adapters.
    pub fn list(&self) -> Result<Vec<AdapterPluginManifest>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT manifest_json FROM adapter_plugins ORDER BY plugin_id")
            .map_err(|e| format!("ADP-P08: prepare failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("ADP-P09: query failed: {e}"))?;
        let mut adapters = Vec::new();
        for row in rows {
            let json = row.map_err(|e| format!("ADP-P10: row read failed: {e}"))?;
            let manifest: AdapterPluginManifest = serde_json::from_str(&json)
                .map_err(|e| format!("ADP-P07: deserialization failed: {e}"))?;
            adapters.push(manifest);
        }
        Ok(adapters)
    }

    /// List adapters filtered by `backend_kind`.
    pub fn list_by_backend(
        &self,
        backend_kind: &str,
    ) -> Result<Vec<AdapterPluginManifest>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT manifest_json FROM adapter_plugins WHERE backend_kind = ?1 ORDER BY plugin_id",
            )
            .map_err(|e| format!("ADP-P11: prepare failed: {e}"))?;
        let rows = stmt
            .query_map(params![backend_kind], |row| row.get::<_, String>(0))
            .map_err(|e| format!("ADP-P12: query failed: {e}"))?;
        let mut adapters = Vec::new();
        for row in rows {
            let json = row.map_err(|e| format!("ADP-P10: row read failed: {e}"))?;
            let manifest: AdapterPluginManifest = serde_json::from_str(&json)
                .map_err(|e| format!("ADP-P07: deserialization failed: {e}"))?;
            adapters.push(manifest);
        }
        Ok(adapters)
    }

    /// Compute an aggregate health summary from a set of health snapshots,
    /// scoped to adapters that are currently registered.
    pub fn health_summary(
        &self,
        snapshots: &[BackendHealthSnapshot],
    ) -> Result<HealthSummary, String> {
        let all = self.list()?;
        let total = all.len();
        let plugin_ids: std::collections::BTreeSet<String> =
            all.iter().map(|m| m.plugin_id.clone()).collect();

        let mut healthy = 0_usize;
        let mut degraded = 0_usize;
        let mut offline = 0_usize;
        let mut seen = std::collections::BTreeSet::new();

        for snapshot in snapshots {
            if !plugin_ids.contains(&snapshot.plugin_ref) {
                continue;
            }
            seen.insert(snapshot.plugin_ref.clone());
            match snapshot.status.as_str() {
                "healthy" | "ok" => healthy += 1,
                "degraded" => degraded += 1,
                _ => offline += 1,
            }
        }
        offline += total - seen.len();

        Ok(HealthSummary { total, healthy, degraded, offline })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::HealthContract;

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

    fn temp_db() -> (tempfile::TempDir, PersistentAdapterRegistry) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("adapters.db");
        let registry = PersistentAdapterRegistry::open(&db_path).unwrap();
        (dir, registry)
    }

    #[test]
    fn register_and_lookup() {
        let (_dir, registry) = temp_db();
        registry.register(&sample_manifest("audio-1", "fake_audio")).unwrap();
        let manifest = registry.lookup("audio-1").unwrap();
        assert_eq!(manifest.backend_kind, "fake_audio");
        assert_eq!(manifest.plugin_id, "audio-1");
    }

    #[test]
    fn duplicate_registration_returns_error() {
        let (_dir, registry) = temp_db();
        registry.register(&sample_manifest("dup", "fake")).unwrap();
        let result = registry.register(&sample_manifest("dup", "fake"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ADP-001"));
    }

    #[test]
    fn unknown_plugin_returns_error() {
        let (_dir, registry) = temp_db();
        let result = registry.lookup("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ADP-002"));
    }

    #[test]
    fn list_and_filter_by_backend() {
        let (_dir, registry) = temp_db();
        registry.register(&sample_manifest("a1", "fake_audio")).unwrap();
        registry.register(&sample_manifest("v1", "fake_visual")).unwrap();
        registry.register(&sample_manifest("a2", "fake_audio")).unwrap();

        assert_eq!(registry.list().unwrap().len(), 3);
        assert_eq!(registry.list_by_backend("fake_audio").unwrap().len(), 2);
        assert_eq!(registry.list_by_backend("fake_visual").unwrap().len(), 1);
        assert!(registry.list_by_backend("unknown").unwrap().is_empty());
    }

    #[test]
    fn data_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("adapters.db");

        {
            let registry = PersistentAdapterRegistry::open(&db_path).unwrap();
            registry.register(&sample_manifest("persist-1", "fake")).unwrap();
            registry.register(&sample_manifest("persist-2", "fake")).unwrap();
        }

        // Reopen the database
        let registry = PersistentAdapterRegistry::open(&db_path).unwrap();
        let all = registry.list().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].plugin_id, "persist-1");
        assert_eq!(all[1].plugin_id, "persist-2");
    }

    #[test]
    fn health_summary_aggregates() {
        let (_dir, registry) = temp_db();
        registry.register(&sample_manifest("p1", "fake")).unwrap();
        registry.register(&sample_manifest("p2", "fake")).unwrap();
        registry.register(&sample_manifest("p3", "fake")).unwrap();

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
                latency_ms: None,
                error_count: None,
                last_ack_lag_ms: None,
                degrade_reason: None,
            },
        ];
        let summary = registry.health_summary(&snapshots).unwrap();
        assert_eq!(summary.total, 3);
        assert_eq!(summary.healthy, 1);
        assert_eq!(summary.degraded, 1);
        assert_eq!(summary.offline, 1); // p3 has no snapshot
    }
}

use std::fs;
use std::path::Path;

use rusqlite::{Connection, params};
use vidodo_ir::ResourceHubDescriptor;

use crate::ResolvedResource;

/// Persistent resource-hub registry backed by SQLite.
///
/// Stores `ResourceHubDescriptor` records and supports the same
/// operations as the in-memory `ResourceHubRegistry`, but survives
/// process restarts.
pub struct PersistentHubRegistry {
    conn: Connection,
}

impl PersistentHubRegistry {
    /// Open (or create) the SQLite database at `path` and initialize schema.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("HUB-P01: failed to create directory: {e}"))?;
        }
        let conn =
            Connection::open(path).map_err(|e| format!("HUB-P02: failed to open database: {e}"))?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS resource_hubs (
                hub_id TEXT PRIMARY KEY,
                resource_kind TEXT NOT NULL,
                version TEXT NOT NULL,
                locator TEXT NOT NULL,
                status TEXT,
                descriptor_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS hub_provides (
                hub_id TEXT NOT NULL,
                resource_name TEXT NOT NULL,
                PRIMARY KEY (hub_id, resource_name)
            );

            CREATE TABLE IF NOT EXISTS hub_tags (
                hub_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (hub_id, tag)
            );
            ",
        )
        .map_err(|e| format!("HUB-P03: failed to initialize schema: {e}"))?;
        Ok(Self { conn })
    }

    /// Register a resource hub, persisting it to SQLite.
    /// Returns `Err` if `hub_id` already exists.
    pub fn register_hub(&self, descriptor: &ResourceHubDescriptor) -> Result<(), String> {
        let existing: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM resource_hubs WHERE hub_id = ?1)",
                params![descriptor.hub_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("HUB-P04: query failed: {e}"))?;

        if existing {
            return Err(format!(
                "HUB-001: resource hub '{}' is already registered",
                descriptor.hub_id
            ));
        }

        let json = serde_json::to_string(descriptor)
            .map_err(|e| format!("HUB-P05: serialization failed: {e}"))?;

        self.conn
            .execute(
                "INSERT INTO resource_hubs (hub_id, resource_kind, version, locator, status, descriptor_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    descriptor.hub_id,
                    descriptor.resource_kind,
                    descriptor.version,
                    descriptor.locator,
                    descriptor.status,
                    json,
                ],
            )
            .map_err(|e| format!("HUB-P06: insert failed: {e}"))?;

        for resource_name in &descriptor.provides {
            self.conn
                .execute(
                    "INSERT INTO hub_provides (hub_id, resource_name) VALUES (?1, ?2)",
                    params![descriptor.hub_id, resource_name],
                )
                .map_err(|e| format!("HUB-P07: insert provides failed: {e}"))?;
        }

        for tag in &descriptor.tags {
            self.conn
                .execute(
                    "INSERT INTO hub_tags (hub_id, tag) VALUES (?1, ?2)",
                    params![descriptor.hub_id, tag],
                )
                .map_err(|e| format!("HUB-P08: insert tag failed: {e}"))?;
        }

        Ok(())
    }

    /// Look up a hub by hub_id.
    pub fn lookup(&self, hub_id: &str) -> Result<ResourceHubDescriptor, String> {
        let json: String = self
            .conn
            .query_row(
                "SELECT descriptor_json FROM resource_hubs WHERE hub_id = ?1",
                params![hub_id],
                |row| row.get(0),
            )
            .map_err(|_| format!("HUB-002: unknown hub_id '{hub_id}'"))?;
        serde_json::from_str(&json).map_err(|e| format!("HUB-P09: deserialization failed: {e}"))
    }

    /// List all registered hubs.
    pub fn list_hubs(&self) -> Result<Vec<ResourceHubDescriptor>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT descriptor_json FROM resource_hubs ORDER BY hub_id")
            .map_err(|e| format!("HUB-P10: prepare failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("HUB-P11: query failed: {e}"))?;
        let mut hubs = Vec::new();
        for row in rows {
            let json = row.map_err(|e| format!("HUB-P12: row read failed: {e}"))?;
            let hub: ResourceHubDescriptor = serde_json::from_str(&json)
                .map_err(|e| format!("HUB-P09: deserialization failed: {e}"))?;
            hubs.push(hub);
        }
        Ok(hubs)
    }

    /// List hubs filtered by `resource_kind`.
    pub fn list_by_kind(&self, resource_kind: &str) -> Result<Vec<ResourceHubDescriptor>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT descriptor_json FROM resource_hubs WHERE resource_kind = ?1 ORDER BY hub_id",
            )
            .map_err(|e| format!("HUB-P13: prepare failed: {e}"))?;
        let rows = stmt
            .query_map(params![resource_kind], |row| row.get::<_, String>(0))
            .map_err(|e| format!("HUB-P14: query failed: {e}"))?;
        let mut hubs = Vec::new();
        for row in rows {
            let json = row.map_err(|e| format!("HUB-P12: row read failed: {e}"))?;
            let hub: ResourceHubDescriptor = serde_json::from_str(&json)
                .map_err(|e| format!("HUB-P09: deserialization failed: {e}"))?;
            hubs.push(hub);
        }
        Ok(hubs)
    }

    /// Resolve a resource reference by finding the first hub whose `provides`
    /// list contains the requested resource name.
    pub fn resolve_resource(&self, resource_name: &str) -> Result<ResolvedResource, String> {
        let result: Result<(String, String, String), _> = self.conn.query_row(
            "SELECT h.hub_id, h.locator, h.resource_kind
             FROM hub_provides p
             JOIN resource_hubs h ON p.hub_id = h.hub_id
             WHERE p.resource_name = ?1
             LIMIT 1",
            params![resource_name],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );
        match result {
            Ok((hub_id, locator, resource_kind)) => {
                Ok(ResolvedResource { hub_id, locator, resource_kind })
            }
            Err(_) => Err(format!("HUB-003: no hub provides resource '{resource_name}'")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::HubCompatibility;

    fn sample_hub(id: &str, kind: &str, provides: &[&str]) -> ResourceHubDescriptor {
        ResourceHubDescriptor {
            hub_id: id.to_string(),
            resource_kind: kind.to_string(),
            version: String::from("1.0.0"),
            locator: format!("file:///hubs/{id}"),
            provides: provides.iter().map(|s| s.to_string()).collect(),
            compatibility: Some(HubCompatibility {
                runtime: vec![String::from("vidodo-1.0")],
                backends: Vec::new(),
                schema_versions: Vec::new(),
            }),
            status: Some(String::from("available")),
            tags: vec![String::from("production")],
        }
    }

    fn open_temp_db() -> PersistentHubRegistry {
        PersistentHubRegistry::open(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn register_and_lookup_persisted() {
        let db = open_temp_db();
        let hub = sample_hub("audio-std", "audio_asset_hub", &["kick.wav", "snare.wav"]);
        db.register_hub(&hub).unwrap();
        let loaded = db.lookup("audio-std").unwrap();
        assert_eq!(loaded.hub_id, "audio-std");
        assert_eq!(loaded.resource_kind, "audio_asset_hub");
        assert_eq!(loaded.provides, vec!["kick.wav", "snare.wav"]);
    }

    #[test]
    fn duplicate_hub_returns_error() {
        let db = open_temp_db();
        db.register_hub(&sample_hub("dup", "audio_asset_hub", &["x"])).unwrap();
        let result = db.register_hub(&sample_hub("dup", "audio_asset_hub", &["y"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("HUB-001"));
    }

    #[test]
    fn unknown_hub_returns_error() {
        let db = open_temp_db();
        let result = db.lookup("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("HUB-002"));
    }

    #[test]
    fn list_and_filter_by_kind() {
        let db = open_temp_db();
        db.register_hub(&sample_hub("a1", "audio_asset_hub", &["kick"])).unwrap();
        db.register_hub(&sample_hub("g1", "glsl_scene_hub", &["fade"])).unwrap();
        db.register_hub(&sample_hub("a2", "audio_asset_hub", &["snare"])).unwrap();

        assert_eq!(db.list_hubs().unwrap().len(), 3);
        assert_eq!(db.list_by_kind("audio_asset_hub").unwrap().len(), 2);
        assert_eq!(db.list_by_kind("glsl_scene_hub").unwrap().len(), 1);
        assert!(db.list_by_kind("texture_hub").unwrap().is_empty());
    }

    #[test]
    fn resolve_resource_finds_provider() {
        let db = open_temp_db();
        db.register_hub(&sample_hub("audio-std", "audio_asset_hub", &["kick.wav", "snare.wav"]))
            .unwrap();
        db.register_hub(&sample_hub("glsl-stage", "glsl_scene_hub", &["fade-shader"])).unwrap();

        let resolved = db.resolve_resource("kick.wav").unwrap();
        assert_eq!(resolved.hub_id, "audio-std");
        assert_eq!(resolved.resource_kind, "audio_asset_hub");

        let resolved = db.resolve_resource("fade-shader").unwrap();
        assert_eq!(resolved.hub_id, "glsl-stage");
    }

    #[test]
    fn resolve_unknown_resource_returns_error() {
        let db = open_temp_db();
        let result = db.resolve_resource("missing");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("HUB-003"));
    }
}

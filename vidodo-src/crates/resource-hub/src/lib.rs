use std::collections::BTreeMap;

use vidodo_ir::ResourceHubDescriptor;

pub mod persistence;

/// In-memory registry of external resource hubs.
///
/// Supports registration, lookup by hub_id, filtering by resource_kind,
/// and resource reference resolution.
#[derive(Debug, Default)]
pub struct ResourceHubRegistry {
    hubs: BTreeMap<String, ResourceHubDescriptor>,
}

/// Result of resolving a resource reference against registered hubs.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedResource {
    pub hub_id: String,
    pub locator: String,
    pub resource_kind: String,
}

impl ResourceHubRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a resource hub. Returns `Err` if `hub_id` already exists.
    pub fn register_hub(&mut self, descriptor: ResourceHubDescriptor) -> Result<(), String> {
        if self.hubs.contains_key(&descriptor.hub_id) {
            return Err(format!(
                "HUB-001: resource hub '{}' is already registered",
                descriptor.hub_id
            ));
        }
        self.hubs.insert(descriptor.hub_id.clone(), descriptor);
        Ok(())
    }

    /// Look up a hub by hub_id.
    pub fn lookup(&self, hub_id: &str) -> Result<&ResourceHubDescriptor, String> {
        self.hubs.get(hub_id).ok_or_else(|| format!("HUB-002: unknown hub_id '{hub_id}'"))
    }

    /// List all registered hubs.
    pub fn list_hubs(&self) -> Vec<&ResourceHubDescriptor> {
        self.hubs.values().collect()
    }

    /// List hubs filtered by `resource_kind`.
    pub fn list_by_kind(&self, resource_kind: &str) -> Vec<&ResourceHubDescriptor> {
        self.hubs.values().filter(|hub| hub.resource_kind == resource_kind).collect()
    }

    /// Resolve a resource reference by finding the first hub whose `provides`
    /// list contains the requested resource name.
    ///
    /// Returns the hub's locator and kind, or an error if no hub provides
    /// the requested resource.
    pub fn resolve_resource(&self, resource_name: &str) -> Result<ResolvedResource, String> {
        for hub in self.hubs.values() {
            if hub.provides.iter().any(|p| p == resource_name) {
                return Ok(ResolvedResource {
                    hub_id: hub.hub_id.clone(),
                    locator: hub.locator.clone(),
                    resource_kind: hub.resource_kind.clone(),
                });
            }
        }
        Err(format!("HUB-003: no hub provides resource '{resource_name}'"))
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
            tags: Vec::new(),
        }
    }

    #[test]
    fn register_and_lookup_hub() {
        let mut registry = ResourceHubRegistry::new();
        registry
            .register_hub(sample_hub("audio-std", "audio_asset_hub", &["kick.wav", "snare.wav"]))
            .unwrap();
        let hub = registry.lookup("audio-std").unwrap();
        assert_eq!(hub.resource_kind, "audio_asset_hub");
    }

    #[test]
    fn duplicate_hub_returns_error() {
        let mut registry = ResourceHubRegistry::new();
        registry.register_hub(sample_hub("dup", "audio_asset_hub", &["x"])).unwrap();
        let result = registry.register_hub(sample_hub("dup", "audio_asset_hub", &["y"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("HUB-001"));
    }

    #[test]
    fn unknown_hub_returns_error() {
        let registry = ResourceHubRegistry::new();
        let result = registry.lookup("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("HUB-002"));
    }

    #[test]
    fn list_and_filter_by_kind() {
        let mut registry = ResourceHubRegistry::new();
        registry.register_hub(sample_hub("a1", "audio_asset_hub", &["kick"])).unwrap();
        registry.register_hub(sample_hub("g1", "glsl_scene_hub", &["fade"])).unwrap();
        registry.register_hub(sample_hub("a2", "audio_asset_hub", &["snare"])).unwrap();

        assert_eq!(registry.list_hubs().len(), 3);
        assert_eq!(registry.list_by_kind("audio_asset_hub").len(), 2);
        assert_eq!(registry.list_by_kind("glsl_scene_hub").len(), 1);
        assert!(registry.list_by_kind("texture_hub").is_empty());
    }

    #[test]
    fn resolve_resource_finds_provider() {
        let mut registry = ResourceHubRegistry::new();
        registry
            .register_hub(sample_hub("audio-std", "audio_asset_hub", &["kick.wav", "snare.wav"]))
            .unwrap();
        registry
            .register_hub(sample_hub("glsl-stage", "glsl_scene_hub", &["fade-shader"]))
            .unwrap();

        let resolved = registry.resolve_resource("kick.wav").unwrap();
        assert_eq!(resolved.hub_id, "audio-std");
        assert_eq!(resolved.resource_kind, "audio_asset_hub");

        let resolved = registry.resolve_resource("fade-shader").unwrap();
        assert_eq!(resolved.hub_id, "glsl-stage");
    }

    #[test]
    fn resolve_unknown_resource_returns_error() {
        let registry = ResourceHubRegistry::new();
        let result = registry.resolve_resource("missing");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("HUB-003"));
    }
}

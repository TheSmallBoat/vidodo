use std::collections::HashMap;
use std::path::Path;

use crate::ResolvedResource;
use crate::persistence::PersistentHubRegistry;

/// Cache statistics for the resource resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
}

/// LRU-cached resource resolver backed by [`PersistentHubRegistry`].
///
/// The first `resolve` for a given resource name queries SQLite; subsequent
/// resolves for the same name return from an in-memory cache. The cache uses
/// a bounded capacity with simple entry eviction (oldest-inserted-first).
pub struct ResourceResolver {
    registry: PersistentHubRegistry,
    cache: HashMap<String, ResolvedResource>,
    insertion_order: Vec<String>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl ResourceResolver {
    /// Create a new resolver backed by the SQLite database at `db_path`.
    pub fn open(db_path: &Path, capacity: usize) -> Result<Self, String> {
        let registry = PersistentHubRegistry::open(db_path)?;
        Ok(Self {
            registry,
            cache: HashMap::new(),
            insertion_order: Vec::new(),
            capacity: capacity.max(1),
            hits: 0,
            misses: 0,
        })
    }

    /// Resolve a resource reference. Returns cached result if available,
    /// otherwise queries the persistent hub registry.
    pub fn resolve(&mut self, resource_name: &str) -> Result<ResolvedResource, String> {
        if let Some(cached) = self.cache.get(resource_name) {
            self.hits += 1;
            return Ok(cached.clone());
        }

        self.misses += 1;
        let resolved = self.registry.resolve_resource(resource_name)?;

        // Evict oldest entry if at capacity
        if self.cache.len() >= self.capacity
            && let Some(oldest) = self.insertion_order.first().cloned()
        {
            self.cache.remove(&oldest);
            self.insertion_order.remove(0);
        }

        self.cache.insert(resource_name.to_string(), resolved.clone());
        self.insertion_order.push(resource_name.to_string());

        Ok(resolved)
    }

    /// Return cache hit/miss statistics.
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats { hits: self.hits, misses: self.misses, entries: self.cache.len() }
    }

    /// Clear the cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.insertion_order.clear();
    }

    /// Access the underlying persistent registry for registration operations.
    pub fn registry(&self) -> &PersistentHubRegistry {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::{HubCompatibility, ResourceHubDescriptor};

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

    fn temp_resolver(capacity: usize) -> (tempfile::TempDir, ResourceResolver) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("hubs.db");
        let resolver = ResourceResolver::open(&db_path, capacity).unwrap();
        (dir, resolver)
    }

    #[test]
    fn resolve_from_sqlite_then_cache() {
        let (_dir, mut resolver) = temp_resolver(16);
        resolver
            .registry()
            .register_hub(&sample_hub("audio-std", "audio", &["kick.wav", "snare.wav"]))
            .unwrap();

        // First resolve: miss → SQLite
        let r1 = resolver.resolve("kick.wav").unwrap();
        assert_eq!(r1.hub_id, "audio-std");
        let stats1 = resolver.cache_stats();
        assert_eq!(stats1.hits, 0);
        assert_eq!(stats1.misses, 1);
        assert_eq!(stats1.entries, 1);

        // Second resolve: hit → cache
        let r2 = resolver.resolve("kick.wav").unwrap();
        assert_eq!(r2, r1);
        let stats2 = resolver.cache_stats();
        assert_eq!(stats2.hits, 1);
        assert_eq!(stats2.misses, 1);
    }

    #[test]
    fn cache_eviction() {
        let (_dir, mut resolver) = temp_resolver(2);
        resolver
            .registry()
            .register_hub(&sample_hub("hub-1", "audio", &["a.wav", "b.wav", "c.wav"]))
            .unwrap();

        resolver.resolve("a.wav").unwrap(); // miss, cache: [a]
        resolver.resolve("b.wav").unwrap(); // miss, cache: [a, b]
        resolver.resolve("c.wav").unwrap(); // miss, cache: [b, c] (a evicted)

        let stats = resolver.cache_stats();
        assert_eq!(stats.entries, 2);
        assert_eq!(stats.misses, 3);

        // Re-resolve a.wav — should be a miss again because it was evicted
        resolver.resolve("a.wav").unwrap();
        let stats2 = resolver.cache_stats();
        assert_eq!(stats2.misses, 4);
    }

    #[test]
    fn clear_cache_resets_entries() {
        let (_dir, mut resolver) = temp_resolver(16);
        resolver.registry().register_hub(&sample_hub("hub-x", "audio", &["x.wav"])).unwrap();

        resolver.resolve("x.wav").unwrap();
        assert_eq!(resolver.cache_stats().entries, 1);

        resolver.clear_cache();
        assert_eq!(resolver.cache_stats().entries, 0);

        // After clear, next resolve is a miss
        resolver.resolve("x.wav").unwrap();
        assert_eq!(resolver.cache_stats().misses, 2);
    }

    #[test]
    fn unknown_resource_returns_error() {
        let (_dir, mut resolver) = temp_resolver(16);
        let result = resolver.resolve("nonexistent.wav");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("HUB-003"));
    }
}

//! Uniform buffer manager.
//!
//! Manages a pool of 256-byte–aligned uniform buffers on the CPU side.
//! When wgpu is wired, these buffers will be uploaded via
//! `queue.write_buffer()` each frame.

use crate::uniform::SceneUniformsGPU;
use std::collections::HashMap;

/// A CPU-side uniform buffer slot.
#[derive(Debug, Clone)]
pub struct BufferSlot {
    /// Slot identifier (e.g., scene_id or pipeline label).
    pub label: String,
    /// Current uniform data.
    pub uniforms: SceneUniformsGPU,
    /// Whether data has changed since last upload.
    pub dirty: bool,
    /// Generation counter (increments on each update).
    pub generation: u64,
}

/// Manages named uniform buffer slots.
pub struct BufferManager {
    slots: HashMap<String, BufferSlot>,
}

impl BufferManager {
    pub fn new() -> Self {
        Self { slots: HashMap::new() }
    }

    /// Allocate a new buffer slot with default uniforms.
    pub fn allocate(&mut self, label: impl Into<String>) -> &BufferSlot {
        let label = label.into();
        self.slots.entry(label.clone()).or_insert_with(|| BufferSlot {
            label: label.clone(),
            uniforms: SceneUniformsGPU::default(),
            dirty: true,
            generation: 0,
        })
    }

    /// Update the uniforms for a slot.
    pub fn update(&mut self, label: &str, uniforms: SceneUniformsGPU) -> bool {
        if let Some(slot) = self.slots.get_mut(label) {
            slot.uniforms = uniforms;
            slot.dirty = true;
            slot.generation += 1;
            true
        } else {
            false
        }
    }

    /// Get a slot by label.
    pub fn get(&self, label: &str) -> Option<&BufferSlot> {
        self.slots.get(label)
    }

    /// Collect all dirty slots, mark them clean, and return their data.
    pub fn flush_dirty(&mut self) -> Vec<(String, SceneUniformsGPU)> {
        let mut dirty = Vec::new();
        for slot in self.slots.values_mut() {
            if slot.dirty {
                dirty.push((slot.label.clone(), slot.uniforms));
                slot.dirty = false;
            }
        }
        dirty
    }

    /// Number of allocated slots.
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Free a buffer slot.
    pub fn free(&mut self, label: &str) -> bool {
        self.slots.remove(label).is_some()
    }
}

impl Default for BufferManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod buffer_tests {
    use super::*;

    #[test]
    fn allocate_and_get() {
        let mut mgr = BufferManager::new();
        mgr.allocate("scene-a");
        assert_eq!(mgr.slot_count(), 1);
        assert!(mgr.get("scene-a").is_some());
        assert!(mgr.get("scene-a").unwrap().dirty);
    }

    #[test]
    fn update_marks_dirty_and_increments_generation() {
        let mut mgr = BufferManager::new();
        mgr.allocate("s1");
        mgr.flush_dirty(); // clear initial dirty flag

        let mut u = SceneUniformsGPU::default();
        u.set_time(1.0, 2.0, 1.0, 120.0);
        assert!(mgr.update("s1", u));

        let slot = mgr.get("s1").unwrap();
        assert!(slot.dirty);
        assert_eq!(slot.generation, 1);
    }

    #[test]
    fn flush_dirty_clears_flag() {
        let mut mgr = BufferManager::new();
        mgr.allocate("s1");
        mgr.allocate("s2");

        let dirty = mgr.flush_dirty();
        assert_eq!(dirty.len(), 2);

        // Second flush should be empty
        let dirty2 = mgr.flush_dirty();
        assert!(dirty2.is_empty());
    }

    #[test]
    fn free_removes_slot() {
        let mut mgr = BufferManager::new();
        mgr.allocate("tmp");
        assert!(mgr.free("tmp"));
        assert_eq!(mgr.slot_count(), 0);
        assert!(!mgr.free("tmp")); // already freed
    }

    #[test]
    fn alignment_256_bytes() {
        let mgr_slot = BufferSlot {
            label: String::from("test"),
            uniforms: SceneUniformsGPU::default(),
            dirty: false,
            generation: 0,
        };
        assert_eq!(mgr_slot.uniforms.as_bytes().len(), 256);
    }
}

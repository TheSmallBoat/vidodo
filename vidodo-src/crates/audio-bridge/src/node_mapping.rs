//! Action-to-synth node mapping table.
//!
//! Tracks scsynth node IDs allocated for each Vidodo action,
//! allowing lookup and cleanup of active synth nodes.

use std::collections::HashMap;

/// Maps action_id (layer_id from IR) to scsynth node IDs.
#[derive(Debug, Default)]
pub struct NodeMapping {
    next_node_id: i32,
    /// action_id → allocated node_id
    map: HashMap<String, i32>,
    /// node_id → buffer_num (for buffer-backed playback)
    buffer_map: HashMap<i32, i32>,
    next_buffer_num: i32,
}

impl NodeMapping {
    pub fn new() -> Self {
        Self {
            next_node_id: 1000, // start node IDs at 1000
            map: HashMap::new(),
            buffer_map: HashMap::new(),
            next_buffer_num: 0,
        }
    }

    /// Allocate a new node ID for an action. Returns the node ID.
    pub fn allocate(&mut self, action_id: &str) -> i32 {
        let id = self.next_node_id;
        self.next_node_id += 1;
        self.map.insert(action_id.to_string(), id);
        id
    }

    /// Look up the node ID for an action.
    pub fn lookup(&self, action_id: &str) -> Option<i32> {
        self.map.get(action_id).copied()
    }

    /// Remove a node mapping (on free/stop).
    pub fn remove(&mut self, action_id: &str) -> Option<i32> {
        self.map.remove(action_id)
    }

    /// Allocate a buffer number for a node.
    pub fn allocate_buffer(&mut self, node_id: i32) -> i32 {
        let buf = self.next_buffer_num;
        self.next_buffer_num += 1;
        self.buffer_map.insert(node_id, buf);
        buf
    }

    /// Look up the buffer number for a node.
    pub fn buffer_for_node(&self, node_id: i32) -> Option<i32> {
        self.buffer_map.get(&node_id).copied()
    }

    /// Remove buffer mapping for a node.
    pub fn remove_buffer(&mut self, node_id: i32) -> Option<i32> {
        self.buffer_map.remove(&node_id)
    }

    /// Number of currently mapped actions.
    pub fn active_count(&self) -> usize {
        self.map.len()
    }

    /// All active action IDs.
    pub fn active_actions(&self) -> Vec<&str> {
        self.map.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_and_lookup() {
        let mut m = NodeMapping::new();
        let id = m.allocate("layer-bass");
        assert_eq!(m.lookup("layer-bass"), Some(id));
        assert_eq!(m.lookup("nonexistent"), None);
    }

    #[test]
    fn remove_clears_mapping() {
        let mut m = NodeMapping::new();
        m.allocate("layer-1");
        m.remove("layer-1");
        assert_eq!(m.lookup("layer-1"), None);
        assert_eq!(m.active_count(), 0);
    }

    #[test]
    fn buffer_allocation() {
        let mut m = NodeMapping::new();
        let node = m.allocate("layer-pad");
        let buf = m.allocate_buffer(node);
        assert_eq!(m.buffer_for_node(node), Some(buf));
    }

    #[test]
    fn sequential_node_ids() {
        let mut m = NodeMapping::new();
        let a = m.allocate("a");
        let b = m.allocate("b");
        assert_eq!(b, a + 1);
    }
}

//! Causal chain tracing for IPC messages.
//!
//! Every IPC message carries `message_id`, `parent_id`, and `correlation_id`.
//! The `CausalTracer` records these links and rebuilds causation chains
//! from any starting correlation id.

use std::collections::HashMap;

/// A link in the causal chain.
#[derive(Debug, Clone)]
pub struct CausalLink {
    pub message_id: String,
    pub parent_id: Option<String>,
    pub correlation_id: String,
    pub hop: String,
    pub timestamp_ms: f64,
}

/// Records causal links and reconstructs causation chains.
pub struct CausalTracer {
    /// correlation_id → ordered list of links.
    chains: HashMap<String, Vec<CausalLink>>,
    /// message_id → correlation_id for reverse lookup.
    message_index: HashMap<String, String>,
    /// Running counter for generating message IDs.
    id_counter: u64,
}

impl CausalTracer {
    pub fn new() -> Self {
        Self { chains: HashMap::new(), message_index: HashMap::new(), id_counter: 0 }
    }

    /// Generate a unique message ID.
    pub fn next_message_id(&mut self) -> String {
        self.id_counter += 1;
        format!("msg-{:06}", self.id_counter)
    }

    /// Record a causal link.
    pub fn record(
        &mut self,
        message_id: &str,
        parent_id: Option<&str>,
        correlation_id: &str,
        hop: &str,
        timestamp_ms: f64,
    ) {
        let link = CausalLink {
            message_id: message_id.to_string(),
            parent_id: parent_id.map(String::from),
            correlation_id: correlation_id.to_string(),
            hop: hop.to_string(),
            timestamp_ms,
        };

        self.chains.entry(correlation_id.to_string()).or_default().push(link);

        self.message_index.insert(message_id.to_string(), correlation_id.to_string());
    }

    /// Query the full causation chain for a correlation ID.
    ///
    /// Returns links in chronological order (by timestamp).
    pub fn query_chain(&self, correlation_id: &str) -> Vec<&CausalLink> {
        match self.chains.get(correlation_id) {
            Some(links) => {
                let mut sorted: Vec<_> = links.iter().collect();
                sorted.sort_by(|a, b| a.timestamp_ms.partial_cmp(&b.timestamp_ms).unwrap());
                sorted
            }
            None => Vec::new(),
        }
    }

    /// Get the number of hops in a chain.
    pub fn chain_depth(&self, correlation_id: &str) -> usize {
        self.chains.get(correlation_id).map_or(0, |v| v.len())
    }

    /// Find the correlation ID for a given message ID.
    pub fn correlation_for_message(&self, message_id: &str) -> Option<&str> {
        self.message_index.get(message_id).map(|s| s.as_str())
    }

    /// Total number of recorded correlation chains.
    pub fn chain_count(&self) -> usize {
        self.chains.len()
    }

    /// Total number of recorded links across all chains.
    pub fn total_links(&self) -> usize {
        self.chains.values().map(|v| v.len()).sum()
    }
}

impl Default for CausalTracer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_hop_causal_chain() {
        let mut tracer = CausalTracer::new();
        let corr = "corr-001";

        // Hop 1: scheduler tick → event
        let msg1 = tracer.next_message_id();
        tracer.record(&msg1, None, corr, "scheduler", 100.0);

        // Hop 2: event dispatch → channel
        let msg2 = tracer.next_message_id();
        tracer.record(&msg2, Some(&msg1), corr, "channel", 101.0);

        // Hop 3: channel → runtime ack
        let msg3 = tracer.next_message_id();
        tracer.record(&msg3, Some(&msg2), corr, "runtime", 103.0);

        // Query full chain
        let chain = tracer.query_chain(corr);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].hop, "scheduler");
        assert_eq!(chain[1].hop, "channel");
        assert_eq!(chain[2].hop, "runtime");

        // Parent chain is correct
        assert!(chain[0].parent_id.is_none());
        assert_eq!(chain[1].parent_id.as_deref(), Some(msg1.as_str()));
        assert_eq!(chain[2].parent_id.as_deref(), Some(msg2.as_str()));

        assert_eq!(tracer.chain_depth(corr), 3);
    }

    #[test]
    fn unknown_correlation_returns_empty() {
        let tracer = CausalTracer::new();
        assert!(tracer.query_chain("nonexistent").is_empty());
        assert_eq!(tracer.chain_depth("nonexistent"), 0);
    }

    #[test]
    fn correlation_for_message_reverse_lookup() {
        let mut tracer = CausalTracer::new();
        let msg_id = tracer.next_message_id();
        tracer.record(&msg_id, None, "corr-99", "scheduler", 0.0);

        assert_eq!(tracer.correlation_for_message(&msg_id), Some("corr-99"));
        assert!(tracer.correlation_for_message("no-such-msg").is_none());
    }

    #[test]
    fn multiple_chains_independent() {
        let mut tracer = CausalTracer::new();

        let m1 = tracer.next_message_id();
        tracer.record(&m1, None, "corr-A", "scheduler", 10.0);
        let m2 = tracer.next_message_id();
        tracer.record(&m2, Some(&m1), "corr-A", "runtime", 12.0);

        let m3 = tracer.next_message_id();
        tracer.record(&m3, None, "corr-B", "scheduler", 20.0);

        assert_eq!(tracer.chain_count(), 2);
        assert_eq!(tracer.chain_depth("corr-A"), 2);
        assert_eq!(tracer.chain_depth("corr-B"), 1);
        assert_eq!(tracer.total_links(), 3);
    }
}

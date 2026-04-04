use vidodo_ir::{ControlBinding, ExternalControlAdapter, ExternalControlEvent};

/// Null implementation of [`ExternalControlAdapter`] that never produces events.
///
/// Used as a default adapter when no physical controllers are connected,
/// and as a reference implementation for testing.
#[derive(Debug, Default)]
pub struct NullControlAdapter {
    bindings: Vec<ControlBinding>,
    /// Staged events that will be returned on the next `poll_events()` call.
    /// Useful in tests to inject synthetic events.
    staged: Vec<ExternalControlEvent>,
}

impl NullControlAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inject events that the next `poll_events()` will return.
    pub fn inject(&mut self, events: Vec<ExternalControlEvent>) {
        self.staged.extend(events);
    }
}

impl ExternalControlAdapter for NullControlAdapter {
    fn bind_source(&mut self, source_id: &str, protocol: &str) -> Result<(), String> {
        if self.bindings.iter().any(|b| b.source_id == source_id) {
            return Err(format!("source '{source_id}' is already bound"));
        }
        self.bindings.push(ControlBinding {
            source_id: source_id.to_string(),
            protocol: protocol.to_string(),
        });
        Ok(())
    }

    fn unbind_source(&mut self, source_id: &str) -> Result<(), String> {
        let before = self.bindings.len();
        self.bindings.retain(|b| b.source_id != source_id);
        if self.bindings.len() == before {
            return Err(format!("source '{source_id}' is not bound"));
        }
        Ok(())
    }

    fn poll_events(&mut self) -> Vec<ExternalControlEvent> {
        std::mem::take(&mut self.staged)
    }

    fn list_bindings(&self) -> Vec<ControlBinding> {
        self.bindings.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::MidiCC;

    #[test]
    fn bind_and_list() {
        let mut adapter = NullControlAdapter::new();
        adapter.bind_source("midi-1", "midi").unwrap();
        adapter.bind_source("osc-1", "osc").unwrap();
        let bindings = adapter.list_bindings();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].source_id, "midi-1");
        assert_eq!(bindings[1].source_id, "osc-1");
    }

    #[test]
    fn duplicate_bind_fails() {
        let mut adapter = NullControlAdapter::new();
        adapter.bind_source("midi-1", "midi").unwrap();
        assert!(adapter.bind_source("midi-1", "midi").is_err());
    }

    #[test]
    fn unbind_removes_source() {
        let mut adapter = NullControlAdapter::new();
        adapter.bind_source("midi-1", "midi").unwrap();
        adapter.unbind_source("midi-1").unwrap();
        assert!(adapter.list_bindings().is_empty());
    }

    #[test]
    fn unbind_unknown_fails() {
        let mut adapter = NullControlAdapter::new();
        assert!(adapter.unbind_source("nonexistent").is_err());
    }

    #[test]
    fn poll_returns_injected_events() {
        let mut adapter = NullControlAdapter::new();
        adapter.inject(vec![ExternalControlEvent::MidiCc {
            source_id: String::from("midi-1"),
            midi_cc: MidiCC { channel: 1, cc: 7, value: 100 },
        }]);
        let events = adapter.poll_events();
        assert_eq!(events.len(), 1);
        // Second poll is empty
        assert!(adapter.poll_events().is_empty());
    }

    #[test]
    fn poll_empty_by_default() {
        let mut adapter = NullControlAdapter::new();
        assert!(adapter.poll_events().is_empty());
    }
}

//! Fixture topology and DMX address mapping.
//!
//! Models lighting fixtures with their DMX universe/address information,
//! channel capabilities, and provides topology-level queries.

use serde::{Deserialize, Serialize};

/// Channel capability of a fixture (what each channel controls).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelCapability {
    Dimmer,
    Red,
    Green,
    Blue,
    White,
    Amber,
    Pan,
    Tilt,
    Strobe,
    ColorWheel,
    Gobo,
}

/// A single fixture endpoint with its DMX addressing and capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureEndpoint {
    pub fixture_id: String,
    pub universe: u16,
    /// Start address (1-based, DMX convention).
    pub address: u16,
    pub capabilities: Vec<ChannelCapability>,
    /// Human-readable label.
    pub label: Option<String>,
}

impl FixtureEndpoint {
    /// Number of DMX channels this fixture occupies.
    pub fn channel_count(&self) -> u16 {
        self.capabilities.len() as u16
    }

    /// Compute the absolute channel offset for a given capability.
    /// Returns `None` if the fixture doesn't have that capability.
    pub fn channel_for(&self, cap: ChannelCapability) -> Option<u16> {
        self.capabilities.iter().position(|c| *c == cap).map(|i| self.address + i as u16)
    }

    /// End address (inclusive) in the DMX universe.
    pub fn end_address(&self) -> u16 {
        self.address + self.channel_count() - 1
    }
}

/// A collection of fixtures forming a lighting bus topology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureBusTopology {
    pub topology_id: String,
    pub fixtures: Vec<FixtureEndpoint>,
}

impl FixtureBusTopology {
    pub fn new(topology_id: impl Into<String>) -> Self {
        Self { topology_id: topology_id.into(), fixtures: Vec::new() }
    }

    /// Add a fixture, validating no address overlap in the same universe.
    pub fn add_fixture(&mut self, fixture: FixtureEndpoint) -> Result<(), String> {
        if fixture.address == 0 || fixture.end_address() > 512 {
            return Err(format!(
                "fixture '{}' address range {}-{} out of DMX range 1-512",
                fixture.fixture_id,
                fixture.address,
                fixture.end_address()
            ));
        }

        // Check for address overlap within the same universe.
        for existing in &self.fixtures {
            if existing.universe == fixture.universe
                && ranges_overlap(
                    existing.address,
                    existing.end_address(),
                    fixture.address,
                    fixture.end_address(),
                )
            {
                return Err(format!(
                    "fixture '{}' overlaps with '{}' in universe {}",
                    fixture.fixture_id, existing.fixture_id, fixture.universe
                ));
            }
        }

        self.fixtures.push(fixture);
        Ok(())
    }

    /// Look up a fixture by its ID.
    pub fn find(&self, fixture_id: &str) -> Option<&FixtureEndpoint> {
        self.fixtures.iter().find(|f| f.fixture_id == fixture_id)
    }

    /// List all fixtures in a given universe.
    pub fn fixtures_in_universe(&self, universe: u16) -> Vec<&FixtureEndpoint> {
        self.fixtures.iter().filter(|f| f.universe == universe).collect()
    }

    /// Total number of fixtures.
    pub fn fixture_count(&self) -> usize {
        self.fixtures.len()
    }

    /// Set of distinct universes used by this topology.
    pub fn universes(&self) -> Vec<u16> {
        let mut unis: Vec<u16> = self.fixtures.iter().map(|f| f.universe).collect();
        unis.sort_unstable();
        unis.dedup();
        unis
    }
}

fn ranges_overlap(a_start: u16, a_end: u16, b_start: u16, b_end: u16) -> bool {
    a_start <= b_end && b_start <= a_end
}

/// Load a topology from JSON fixture definitions.
pub fn load_topology_json(topology_id: &str, json: &str) -> Result<FixtureBusTopology, String> {
    let fixtures: Vec<FixtureEndpoint> =
        serde_json::from_str(json).map_err(|e| format!("JSON parse error: {e}"))?;
    let mut topo = FixtureBusTopology::new(topology_id);
    for f in fixtures {
        topo.add_fixture(f)?;
    }
    Ok(topo)
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    fn dimmer_fixture(id: &str, universe: u16, address: u16) -> FixtureEndpoint {
        FixtureEndpoint {
            fixture_id: id.into(),
            universe,
            address,
            capabilities: vec![ChannelCapability::Dimmer],
            label: None,
        }
    }

    fn rgb_fixture(id: &str, universe: u16, address: u16) -> FixtureEndpoint {
        FixtureEndpoint {
            fixture_id: id.into(),
            universe,
            address,
            capabilities: vec![
                ChannelCapability::Dimmer,
                ChannelCapability::Red,
                ChannelCapability::Green,
                ChannelCapability::Blue,
            ],
            label: Some(format!("RGB {id}")),
        }
    }

    #[test]
    fn dimmer_occupies_one_channel() {
        let f = dimmer_fixture("dim-1", 0, 1);
        assert_eq!(f.channel_count(), 1);
        assert_eq!(f.end_address(), 1);
        assert_eq!(f.channel_for(ChannelCapability::Dimmer), Some(1));
        assert_eq!(f.channel_for(ChannelCapability::Red), None);
    }

    #[test]
    fn rgb_occupies_four_channels() {
        let f = rgb_fixture("rgb-1", 0, 10);
        assert_eq!(f.channel_count(), 4);
        assert_eq!(f.end_address(), 13);
        assert_eq!(f.channel_for(ChannelCapability::Dimmer), Some(10));
        assert_eq!(f.channel_for(ChannelCapability::Red), Some(11));
        assert_eq!(f.channel_for(ChannelCapability::Green), Some(12));
        assert_eq!(f.channel_for(ChannelCapability::Blue), Some(13));
    }

    #[test]
    fn lookup_by_fixture_id() {
        let mut topo = FixtureBusTopology::new("stage");
        topo.add_fixture(dimmer_fixture("dim-1", 0, 1)).unwrap();
        topo.add_fixture(rgb_fixture("rgb-1", 0, 10)).unwrap();

        let found = topo.find("rgb-1").unwrap();
        assert_eq!(found.universe, 0);
        assert_eq!(found.address, 10);
        assert!(topo.find("nonexistent").is_none());
    }

    #[test]
    fn address_overlap_rejected() {
        let mut topo = FixtureBusTopology::new("stage");
        topo.add_fixture(rgb_fixture("rgb-1", 0, 10)).unwrap(); // 10-13
        assert!(topo.add_fixture(dimmer_fixture("dim-bad", 0, 12)).is_err());
    }

    #[test]
    fn different_universe_no_overlap() {
        let mut topo = FixtureBusTopology::new("stage");
        topo.add_fixture(rgb_fixture("rgb-1", 0, 10)).unwrap();
        topo.add_fixture(rgb_fixture("rgb-2", 1, 10)).unwrap(); // same address, different universe
        assert_eq!(topo.fixture_count(), 2);
        assert_eq!(topo.universes(), vec![0, 1]);
    }

    #[test]
    fn load_topology_from_json() {
        let json = r#"[
            {"fixture_id": "dim-1", "universe": 0, "address": 1, "capabilities": ["Dimmer"], "label": null},
            {"fixture_id": "rgb-1", "universe": 0, "address": 10, "capabilities": ["Dimmer", "Red", "Green", "Blue"], "label": "Main RGB"}
        ]"#;
        let topo = load_topology_json("test-stage", json).unwrap();
        assert_eq!(topo.fixture_count(), 2);
        assert_eq!(topo.fixtures_in_universe(0).len(), 2);
    }

    #[test]
    fn address_zero_rejected() {
        let mut topo = FixtureBusTopology::new("stage");
        assert!(
            topo.add_fixture(FixtureEndpoint {
                fixture_id: "bad".into(),
                universe: 0,
                address: 0,
                capabilities: vec![ChannelCapability::Dimmer],
                label: None,
            })
            .is_err()
        );
    }

    #[test]
    fn serde_roundtrip() {
        let f = rgb_fixture("test-rgb", 2, 100);
        let json = serde_json::to_string(&f).unwrap();
        let decoded: FixtureEndpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.fixture_id, "test-rgb");
        assert_eq!(decoded.channel_count(), 4);
    }
}

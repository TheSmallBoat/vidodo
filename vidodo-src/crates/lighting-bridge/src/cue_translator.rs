//! Cue → DMX frame translator.
//!
//! Translates lighting cue entries into DMX frames by mapping fixture
//! capabilities (dimmer, RGB, pan/tilt) to the correct channels based
//! on the fixture topology.

use crate::dmx::DmxFrame;
use crate::fixture_topology::{ChannelCapability, FixtureBusTopology, FixtureEndpoint};
use std::collections::HashMap;

/// A cue entry to translate into DMX values.
#[derive(Debug, Clone)]
pub struct CueEntry {
    /// Target fixture IDs (empty = all fixtures).
    pub fixture_ids: Vec<String>,
    /// Intensity (0.0–1.0).
    pub intensity: f64,
    /// RGB color [R, G, B] each 0.0–1.0.
    pub color: Option<[f64; 3]>,
    /// Pan position (0.0–1.0 maps to 0–255).
    pub pan: Option<f64>,
    /// Tilt position (0.0–1.0 maps to 0–255).
    pub tilt: Option<f64>,
    /// Strobe rate (0.0 = off, 1.0 = max).
    pub strobe: Option<f64>,
}

/// Result of translating a cue: one DMX frame per universe touched.
#[derive(Debug)]
pub struct CueTranslation {
    pub frames: Vec<DmxFrame>,
    /// Fixtures that were addressed.
    pub addressed_count: usize,
}

/// Translate a `CueEntry` into DMX frames using the given fixture topology.
///
/// Returns one `DmxFrame` per universe that is touched by the cue's target
/// fixtures. If `fixture_ids` is empty, all fixtures in the topology are
/// targeted.
pub fn translate_cue(cue: &CueEntry, topology: &FixtureBusTopology) -> CueTranslation {
    let fixtures: Vec<&FixtureEndpoint> = if cue.fixture_ids.is_empty() {
        topology.fixtures.iter().collect()
    } else {
        cue.fixture_ids.iter().filter_map(|id| topology.find(id)).collect()
    };

    let mut frames_by_universe: HashMap<u16, DmxFrame> = HashMap::new();
    let addressed_count = fixtures.len();

    for fixture in &fixtures {
        let frame = frames_by_universe
            .entry(fixture.universe)
            .or_insert_with(|| DmxFrame::new(fixture.universe));

        apply_fixture_values(frame, fixture, cue);
    }

    CueTranslation { frames: frames_by_universe.into_values().collect(), addressed_count }
}

/// Apply cue values to the DMX frame for a single fixture,
/// mapping capabilities to the correct channels.
fn apply_fixture_values(frame: &mut DmxFrame, fixture: &FixtureEndpoint, cue: &CueEntry) {
    // Dimmer channel
    if let Some(ch) = fixture.channel_for(ChannelCapability::Dimmer) {
        let _ = frame.set_channel(ch, float_to_dmx(cue.intensity));
    }

    // RGB channels
    if let Some([r, g, b]) = cue.color {
        if let Some(ch) = fixture.channel_for(ChannelCapability::Red) {
            let _ = frame.set_channel(ch, float_to_dmx(r));
        }
        if let Some(ch) = fixture.channel_for(ChannelCapability::Green) {
            let _ = frame.set_channel(ch, float_to_dmx(g));
        }
        if let Some(ch) = fixture.channel_for(ChannelCapability::Blue) {
            let _ = frame.set_channel(ch, float_to_dmx(b));
        }
    }

    // Pan/Tilt
    if let Some(pan) = cue.pan
        && let Some(ch) = fixture.channel_for(ChannelCapability::Pan)
    {
        let _ = frame.set_channel(ch, float_to_dmx(pan));
    }
    if let Some(tilt) = cue.tilt
        && let Some(ch) = fixture.channel_for(ChannelCapability::Tilt)
    {
        let _ = frame.set_channel(ch, float_to_dmx(tilt));
    }

    // Strobe
    if let Some(strobe) = cue.strobe
        && let Some(ch) = fixture.channel_for(ChannelCapability::Strobe)
    {
        let _ = frame.set_channel(ch, float_to_dmx(strobe));
    }
}

/// Convert a 0.0–1.0 float to a 0–255 DMX value.
fn float_to_dmx(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod cue_translator_tests {
    use super::*;
    use crate::fixture_topology::FixtureBusTopology;

    fn rgb_fixture(id: &str, universe: u16, address: u16) -> FixtureEndpoint {
        FixtureEndpoint {
            fixture_id: id.to_string(),
            universe,
            address,
            capabilities: vec![
                ChannelCapability::Dimmer,
                ChannelCapability::Red,
                ChannelCapability::Green,
                ChannelCapability::Blue,
            ],
            label: None,
        }
    }

    fn mover_fixture(id: &str, universe: u16, address: u16) -> FixtureEndpoint {
        FixtureEndpoint {
            fixture_id: id.to_string(),
            universe,
            address,
            capabilities: vec![
                ChannelCapability::Dimmer,
                ChannelCapability::Red,
                ChannelCapability::Green,
                ChannelCapability::Blue,
                ChannelCapability::Pan,
                ChannelCapability::Tilt,
            ],
            label: None,
        }
    }

    #[test]
    fn intensity_half_rgb_red() {
        let mut topo = FixtureBusTopology::new("test");
        topo.add_fixture(rgb_fixture("par-1", 0, 1)).unwrap();

        let cue = CueEntry {
            fixture_ids: vec![],
            intensity: 0.5,
            color: Some([1.0, 0.0, 0.0]),
            pan: None,
            tilt: None,
            strobe: None,
        };

        let result = translate_cue(&cue, &topo);
        assert_eq!(result.addressed_count, 1);
        assert_eq!(result.frames.len(), 1);

        let frame = &result.frames[0];
        assert_eq!(frame.get_channel(1).unwrap(), 128); // dimmer = 0.5 → 128 (round)
        assert_eq!(frame.get_channel(2).unwrap(), 255); // R = 1.0 → 255
        assert_eq!(frame.get_channel(3).unwrap(), 0); // G = 0.0 → 0
        assert_eq!(frame.get_channel(4).unwrap(), 0); // B = 0.0 → 0
    }

    #[test]
    fn cross_universe_generates_two_frames() {
        let mut topo = FixtureBusTopology::new("test");
        topo.add_fixture(rgb_fixture("par-1", 0, 1)).unwrap();
        topo.add_fixture(rgb_fixture("par-2", 1, 1)).unwrap();

        let cue = CueEntry {
            fixture_ids: vec![],
            intensity: 1.0,
            color: Some([0.0, 1.0, 0.0]),
            pan: None,
            tilt: None,
            strobe: None,
        };

        let result = translate_cue(&cue, &topo);
        assert_eq!(result.addressed_count, 2);
        assert_eq!(result.frames.len(), 2);
    }

    #[test]
    fn pan_tilt_mover() {
        let mut topo = FixtureBusTopology::new("test");
        topo.add_fixture(mover_fixture("mov-1", 0, 1)).unwrap();

        let cue = CueEntry {
            fixture_ids: vec!["mov-1".into()],
            intensity: 1.0,
            color: None,
            pan: Some(0.5),
            tilt: Some(0.75),
            strobe: None,
        };

        let result = translate_cue(&cue, &topo);
        let frame = &result.frames[0];
        assert_eq!(frame.get_channel(5).unwrap(), 128); // pan 0.5 → 128
        assert_eq!(frame.get_channel(6).unwrap(), 191); // tilt 0.75 → 191
    }

    #[test]
    fn specific_fixture_ids_filter() {
        let mut topo = FixtureBusTopology::new("test");
        topo.add_fixture(rgb_fixture("par-1", 0, 1)).unwrap();
        topo.add_fixture(rgb_fixture("par-2", 0, 10)).unwrap();

        let cue = CueEntry {
            fixture_ids: vec!["par-2".into()],
            intensity: 0.8,
            color: Some([0.0, 0.0, 1.0]),
            pan: None,
            tilt: None,
            strobe: None,
        };

        let result = translate_cue(&cue, &topo);
        assert_eq!(result.addressed_count, 1);

        let frame = &result.frames[0];
        // par-1's channels should be 0 (not targeted)
        assert_eq!(frame.get_channel(1).unwrap(), 0);
        // par-2 at address 10
        assert_eq!(frame.get_channel(10).unwrap(), 204); // dimmer 0.8 → 204
        assert_eq!(frame.get_channel(13).unwrap(), 255); // B = 1.0 → 255
    }

    #[test]
    fn empty_fixture_ids_targets_all() {
        let mut topo = FixtureBusTopology::new("test");
        topo.add_fixture(rgb_fixture("a", 0, 1)).unwrap();
        topo.add_fixture(rgb_fixture("b", 0, 10)).unwrap();

        let cue = CueEntry {
            fixture_ids: vec![],
            intensity: 1.0,
            color: None,
            pan: None,
            tilt: None,
            strobe: None,
        };

        let result = translate_cue(&cue, &topo);
        assert_eq!(result.addressed_count, 2);
    }

    #[test]
    fn float_to_dmx_clamp() {
        assert_eq!(float_to_dmx(0.0), 0);
        assert_eq!(float_to_dmx(1.0), 255);
        assert_eq!(float_to_dmx(-0.5), 0);
        assert_eq!(float_to_dmx(1.5), 255);
        assert_eq!(float_to_dmx(0.5), 128);
    }
}

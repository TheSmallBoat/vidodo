//! DMX-512 frame management.

use serde::{Deserialize, Serialize};

/// Maximum channels in a DMX-512 universe.
pub const DMX_UNIVERSE_SIZE: usize = 512;

/// A single DMX universe frame (512 channels, 0–255 each).
#[derive(Debug, Clone)]
pub struct DmxFrame {
    /// Universe number (0-based, ArtNet supports 0–32767).
    pub universe: u16,
    /// 512-channel data.
    pub channels: [u8; DMX_UNIVERSE_SIZE],
    /// Sequence counter for ordering (0–255, wraps).
    pub sequence: u8,
}

impl DmxFrame {
    /// Create a new frame with all channels at zero.
    pub fn new(universe: u16) -> Self {
        Self { universe, channels: [0u8; DMX_UNIVERSE_SIZE], sequence: 0 }
    }

    /// Set a single channel (1-based addressing, DMX convention).
    pub fn set_channel(&mut self, channel: u16, value: u8) -> Result<(), String> {
        if channel == 0 || channel as usize > DMX_UNIVERSE_SIZE {
            return Err(format!("DMX channel {channel} out of range 1..512"));
        }
        self.channels[(channel - 1) as usize] = value;
        Ok(())
    }

    /// Get a channel value (1-based).
    pub fn get_channel(&self, channel: u16) -> Result<u8, String> {
        if channel == 0 || channel as usize > DMX_UNIVERSE_SIZE {
            return Err(format!("DMX channel {channel} out of range 1..512"));
        }
        Ok(self.channels[(channel - 1) as usize])
    }

    /// Set a range of channels starting at `start` (1-based).
    pub fn set_range(&mut self, start: u16, values: &[u8]) -> Result<(), String> {
        let end = start as usize + values.len();
        if start == 0 || end - 1 > DMX_UNIVERSE_SIZE {
            return Err(format!("range {start}..{end} exceeds DMX universe"));
        }
        let offset = (start - 1) as usize;
        self.channels[offset..offset + values.len()].copy_from_slice(values);
        Ok(())
    }

    /// Zero all channels (blackout).
    pub fn blackout(&mut self) {
        self.channels.fill(0);
    }

    /// Advance the sequence counter (wraps at 255).
    pub fn next_sequence(&mut self) {
        self.sequence = self.sequence.wrapping_add(1);
    }
}

/// Fixture addressing range within a DMX universe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureAddress {
    /// Fixture identifier.
    pub fixture_id: String,
    /// Universe number.
    pub universe: u16,
    /// Start channel (1-based).
    pub start_channel: u16,
    /// Number of channels this fixture occupies.
    pub channel_count: u16,
}

#[cfg(test)]
mod dmx_tests {
    use super::*;

    #[test]
    fn new_frame_is_zeroed() {
        let frame = DmxFrame::new(0);
        assert!(frame.channels.iter().all(|&c| c == 0));
        assert_eq!(frame.universe, 0);
    }

    #[test]
    fn set_and_get_channel() {
        let mut frame = DmxFrame::new(0);
        frame.set_channel(1, 255).unwrap();
        frame.set_channel(512, 128).unwrap();
        assert_eq!(frame.get_channel(1).unwrap(), 255);
        assert_eq!(frame.get_channel(512).unwrap(), 128);
    }

    #[test]
    fn channel_zero_is_invalid() {
        let mut frame = DmxFrame::new(0);
        assert!(frame.set_channel(0, 100).is_err());
        assert!(frame.get_channel(0).is_err());
    }

    #[test]
    fn channel_513_is_invalid() {
        let mut frame = DmxFrame::new(0);
        assert!(frame.set_channel(513, 100).is_err());
    }

    #[test]
    fn set_range() {
        let mut frame = DmxFrame::new(0);
        // Set RGB for a 3-channel fixture starting at channel 10
        frame.set_range(10, &[255, 128, 64]).unwrap();
        assert_eq!(frame.get_channel(10).unwrap(), 255);
        assert_eq!(frame.get_channel(11).unwrap(), 128);
        assert_eq!(frame.get_channel(12).unwrap(), 64);
    }

    #[test]
    fn set_range_overflow() {
        let mut frame = DmxFrame::new(0);
        assert!(frame.set_range(511, &[1, 2, 3]).is_err());
    }

    #[test]
    fn blackout() {
        let mut frame = DmxFrame::new(0);
        frame.set_channel(1, 255).unwrap();
        frame.blackout();
        assert_eq!(frame.get_channel(1).unwrap(), 0);
    }

    #[test]
    fn sequence_wraps() {
        let mut frame = DmxFrame::new(0);
        frame.sequence = 254;
        frame.next_sequence();
        assert_eq!(frame.sequence, 255);
        frame.next_sequence();
        assert_eq!(frame.sequence, 0);
    }
}

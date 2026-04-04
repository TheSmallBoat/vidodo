//! ArtNet packet serialization (Art-Net 4 protocol).
//!
//! Implements OpDmx (opcode 0x5000) packet construction.

use super::dmx::DmxFrame;
use serde::{Deserialize, Serialize};

/// ArtNet protocol version.
pub const ARTNET_PROTOCOL_VERSION: u16 = 14;

/// ArtNet OpDmx opcode.
pub const ARTNET_OPCODE_DMX: u16 = 0x5000;

/// ArtNet magic header.
pub const ARTNET_HEADER: &[u8; 8] = b"Art-Net\0";

/// Configuration for an ArtNet sender node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtNetConfig {
    /// Target IP address (unicast) or broadcast address.
    pub target_addr: String,
    /// Target port (default 6454).
    pub port: u16,
    /// Source node short name.
    pub short_name: String,
}

impl Default for ArtNetConfig {
    fn default() -> Self {
        Self {
            target_addr: String::from("255.255.255.255"),
            port: 6454,
            short_name: String::from("Vidodo"),
        }
    }
}

/// Serialize a DmxFrame into an ArtNet OpDmx packet (raw bytes).
///
/// Packet structure (Art-Net 4):
/// - Bytes 0–7: "Art-Net\0" magic
/// - Bytes 8–9: OpCode 0x5000 (little-endian)
/// - Bytes 10–11: Protocol version 14 (big-endian)
/// - Byte 12: Sequence
/// - Byte 13: Physical port (0)
/// - Bytes 14–15: Universe (little-endian)
/// - Bytes 16–17: Length (big-endian, always 512)
/// - Bytes 18–529: DMX data (512 channels)
pub fn build_opdmx_packet(frame: &DmxFrame) -> Vec<u8> {
    let mut packet = Vec::with_capacity(18 + 512);

    // Header
    packet.extend_from_slice(ARTNET_HEADER);

    // OpCode (little-endian)
    packet.extend_from_slice(&ARTNET_OPCODE_DMX.to_le_bytes());

    // Protocol version (big-endian)
    packet.extend_from_slice(&ARTNET_PROTOCOL_VERSION.to_be_bytes());

    // Sequence
    packet.push(frame.sequence);

    // Physical port
    packet.push(0);

    // Universe (little-endian)
    packet.extend_from_slice(&frame.universe.to_le_bytes());

    // Data length (big-endian, always 512)
    packet.extend_from_slice(&(512u16).to_be_bytes());

    // DMX data
    packet.extend_from_slice(&frame.channels);

    packet
}

/// Parse an ArtNet OpDmx packet back into a DmxFrame.
pub fn parse_opdmx_packet(data: &[u8]) -> Result<DmxFrame, String> {
    if data.len() < 18 + 512 {
        return Err(format!("packet too short: {} bytes", data.len()));
    }

    // Verify header
    if &data[0..8] != ARTNET_HEADER {
        return Err("invalid ArtNet header".into());
    }

    // Verify opcode
    let opcode = u16::from_le_bytes([data[8], data[9]]);
    if opcode != ARTNET_OPCODE_DMX {
        return Err(format!("unexpected opcode: 0x{opcode:04X}"));
    }

    let sequence = data[12];
    let universe = u16::from_le_bytes([data[14], data[15]]);

    let mut channels = [0u8; 512];
    channels.copy_from_slice(&data[18..530]);

    Ok(DmxFrame { universe, channels, sequence })
}

/// Manages ArtNet packet sending (abstraction over UDP).
///
/// In the current phase, collects packets for testing. When `std::net::UdpSocket`
/// is wired, `send` will transmit over the network.
pub struct ArtNetSender {
    config: ArtNetConfig,
    sent_packets: Vec<Vec<u8>>,
}

impl ArtNetSender {
    pub fn new(config: ArtNetConfig) -> Self {
        Self { config, sent_packets: Vec::new() }
    }

    /// Build and collect an OpDmx packet for a frame.
    pub fn send(&mut self, frame: &DmxFrame) -> Result<usize, String> {
        let packet = build_opdmx_packet(frame);
        let len = packet.len();
        self.sent_packets.push(packet);
        Ok(len)
    }

    pub fn config(&self) -> &ArtNetConfig {
        &self.config
    }

    /// Number of packets sent.
    pub fn sent_count(&self) -> usize {
        self.sent_packets.len()
    }

    /// Access the raw sent packets for testing.
    pub fn sent_packets(&self) -> &[Vec<u8>] {
        &self.sent_packets
    }
}

#[cfg(test)]
mod artnet_tests {
    use super::*;

    #[test]
    fn opdmx_packet_structure() {
        let mut frame = DmxFrame::new(1);
        frame.set_channel(1, 255).unwrap();
        frame.sequence = 42;

        let packet = build_opdmx_packet(&frame);
        assert_eq!(packet.len(), 530);
        assert_eq!(&packet[0..8], b"Art-Net\0");

        // Opcode
        let opcode = u16::from_le_bytes([packet[8], packet[9]]);
        assert_eq!(opcode, 0x5000);

        // Protocol version
        let version = u16::from_be_bytes([packet[10], packet[11]]);
        assert_eq!(version, 14);

        // Sequence
        assert_eq!(packet[12], 42);

        // Universe
        let universe = u16::from_le_bytes([packet[14], packet[15]]);
        assert_eq!(universe, 1);

        // First channel data
        assert_eq!(packet[18], 255);
    }

    #[test]
    fn roundtrip_build_parse() {
        let mut frame = DmxFrame::new(7);
        frame.set_channel(100, 200).unwrap();
        frame.set_channel(256, 128).unwrap();
        frame.sequence = 99;

        let packet = build_opdmx_packet(&frame);
        let parsed = parse_opdmx_packet(&packet).unwrap();

        assert_eq!(parsed.universe, 7);
        assert_eq!(parsed.sequence, 99);
        assert_eq!(parsed.get_channel(100).unwrap(), 200);
        assert_eq!(parsed.get_channel(256).unwrap(), 128);
    }

    #[test]
    fn parse_short_packet_fails() {
        assert!(parse_opdmx_packet(&[0u8; 10]).is_err());
    }

    #[test]
    fn parse_bad_header_fails() {
        let mut packet = vec![0u8; 530];
        packet[0..4].copy_from_slice(b"Nope");
        assert!(parse_opdmx_packet(&packet).is_err());
    }

    #[test]
    fn sender_collects_packets() {
        let mut sender = ArtNetSender::new(ArtNetConfig::default());
        let frame = DmxFrame::new(0);
        let len = sender.send(&frame).unwrap();
        assert_eq!(len, 530);
        assert_eq!(sender.sent_count(), 1);
    }
}

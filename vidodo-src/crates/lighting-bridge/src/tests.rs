#[cfg(test)]
mod integration {
    use crate::artnet::{ArtNetConfig, ArtNetSender, parse_opdmx_packet};
    use crate::dmx::DmxFrame;

    #[test]
    fn multi_universe_send_and_verify() {
        let mut sender = ArtNetSender::new(ArtNetConfig::default());

        for uni in 0..4u16 {
            let mut frame = DmxFrame::new(uni);
            frame.set_channel(1, (uni * 50) as u8).unwrap();
            frame.sequence = uni as u8;
            sender.send(&frame).unwrap();
        }

        assert_eq!(sender.sent_count(), 4);

        for (i, packet) in sender.sent_packets().iter().enumerate() {
            let parsed = parse_opdmx_packet(packet).unwrap();
            assert_eq!(parsed.universe, i as u16);
            assert_eq!(parsed.get_channel(1).unwrap(), (i as u16 * 50) as u8);
        }
    }

    #[test]
    fn fixture_rgb_workflow() {
        let mut frame = DmxFrame::new(0);
        // Simulate a 4-channel RGBW fixture at address 10
        frame.set_range(10, &[255, 0, 128, 200]).unwrap(); // R=255, G=0, B=128, W=200

        let mut sender = ArtNetSender::new(ArtNetConfig::default());
        sender.send(&frame).unwrap();

        let parsed = parse_opdmx_packet(&sender.sent_packets()[0]).unwrap();
        assert_eq!(parsed.get_channel(10).unwrap(), 255);
        assert_eq!(parsed.get_channel(11).unwrap(), 0);
        assert_eq!(parsed.get_channel(12).unwrap(), 128);
        assert_eq!(parsed.get_channel(13).unwrap(), 200);
    }
}

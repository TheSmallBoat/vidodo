//! Transport state machine with play/pause/seek/set_tempo.
//!
//! Wraps [`RealtimeClock`] with higher-level transport control including
//! seek-to-beat, which the raw clock doesn't support.

use super::realtime_clock::{ClockState, RealtimeClock, TickSnapshot};

/// Transport state machine controlling the realtime clock.
///
/// Provides play/pause/stop/seek/set_tempo semantics. Seek repositions
/// the clock to an arbitrary beat position.
#[derive(Debug)]
pub struct Transport {
    clock: RealtimeClock,
}

/// Result of a transport state query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    Playing,
    Paused,
    Stopped,
}

impl From<ClockState> for TransportState {
    fn from(cs: ClockState) -> Self {
        match cs {
            ClockState::Playing => TransportState::Playing,
            ClockState::Paused => TransportState::Paused,
            ClockState::Stopped => TransportState::Stopped,
        }
    }
}

impl Transport {
    /// Create a new stopped transport.
    pub fn new(tempo: f64, time_signature: [u32; 2]) -> Self {
        Self { clock: RealtimeClock::new(tempo, time_signature) }
    }

    /// Start or resume playback.
    pub fn play(&mut self) {
        self.clock.play();
    }

    /// Pause, freezing the current beat position.
    pub fn pause(&mut self) {
        self.clock.pause();
    }

    /// Stop and reset to beat 0.
    pub fn stop(&mut self) {
        self.clock.stop();
    }

    /// Seek to a specific beat position.
    ///
    /// Works in any state (Playing, Paused, Stopped).
    /// If playing, playback continues from the new position.
    /// If paused, the new position is frozen until play() is called.
    pub fn seek(&mut self, beat: f64) {
        let was_playing = self.state() == TransportState::Playing;
        let tempo = self.clock.tempo();
        let ts = self.time_signature();

        // Freeze current state
        if was_playing {
            self.clock.pause();
        }

        // Compute ms offset for the target beat
        let ms_per_beat = 60_000.0 / tempo;
        let target_ms = beat * ms_per_beat;

        // Reset and set accumulated time to target position
        self.clock.stop();
        self.clock.seek_to_ms(target_ms);

        // Restore playing state if it was playing
        if was_playing {
            self.clock.play();
        }

        // If we were stopped or paused, go to paused so position is held
        if !was_playing && self.state() == TransportState::Stopped {
            // Set accumulated and stay paused
            // Already handled by seek_to_ms + not calling play
        }

        let _ = ts; // used for future bar-level seek
    }

    /// Set tempo in BPM.
    pub fn set_tempo(&mut self, bpm: f64) {
        self.clock.set_tempo(bpm);
    }

    /// Get current transport state.
    pub fn state(&self) -> TransportState {
        self.clock.state().into()
    }

    /// Sample the clock and produce a tick snapshot.
    pub fn tick(&self) -> TickSnapshot {
        self.clock.tick()
    }

    /// Current tempo.
    pub fn tempo(&self) -> f64 {
        self.clock.tempo()
    }

    /// Time signature.
    pub fn time_signature(&self) -> [u32; 2] {
        self.clock.time_signature()
    }

    /// Enter a new section (updates metadata on the clock).
    pub fn enter_section(&mut self, section: String, phrase: u32) {
        self.clock.enter_section(section, phrase);
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    #[test]
    fn initial_state_is_stopped() {
        let t = Transport::new(120.0, [4, 4]);
        assert_eq!(t.state(), TransportState::Stopped);
        let snap = t.tick();
        assert!(snap.musical_time.beat < 0.01);
    }

    #[test]
    fn play_pause_play_beats_advance_only_during_play() {
        let mut t = Transport::new(120.0, [4, 4]);
        t.play();
        assert_eq!(t.state(), TransportState::Playing);

        t.pause();
        assert_eq!(t.state(), TransportState::Paused);
        let paused_beat = t.tick().musical_time.beat;

        // Resume
        t.play();
        assert_eq!(t.state(), TransportState::Playing);

        // Beat should be >= paused beat (may be slightly > due to timing)
        let resumed_beat = t.tick().musical_time.beat;
        assert!(resumed_beat >= paused_beat);
    }

    #[test]
    fn seek_positions_clock_at_target_beat() {
        // 120 BPM, 4/4 → beat 16 = bar 5 beat 0
        let mut t = Transport::new(120.0, [4, 4]);
        t.seek(16.0);

        // After seek in stopped mode, we should be at beat ~16
        let snap = t.tick();
        assert!(
            (snap.musical_time.beat - 16.0).abs() < 0.01,
            "beat was {} expected ~16.0",
            snap.musical_time.beat
        );
        // bar = floor(16/4) + 1 = 5
        assert_eq!(snap.musical_time.bar, 5);
    }

    #[test]
    fn seek_while_playing_continues_from_new_position() {
        let mut t = Transport::new(120.0, [4, 4]);
        t.play();
        t.seek(8.0);
        assert_eq!(t.state(), TransportState::Playing);

        let snap = t.tick();
        assert!(snap.musical_time.beat >= 8.0);
    }

    #[test]
    fn stop_resets_to_zero() {
        let mut t = Transport::new(120.0, [4, 4]);
        t.play();
        t.seek(32.0);
        t.stop();

        let snap = t.tick();
        assert!(snap.musical_time.beat < 0.01);
        assert_eq!(t.state(), TransportState::Stopped);
    }

    #[test]
    fn set_tempo_changes_bpm() {
        let mut t = Transport::new(120.0, [4, 4]);
        assert!((t.tempo() - 120.0).abs() < f64::EPSILON);
        t.set_tempo(140.0);
        assert!((t.tempo() - 140.0).abs() < f64::EPSILON);
    }
}

//! Wall-clock–driven realtime musical clock.
//!
//! Unlike the offline [`MusicalClock`] whose `advance_ms` is called manually,
//! `RealtimeClock` samples `std::time::Instant` to compute elapsed time and
//! derives beat position from tempo + time signature.

use std::time::Instant;

use vidodo_ir::MusicalTime;

/// A realtime musical clock driven by `std::time::Instant`.
///
/// Call [`tick`] each frame; the clock returns an up-to-date [`MusicalTime`]
/// based on wall-clock elapsed time since the last `play`.
#[derive(Debug)]
pub struct RealtimeClock {
    tempo: f64,
    time_signature: [u32; 2],
    section: String,
    phrase: u32,

    /// Wall-clock anchor (set on `play` / `reset`).
    origin: Option<Instant>,
    /// Accumulated time from previous play segments (to support pause/resume).
    accumulated_ms: f64,
    /// State.
    state: ClockState,
}

/// Transport state for the realtime clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockState {
    Stopped,
    Playing,
    Paused,
}

/// Snapshot produced by [`RealtimeClock::tick`].
#[derive(Debug, Clone)]
pub struct TickSnapshot {
    pub musical_time: MusicalTime,
    pub elapsed_ms: f64,
    pub state: ClockState,
}

impl RealtimeClock {
    /// Create a new stopped clock at beat 0.
    pub fn new(tempo: f64, time_signature: [u32; 2]) -> Self {
        Self {
            tempo,
            time_signature,
            section: String::from("intro"),
            phrase: 1,
            origin: None,
            accumulated_ms: 0.0,
            state: ClockState::Stopped,
        }
    }

    /// Start or resume playback.
    pub fn play(&mut self) {
        match self.state {
            ClockState::Stopped | ClockState::Paused => {
                self.origin = Some(Instant::now());
                self.state = ClockState::Playing;
            }
            ClockState::Playing => {} // already playing
        }
    }

    /// Pause playback, freezing the current position.
    pub fn pause(&mut self) {
        if self.state == ClockState::Playing {
            if let Some(origin) = self.origin.take() {
                self.accumulated_ms += origin.elapsed().as_secs_f64() * 1000.0;
            }
            self.state = ClockState::Paused;
        }
    }

    /// Stop and reset to beat 0.
    pub fn stop(&mut self) {
        self.origin = None;
        self.accumulated_ms = 0.0;
        self.state = ClockState::Stopped;
    }

    /// Set tempo (BPM). Takes effect on next tick.
    pub fn set_tempo(&mut self, tempo: f64) {
        // Freeze current position at old tempo before changing
        if self.state == ClockState::Playing
            && let Some(origin) = self.origin.take()
        {
            self.accumulated_ms += origin.elapsed().as_secs_f64() * 1000.0;
            self.origin = Some(Instant::now());
        }
        self.tempo = tempo;
    }

    /// Enter a new section.
    pub fn enter_section(&mut self, section: String, phrase: u32) {
        self.section = section;
        self.phrase = phrase;
    }

    /// Sample the clock and produce a [`TickSnapshot`].
    pub fn tick(&self) -> TickSnapshot {
        let elapsed_ms = self.elapsed_ms();
        let musical_time = self.compute_musical_time(elapsed_ms);
        TickSnapshot { musical_time, elapsed_ms, state: self.state }
    }

    /// Compute elapsed wall-clock time in milliseconds.
    pub fn elapsed_ms(&self) -> f64 {
        let live = match (self.state, &self.origin) {
            (ClockState::Playing, Some(origin)) => origin.elapsed().as_secs_f64() * 1000.0,
            _ => 0.0,
        };
        self.accumulated_ms + live
    }

    /// Compute a `MusicalTime` from elapsed milliseconds and current tempo.
    fn compute_musical_time(&self, elapsed_ms: f64) -> MusicalTime {
        let beats_per_ms = self.tempo / 60_000.0;
        let total_beats = elapsed_ms * beats_per_ms;
        let beats_per_bar = self.time_signature[0] as f64;
        let bar = (total_beats / beats_per_bar).floor() as u32 + 1;
        let beat_in_bar = total_beats % beats_per_bar;

        MusicalTime {
            beat: total_beats,
            bar,
            beat_in_bar,
            phrase: self.phrase,
            section: self.section.clone(),
            tempo: self.tempo,
            time_signature: self.time_signature,
        }
    }

    pub fn state(&self) -> ClockState {
        self.state
    }

    pub fn tempo(&self) -> f64 {
        self.tempo
    }

    /// Get the time signature.
    pub fn time_signature(&self) -> [u32; 2] {
        self.time_signature
    }

    /// Set the accumulated time directly (for seek support).
    /// Only valid when the clock is stopped. Leaves clock in Paused state
    /// so the position is visible via tick().
    pub fn seek_to_ms(&mut self, ms: f64) {
        self.accumulated_ms = ms;
        if self.state == ClockState::Stopped {
            self.state = ClockState::Paused;
        }
    }
}

/// Create a `RealtimeClock` and immediately derive a snapshot at a given
/// elapsed time (for testing without wall-clock waits).
pub fn snapshot_at_ms(tempo: f64, time_signature: [u32; 2], elapsed_ms: f64) -> TickSnapshot {
    let clock = RealtimeClock::new(tempo, time_signature);
    let musical_time = clock.compute_musical_time(elapsed_ms);
    TickSnapshot { musical_time, elapsed_ms, state: ClockState::Stopped }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopped_clock_at_zero() {
        let clock = RealtimeClock::new(120.0, [4, 4]);
        let snap = clock.tick();
        assert_eq!(snap.state, ClockState::Stopped);
        assert!(snap.elapsed_ms < 1.0);
        assert_eq!(snap.musical_time.bar, 1);
    }

    #[test]
    fn play_pause_resume() {
        let mut clock = RealtimeClock::new(120.0, [4, 4]);
        clock.play();
        assert_eq!(clock.state(), ClockState::Playing);

        clock.pause();
        assert_eq!(clock.state(), ClockState::Paused);
        let paused_ms = clock.elapsed_ms();

        clock.play();
        assert_eq!(clock.state(), ClockState::Playing);
        // After resume, elapsed should be >= paused_ms
        assert!(clock.elapsed_ms() >= paused_ms);
    }

    #[test]
    fn stop_resets_position() {
        let mut clock = RealtimeClock::new(120.0, [4, 4]);
        clock.play();
        clock.pause();
        clock.stop();
        assert_eq!(clock.state(), ClockState::Stopped);
        assert!(clock.elapsed_ms() < f64::EPSILON);
    }

    #[test]
    fn snapshot_at_known_time() {
        // 120 BPM, 4/4 → 1 bar = 2 seconds = 2000ms
        let snap = snapshot_at_ms(120.0, [4, 4], 2000.0);
        assert_eq!(snap.musical_time.bar, 2); // bar 1 = 0..2000ms, bar 2 starts at 2000
        assert!((snap.musical_time.beat - 4.0).abs() < 0.01); // 4 beats in 2s at 120bpm
    }

    #[test]
    fn bar_calculation_at_various_tempos() {
        // 60 BPM, 4/4 → 1 beat = 1000ms → 1 bar = 4000ms
        let snap = snapshot_at_ms(60.0, [4, 4], 4000.0);
        assert_eq!(snap.musical_time.bar, 2);

        // 240 BPM, 4/4 → 1 beat = 250ms → 1 bar = 1000ms
        let snap = snapshot_at_ms(240.0, [4, 4], 1000.0);
        assert_eq!(snap.musical_time.bar, 2);
    }

    #[test]
    fn three_four_time() {
        // 120 BPM, 3/4 → 1 bar = 3 beats = 1500ms
        let snap = snapshot_at_ms(120.0, [3, 4], 1500.0);
        assert_eq!(snap.musical_time.bar, 2);
        assert!((snap.musical_time.beat_in_bar).abs() < 0.01); // at start of bar 2
    }

    #[test]
    fn enter_section_updates_metadata() {
        let mut clock = RealtimeClock::new(120.0, [4, 4]);
        clock.enter_section("chorus".into(), 3);
        let snap = clock.tick();
        assert_eq!(snap.musical_time.section, "chorus");
        assert_eq!(snap.musical_time.phrase, 3);
    }

    #[test]
    fn set_tempo() {
        let mut clock = RealtimeClock::new(120.0, [4, 4]);
        clock.set_tempo(180.0);
        assert!((clock.tempo() - 180.0).abs() < f64::EPSILON);
    }
}

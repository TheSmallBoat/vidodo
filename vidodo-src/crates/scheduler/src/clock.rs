use vidodo_ir::{MusicalTime, StructureSection};

/// Virtual musical clock for offline and real-time scheduling.
///
/// Steps through sections in bar order, producing a [`MusicalTime`]
/// snapshot at each position.  In offline mode the clock is advanced
/// programmatically; a future real-time mode would drive it from an
/// external timer.
#[derive(Debug, Clone)]
pub struct MusicalClock {
    tempo: f64,
    time_signature: [u32; 2],
    current_bar: u32,
    current_phrase: u32,
    current_section: String,
    scheduler_time_ms: u64,
}

impl MusicalClock {
    /// Create a clock starting at bar 1 with the given tempo and time signature.
    pub fn new(tempo: f64, time_signature: [u32; 2]) -> Self {
        Self {
            tempo,
            time_signature,
            current_bar: 1,
            current_phrase: 1,
            current_section: String::from("intro"),
            scheduler_time_ms: 0,
        }
    }

    /// Snap the clock to the beginning of a section.
    pub fn enter_section(&mut self, section: &StructureSection) {
        self.current_bar = section.span.start_bar;
        self.current_phrase = section.order as u32 + 1;
        self.current_section = section.section_id.clone();
    }

    /// Advance the scheduler wall-clock by `ms` milliseconds.
    pub fn advance_ms(&mut self, ms: u64) {
        self.scheduler_time_ms += ms;
    }

    /// Current virtual wall-clock time in milliseconds.
    pub fn time_ms(&self) -> u64 {
        self.scheduler_time_ms
    }

    /// Current tempo in BPM.
    pub fn tempo(&self) -> f64 {
        self.tempo
    }

    /// Produce a [`MusicalTime`] snapshot at the current position.
    pub fn musical_time(&self) -> MusicalTime {
        MusicalTime::at_bar(
            self.current_bar,
            self.current_phrase,
            self.current_section.clone(),
            self.tempo,
        )
    }

    /// Produce a [`MusicalTime`] snapshot at an arbitrary bar within
    /// the current section/phrase.
    pub fn musical_time_at_bar(&self, bar: u32) -> MusicalTime {
        MusicalTime::at_bar(bar, self.current_phrase, self.current_section.clone(), self.tempo)
    }

    pub fn current_bar(&self) -> u32 {
        self.current_bar
    }

    pub fn current_section(&self) -> &str {
        &self.current_section
    }

    pub fn current_phrase(&self) -> u32 {
        self.current_phrase
    }

    pub fn time_signature(&self) -> [u32; 2] {
        self.time_signature
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::{StructureSection, StructureSpan};

    #[test]
    fn advances_through_sections() {
        let mut clock = MusicalClock::new(128.0, [4, 4]);
        assert_eq!(clock.current_bar(), 1);
        assert_eq!(clock.current_section(), "intro");

        let section = StructureSection {
            section_id: String::from("build"),
            order: 1,
            span: StructureSpan { start_bar: 9, end_bar: 16 },
            targets: std::collections::BTreeMap::new(),
            locks: std::collections::BTreeMap::new(),
        };
        clock.enter_section(&section);
        assert_eq!(clock.current_bar(), 9);
        assert_eq!(clock.current_section(), "build");
        assert_eq!(clock.current_phrase(), 2);
    }

    #[test]
    fn musical_time_snapshot_reflects_state() {
        let mut clock = MusicalClock::new(120.0, [4, 4]);
        clock.advance_ms(5000);
        let mt = clock.musical_time();
        assert_eq!(mt.bar, 1);
        assert_eq!(mt.tempo, 120.0);
        assert_eq!(clock.time_ms(), 5000);
    }

    #[test]
    fn virtualizable_offline_clock() {
        let mut clock = MusicalClock::new(128.0, [4, 4]);
        // Simulate rapid offline advancement — no wall-clock dependency
        for _ in 0..100 {
            clock.advance_ms(1_000);
        }
        assert_eq!(clock.time_ms(), 100_000);
    }
}

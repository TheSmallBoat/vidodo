//! Keyframe-based uniform automation engine.
//!
//! Drives shader uniform parameters over time by interpolating between
//! keyframes placed on beat positions. Each beat-normalised progress
//! value produces an interpolated `f32` output.

/// A single keyframe: beat position + value.
#[derive(Debug, Clone, Copy)]
pub struct Keyframe {
    /// Beat position (0-based).
    pub beat: f64,
    /// Scalar value at this beat.
    pub value: f32,
}

/// Interpolation mode used between keyframes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpolationMode {
    /// Linear interpolation (default).
    Linear,
    /// Step (hold previous value until the next keyframe).
    Step,
}

/// Automation track for a single uniform parameter.
#[derive(Debug, Clone)]
pub struct UniformAutomation {
    /// Name of the uniform being automated (e.g. "particle_color").
    pub uniform_name: String,
    /// Sorted keyframes (by beat).
    keyframes: Vec<Keyframe>,
    /// Interpolation mode.
    mode: InterpolationMode,
}

impl UniformAutomation {
    /// Create a new automation track for the named uniform.
    pub fn new(uniform_name: impl Into<String>, mode: InterpolationMode) -> Self {
        Self { uniform_name: uniform_name.into(), keyframes: Vec::new(), mode }
    }

    /// Add a keyframe (auto-sorted by beat).
    pub fn add_keyframe(&mut self, beat: f64, value: f32) {
        let kf = Keyframe { beat, value };
        let pos = self.keyframes.iter().position(|k| k.beat > beat).unwrap_or(self.keyframes.len());
        self.keyframes.insert(pos, kf);
    }

    /// Evaluate the automation at a given beat position.
    ///
    /// - Before the first keyframe: returns first keyframe value.
    /// - After the last keyframe: returns last keyframe value.
    /// - Between keyframes: interpolates according to the mode.
    pub fn evaluate(&self, beat: f64) -> f32 {
        if self.keyframes.is_empty() {
            return 0.0;
        }
        if self.keyframes.len() == 1 || beat <= self.keyframes[0].beat {
            return self.keyframes[0].value;
        }
        let last = &self.keyframes[self.keyframes.len() - 1];
        if beat >= last.beat {
            return last.value;
        }

        // Find the two surrounding keyframes
        let right_idx =
            self.keyframes.iter().position(|k| k.beat >= beat).unwrap_or(self.keyframes.len() - 1);
        let left_idx = if right_idx > 0 { right_idx - 1 } else { 0 };
        let left = &self.keyframes[left_idx];
        let right = &self.keyframes[right_idx];

        match self.mode {
            InterpolationMode::Step => left.value,
            InterpolationMode::Linear => {
                let span = right.beat - left.beat;
                if span <= 0.0 {
                    return left.value;
                }
                let t = ((beat - left.beat) / span) as f32;
                left.value + t * (right.value - left.value)
            }
        }
    }

    /// Number of keyframes.
    pub fn keyframe_count(&self) -> usize {
        self.keyframes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_interpolation_midpoint() {
        let mut auto = UniformAutomation::new("brightness", InterpolationMode::Linear);
        auto.add_keyframe(0.0, 0.0);
        auto.add_keyframe(16.0, 1.0);
        let val = auto.evaluate(8.0);
        assert!((val - 0.5).abs() < 0.001, "expected ~0.5, got {val}");
    }

    #[test]
    fn step_holds_previous_value() {
        let mut auto = UniformAutomation::new("color_r", InterpolationMode::Step);
        auto.add_keyframe(0.0, 0.0);
        auto.add_keyframe(8.0, 1.0);
        assert!((auto.evaluate(4.0) - 0.0).abs() < f32::EPSILON);
        assert!((auto.evaluate(8.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn before_first_keyframe_returns_first() {
        let mut auto = UniformAutomation::new("x", InterpolationMode::Linear);
        auto.add_keyframe(4.0, 0.5);
        auto.add_keyframe(8.0, 1.0);
        assert!((auto.evaluate(0.0) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn after_last_keyframe_returns_last() {
        let mut auto = UniformAutomation::new("x", InterpolationMode::Linear);
        auto.add_keyframe(0.0, 0.0);
        auto.add_keyframe(8.0, 0.8);
        assert!((auto.evaluate(100.0) - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn particle_color_16_beat_smooth() {
        // Acceptance: 16-beat linear ramp from 0.0→1.0 should be smooth
        let mut auto = UniformAutomation::new("particle_color", InterpolationMode::Linear);
        auto.add_keyframe(0.0, 0.0);
        auto.add_keyframe(16.0, 1.0);

        for beat in 0..=16 {
            let expected = beat as f32 / 16.0;
            let actual = auto.evaluate(beat as f64);
            assert!(
                (actual - expected).abs() < 0.001,
                "beat {beat}: expected {expected}, got {actual}"
            );
        }
    }
}

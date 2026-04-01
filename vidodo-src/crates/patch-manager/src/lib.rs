use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchWindow {
    pub current_bar: u32,
    pub window_end: u32,
}

pub fn can_apply_patch(window: &PatchWindow, target_bar: u32) -> bool {
    target_bar >= window.current_bar && target_bar <= window.window_end
}

#[cfg(test)]
mod tests {
    use super::{PatchWindow, can_apply_patch};

    #[test]
    fn allows_target_inside_window() {
        let window = PatchWindow { current_bar: 8, window_end: 16 };

        assert!(can_apply_patch(&window, 12));
        assert!(!can_apply_patch(&window, 20));
    }
}

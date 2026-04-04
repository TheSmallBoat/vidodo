use std::collections::BTreeMap;

use vidodo_ir::SceneDescriptor;

/// Tracks active scenes and handles transitions between them.
#[derive(Debug, Default)]
pub struct SceneManager {
    active_scenes: BTreeMap<String, ActiveScene>,
}

/// A scene that is currently active in the runtime.
#[derive(Debug, Clone)]
pub struct ActiveScene {
    pub descriptor: SceneDescriptor,
    pub transition_progress: f64,
}

/// Event emitted when a scene transition occurs.
#[derive(Debug, Clone)]
pub struct SceneTransitionEvent {
    pub from_scene: Option<String>,
    pub to_scene: String,
    pub progress: f64,
    pub kind: String,
}

impl SceneManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Activate a scene. Returns a SceneTransitionEvent.
    pub fn activate_scene(&mut self, descriptor: SceneDescriptor) -> SceneTransitionEvent {
        let scene_id = descriptor.scene_id.clone();
        let event = SceneTransitionEvent {
            from_scene: None,
            to_scene: scene_id.clone(),
            progress: 1.0,
            kind: String::from("activate"),
        };
        self.active_scenes.insert(scene_id, ActiveScene { descriptor, transition_progress: 1.0 });
        event
    }

    /// List currently active scene ids.
    pub fn list_active_scenes(&self) -> Vec<String> {
        self.active_scenes.keys().cloned().collect()
    }

    /// Transition from one active scene to another with cross-fade progress.
    /// Returns a SceneTransitionEvent or an error if the target scene is unknown.
    pub fn transition(
        &mut self,
        from_scene_id: &str,
        to_descriptor: SceneDescriptor,
        progress: f64,
    ) -> Result<SceneTransitionEvent, String> {
        if !self.active_scenes.contains_key(from_scene_id) {
            return Err(format!("scene '{from_scene_id}' is not active"));
        }
        let to_id = to_descriptor.scene_id.clone();
        let clamped = progress.clamp(0.0, 1.0);

        // Update "from" scene progress inversely
        if let Some(from) = self.active_scenes.get_mut(from_scene_id) {
            from.transition_progress = 1.0 - clamped;
        }

        // Insert or update "to" scene
        self.active_scenes
            .entry(to_id.clone())
            .and_modify(|s| s.transition_progress = clamped)
            .or_insert_with(|| ActiveScene {
                descriptor: to_descriptor,
                transition_progress: clamped,
            });

        // If fully transitioned, remove the old scene
        if clamped >= 1.0 {
            self.active_scenes.remove(from_scene_id);
        }

        Ok(SceneTransitionEvent {
            from_scene: Some(from_scene_id.to_string()),
            to_scene: to_id,
            progress: clamped,
            kind: String::from("transition"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_scene(id: &str) -> SceneDescriptor {
        SceneDescriptor {
            scene_id: id.to_string(),
            label: format!("Scene {id}"),
            asset_refs: vec![],
            visual_program_ref: None,
            energy_range: None,
            tags: vec![],
        }
    }

    #[test]
    fn activate_scene_adds_to_active() {
        let mut mgr = SceneManager::new();
        let event = mgr.activate_scene(test_scene("scene_a"));
        assert_eq!(event.to_scene, "scene_a");
        assert_eq!(event.kind, "activate");
        assert_eq!(event.progress, 1.0);
        assert!(event.from_scene.is_none());
        assert_eq!(mgr.list_active_scenes(), vec!["scene_a"]);
    }

    #[test]
    fn list_active_scenes_returns_all() {
        let mut mgr = SceneManager::new();
        mgr.activate_scene(test_scene("scene_a"));
        mgr.activate_scene(test_scene("scene_b"));
        let mut scenes = mgr.list_active_scenes();
        scenes.sort();
        assert_eq!(scenes, vec!["scene_a", "scene_b"]);
    }

    #[test]
    fn transition_replaces_scene_at_full_progress() {
        let mut mgr = SceneManager::new();
        mgr.activate_scene(test_scene("scene_a"));
        let event = mgr.transition("scene_a", test_scene("scene_b"), 1.0).unwrap();
        assert_eq!(event.from_scene.as_deref(), Some("scene_a"));
        assert_eq!(event.to_scene, "scene_b");
        assert_eq!(event.kind, "transition");
        let scenes = mgr.list_active_scenes();
        assert_eq!(scenes, vec!["scene_b"]);
    }

    #[test]
    fn transition_partial_keeps_both_scenes() {
        let mut mgr = SceneManager::new();
        mgr.activate_scene(test_scene("scene_a"));
        let event = mgr.transition("scene_a", test_scene("scene_b"), 0.5).unwrap();
        assert_eq!(event.progress, 0.5);
        let mut scenes = mgr.list_active_scenes();
        scenes.sort();
        assert_eq!(scenes, vec!["scene_a", "scene_b"]);
    }

    #[test]
    fn transition_from_unknown_scene_fails() {
        let mut mgr = SceneManager::new();
        let result = mgr.transition("nonexistent", test_scene("scene_b"), 1.0);
        assert!(result.is_err());
    }
}

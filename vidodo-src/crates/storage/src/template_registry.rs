use std::collections::BTreeMap;

use vidodo_ir::{SceneDescriptor, ScenePack, ShowTemplate};

use crate::artifact_layout::ArtifactLayout;
use crate::{read_json, write_json};

/// In-memory registry of show templates and scene packs.
#[derive(Debug, Default)]
pub struct TemplateRegistry {
    templates: BTreeMap<String, ShowTemplate>,
    scene_packs: BTreeMap<String, ScenePack>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a show template. Returns `Err` if template_id is already registered.
    pub fn register_template(&mut self, template: ShowTemplate) -> Result<(), String> {
        if self.templates.contains_key(&template.template_id) {
            return Err(format!("template '{}' is already registered", template.template_id));
        }
        self.templates.insert(template.template_id.clone(), template);
        Ok(())
    }

    /// List all registered templates.
    pub fn list_templates(&self) -> Vec<&ShowTemplate> {
        self.templates.values().collect()
    }

    /// Load a template by id.
    pub fn load_template(&self, template_id: &str) -> Result<&ShowTemplate, String> {
        self.templates.get(template_id).ok_or_else(|| format!("template '{template_id}' not found"))
    }

    /// Register a scene pack. Returns `Err` if pack_id is already registered.
    pub fn register_scene_pack(&mut self, pack: ScenePack) -> Result<(), String> {
        if self.scene_packs.contains_key(&pack.pack_id) {
            return Err(format!("scene pack '{}' is already registered", pack.pack_id));
        }
        self.scene_packs.insert(pack.pack_id.clone(), pack);
        Ok(())
    }

    /// Resolve a scene pack by pack_id. Returns all scene descriptors.
    pub fn resolve_scene_pack(&self, pack_id: &str) -> Result<&ScenePack, String> {
        self.scene_packs.get(pack_id).ok_or_else(|| format!("scene pack '{pack_id}' not found"))
    }

    /// Resolve a specific scene within a pack.
    pub fn resolve_scene(&self, pack_id: &str, scene_id: &str) -> Result<&SceneDescriptor, String> {
        let pack = self.resolve_scene_pack(pack_id)?;
        pack.scenes
            .iter()
            .find(|s| s.scene_id == scene_id)
            .ok_or_else(|| format!("scene '{scene_id}' not found in pack '{pack_id}'"))
    }
}

/// Persist a template to disk under `artifacts/templates/{template_id}.json`.
pub fn save_template(layout: &ArtifactLayout, template: &ShowTemplate) -> Result<(), String> {
    layout.ensure()?;
    let dir = layout.root.join("templates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir templates: {e}"))?;
    let path = dir.join(format!("{}.json", template.template_id));
    write_json(&path, template)
}

/// Load a template from disk.
pub fn load_template_from_disk(
    layout: &ArtifactLayout,
    template_id: &str,
) -> Result<ShowTemplate, String> {
    let path = layout.root.join("templates").join(format!("{template_id}.json"));
    read_json(&path)
}

/// Persist a scene pack to disk under `artifacts/scene-packs/{pack_id}.json`.
pub fn save_scene_pack(layout: &ArtifactLayout, pack: &ScenePack) -> Result<(), String> {
    layout.ensure()?;
    let dir = layout.root.join("scene-packs");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir scene-packs: {e}"))?;
    let path = dir.join(format!("{}.json", pack.pack_id));
    write_json(&path, pack)
}

/// Load a scene pack from disk.
pub fn load_scene_pack_from_disk(
    layout: &ArtifactLayout,
    pack_id: &str,
) -> Result<ScenePack, String> {
    let path = layout.root.join("scene-packs").join(format!("{pack_id}.json"));
    read_json(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::{
        SceneDescriptor, ScenePack, ShowTemplate, TemplateDefaultParams, TemplateSectionRef,
        TransitionStrategy,
    };

    fn sample_template(id: &str) -> ShowTemplate {
        ShowTemplate {
            template_type: String::from("show_template"),
            template_id: id.to_string(),
            name: format!("Template {id}"),
            description: None,
            mode: String::from("live"),
            sections: vec![TemplateSectionRef {
                section_id: String::from("intro"),
                order: 0,
                label: String::from("Intro"),
                duration_bars: Some(8),
                scene_pack_ref: None,
                energy_target: None,
            }],
            default_params: TemplateDefaultParams {
                tempo_bpm: 128.0,
                time_signature: [4, 4],
                style_tags: vec![],
            },
            scene_pack_refs: vec![],
            tags: vec![],
        }
    }

    fn sample_scene_pack(id: &str) -> ScenePack {
        ScenePack {
            pack_type: String::from("scene_pack"),
            pack_id: id.to_string(),
            name: format!("Pack {id}"),
            description: None,
            scenes: vec![SceneDescriptor {
                scene_id: String::from("scene-a"),
                label: String::from("Scene A"),
                asset_refs: vec![String::from("audio.loop.pad-a")],
                visual_program_ref: None,
                energy_range: None,
                tags: vec![],
            }],
            transition_strategy: Some(TransitionStrategy {
                default_mode: String::from("crossfade"),
                crossfade_beats: Some(4.0),
            }),
            tags: vec![],
        }
    }

    #[test]
    fn register_and_list_templates() {
        let mut reg = TemplateRegistry::new();
        reg.register_template(sample_template("tpl-1")).unwrap();
        reg.register_template(sample_template("tpl-2")).unwrap();
        assert_eq!(reg.list_templates().len(), 2);
    }

    #[test]
    fn duplicate_template_registration_fails() {
        let mut reg = TemplateRegistry::new();
        reg.register_template(sample_template("tpl-dup")).unwrap();
        assert!(reg.register_template(sample_template("tpl-dup")).is_err());
    }

    #[test]
    fn load_template_by_id() {
        let mut reg = TemplateRegistry::new();
        reg.register_template(sample_template("tpl-1")).unwrap();
        let tpl = reg.load_template("tpl-1").unwrap();
        assert_eq!(tpl.name, "Template tpl-1");
    }

    #[test]
    fn load_unknown_template_fails() {
        let reg = TemplateRegistry::new();
        assert!(reg.load_template("nonexistent").is_err());
    }

    #[test]
    fn register_and_resolve_scene_pack() {
        let mut reg = TemplateRegistry::new();
        reg.register_scene_pack(sample_scene_pack("pack-1")).unwrap();
        let pack = reg.resolve_scene_pack("pack-1").unwrap();
        assert_eq!(pack.scenes.len(), 1);
        assert_eq!(pack.scenes[0].scene_id, "scene-a");
    }

    #[test]
    fn resolve_specific_scene() {
        let mut reg = TemplateRegistry::new();
        reg.register_scene_pack(sample_scene_pack("pack-1")).unwrap();
        let scene = reg.resolve_scene("pack-1", "scene-a").unwrap();
        assert_eq!(scene.label, "Scene A");
    }

    #[test]
    fn resolve_unknown_scene_fails() {
        let mut reg = TemplateRegistry::new();
        reg.register_scene_pack(sample_scene_pack("pack-1")).unwrap();
        assert!(reg.resolve_scene("pack-1", "no-such-scene").is_err());
    }
}

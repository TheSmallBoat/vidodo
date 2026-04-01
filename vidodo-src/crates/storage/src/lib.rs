use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactLayout {
    pub root: PathBuf,
    pub assets: PathBuf,
    pub analysis: PathBuf,
    pub revisions: PathBuf,
    pub traces: PathBuf,
    pub exports: PathBuf,
    pub registry: PathBuf,
}

impl ArtifactLayout {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        Self {
            assets: root.join("assets"),
            analysis: root.join("analysis"),
            revisions: root.join("revisions"),
            traces: root.join("traces"),
            exports: root.join("exports"),
            registry: root.join("registry.db"),
            root,
        }
    }

    pub fn discover() -> Result<Self, String> {
        let repo_root = discover_repo_root()?;
        Ok(Self::new(repo_root.join("artifacts")))
    }

    pub fn ensure(&self) -> Result<(), String> {
        for directory in
            [&self.root, &self.assets, &self.analysis, &self.revisions, &self.traces, &self.exports]
        {
            fs::create_dir_all(directory)
                .map_err(|error| format!("failed to create {}: {error}", directory.display()))?;
        }

        if !self.registry.exists() {
            File::create(&self.registry).map_err(|error| {
                format!("failed to create {}: {error}", self.registry.display())
            })?;
        }

        Ok(())
    }

    pub fn revision_dir(&self, show_id: &str, revision: u64) -> PathBuf {
        self.revisions.join(slug(show_id)).join(format!("revision-{revision}"))
    }

    pub fn trace_dir(&self, run_id: &str) -> PathBuf {
        self.traces.join(slug(run_id))
    }

    pub fn run_status_path(&self, show_id: &str) -> PathBuf {
        self.traces.join(format!("{}-status.json", slug(show_id)))
    }
}

pub fn discover_repo_root() -> Result<PathBuf, String> {
    let current_dir =
        env::current_dir().map_err(|error| format!("failed to read current dir: {error}"))?;
    repo_root_from(&current_dir)
}

pub fn repo_root_from(start: &Path) -> Result<PathBuf, String> {
    let mut cursor = Some(start);
    while let Some(candidate) = cursor {
        if candidate.join(".git").exists() && candidate.join("vidodo-src").exists() {
            return Ok(candidate.to_path_buf());
        }
        cursor = candidate.parent();
    }

    Err(format!("failed to discover repo root from {}", start.display()))
}

pub fn write_json<T>(path: &Path, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    let content = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    fs::write(path, format!("{content}\n"))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub fn read_json<T>(path: &Path) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let content = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("failed to deserialize {}: {error}", path.display()))
}

pub fn write_jsonl<T>(path: &Path, values: &[T]) -> Result<(), String>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    let mut file = File::create(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    for value in values {
        let line = serde_json::to_string(value).map_err(|error| {
            format!("failed to serialize JSONL record for {}: {error}", path.display())
        })?;
        writeln!(file, "{line}")
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }
    Ok(())
}

pub fn read_jsonl<T>(path: &Path) -> Result<Vec<T>, String>
where
    T: DeserializeOwned,
{
    let file =
        File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let reader = BufReader::new(file);
    let mut values = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        values.push(serde_json::from_str(&line).map_err(|error| {
            format!("failed to parse JSONL record in {}: {error}", path.display())
        })?);
    }
    Ok(values)
}

pub fn slug(input: &str) -> String {
    input
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => character,
            _ => '-',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ArtifactLayout, slug};

    #[test]
    fn builds_expected_subdirectories() {
        let layout = ArtifactLayout::new("artifacts");

        assert!(layout.traces.ends_with("artifacts/traces"));
        assert!(layout.exports.ends_with("artifacts/exports"));
        assert!(layout.revisions.ends_with("artifacts/revisions"));
    }

    #[test]
    fn slugs_unknown_characters() {
        assert_eq!(slug("show/phase0 demo"), "show-phase0-demo");
    }
}

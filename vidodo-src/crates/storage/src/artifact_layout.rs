//! Layout, path discovery, and persistence primitives for the artifact store.

use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use vidodo_ir::{AssetRecord, Diagnostic};

// ---------------------------------------------------------------------------
// ArtifactLayout
// ---------------------------------------------------------------------------

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
        for directory in [
            &self.root,
            &self.assets,
            &self.asset_raw_dir(),
            &self.asset_normalized_dir(),
            &self.asset_registry_dir(),
            &self.analysis,
            &self.analysis_cache_dir(),
            &self.analysis_reports_dir(),
            &self.revisions,
            &self.traces,
            &self.exports,
        ] {
            fs::create_dir_all(directory)
                .map_err(|error| format!("failed to create {}: {error}", directory.display()))?;
        }

        connect_registry(&self.registry)?;
        Ok(())
    }

    pub fn asset_raw_dir(&self) -> PathBuf {
        self.assets.join("raw")
    }

    pub fn asset_normalized_dir(&self) -> PathBuf {
        self.assets.join("normalized")
    }

    pub fn asset_registry_dir(&self) -> PathBuf {
        self.assets.join("registry")
    }

    pub fn asset_registry_file(&self) -> PathBuf {
        self.asset_registry_dir().join("asset-records.json")
    }

    pub fn ingestion_report_path(&self, run_id: &str) -> PathBuf {
        self.asset_registry_dir().join(format!("{run_id}.json"))
    }

    pub fn analysis_cache_dir(&self) -> PathBuf {
        self.analysis.join("cache")
    }

    pub fn analysis_reports_dir(&self) -> PathBuf {
        self.analysis.join("reports")
    }

    pub fn analysis_entry_path(&self, cache_key: &str) -> PathBuf {
        self.analysis_cache_dir().join(format!("{}.json", slug(cache_key)))
    }

    pub fn analysis_job_path(&self, job_id: &str) -> PathBuf {
        self.analysis_reports_dir().join(format!("{job_id}.json"))
    }

    pub fn analysis_payload_path(&self, result_ref: &str) -> PathBuf {
        self.analysis_reports_dir().join(format!("{result_ref}.json"))
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

// ---------------------------------------------------------------------------
// Repo discovery
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// JSON I/O
// ---------------------------------------------------------------------------

pub fn write_json<T>(path: &Path, value: &T) -> Result<(), String>
where
    T: Serialize + ?Sized,
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

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

pub fn slug(input: &str) -> String {
    input
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => character,
            _ => '-',
        })
        .collect()
}

pub(crate) fn timestamp_now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}", duration.as_secs()))
        .unwrap_or_else(|_| String::from("0"))
}

pub(crate) fn unix_timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_secs()).unwrap_or(0)
}

pub(crate) fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

pub(crate) fn hash_string(value: &str) -> String {
    content_hash(value.as_bytes())
}

pub(crate) fn short_hash(value: &str) -> String {
    if let Some(digest) = value.strip_prefix("sha256:") {
        return digest.chars().take(8).collect();
    }

    hash_string(value).trim_start_matches("sha256:").chars().take(8).collect()
}

pub(crate) fn artifact_ref(layout: &ArtifactLayout, path: &Path) -> String {
    let repo_root = layout.root.parent().unwrap_or(&layout.root);
    path.strip_prefix(repo_root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

pub(crate) fn diagnostic_from_string(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::error("AST-000", message)]
}

pub(crate) fn dbfs_tenths(level_ratio: f64) -> i32 {
    if level_ratio <= 0.000_000_1 {
        return -1200;
    }

    (20.0 * level_ratio.log10() * 10.0).round() as i32
}

// ---------------------------------------------------------------------------
// SQLite registry
// ---------------------------------------------------------------------------

pub(crate) fn connect_registry(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    let connection = Connection::open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS assets (
              asset_id TEXT PRIMARY KEY,
              asset_kind TEXT NOT NULL,
              content_hash TEXT NOT NULL,
              status TEXT NOT NULL,
              warm_status TEXT,
              readiness TEXT
            );

            CREATE TABLE IF NOT EXISTS asset_tags (
              asset_id TEXT NOT NULL,
              tag TEXT NOT NULL,
              PRIMARY KEY (asset_id, tag)
            );

            CREATE TABLE IF NOT EXISTS ingestion_runs (
              ingestion_run_id TEXT PRIMARY KEY,
              source TEXT NOT NULL,
              status TEXT NOT NULL,
              discovered INTEGER NOT NULL DEFAULT 0,
              published INTEGER NOT NULL DEFAULT 0,
              reused INTEGER NOT NULL DEFAULT 0,
              failed INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS analysis_jobs (
              analysis_job_id TEXT PRIMARY KEY,
              asset_id TEXT NOT NULL,
              analyzer TEXT NOT NULL,
              status TEXT NOT NULL,
              cache_key TEXT,
              result_ref TEXT
            );

            CREATE TABLE IF NOT EXISTS analysis_entries (
              cache_key TEXT PRIMARY KEY,
              asset_id TEXT NOT NULL,
              analyzer TEXT NOT NULL,
              status TEXT NOT NULL,
              payload_ref TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS revisions (
              show_id TEXT NOT NULL,
              revision INTEGER NOT NULL,
              status TEXT NOT NULL DEFAULT 'candidate',
              compile_run_id TEXT NOT NULL,
              artifact_ref TEXT NOT NULL,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              PRIMARY KEY (show_id, revision)
            );
            ",
        )
        .map_err(|error| format!("failed to initialize registry schema: {error}"))?;
    Ok(connection)
}

// ---------------------------------------------------------------------------
// Asset record persistence (shared by ingest + query)
// ---------------------------------------------------------------------------

pub(crate) fn load_asset_records(layout: &ArtifactLayout) -> Result<Vec<AssetRecord>, String> {
    let path = layout.asset_registry_file();
    if !path.exists() {
        return Ok(Vec::new());
    }

    read_json(&path)
}

pub(crate) fn save_asset_records(
    layout: &ArtifactLayout,
    records: &[AssetRecord],
) -> Result<(), String> {
    write_json(&layout.asset_registry_file(), records)
}

pub(crate) fn upsert_asset(layout: &ArtifactLayout, record: &AssetRecord) -> Result<(), String> {
    let connection = connect_registry(&layout.registry)?;
    connection
        .execute(
            "
            INSERT INTO assets (asset_id, asset_kind, content_hash, status, warm_status, readiness)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(asset_id) DO UPDATE SET
              asset_kind = excluded.asset_kind,
              content_hash = excluded.content_hash,
              status = excluded.status,
              warm_status = excluded.warm_status,
              readiness = excluded.readiness
            ",
            params![
                record.asset_id,
                record.asset_kind,
                record.content_hash,
                record.status,
                record.warm_status,
                record.readiness,
            ],
        )
        .map_err(|error| format!("failed to upsert asset {}: {error}", record.asset_id))?;

    connection
        .execute("DELETE FROM asset_tags WHERE asset_id = ?1", params![record.asset_id])
        .map_err(|error| format!("failed to clear tags for {}: {error}", record.asset_id))?;
    for tag in &record.tags {
        connection
            .execute(
                "INSERT INTO asset_tags (asset_id, tag) VALUES (?1, ?2)",
                params![record.asset_id, tag],
            )
            .map_err(|error| {
                format!("failed to write tag {} for {}: {error}", tag, record.asset_id)
            })?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_expected_subdirectories() {
        let layout = ArtifactLayout::new("artifacts");

        assert!(layout.traces.ends_with("artifacts/traces"));
        assert!(layout.exports.ends_with("artifacts/exports"));
        assert!(layout.revisions.ends_with("artifacts/revisions"));
        assert!(layout.asset_raw_dir().ends_with("artifacts/assets/raw"));
    }

    #[test]
    fn slugs_unknown_characters() {
        assert_eq!(slug("show/phase0 demo"), "show-phase0-demo");
    }
}

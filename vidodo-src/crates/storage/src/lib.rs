use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use vidodo_ir::{
    AnalysisCacheEntry, AnalysisJob, AssetIngestReport, AssetRecord, AudioProbeSummary,
    BeatTrackAnalysis, Diagnostic, IngestionCandidate, IngestionRun,
};

const BEAT_TRACK_ANALYZER: &str = "beat_track";
const BEAT_TRACK_ANALYZER_VERSION: &str = "0.1.0";
const BEAT_TRACK_PARAMS: &str = "probe=v1;window_ms=10;min_gap_ms=160;supported=wav/pcm_s16le";
const ASSET_PACK_MANIFEST_FILE: &str = "vidodo-asset-pack.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct AssetPackManifest {
    #[serde(default)]
    asset_namespace: Option<String>,
    #[serde(default)]
    asset_id_overrides: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WavFormatSummary {
    sample_rate_hz: u32,
    channel_count: u16,
    bits_per_sample: u16,
    block_align: u16,
}

#[derive(Debug, Clone, PartialEq)]
struct ProbedAudio {
    summary: AudioProbeSummary,
    mono_samples: Vec<f32>,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetIngestRequest {
    pub source: PathBuf,
    pub declared_kind: String,
    pub tags: Vec<String>,
    pub asset_namespace: Option<String>,
    pub asset_id_overrides: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssetQuery {
    pub asset_kind: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileAssetSelection {
    pub eligible_assets: Vec<AssetRecord>,
    pub published_asset_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StagedCandidate {
    candidate: IngestionCandidate,
    source_path: PathBuf,
    relative_path: String,
    file_name: String,
    extension: String,
    bytes: Vec<u8>,
    content_hash: String,
    asset_id: String,
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

pub fn ingest_assets(
    layout: &ArtifactLayout,
    request: &AssetIngestRequest,
) -> Result<AssetIngestReport, Vec<Diagnostic>> {
    layout.ensure().map_err(diagnostic_from_string)?;

    if !request.source.exists() || !request.source.is_dir() {
        return Err(vec![Diagnostic::error(
            "AST-001",
            format!("asset source directory does not exist: {}", request.source.display()),
        )]);
    }

    let candidates =
        scan_candidates(&request.source, &request.declared_kind).map_err(diagnostic_from_string)?;
    if candidates.is_empty() {
        return Err(vec![Diagnostic::error(
            "AST-002",
            format!("no asset candidates found under {}", request.source.display()),
        )]);
    }

    let started_at = unix_timestamp();
    let run_id = format!(
        "ing-{}-{}",
        slug(request.source.file_name().and_then(|name| name.to_str()).unwrap_or("source")),
        started_at
    );

    let mut records = load_asset_records(layout).map_err(diagnostic_from_string)?;
    let staged_candidates = stage_candidates(candidates, request)?;
    let naming_diagnostics = validate_asset_naming_policy(&staged_candidates, &records);
    if !naming_diagnostics.is_empty() {
        return Err(naming_diagnostics);
    }

    let candidates =
        staged_candidates.iter().map(|staged| staged.candidate.clone()).collect::<Vec<_>>();
    let mut record_by_identity = BTreeMap::new();
    for record in &records {
        record_by_identity.insert(
            asset_identity_key(&record.asset_kind, &record.asset_id, &record.content_hash),
            record.clone(),
        );
    }

    let mut published = Vec::new();
    let mut analysis_jobs = Vec::new();
    let mut analysis_entries = Vec::new();
    let mut reused = 0_u32;

    for staged in &staged_candidates {
        let candidate_path = &staged.source_path;
        let identity_key =
            asset_identity_key(&request.declared_kind, &staged.asset_id, &staged.content_hash);

        if let Some(existing) = record_by_identity.get(&identity_key) {
            reused = reused.saturating_add(1);
            let merged = merge_asset_tags(existing, &request.tags);
            upsert_asset(layout, &merged).map_err(diagnostic_from_string)?;
            store_asset_record(&mut records, merged.clone());
            published.push(merged);
            continue;
        }

        let raw_path = layout.asset_raw_dir().join(&staged.asset_id).join(&staged.file_name);
        if let Some(parent) = raw_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                vec![Diagnostic::error(
                    "AST-004",
                    format!("failed to create {}: {error}", parent.display()),
                )]
            })?;
        }
        fs::write(&raw_path, &staged.bytes).map_err(|error| {
            vec![Diagnostic::error(
                "AST-005",
                format!(
                    "failed to stage {} into {}: {error}",
                    candidate_path.display(),
                    raw_path.display()
                ),
            )]
        })?;

        let normalized_path = layout.asset_normalized_dir().join(format!(
            "{}.{}",
            slug(&staged.asset_id),
            staged.extension
        ));
        fs::write(&normalized_path, &staged.bytes).map_err(|error| {
            vec![Diagnostic::error(
                "AST-006",
                format!(
                    "failed to normalize {} into {}: {error}",
                    candidate_path.display(),
                    normalized_path.display()
                ),
            )]
        })?;

        let payload = build_beat_track_analysis(
            &staged.asset_id,
            &normalized_path,
            staged.bytes.len() as u64,
        )
        .map_err(|message| vec![Diagnostic::error("AST-007", message)])?;
        let params_hash = hash_string(BEAT_TRACK_PARAMS);
        let cache_key = build_cache_key(
            &staged.asset_id,
            &staged.content_hash,
            &request.declared_kind,
            &payload.probe,
            &params_hash,
        );
        let result_ref = format!("analysis-{}", short_hash(&cache_key));
        let payload_path = layout.analysis_payload_path(&result_ref);
        write_json(&payload_path, &payload).map_err(diagnostic_from_string)?;

        let entry = AnalysisCacheEntry {
            cache_key: cache_key.clone(),
            asset_id: staged.asset_id.clone(),
            analyzer: String::from(BEAT_TRACK_ANALYZER),
            analyzer_version: String::from(BEAT_TRACK_ANALYZER_VERSION),
            input_fingerprint: staged.content_hash.clone(),
            dependency_fingerprint: probe_fingerprint(&payload.probe),
            created_at: unix_timestamp(),
            status: String::from("ready"),
            payload_ref: artifact_ref(layout, &payload_path),
        };
        write_json(&layout.analysis_entry_path(&cache_key), &entry)
            .map_err(diagnostic_from_string)?;

        let job = AnalysisJob {
            analysis_job_id: format!(
                "job-{}",
                short_hash(&format!("{}:{cache_key}", staged.asset_id))
            ),
            asset_id: staged.asset_id.clone(),
            analyzer: String::from(BEAT_TRACK_ANALYZER),
            analyzer_version: String::from(BEAT_TRACK_ANALYZER_VERSION),
            params_hash,
            status: String::from("completed"),
            cache_key: cache_key.clone(),
            result_ref: result_ref.clone(),
        };
        write_json(&layout.analysis_job_path(&job.analysis_job_id), &job)
            .map_err(diagnostic_from_string)?;

        let record = AssetRecord {
            asset_id: staged.asset_id.clone(),
            asset_kind: request.declared_kind.clone(),
            content_hash: staged.content_hash.clone(),
            raw_locator: Some(artifact_ref(layout, &raw_path)),
            normalized_locator: Some(artifact_ref(layout, &normalized_path)),
            status: String::from("published"),
            analysis_refs: vec![result_ref],
            derived_refs: Vec::new(),
            tags: merge_tags(&request.tags, &[String::from("ingested")]),
            warm_status: Some(String::from("cold")),
            readiness: Some(String::from("compile_ready")),
        };

        upsert_asset(layout, &record).map_err(diagnostic_from_string)?;
        store_asset_record(&mut records, record.clone());
        published.push(record.clone());
        analysis_jobs.push(job);
        analysis_entries.push(entry);
        record_by_identity.insert(identity_key, record);
    }

    records.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    save_asset_records(layout, &records).map_err(diagnostic_from_string)?;

    let run = IngestionRun {
        ingestion_run_id: run_id.clone(),
        source: request.source.display().to_string(),
        mode: String::from("batch"),
        status: String::from("completed"),
        started_at,
        completed_at: unix_timestamp(),
        discovered: candidates.len() as u32,
        published: analysis_entries.len() as u32,
        reused,
        failed: 0,
    };
    upsert_ingestion_run(layout, &run).map_err(diagnostic_from_string)?;

    let report =
        AssetIngestReport { run, candidates, assets: published, analysis_jobs, analysis_entries };
    write_json(&layout.ingestion_report_path(&run_id), &report).map_err(diagnostic_from_string)?;
    Ok(report)
}

pub fn list_assets(
    layout: &ArtifactLayout,
    query: &AssetQuery,
) -> Result<Vec<AssetRecord>, String> {
    layout.ensure()?;
    let records = load_asset_records(layout)?;
    if records.is_empty() {
        return Ok(Vec::new());
    }

    let filtered_ids = query_asset_ids(layout, query)?;
    let mut records_by_id = BTreeMap::new();
    for record in records {
        records_by_id.insert(record.asset_id.clone(), record);
    }

    let mut assets = Vec::new();
    for asset_id in filtered_ids {
        if let Some(record) = records_by_id.get(&asset_id) {
            assets.push(record.clone());
        }
    }
    Ok(assets)
}

pub fn list_compile_assets(layout: &ArtifactLayout) -> Result<CompileAssetSelection, String> {
    layout.ensure()?;

    let published_asset_count = query_published_asset_count(layout)?;
    if published_asset_count == 0 {
        return Ok(CompileAssetSelection { eligible_assets: Vec::new(), published_asset_count });
    }

    let records = load_asset_records(layout)?;
    let mut records_by_id = BTreeMap::new();
    for record in records {
        records_by_id.insert(record.asset_id.clone(), record);
    }

    let eligible_asset_ids = query_compile_asset_ids(layout)?;
    let mut eligible_assets = Vec::new();
    for asset_id in eligible_asset_ids {
        let record = records_by_id.remove(&asset_id).ok_or_else(|| {
            format!(
                "compile asset {} is present in registry query results but missing from {}",
                asset_id,
                layout.asset_registry_file().display()
            )
        })?;
        eligible_assets.push(record);
    }

    Ok(CompileAssetSelection { eligible_assets, published_asset_count })
}

pub fn get_asset(layout: &ArtifactLayout, asset_id: &str) -> Result<Option<AssetRecord>, String> {
    let records = load_asset_records(layout)?;
    Ok(records.into_iter().find(|record| record.asset_id == asset_id))
}

pub fn list_asset_analysis(
    layout: &ArtifactLayout,
    asset_id: &str,
) -> Result<Vec<AnalysisCacheEntry>, String> {
    layout.ensure()?;
    let mut entries = Vec::new();
    let cache_dir = layout.analysis_cache_dir();
    for entry in fs::read_dir(&cache_dir)
        .map_err(|error| format!("failed to read {}: {error}", cache_dir.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read analysis cache entry: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let cached: AnalysisCacheEntry = read_json(&path)?;
        if cached.asset_id == asset_id {
            entries.push(cached);
        }
    }
    entries.sort_by(|left, right| left.cache_key.cmp(&right.cache_key));
    Ok(entries)
}

pub fn list_asset_jobs(
    layout: &ArtifactLayout,
    asset_id: &str,
) -> Result<Vec<AnalysisJob>, String> {
    layout.ensure()?;
    let mut jobs = Vec::new();
    let reports_dir = layout.analysis_reports_dir();
    for entry in fs::read_dir(&reports_dir)
        .map_err(|error| format!("failed to read {}: {error}", reports_dir.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read analysis report entry: {error}"))?;
        let path = entry.path();
        let is_job_file = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.starts_with("job-") && value.ends_with(".json"))
            .unwrap_or(false);
        if !is_job_file {
            continue;
        }

        let job: AnalysisJob = read_json(&path)?;
        if job.asset_id == asset_id {
            jobs.push(job);
        }
    }
    jobs.sort_by(|left, right| left.analysis_job_id.cmp(&right.analysis_job_id));
    Ok(jobs)
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

fn connect_registry(path: &Path) -> Result<Connection, String> {
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
            ",
        )
        .map_err(|error| format!("failed to initialize registry schema: {error}"))?;
    Ok(connection)
}

fn scan_candidates(source: &Path, declared_kind: &str) -> Result<Vec<IngestionCandidate>, String> {
    let mut files = collect_files(source)?;
    files.sort_by_key(|path| path.display().to_string());

    let mut candidates = Vec::new();
    for (index, path) in files.iter().enumerate() {
        let metadata = fs::metadata(path)
            .map_err(|error| format!("failed to stat {}: {error}", path.display()))?;
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);

        candidates.push(IngestionCandidate {
            candidate_id: format!("cand-{:03}", index + 1),
            path: path.display().to_string(),
            declared_kind: declared_kind.to_string(),
            size_bytes: metadata.len(),
            modified_at,
        });
    }

    Ok(candidates)
}

fn collect_files(source: &Path) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![source.to_path_buf()];
    let mut files = Vec::new();
    let manifest_path = source.join(ASSET_PACK_MANIFEST_FILE);

    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path == manifest_path {
                continue;
            } else if path.is_file() {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn stage_candidates(
    candidates: Vec<IngestionCandidate>,
    request: &AssetIngestRequest,
) -> Result<Vec<StagedCandidate>, Vec<Diagnostic>> {
    let manifest = load_asset_pack_manifest(&request.source).map_err(diagnostic_from_string)?;
    let normalized_namespace = normalize_asset_namespace(
        request.asset_namespace.as_deref().or(manifest.asset_namespace.as_deref()),
    )?;
    let mut merged_overrides = manifest.asset_id_overrides;
    for (relative_path, asset_id) in &request.asset_id_overrides {
        merged_overrides.insert(relative_path.clone(), asset_id.clone());
    }
    let normalized_overrides =
        normalize_asset_id_overrides(&request.declared_kind, &merged_overrides)?;
    let mut staged = Vec::with_capacity(candidates.len());
    let mut used_override_paths = BTreeSet::new();
    for candidate in candidates {
        let source_path = PathBuf::from(&candidate.path);
        let relative_path = relative_source_path(&request.source, &source_path)
            .map_err(|message| vec![Diagnostic::error("AST-013", message)])?;
        let bytes = fs::read(&source_path).map_err(|error| {
            vec![Diagnostic::error(
                "AST-003",
                format!("failed to read {}: {error}", source_path.display()),
            )]
        })?;
        let file_name = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("asset.bin")
            .to_string();
        let extension =
            source_path.extension().and_then(|value| value.to_str()).unwrap_or("bin").to_string();
        let content_hash = content_hash(&bytes);
        let asset_id = if let Some(explicit_asset_id) = normalized_overrides.get(&relative_path) {
            used_override_paths.insert(relative_path.clone());
            explicit_asset_id.clone()
        } else {
            build_asset_id(&request.declared_kind, &source_path, normalized_namespace.as_deref())
        };

        staged.push(StagedCandidate {
            candidate,
            source_path,
            relative_path,
            file_name,
            extension,
            bytes,
            content_hash,
            asset_id,
        });
    }

    let mut diagnostics = Vec::new();
    for relative_path in normalized_overrides.keys() {
        if used_override_paths.contains(relative_path) {
            continue;
        }

        diagnostics.push(Diagnostic::error(
            "AST-012",
            format!(
                "asset_id override path {} does not match any discovered source under {}",
                relative_path,
                request.source.display()
            ),
        ));
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(staged)
}

fn load_asset_pack_manifest(source_root: &Path) -> Result<AssetPackManifest, String> {
    let manifest_path = source_root.join(ASSET_PACK_MANIFEST_FILE);
    if !manifest_path.exists() {
        return Ok(AssetPackManifest::default());
    }

    read_json(&manifest_path)
}

fn normalize_asset_namespace(
    raw_namespace: Option<&str>,
) -> Result<Option<String>, Vec<Diagnostic>> {
    let Some(raw_namespace) = raw_namespace else {
        return Ok(None);
    };

    let trimmed = raw_namespace.trim();
    if trimmed.is_empty() {
        return Err(vec![Diagnostic::error("AST-010", "asset namespace must not be empty")]);
    }

    let normalized = slug(trimmed);
    if !normalized.chars().any(|character| character.is_ascii_alphanumeric()) {
        return Err(vec![Diagnostic::error(
            "AST-010",
            format!(
                "asset namespace {} must contain at least one alphanumeric character",
                raw_namespace
            ),
        )]);
    }

    Ok(Some(normalized))
}

fn normalize_asset_id_overrides(
    declared_kind: &str,
    raw_overrides: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, Vec<Diagnostic>> {
    let mut normalized = BTreeMap::new();
    let mut diagnostics = Vec::new();
    let kind_prefix = declared_kind.replace('_', ".");

    for (raw_relative_path, raw_asset_id) in raw_overrides {
        let relative_path = normalize_override_path(raw_relative_path);
        if relative_path.is_empty() {
            diagnostics
                .push(Diagnostic::error("AST-011", "asset_id override paths must not be empty"));
            continue;
        }

        match canonicalize_asset_id_override(&kind_prefix, raw_asset_id) {
            Ok(asset_id) => {
                normalized.insert(relative_path, asset_id);
            }
            Err(diagnostic) => diagnostics.push(*diagnostic),
        }
    }

    if diagnostics.is_empty() { Ok(normalized) } else { Err(diagnostics) }
}

fn normalize_override_path(raw_relative_path: &str) -> String {
    raw_relative_path
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string()
}

fn canonicalize_asset_id_override(
    kind_prefix: &str,
    raw_asset_id: &str,
) -> Result<String, Box<Diagnostic>> {
    let trimmed = raw_asset_id.trim();
    if trimmed.is_empty() {
        return Err(Box::new(Diagnostic::error(
            "AST-011",
            "asset_id override values must not be empty",
        )));
    }

    let segments = trimmed
        .split('.')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(slug)
        .collect::<Vec<_>>();
    if segments.len() < 2 {
        return Err(Box::new(Diagnostic::error(
            "AST-011",
            format!(
                "asset_id override {} must contain the declared kind prefix and a leaf name",
                raw_asset_id
            ),
        )));
    }

    let canonical = segments.join(".");
    let expected_prefix = format!("{kind_prefix}.");
    if !canonical.starts_with(&expected_prefix) {
        return Err(Box::new(Diagnostic::error(
            "AST-011",
            format!("asset_id override {} must start with {}", raw_asset_id, kind_prefix),
        )));
    }

    Ok(canonical)
}

fn relative_source_path(source_root: &Path, source_path: &Path) -> Result<String, String> {
    let relative = source_path.strip_prefix(source_root).map_err(|error| {
        format!(
            "candidate {} is not under source root {}: {error}",
            source_path.display(),
            source_root.display()
        )
    })?;

    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn validate_asset_naming_policy(
    staged_candidates: &[StagedCandidate],
    existing_records: &[AssetRecord],
) -> Vec<Diagnostic> {
    let batch_conflicts = detect_batch_asset_id_conflicts(staged_candidates);
    if !batch_conflicts.is_empty() {
        return batch_conflicts;
    }

    detect_existing_asset_id_conflicts(staged_candidates, existing_records)
}

fn detect_batch_asset_id_conflicts(staged_candidates: &[StagedCandidate]) -> Vec<Diagnostic> {
    let mut candidates_by_asset_id: BTreeMap<&str, Vec<&StagedCandidate>> = BTreeMap::new();
    for staged in staged_candidates {
        candidates_by_asset_id.entry(&staged.asset_id).or_default().push(staged);
    }

    let mut diagnostics = Vec::new();
    for (asset_id, claims) in candidates_by_asset_id {
        if claims.len() < 2 {
            continue;
        }

        let mut sources =
            claims.iter().map(|claim| claim.source_path.display().to_string()).collect::<Vec<_>>();
        sources.sort();
        sources.dedup();
        if sources.len() < 2 {
            continue;
        }

        diagnostics.push(Diagnostic::error(
            "AST-008",
            format!(
                "asset naming policy rejected {}: multiple source files in the same asset_kind resolve to the same asset_id ({})",
                asset_id,
                sources.join(", ")
            ),
        ));
    }

    diagnostics
}

fn detect_existing_asset_id_conflicts(
    staged_candidates: &[StagedCandidate],
    existing_records: &[AssetRecord],
) -> Vec<Diagnostic> {
    let mut existing_by_asset_id = BTreeMap::new();
    for record in existing_records {
        existing_by_asset_id.insert(record.asset_id.as_str(), record);
    }

    let mut diagnostics = Vec::new();
    for staged in staged_candidates {
        let Some(existing) = existing_by_asset_id.get(staged.asset_id.as_str()) else {
            continue;
        };

        if existing.content_hash == staged.content_hash {
            continue;
        }

        diagnostics.push(Diagnostic::error(
            "AST-009",
            format!(
                "asset naming policy rejected {}: {} conflicts with published content hash {} already bound to this asset_id; add asset_namespace, provide an asset_id override, rename the source file, or change declared_kind",
                staged.asset_id,
                staged.source_path.display(),
                existing.content_hash
            ),
        ));
    }

    diagnostics
}

fn load_asset_records(layout: &ArtifactLayout) -> Result<Vec<AssetRecord>, String> {
    let path = layout.asset_registry_file();
    if !path.exists() {
        return Ok(Vec::new());
    }

    read_json(&path)
}

fn save_asset_records(layout: &ArtifactLayout, records: &[AssetRecord]) -> Result<(), String> {
    write_json(&layout.asset_registry_file(), records)
}

fn query_asset_ids(layout: &ArtifactLayout, query: &AssetQuery) -> Result<Vec<String>, String> {
    let connection = connect_registry(&layout.registry)?;
    let (sql, parameters): (&str, Vec<String>) = match (&query.asset_kind, &query.tag) {
        (Some(asset_kind), Some(tag)) => (
            "
            SELECT DISTINCT assets.asset_id
            FROM assets
            JOIN asset_tags ON asset_tags.asset_id = assets.asset_id
            WHERE assets.asset_kind = ?1 AND asset_tags.tag = ?2
            ORDER BY assets.asset_id
            ",
            vec![asset_kind.clone(), tag.clone()],
        ),
        (Some(asset_kind), None) => (
            "SELECT asset_id FROM assets WHERE asset_kind = ?1 ORDER BY asset_id",
            vec![asset_kind.clone()],
        ),
        (None, Some(tag)) => (
            "
            SELECT DISTINCT assets.asset_id
            FROM assets
            JOIN asset_tags ON asset_tags.asset_id = assets.asset_id
            WHERE asset_tags.tag = ?1
            ORDER BY assets.asset_id
            ",
            vec![tag.clone()],
        ),
        (None, None) => ("SELECT asset_id FROM assets ORDER BY asset_id", Vec::new()),
    };

    let mut statement = connection
        .prepare(sql)
        .map_err(|error| format!("failed to prepare asset query: {error}"))?;
    let mut rows = statement
        .query(rusqlite::params_from_iter(parameters.iter()))
        .map_err(|error| format!("failed to execute asset query: {error}"))?;
    let mut asset_ids = Vec::new();
    while let Some(row) =
        rows.next().map_err(|error| format!("failed to read asset query row: {error}"))?
    {
        asset_ids.push(
            row.get::<_, String>(0)
                .map_err(|error| format!("failed to decode asset query row: {error}"))?,
        );
    }

    Ok(asset_ids)
}

fn query_compile_asset_ids(layout: &ArtifactLayout) -> Result<Vec<String>, String> {
    let connection = connect_registry(&layout.registry)?;
    let mut statement = connection
        .prepare(
            "
            SELECT asset_id
            FROM assets
            WHERE status = 'published'
              AND (readiness IN ('compile_ready', 'warmed') OR warm_status = 'warmed')
            ORDER BY asset_id
            ",
        )
        .map_err(|error| format!("failed to prepare compile asset query: {error}"))?;
    let mut rows = statement
        .query([])
        .map_err(|error| format!("failed to execute compile asset query: {error}"))?;
    let mut asset_ids = Vec::new();
    while let Some(row) =
        rows.next().map_err(|error| format!("failed to read compile asset row: {error}"))?
    {
        asset_ids.push(
            row.get::<_, String>(0)
                .map_err(|error| format!("failed to decode compile asset row: {error}"))?,
        );
    }

    Ok(asset_ids)
}

fn query_published_asset_count(layout: &ArtifactLayout) -> Result<usize, String> {
    let connection = connect_registry(&layout.registry)?;
    let count = connection
        .query_row("SELECT COUNT(*) FROM assets WHERE status = 'published'", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("failed to count published assets: {error}"))?;

    usize::try_from(count)
        .map_err(|error| format!("published asset count overflowed usize: {error}"))
}

fn upsert_asset(layout: &ArtifactLayout, record: &AssetRecord) -> Result<(), String> {
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

fn upsert_ingestion_run(layout: &ArtifactLayout, run: &IngestionRun) -> Result<(), String> {
    let connection = connect_registry(&layout.registry)?;
    connection
        .execute(
            "
            INSERT INTO ingestion_runs (ingestion_run_id, source, status, discovered, published, reused, failed)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(ingestion_run_id) DO UPDATE SET
              source = excluded.source,
              status = excluded.status,
              discovered = excluded.discovered,
              published = excluded.published,
              reused = excluded.reused,
              failed = excluded.failed
            ",
            params![
                run.ingestion_run_id,
                run.source,
                run.status,
                run.discovered,
                run.published,
                run.reused,
                run.failed,
            ],
        )
        .map_err(|error| format!("failed to write ingestion run {}: {error}", run.ingestion_run_id))?;
    Ok(())
}

fn build_beat_track_analysis(
    asset_id: &str,
    normalized_path: &Path,
    source_size_bytes: u64,
) -> Result<BeatTrackAnalysis, String> {
    let probed = probe_audio_file(normalized_path)?;
    let (estimated_tempo_bpm, transient_count) =
        estimate_tempo_bpm(&probed.mono_samples, probed.summary.sample_rate_hz);
    let estimated_bars = estimate_bars(probed.summary.duration_ms, estimated_tempo_bpm);

    Ok(BeatTrackAnalysis {
        asset_id: asset_id.to_string(),
        analyzer: String::from(BEAT_TRACK_ANALYZER),
        analyzer_version: String::from(BEAT_TRACK_ANALYZER_VERSION),
        probe: probed.summary,
        estimated_tempo_bpm,
        downbeat_bar: 1,
        estimated_bars,
        transient_count,
        source_size_bytes,
    })
}

fn build_cache_key(
    asset_id: &str,
    content_hash: &str,
    declared_kind: &str,
    probe: &AudioProbeSummary,
    params_hash: &str,
) -> String {
    format!(
        "acache:{}",
        hash_string(&format!(
            "{asset_id}:{content_hash}:{declared_kind}:{}:{params_hash}:{BEAT_TRACK_ANALYZER}:{BEAT_TRACK_ANALYZER_VERSION}",
            probe_fingerprint(probe)
        ))
    )
}

fn probe_fingerprint(probe: &AudioProbeSummary) -> String {
    hash_string(&format!(
        "{}:{}:{}:{}:{}:{}",
        probe.container,
        probe.codec,
        probe.sample_rate_hz,
        probe.channel_count,
        probe.bits_per_sample,
        probe.frame_count
    ))
}

fn probe_audio_file(path: &Path) -> Result<ProbedAudio, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read audio file {}: {error}", path.display()))?;
    parse_wav_pcm_s16le(&bytes, path)
}

fn parse_wav_pcm_s16le(bytes: &[u8], path: &Path) -> Result<ProbedAudio, String> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(format!(
            "audio probe currently supports WAV/PCM input only: {}",
            path.display()
        ));
    }

    let mut offset = 12_usize;
    let mut fmt: Option<WavFormatSummary> = None;
    let mut data_range: Option<(usize, usize)> = None;

    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let chunk_start = offset + 8;
        let chunk_end = chunk_start.saturating_add(chunk_size);
        if chunk_end > bytes.len() {
            return Err(format!("wav chunk exceeded file bounds: {}", path.display()));
        }

        match chunk_id {
            b"fmt " => {
                if chunk_size < 16 {
                    return Err(format!("wav fmt chunk too small: {}", path.display()));
                }

                let audio_format = u16::from_le_bytes([bytes[chunk_start], bytes[chunk_start + 1]]);
                let channel_count =
                    u16::from_le_bytes([bytes[chunk_start + 2], bytes[chunk_start + 3]]);
                let sample_rate_hz = u32::from_le_bytes([
                    bytes[chunk_start + 4],
                    bytes[chunk_start + 5],
                    bytes[chunk_start + 6],
                    bytes[chunk_start + 7],
                ]);
                let block_align =
                    u16::from_le_bytes([bytes[chunk_start + 12], bytes[chunk_start + 13]]);
                let bits_per_sample =
                    u16::from_le_bytes([bytes[chunk_start + 14], bytes[chunk_start + 15]]);

                if audio_format != 1 {
                    return Err(format!(
                        "audio probe supports PCM format only, found format {}: {}",
                        audio_format,
                        path.display()
                    ));
                }
                if bits_per_sample != 16 {
                    return Err(format!(
                        "audio probe supports 16-bit PCM only, found {} bits: {}",
                        bits_per_sample,
                        path.display()
                    ));
                }
                if channel_count == 0 || sample_rate_hz == 0 || block_align == 0 {
                    return Err(format!(
                        "wav fmt chunk contains zero-valued fields: {}",
                        path.display()
                    ));
                }

                fmt = Some(WavFormatSummary {
                    sample_rate_hz,
                    channel_count,
                    bits_per_sample,
                    block_align,
                });
            }
            b"data" => {
                data_range = Some((chunk_start, chunk_end));
            }
            _ => {}
        }

        offset = chunk_end + (chunk_size % 2);
    }

    let fmt = fmt.ok_or_else(|| format!("wav fmt chunk was not found: {}", path.display()))?;
    let (data_start, data_end) =
        data_range.ok_or_else(|| format!("wav data chunk was not found: {}", path.display()))?;
    let data = &bytes[data_start..data_end];
    if !data.len().is_multiple_of(fmt.block_align as usize) {
        return Err(format!("wav data length is not aligned to frame size: {}", path.display()));
    }

    let frame_count = data.len() as u64 / fmt.block_align as u64;
    let channel_count = usize::from(fmt.channel_count);
    let mut mono_samples = Vec::with_capacity(frame_count as usize);
    let mut peak_ratio = 0.0_f64;
    let mut sum_squares = 0.0_f64;
    let mut sample_count = 0_u64;

    for frame in data.chunks_exact(fmt.block_align as usize) {
        let mut mono_accumulator = 0.0_f64;
        for channel_index in 0..channel_count {
            let sample_offset = channel_index * 2;
            let sample = i16::from_le_bytes([frame[sample_offset], frame[sample_offset + 1]]);
            let sample_i32 = i32::from(sample);
            let magnitude = sample_i32.unsigned_abs() as f64 / i16::MAX as f64;
            let normalized = sample_i32 as f64 / i16::MAX as f64;
            peak_ratio = peak_ratio.max(magnitude.min(1.0));
            sum_squares += normalized * normalized;
            sample_count = sample_count.saturating_add(1);
            mono_accumulator += normalized;
        }
        mono_samples.push((mono_accumulator / channel_count as f64) as f32);
    }

    let rms_ratio =
        if sample_count == 0 { 0.0 } else { (sum_squares / sample_count as f64).sqrt() };
    let duration_ms = frame_count.saturating_mul(1000) / u64::from(fmt.sample_rate_hz);

    Ok(ProbedAudio {
        summary: AudioProbeSummary {
            container: String::from("wav"),
            codec: String::from("pcm_s16le"),
            sample_rate_hz: fmt.sample_rate_hz,
            channel_count: fmt.channel_count,
            bits_per_sample: fmt.bits_per_sample,
            frame_count,
            duration_ms,
            peak_level_dbfs_tenths: dbfs_tenths(peak_ratio),
            rms_level_dbfs_tenths: dbfs_tenths(rms_ratio),
        },
        mono_samples,
    })
}

fn estimate_tempo_bpm(samples: &[f32], sample_rate_hz: u32) -> (u32, u32) {
    if samples.is_empty() || sample_rate_hz == 0 {
        return (120, 0);
    }

    let window_size = usize::try_from((sample_rate_hz / 100).max(240)).unwrap_or(240);
    let mut envelope = Vec::new();
    for chunk in samples.chunks(window_size) {
        let energy =
            chunk.iter().map(|sample| f64::from(sample.abs())).sum::<f64>() / chunk.len() as f64;
        envelope.push(energy);
    }

    if envelope.len() < 3 {
        return (120, 0);
    }

    let average_energy = envelope.iter().sum::<f64>() / envelope.len() as f64;
    let threshold = average_energy.max(0.05) * 1.35;
    let min_gap_windows = ((sample_rate_hz as f64 * 0.16) / window_size as f64).ceil() as usize;
    let mut onsets = Vec::new();
    let mut last_onset_index = None;

    for index in 1..envelope.len() {
        let previous = envelope[index - 1];
        let current = envelope[index];
        if current < threshold || current <= previous * 1.45 {
            continue;
        }

        if let Some(last_index) = last_onset_index
            && index.saturating_sub(last_index) < min_gap_windows.max(1)
        {
            continue;
        }

        let time_seconds = index as f64 * window_size as f64 / sample_rate_hz as f64;
        onsets.push(time_seconds);
        last_onset_index = Some(index);
    }

    if onsets.len() < 2 {
        return (120, onsets.len() as u32);
    }

    let average_interval = onsets.windows(2).map(|window| window[1] - window[0]).sum::<f64>()
        / (onsets.len() - 1) as f64;
    if average_interval <= f64::EPSILON {
        return (120, onsets.len() as u32);
    }

    let mut estimated_tempo_bpm = (60.0 / average_interval).round() as u32;
    while estimated_tempo_bpm < 70 {
        estimated_tempo_bpm = estimated_tempo_bpm.saturating_mul(2);
    }
    while estimated_tempo_bpm > 180 {
        estimated_tempo_bpm = (estimated_tempo_bpm / 2).max(1);
    }

    (estimated_tempo_bpm.max(1), onsets.len() as u32)
}

fn estimate_bars(duration_ms: u64, tempo_bpm: u32) -> u32 {
    if duration_ms == 0 || tempo_bpm == 0 {
        return 1;
    }

    let beats = duration_ms as f64 / 1000.0 * tempo_bpm as f64 / 60.0;
    (beats / 4.0).ceil().max(1.0) as u32
}

fn dbfs_tenths(level_ratio: f64) -> i32 {
    if level_ratio <= 0.000_000_1 {
        return -1200;
    }

    (20.0 * level_ratio.log10() * 10.0).round() as i32
}

fn build_asset_id(
    declared_kind: &str,
    source_path: &Path,
    asset_namespace: Option<&str>,
) -> String {
    let kind_prefix = declared_kind.replace('_', ".");
    let base_name = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(slug)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| String::from("asset"));

    match asset_namespace {
        Some(namespace) => format!("{kind_prefix}.{namespace}.{base_name}"),
        None => format!("{kind_prefix}.{base_name}"),
    }
}

fn asset_identity_key(asset_kind: &str, asset_id: &str, content_hash: &str) -> String {
    format!("{asset_kind}:{asset_id}:{content_hash}")
}

fn merge_asset_tags(existing: &AssetRecord, requested_tags: &[String]) -> AssetRecord {
    let mut merged = existing.clone();
    merged.tags = merge_tags(&existing.tags, requested_tags);
    merged
}

fn merge_tags(left: &[String], right: &[String]) -> Vec<String> {
    let mut tags = BTreeSet::new();
    for tag in left {
        tags.insert(tag.clone());
    }
    for tag in right {
        tags.insert(tag.clone());
    }
    tags.into_iter().collect()
}

fn store_asset_record(records: &mut Vec<AssetRecord>, record: AssetRecord) {
    if let Some(existing) =
        records.iter_mut().find(|candidate| candidate.asset_id == record.asset_id)
    {
        *existing = record;
        return;
    }
    records.push(record);
}

fn artifact_ref(layout: &ArtifactLayout, path: &Path) -> String {
    let repo_root = layout.root.parent().unwrap_or(&layout.root);
    path.strip_prefix(repo_root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn hash_string(value: &str) -> String {
    content_hash(value.as_bytes())
}

fn short_hash(value: &str) -> String {
    if let Some(digest) = value.strip_prefix("sha256:") {
        return digest.chars().take(8).collect();
    }

    hash_string(value).trim_start_matches("sha256:").chars().take(8).collect()
}

fn unix_timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_secs()).unwrap_or(0)
}

fn diagnostic_from_string(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::error("AST-000", message)]
}

#[cfg(test)]
mod tests {
    use super::{
        ASSET_PACK_MANIFEST_FILE, ArtifactLayout, AssetIngestRequest, AssetQuery, get_asset,
        ingest_assets, list_asset_analysis, list_asset_jobs, list_assets, list_compile_assets,
        read_json, save_asset_records, slug, upsert_asset,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};
    use vidodo_ir::{AssetRecord, BeatTrackAnalysis};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "vidodo-storage-test-{}-{}",
            name,
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_test_wav(path: &Path, channels: u16, pulse_offsets: &[u32], total_frames: u32) {
        let sample_rate_hz = 48_000_u32;
        let bits_per_sample = 16_u16;
        let block_align = channels * (bits_per_sample / 8);
        let byte_rate = sample_rate_hz * u32::from(block_align);
        let mut samples = vec![0_i16; total_frames as usize * channels as usize];

        for &pulse_offset in pulse_offsets {
            for frame_index in 0..2_400_u32 {
                let frame = pulse_offset.saturating_add(frame_index);
                if frame >= total_frames {
                    break;
                }

                let progress = frame_index as f32 / 2_400.0;
                let envelope = (1.0 - progress).max(0.0);
                let sample = (envelope * i16::MAX as f32 * 0.65) as i16;
                for channel_index in 0..channels as usize {
                    samples[frame as usize * channels as usize + channel_index] = sample;
                }
            }
        }

        let data_len = samples.len() * std::mem::size_of::<i16>();
        let riff_len = 36 + data_len;
        let mut bytes = Vec::with_capacity(44 + data_len);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(riff_len as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate_hz.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        fs::write(path, bytes).unwrap();
    }

    fn write_asset_pack_manifest(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
    }

    #[test]
    fn builds_expected_subdirectories() {
        let layout = ArtifactLayout::new("artifacts");

        assert!(layout.traces.ends_with("artifacts/traces"));
        assert!(layout.exports.ends_with("artifacts/exports"));
        assert!(layout.revisions.ends_with("artifacts/revisions"));
        assert!(layout.asset_raw_dir().ends_with("artifacts/assets/raw"));
    }

    #[test]
    fn ingests_assets_and_queries_registry() {
        let root = unique_temp_dir("ingest");
        let source_dir = root.join("imports");
        fs::create_dir_all(&source_dir).unwrap();
        write_test_wav(&source_dir.join("kick-a.wav"), 1, &[0, 24_000, 48_000, 72_000], 96_000);
        write_test_wav(&source_dir.join("pad-a.wav"), 2, &[12_000, 36_000, 60_000, 84_000], 96_000);

        let layout = ArtifactLayout::new(root.join("artifacts"));
        let request = AssetIngestRequest {
            source: source_dir,
            declared_kind: String::from("audio_loop"),
            tags: vec![String::from("fixture"), String::from("rhythm")],
            asset_namespace: None,
            asset_id_overrides: BTreeMap::new(),
        };

        let report = ingest_assets(&layout, &request).unwrap();
        assert_eq!(report.run.discovered, 2);
        assert_eq!(report.run.published, 2);
        assert_eq!(report.analysis_entries.len(), 2);

        let listed = list_assets(&layout, &AssetQuery::default()).unwrap();
        assert_eq!(listed.len(), 2);

        let filtered = list_assets(
            &layout,
            &AssetQuery {
                asset_kind: Some(String::from("audio_loop")),
                tag: Some(String::from("fixture")),
            },
        )
        .unwrap();
        assert_eq!(filtered.len(), 2);

        let asset = get_asset(&layout, &filtered[0].asset_id).unwrap().unwrap();
        assert_eq!(asset.status, "published");
        assert_eq!(asset.readiness.as_deref(), Some("compile_ready"));

        let analysis = list_asset_analysis(&layout, &asset.asset_id).unwrap();
        assert_eq!(analysis.len(), 1);
        let payload_path = root.join(&analysis[0].payload_ref);
        let payload: BeatTrackAnalysis = read_json(&payload_path).unwrap();
        assert_eq!(payload.asset_id, "audio.loop.kick-a");
        assert_eq!(payload.probe.codec, "pcm_s16le");
        assert_eq!(payload.probe.sample_rate_hz, 48_000);
        assert_eq!(payload.probe.channel_count, 1);
        assert!(payload.probe.duration_ms >= 1_900);
        assert!(payload.estimated_tempo_bpm >= 110 && payload.estimated_tempo_bpm <= 130);
        assert!(payload.transient_count >= 3);

        let jobs = list_asset_jobs(&layout, &asset.asset_id).unwrap();
        assert_eq!(jobs.len(), 1);

        let stable_ids = listed.iter().map(|record| record.asset_id.as_str()).collect::<Vec<_>>();
        assert_eq!(stable_ids, vec!["audio.loop.kick-a", "audio.loop.pad-a"]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_name_conflicts_within_same_ingest_batch() {
        let root = unique_temp_dir("batch-conflict");
        let source_dir = root.join("imports");
        fs::create_dir_all(source_dir.join("crate-a")).unwrap();
        fs::create_dir_all(source_dir.join("crate-b")).unwrap();
        write_test_wav(
            &source_dir.join("crate-a").join("kick.wav"),
            1,
            &[0, 24_000, 48_000],
            72_000,
        );
        write_test_wav(
            &source_dir.join("crate-b").join("kick.wav"),
            1,
            &[12_000, 36_000, 60_000],
            72_000,
        );

        let layout = ArtifactLayout::new(root.join("artifacts"));
        let diagnostics = ingest_assets(
            &layout,
            &AssetIngestRequest {
                source: source_dir,
                declared_kind: String::from("audio_loop"),
                tags: vec![String::from("fixture")],
                asset_namespace: None,
                asset_id_overrides: BTreeMap::new(),
            },
        )
        .unwrap_err();

        assert!(diagnostics.iter().any(|diagnostic| diagnostic.code == "AST-008"));
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.message.contains("audio.loop.kick"))
        );
        assert!(list_assets(&layout, &AssetQuery::default()).unwrap().is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_name_conflicts_against_published_registry_asset() {
        let root = unique_temp_dir("registry-conflict");
        let first_dir = root.join("first-import");
        let second_dir = root.join("second-import");
        fs::create_dir_all(&first_dir).unwrap();
        fs::create_dir_all(&second_dir).unwrap();
        write_test_wav(&first_dir.join("kick.wav"), 1, &[0, 24_000, 48_000], 72_000);
        write_test_wav(&second_dir.join("kick.wav"), 1, &[6_000, 30_000, 54_000], 72_000);

        let layout = ArtifactLayout::new(root.join("artifacts"));
        ingest_assets(
            &layout,
            &AssetIngestRequest {
                source: first_dir,
                declared_kind: String::from("audio_loop"),
                tags: vec![String::from("fixture")],
                asset_namespace: None,
                asset_id_overrides: BTreeMap::new(),
            },
        )
        .unwrap();

        let diagnostics = ingest_assets(
            &layout,
            &AssetIngestRequest {
                source: second_dir,
                declared_kind: String::from("audio_loop"),
                tags: vec![String::from("fixture")],
                asset_namespace: None,
                asset_id_overrides: BTreeMap::new(),
            },
        )
        .unwrap_err();

        assert!(diagnostics.iter().any(|diagnostic| diagnostic.code == "AST-009"));
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.message.contains("audio.loop.kick"))
        );

        let listed = list_assets(&layout, &AssetQuery::default()).unwrap();
        assert_eq!(listed.len(), 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lists_compile_assets_from_registry_policy() {
        let root = unique_temp_dir("compile-assets");
        let layout = ArtifactLayout::new(root.join("artifacts"));
        layout.ensure().unwrap();

        let compile_ready = AssetRecord {
            asset_id: String::from("audio.loop.kick-a"),
            asset_kind: String::from("audio_loop"),
            content_hash: String::from("sha256:compile-ready"),
            raw_locator: Some(String::from("artifacts/assets/raw/audio.loop.kick-a/kick-a.wav")),
            normalized_locator: Some(String::from(
                "artifacts/assets/normalized/audio-loop-kick-a.wav",
            )),
            status: String::from("published"),
            analysis_refs: Vec::new(),
            derived_refs: Vec::new(),
            tags: vec![String::from("fixture")],
            warm_status: Some(String::from("cold")),
            readiness: Some(String::from("compile_ready")),
        };
        let warmed = AssetRecord {
            asset_id: String::from("audio.loop.pad-a"),
            asset_kind: String::from("audio_loop"),
            content_hash: String::from("sha256:warmed"),
            raw_locator: Some(String::from("artifacts/assets/raw/audio.loop.pad-a/pad-a.wav")),
            normalized_locator: Some(String::from(
                "artifacts/assets/normalized/audio-loop-pad-a.wav",
            )),
            status: String::from("published"),
            analysis_refs: Vec::new(),
            derived_refs: Vec::new(),
            tags: vec![String::from("fixture")],
            warm_status: Some(String::from("warmed")),
            readiness: Some(String::from("metadata_only")),
        };
        let metadata_only = AssetRecord {
            asset_id: String::from("audio.loop.sketch-a"),
            asset_kind: String::from("audio_loop"),
            content_hash: String::from("sha256:metadata-only"),
            raw_locator: None,
            normalized_locator: None,
            status: String::from("published"),
            analysis_refs: Vec::new(),
            derived_refs: Vec::new(),
            tags: vec![String::from("scratch")],
            warm_status: Some(String::from("cold")),
            readiness: Some(String::from("metadata_only")),
        };

        for record in [&compile_ready, &warmed, &metadata_only] {
            upsert_asset(&layout, record).unwrap();
        }
        save_asset_records(
            &layout,
            &[compile_ready.clone(), metadata_only.clone(), warmed.clone()],
        )
        .unwrap();

        let selection = list_compile_assets(&layout).unwrap();
        assert_eq!(selection.published_asset_count, 3);
        let selected_ids = selection
            .eligible_assets
            .iter()
            .map(|record| record.asset_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(selected_ids, vec!["audio.loop.kick-a", "audio.loop.pad-a"]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn allows_same_basename_across_namespaced_ingest_batches() {
        let root = unique_temp_dir("namespaced-batches");
        let bundle_a = root.join("bundle-a");
        let bundle_b = root.join("bundle-b");
        fs::create_dir_all(&bundle_a).unwrap();
        fs::create_dir_all(&bundle_b).unwrap();
        write_test_wav(&bundle_a.join("kick.wav"), 1, &[0, 24_000, 48_000], 72_000);
        write_test_wav(&bundle_b.join("kick.wav"), 1, &[0, 24_000, 48_000], 72_000);
        write_asset_pack_manifest(
            &bundle_a.join(ASSET_PACK_MANIFEST_FILE),
            r#"{
  "asset_namespace": "bundle-a"
}
"#,
        );
        write_asset_pack_manifest(
            &bundle_b.join(ASSET_PACK_MANIFEST_FILE),
            r#"{
  "asset_namespace": "bundle-b"
}
"#,
        );

        let layout = ArtifactLayout::new(root.join("artifacts"));
        ingest_assets(
            &layout,
            &AssetIngestRequest {
                source: bundle_a,
                declared_kind: String::from("audio_loop"),
                tags: vec![String::from("fixture")],
                asset_namespace: None,
                asset_id_overrides: BTreeMap::new(),
            },
        )
        .unwrap();
        ingest_assets(
            &layout,
            &AssetIngestRequest {
                source: bundle_b,
                declared_kind: String::from("audio_loop"),
                tags: vec![String::from("fixture")],
                asset_namespace: None,
                asset_id_overrides: BTreeMap::new(),
            },
        )
        .unwrap();

        let listed = list_assets(&layout, &AssetQuery::default()).unwrap();
        let ids = listed.iter().map(|record| record.asset_id.as_str()).collect::<Vec<_>>();
        assert_eq!(ids, vec!["audio.loop.bundle-a.kick", "audio.loop.bundle-b.kick"]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn allows_same_basename_with_manifest_asset_id_overrides() {
        let root = unique_temp_dir("manifest-override-batch");
        let source_dir = root.join("imports");
        fs::create_dir_all(source_dir.join("crate-a")).unwrap();
        fs::create_dir_all(source_dir.join("crate-b")).unwrap();
        write_test_wav(
            &source_dir.join("crate-a").join("kick.wav"),
            1,
            &[0, 24_000, 48_000],
            72_000,
        );
        write_test_wav(
            &source_dir.join("crate-b").join("kick.wav"),
            1,
            &[0, 24_000, 48_000],
            72_000,
        );
        write_asset_pack_manifest(
            &source_dir.join(ASSET_PACK_MANIFEST_FILE),
            r#"{
  "asset_id_overrides": {
    "crate-a/kick.wav": "audio.loop.bundle-a.kick",
    "crate-b/kick.wav": "audio.loop.bundle-b.kick"
  }
}
"#,
        );

        let layout = ArtifactLayout::new(root.join("artifacts"));
        let report = ingest_assets(
            &layout,
            &AssetIngestRequest {
                source: source_dir,
                declared_kind: String::from("audio_loop"),
                tags: vec![String::from("fixture")],
                asset_namespace: None,
                asset_id_overrides: BTreeMap::new(),
            },
        )
        .unwrap();

        assert_eq!(report.run.discovered, 2);
        let published_ids =
            report.assets.iter().map(|record| record.asset_id.as_str()).collect::<Vec<_>>();
        assert_eq!(published_ids, vec!["audio.loop.bundle-a.kick", "audio.loop.bundle-b.kick"]);
        assert_eq!(report.analysis_entries.len(), 2);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn slugs_unknown_characters() {
        assert_eq!(slug("show/phase0 demo"), "show-phase0-demo");
    }
}

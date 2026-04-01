use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use vidodo_ir::{
    AnalysisCacheEntry, AnalysisJob, AssetIngestReport, AssetRecord, BeatTrackAnalysis, Diagnostic,
    IngestionCandidate, IngestionRun,
};

const BEAT_TRACK_ANALYZER: &str = "beat_track";
const BEAT_TRACK_ANALYZER_VERSION: &str = "0.1.0";

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
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssetQuery {
    pub asset_kind: Option<String>,
    pub tag: Option<String>,
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
    let mut record_by_hash = BTreeMap::new();
    for record in &records {
        record_by_hash
            .insert(asset_hash_key(&record.asset_kind, &record.content_hash), record.clone());
    }

    let mut published = Vec::new();
    let mut analysis_jobs = Vec::new();
    let mut analysis_entries = Vec::new();
    let mut reused = 0_u32;

    for candidate in &candidates {
        let candidate_path = PathBuf::from(&candidate.path);
        let bytes = fs::read(&candidate_path).map_err(|error| {
            vec![Diagnostic::error(
                "AST-003",
                format!("failed to read {}: {error}", candidate_path.display()),
            )]
        })?;
        let content_hash = content_hash(&bytes);
        let hash_key = asset_hash_key(&request.declared_kind, &content_hash);

        if let Some(existing) = record_by_hash.get(&hash_key) {
            reused = reused.saturating_add(1);
            let merged = merge_asset_tags(existing, &request.tags);
            upsert_asset(layout, &merged).map_err(diagnostic_from_string)?;
            store_asset_record(&mut records, merged.clone());
            published.push(merged);
            continue;
        }

        let asset_id = build_asset_id(&request.declared_kind, &candidate_path, &content_hash);
        let file_name =
            candidate_path.file_name().and_then(|name| name.to_str()).unwrap_or("asset.bin");
        let extension =
            candidate_path.extension().and_then(|value| value.to_str()).unwrap_or("bin");

        let raw_path = layout.asset_raw_dir().join(&asset_id).join(file_name);
        if let Some(parent) = raw_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                vec![Diagnostic::error(
                    "AST-004",
                    format!("failed to create {}: {error}", parent.display()),
                )]
            })?;
        }
        fs::copy(&candidate_path, &raw_path).map_err(|error| {
            vec![Diagnostic::error(
                "AST-005",
                format!(
                    "failed to copy {} to {}: {error}",
                    candidate_path.display(),
                    raw_path.display()
                ),
            )]
        })?;

        let normalized_path =
            layout.asset_normalized_dir().join(format!("{}.{}", slug(&asset_id), extension));
        fs::copy(&candidate_path, &normalized_path).map_err(|error| {
            vec![Diagnostic::error(
                "AST-006",
                format!(
                    "failed to normalize {} into {}: {error}",
                    candidate_path.display(),
                    normalized_path.display()
                ),
            )]
        })?;

        let payload = build_beat_track_analysis(&asset_id, bytes.len() as u64, &content_hash);
        let params_hash = hash_string("{}");
        let cache_key = build_cache_key(&content_hash, &request.declared_kind, &params_hash);
        let result_ref = format!("analysis-{}", short_hash(&cache_key));
        let payload_path = layout.analysis_payload_path(&result_ref);
        write_json(&payload_path, &payload).map_err(diagnostic_from_string)?;

        let entry = AnalysisCacheEntry {
            cache_key: cache_key.clone(),
            asset_id: asset_id.clone(),
            analyzer: String::from(BEAT_TRACK_ANALYZER),
            analyzer_version: String::from(BEAT_TRACK_ANALYZER_VERSION),
            input_fingerprint: content_hash.clone(),
            dependency_fingerprint: hash_string("static-deps"),
            created_at: unix_timestamp(),
            status: String::from("ready"),
            payload_ref: artifact_ref(layout, &payload_path),
        };
        write_json(&layout.analysis_entry_path(&cache_key), &entry)
            .map_err(diagnostic_from_string)?;

        let job = AnalysisJob {
            analysis_job_id: format!("job-{}", short_hash(&format!("{asset_id}:{cache_key}"))),
            asset_id: asset_id.clone(),
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
            asset_id: asset_id.clone(),
            asset_kind: request.declared_kind.clone(),
            content_hash: content_hash.clone(),
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
        record_by_hash.insert(hash_key, record);
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
            } else if path.is_file() {
                files.push(path);
            }
        }
    }

    Ok(files)
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
    size_bytes: u64,
    content_hash: &str,
) -> BeatTrackAnalysis {
    let hash_seed = short_hash(content_hash)
        .bytes()
        .fold(0_u32, |accumulator, value| accumulator.saturating_add(value as u32));
    let estimated_tempo_bpm = 110 + (hash_seed % 24);
    let estimated_bars = ((size_bytes / 128).max(4) as u32).min(64);

    BeatTrackAnalysis {
        asset_id: asset_id.to_string(),
        analyzer: String::from(BEAT_TRACK_ANALYZER),
        analyzer_version: String::from(BEAT_TRACK_ANALYZER_VERSION),
        estimated_tempo_bpm,
        downbeat_bar: 1,
        estimated_bars,
        source_size_bytes: size_bytes,
    }
}

fn build_cache_key(content_hash: &str, declared_kind: &str, params_hash: &str) -> String {
    format!(
        "acache:{}",
        hash_string(&format!(
            "{content_hash}:{declared_kind}:{BEAT_TRACK_ANALYZER}:{BEAT_TRACK_ANALYZER_VERSION}:{params_hash}"
        ))
    )
}

fn build_asset_id(declared_kind: &str, source_path: &Path, content_hash: &str) -> String {
    let kind_prefix = declared_kind.replace('_', ".");
    let base_name = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(slug)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| String::from("asset"));
    format!("{kind_prefix}.{base_name}-{}", short_hash(content_hash))
}

fn asset_hash_key(asset_kind: &str, content_hash: &str) -> String {
    format!("{asset_kind}:{content_hash}")
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
        ArtifactLayout, AssetIngestRequest, AssetQuery, get_asset, ingest_assets,
        list_asset_analysis, list_asset_jobs, list_assets, slug,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "vidodo-storage-test-{}-{}",
            name,
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
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
        fs::write(source_dir.join("kick-a.wav"), b"kick fixture bytes").unwrap();
        fs::write(source_dir.join("pad-a.wav"), b"pad fixture bytes").unwrap();

        let layout = ArtifactLayout::new(root.join("artifacts"));
        let request = AssetIngestRequest {
            source: source_dir,
            declared_kind: String::from("audio_loop"),
            tags: vec![String::from("fixture"), String::from("rhythm")],
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

        let jobs = list_asset_jobs(&layout, &asset.asset_id).unwrap();
        assert_eq!(jobs.len(), 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn slugs_unknown_characters() {
        assert_eq!(slug("show/phase0 demo"), "show-phase0-demo");
    }
}

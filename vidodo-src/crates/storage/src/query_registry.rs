//! Registry queries: asset listing, compile selection, revision catalog.

use std::collections::BTreeMap;
use std::fs;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use vidodo_ir::{AnalysisCacheEntry, AnalysisJob, AssetRecord};

use crate::artifact_layout::{
    ArtifactLayout, connect_registry, load_asset_records, read_json, timestamp_now,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionRecord {
    pub show_id: String,
    pub revision: u64,
    pub status: String,
    pub compile_run_id: String,
    pub artifact_ref: String,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Asset queries
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Revision catalog
// ---------------------------------------------------------------------------

pub fn insert_revision(layout: &ArtifactLayout, record: &RevisionRecord) -> Result<(), String> {
    let connection = connect_registry(&layout.registry)?;
    connection
        .execute(
            "INSERT INTO revisions (show_id, revision, status, compile_run_id, artifact_ref, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.show_id,
                record.revision,
                record.status,
                record.compile_run_id,
                record.artifact_ref,
                record.created_at,
                record.updated_at,
            ],
        )
        .map_err(|error| format!("insert revision failed: {error}"))?;
    Ok(())
}

pub fn update_revision_status(
    layout: &ArtifactLayout,
    show_id: &str,
    revision: u64,
    new_status: &str,
) -> Result<(), String> {
    let connection = connect_registry(&layout.registry)?;
    let now = timestamp_now();
    let changed = connection
        .execute(
            "UPDATE revisions SET status = ?1, updated_at = ?2 WHERE show_id = ?3 AND revision = ?4",
            params![new_status, now, show_id, revision],
        )
        .map_err(|error| format!("update revision status failed: {error}"))?;
    if changed == 0 {
        return Err(format!("revision {revision} not found for show {show_id}"));
    }
    Ok(())
}

pub fn list_revisions(
    layout: &ArtifactLayout,
    show_id: &str,
) -> Result<Vec<RevisionRecord>, String> {
    let connection = connect_registry(&layout.registry)?;
    let mut statement = connection
        .prepare(
            "SELECT show_id, revision, status, compile_run_id, artifact_ref, created_at, updated_at
             FROM revisions WHERE show_id = ?1 ORDER BY revision",
        )
        .map_err(|error| format!("prepare list_revisions failed: {error}"))?;
    let rows = statement
        .query_map(params![show_id], |row| {
            Ok(RevisionRecord {
                show_id: row.get(0)?,
                revision: row.get(1)?,
                status: row.get(2)?,
                compile_run_id: row.get(3)?,
                artifact_ref: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|error| format!("query list_revisions failed: {error}"))?;
    let mut records = Vec::new();
    for row in rows {
        records.push(row.map_err(|error| format!("read revision row failed: {error}"))?);
    }
    Ok(records)
}

// ---------------------------------------------------------------------------
// Private query helpers
// ---------------------------------------------------------------------------

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

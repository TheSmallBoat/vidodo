#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
artifact_root="${1:-$repo_root/artifacts}"
db_path="$artifact_root/registry.db"

mkdir -p \
  "$artifact_root/assets" \
  "$artifact_root/analysis" \
  "$artifact_root/revisions" \
  "$artifact_root/traces" \
  "$artifact_root/exports"

if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "sqlite3 is required to initialize the artifact store" >&2
  exit 1
fi

sqlite3 "$db_path" <<'SQL'
CREATE TABLE IF NOT EXISTS assets (
  asset_id TEXT PRIMARY KEY,
  asset_kind TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  status TEXT NOT NULL,
  warm_status TEXT,
  readiness TEXT
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
SQL

echo "initialized artifact store at $artifact_root"
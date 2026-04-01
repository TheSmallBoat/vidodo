#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
source_dir="$repo_root/tests/fixtures/imports/minimal-audio-pack"
artifact_root="$repo_root/artifacts"
kick_show_json="$(mktemp)"
pad_show_json="$(mktemp)"

cleanup() {
  rm -f "$kick_show_json" "$pad_show_json"
}

trap cleanup EXIT

short_hash() {
  shasum -a 256 "$1" | awk '{print substr($1,1,8)}'
}

slug() {
  printf '%s' "$1" | sed 's/[^[:alnum:]_-]/-/g'
}

rm -rf "$artifact_root"
"$repo_root/scripts/init-artifact-store.sh"

kick_id="audio.loop.kick-a-$(short_hash "$source_dir/kick-a.wav")"
pad_id="audio.loop.pad-a-$(short_hash "$source_dir/pad-a.wav")"
kick_norm="$(slug "$kick_id")"
pad_norm="$(slug "$pad_id")"

cd "$repo_root/vidodo-src"

cargo run -p avctl -- asset ingest --source-dir "$source_dir" --declared-kind audio_loop --tags fixture,smoke >/dev/null
cargo run -p avctl -- asset list --kind audio_loop --tag smoke >/dev/null
cargo run -p avctl -- asset show --asset-id "$kick_id" >"$kick_show_json"
cargo run -p avctl -- asset show --asset-id "$pad_id" >"$pad_show_json"

test -f "$artifact_root/assets/registry/asset-records.json"
test -f "$artifact_root/assets/raw/$kick_id/kick-a.wav"
test -f "$artifact_root/assets/raw/$pad_id/pad-a.wav"
test -f "$artifact_root/assets/normalized/$kick_norm.wav"
test -f "$artifact_root/assets/normalized/$pad_norm.wav"

test -n "$(find "$artifact_root/analysis/cache" -maxdepth 1 -name '*.json' -print -quit)"
test -n "$(find "$artifact_root/analysis/reports" -maxdepth 1 -name 'job-*.json' -print -quit)"
test -n "$(find "$artifact_root/analysis/reports" -maxdepth 1 -name 'analysis-*.json' -print -quit)"

grep -q "$kick_id" "$artifact_root/assets/registry/asset-records.json"
grep -q "$pad_id" "$artifact_root/assets/registry/asset-records.json"
grep -q '"codec": "pcm_s16le"' "$kick_show_json"
grep -q '"sample_rate_hz": 48000' "$kick_show_json"
grep -q '"estimated_tempo_bpm": 120' "$kick_show_json"
grep -q '"transient_count": 3' "$kick_show_json"
grep -q '"channel_count": 2' "$pad_show_json"
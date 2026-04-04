//! Analysis cache loader for the compile pipeline.
//!
//! Reads beat/harmony/section analysis results from the artifact store's
//! analysis cache directory and converts them into IR-level hints that
//! enrich `PerformanceIr`.

use std::collections::HashMap;
use std::path::Path;

use vidodo_ir::{AnalysisSectionBoundary, Diagnostic};

/// Analysis hints extracted from the cache, ready for injection into IR.
#[derive(Debug, Clone, Default)]
pub struct AnalysisHints {
    /// Beat positions in seconds.
    pub beat_map: Vec<f64>,
    /// Detected key (e.g., "C major").
    pub detected_key: Option<String>,
    /// Section boundaries from section segmentation.
    pub section_boundaries: Vec<AnalysisSectionBoundary>,
}

/// Attempt to load analysis hints for the given asset IDs from
/// the analysis cache directory.
///
/// Returns `(hints, warnings)`. If the cache is unavailable or
/// empty, returns empty hints and a warning diagnostic.
pub fn load_analysis_hints(
    cache_dir: &Path,
    asset_ids: &[String],
) -> (HashMap<String, AnalysisHints>, Vec<Diagnostic>) {
    let mut hints_map = HashMap::new();
    let mut warnings = Vec::new();

    if !cache_dir.is_dir() {
        warnings.push(Diagnostic {
            code: String::from("AA-001"),
            namespace: String::from("analysis"),
            severity: String::from("warning"),
            message: format!("analysis cache directory not found: {}", cache_dir.display()),
            target: None,
            retryable: false,
            details: std::collections::BTreeMap::new(),
            suggestion: Some(String::from(
                "run 'avctl asset analyze' to populate the analysis cache",
            )),
        });
        return (hints_map, warnings);
    }

    for asset_id in asset_ids {
        let mut hints = AnalysisHints::default();
        let mut found_any = false;

        // Try loading beat analysis
        let beat_path = cache_dir.join(format!("{asset_id}_beat_track.json"));
        if let Some(beat_map) = load_beat_map(&beat_path) {
            hints.beat_map = beat_map;
            found_any = true;
        }

        // Try loading harmony analysis
        let harmony_path = cache_dir.join(format!("{asset_id}_harmony.json"));
        if let Some(key) = load_detected_key(&harmony_path) {
            hints.detected_key = Some(key);
            found_any = true;
        }

        // Try loading section segmentation
        let section_path = cache_dir.join(format!("{asset_id}_section_segmentation.json"));
        if let Some(boundaries) = load_section_boundaries(&section_path) {
            hints.section_boundaries = boundaries;
            found_any = true;
        }

        if !found_any {
            warnings.push(Diagnostic {
                code: String::from("AA-002"),
                namespace: String::from("analysis"),
                severity: String::from("warning"),
                message: format!("no analysis cache found for asset '{asset_id}'"),
                target: None,
                retryable: false,
                details: std::collections::BTreeMap::new(),
                suggestion: Some(format!(
                    "run 'avctl asset analyze {asset_id}' to generate analysis"
                )),
            });
        }

        hints_map.insert(asset_id.clone(), hints);
    }

    (hints_map, warnings)
}

/// Parse beat positions from a beat_track analysis JSON file.
fn load_beat_map(path: &Path) -> Option<Vec<f64>> {
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let beats = value.get("beats")?.as_array()?;
    Some(beats.iter().filter_map(|b| b.get("time_sec").and_then(|t| t.as_f64())).collect())
}

/// Parse detected key from a harmony analysis JSON file.
fn load_detected_key(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value.get("key").and_then(|k| k.get("key")).and_then(|k| k.as_str()).map(String::from)
}

/// Parse section boundaries from a section_segmentation analysis JSON.
fn load_section_boundaries(path: &Path) -> Option<Vec<AnalysisSectionBoundary>> {
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let sections = value.get("sections")?.as_array()?;
    Some(
        sections
            .iter()
            .filter_map(|s| {
                Some(AnalysisSectionBoundary {
                    start_sec: s.get("start_sec")?.as_f64()?,
                    end_sec: s.get("end_sec")?.as_f64()?,
                    label: s.get("label").and_then(|l| l.as_str()).unwrap_or("").to_string(),
                    confidence: s.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0),
                })
            })
            .collect(),
    )
}

/// Merge hints from multiple assets into a single combined `AnalysisHints`.
pub fn merge_hints(hints_map: &HashMap<String, AnalysisHints>) -> AnalysisHints {
    let mut merged = AnalysisHints::default();
    for hints in hints_map.values() {
        merged.beat_map.extend_from_slice(&hints.beat_map);
        if merged.detected_key.is_none() {
            merged.detected_key.clone_from(&hints.detected_key);
        }
        merged.section_boundaries.extend_from_slice(&hints.section_boundaries);
    }
    // Sort beat map
    merged.beat_map.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    merged
}

#[cfg(test)]
mod analysis_cache_tests {
    use super::*;
    use std::fs;

    #[test]
    fn missing_cache_dir_returns_warning() {
        let (hints, warnings) =
            load_analysis_hints(Path::new("/nonexistent/cache"), &["test".into()]);
        assert!(hints.is_empty());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "AA-001");
    }

    #[test]
    fn load_beat_map_from_json() {
        let dir = tempfile::tempdir().unwrap();
        let beat_json = r#"{
            "asset_id": "bass",
            "beats": [
                {"time_sec": 0.5, "confidence": 0.9, "beat_number": 0},
                {"time_sec": 1.0, "confidence": 0.8, "beat_number": 1}
            ],
            "status": "success"
        }"#;
        fs::write(dir.path().join("bass_beat_track.json"), beat_json).unwrap();

        let (hints, warnings) = load_analysis_hints(dir.path(), &["bass".into()]);
        // No AA-001 warning (dir exists); may have AA-002 for missing harmony/section
        assert!(warnings.iter().all(|w| w.code != "AA-001"));
        let bass_hints = hints.get("bass").unwrap();
        assert_eq!(bass_hints.beat_map, vec![0.5, 1.0]);
    }

    #[test]
    fn missing_asset_produces_per_asset_warning() {
        let dir = tempfile::tempdir().unwrap();
        let (_, warnings) = load_analysis_hints(dir.path(), &["nothing".into()]);
        assert!(warnings.iter().any(|w| w.code == "AA-002"));
    }

    #[test]
    fn merge_combines_multiple_assets() {
        let mut map = HashMap::new();
        map.insert(
            "a".into(),
            AnalysisHints {
                beat_map: vec![1.0, 3.0],
                detected_key: Some("C major".into()),
                section_boundaries: vec![],
            },
        );
        map.insert(
            "b".into(),
            AnalysisHints {
                beat_map: vec![2.0],
                detected_key: None,
                section_boundaries: vec![AnalysisSectionBoundary {
                    start_sec: 0.0,
                    end_sec: 10.0,
                    label: "verse".into(),
                    confidence: 0.9,
                }],
            },
        );

        let merged = merge_hints(&map);
        assert_eq!(merged.beat_map, vec![1.0, 2.0, 3.0]); // sorted
        assert_eq!(merged.detected_key, Some("C major".into()));
        assert_eq!(merged.section_boundaries.len(), 1);
    }
}

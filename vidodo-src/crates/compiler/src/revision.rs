use vidodo_ir::CompiledRevision;
use vidodo_storage::{
    ArtifactLayout, RevisionRecord, insert_revision, list_revisions, update_revision_status,
    write_json,
};

/// Register a newly compiled revision as a candidate.
///
/// Persists the compilation artifact to disk and writes
/// a `RevisionRecord` with status `candidate` into SQLite.
pub fn register_candidate(
    layout: &ArtifactLayout,
    compiled: &CompiledRevision,
) -> Result<RevisionRecord, String> {
    let rev_dir = layout.revision_dir(&compiled.show_id, compiled.revision);
    std::fs::create_dir_all(&rev_dir).map_err(|e| format!("failed to create revision dir: {e}"))?;
    let artifact_path = rev_dir.join("revision.json");
    write_json(&artifact_path, compiled)?;

    let artifact_ref = format!(
        "artifacts/revisions/{}/revision-{}",
        vidodo_storage::slug(&compiled.show_id),
        compiled.revision
    );

    let now = now_string();
    let record = RevisionRecord {
        show_id: compiled.show_id.clone(),
        revision: compiled.revision,
        status: String::from("candidate"),
        compile_run_id: compiled.compile_run_id.clone(),
        artifact_ref,
        created_at: now.clone(),
        updated_at: now,
    };
    insert_revision(layout, &record)?;
    Ok(record)
}

/// Promote a candidate revision to published.
pub fn publish_revision(
    layout: &ArtifactLayout,
    show_id: &str,
    revision: u64,
) -> Result<(), String> {
    update_revision_status(layout, show_id, revision, "published")
}

/// Archive a previously published or candidate revision.
pub fn archive_revision(
    layout: &ArtifactLayout,
    show_id: &str,
    revision: u64,
) -> Result<(), String> {
    update_revision_status(layout, show_id, revision, "archived")
}

/// List all revisions for a show ordered by revision number.
pub fn query_revisions(
    layout: &ArtifactLayout,
    show_id: &str,
) -> Result<Vec<RevisionRecord>, String> {
    list_revisions(layout, show_id)
}

fn now_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_else(|_| String::from("0"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::PlanBundle;

    fn compile_test_plan() -> CompiledRevision {
        crate::compile_plan(&PlanBundle::minimal("show-rev-test")).expect("compile")
    }

    #[test]
    fn register_candidate_creates_artifact_and_record() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = ArtifactLayout::new(tmp.path());
        layout.ensure().expect("ensure");

        let compiled = compile_test_plan();
        let record = register_candidate(&layout, &compiled).expect("register");

        assert_eq!(record.status, "candidate");
        assert_eq!(record.revision, compiled.revision);

        // artifact file should exist
        let artifact =
            layout.revision_dir(&compiled.show_id, compiled.revision).join("revision.json");
        assert!(artifact.exists());

        // SQLite should have the record
        let records = query_revisions(&layout, &compiled.show_id).expect("query");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, "candidate");
    }

    #[test]
    fn publish_then_archive_lifecycle() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = ArtifactLayout::new(tmp.path());
        layout.ensure().expect("ensure");

        let compiled = compile_test_plan();
        register_candidate(&layout, &compiled).expect("register");

        publish_revision(&layout, &compiled.show_id, compiled.revision).expect("publish");
        let records = query_revisions(&layout, &compiled.show_id).expect("query after publish");
        assert_eq!(records[0].status, "published");

        archive_revision(&layout, &compiled.show_id, compiled.revision).expect("archive");
        let records = query_revisions(&layout, &compiled.show_id).expect("query after archive");
        assert_eq!(records[0].status, "archived");
    }

    #[test]
    fn publish_unknown_revision_fails() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = ArtifactLayout::new(tmp.path());
        layout.ensure().expect("ensure");

        let result = publish_revision(&layout, "nonexistent", 999);
        assert!(result.is_err());
    }
}

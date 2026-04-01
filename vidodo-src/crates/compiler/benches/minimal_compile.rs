use std::fs;
use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};
use vidodo_compiler::compile_plan;
use vidodo_ir::{AssetRecord, AudioDsl, ConstraintSet, PlanBundle, SetPlan, VisualDsl};

fn compile_minimal_plan(criterion: &mut Criterion) {
    let plan = PlanBundle::minimal("bench-show");

    criterion.bench_function("compile_minimal_plan", |bencher| {
        bencher.iter(|| compile_plan(&plan).expect("minimal plan should compile"));
    });
}

fn compile_fixture_plan(criterion: &mut Criterion) {
    let plan = load_fixture_plan();

    criterion.bench_function("compile_fixture_plan", |bencher| {
        bencher.iter(|| compile_plan(&plan).expect("fixture plan should compile"));
    });
}

fn load_fixture_plan() -> PlanBundle {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let plan_dir = repo_root.join("tests/fixtures/plans/minimal-show");
    let assets_file = repo_root.join("tests/fixtures/assets/asset-records.json");

    let set_plan: SetPlan = read_fixture_json(&plan_dir.join("set-plan.json"));
    let audio_dsl: AudioDsl = read_fixture_json(&plan_dir.join("audio-dsl.json"));
    let visual_dsl: VisualDsl = read_fixture_json(&plan_dir.join("visual-dsl.json"));
    let constraint_set: ConstraintSet = read_fixture_json(&plan_dir.join("constraint-set.json"));
    let asset_records: Vec<AssetRecord> = read_fixture_json(&assets_file);

    PlanBundle {
        show_id: set_plan.show_id.clone(),
        base_revision: 0,
        set_plan,
        audio_dsl,
        visual_dsl,
        constraint_set,
        asset_records,
    }
}

fn read_fixture_json<T>(path: &PathBuf) -> T
where
    T: serde::de::DeserializeOwned,
{
    let content = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read fixture {}: {error}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|error| panic!("failed to parse fixture {}: {error}", path.display()))
}

criterion_group!(benches, compile_minimal_plan, compile_fixture_plan);
criterion_main!(benches);

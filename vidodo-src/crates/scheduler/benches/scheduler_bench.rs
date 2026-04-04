use criterion::{Criterion, criterion_group, criterion_main};
use vidodo_compiler::compile_plan;
use vidodo_ir::PlanBundle;
use vidodo_scheduler::{FakeBackendClient, simulate_run, simulate_run_with_backend};

fn bench_compile_minimal(c: &mut Criterion) {
    c.bench_function("compile_minimal_plan", |b| {
        b.iter(|| {
            compile_plan(&PlanBundle::minimal("show-bench")).expect("compile should succeed");
        });
    });
}

fn bench_simulate_run_minimal(c: &mut Criterion) {
    let compiled = compile_plan(&PlanBundle::minimal("show-bench")).expect("plan should compile");
    c.bench_function("simulate_run_minimal", |b| {
        b.iter(|| {
            simulate_run(&compiled, "bench-run");
        });
    });
}

fn bench_simulate_with_backend(c: &mut Criterion) {
    let compiled = compile_plan(&PlanBundle::minimal("show-bench")).expect("plan should compile");
    let backend = FakeBackendClient;
    c.bench_function("simulate_run_with_backend", |b| {
        b.iter(|| {
            simulate_run_with_backend(&compiled, "bench-run", &backend);
        });
    });
}

criterion_group!(
    benches,
    bench_compile_minimal,
    bench_simulate_run_minimal,
    bench_simulate_with_backend
);
criterion_main!(benches);

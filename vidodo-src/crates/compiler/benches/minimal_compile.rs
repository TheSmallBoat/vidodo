use criterion::{Criterion, criterion_group, criterion_main};
use vidodo_compiler::compile_plan;
use vidodo_ir::PlanBundle;

fn compile_minimal_plan(criterion: &mut Criterion) {
    let plan = PlanBundle::minimal("bench-show");

    criterion.bench_function("compile_minimal_plan", |bencher| {
        bencher.iter(|| compile_plan(&plan).expect("minimal plan should compile"));
    });
}

criterion_group!(benches, compile_minimal_plan);
criterion_main!(benches);

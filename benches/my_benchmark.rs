
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use criterion::async_executor::AsyncExecutor;
use tokio::runtime::Runtime;

use memoized_kerney::{uncached_distance,Position,distance};

const A: Position = Position::new(37.882704,-121.9807130);
const B: Position = Position::new(37.883463,-121.980988);

pub fn baseline_benchmark(c: &mut Criterion) {
    c.bench_function("baseline_distance", |b| b.iter(|| uncached_distance(black_box(&A),black_box(&B))));
}

pub fn async_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    c.bench_function("async_distance", |b| b.to_async(&rt).iter(|| async { distance(black_box(&A),black_box(&B)).await }));
}

criterion_group!(benches, baseline_benchmark,async_benchmark);
criterion_main!(benches);

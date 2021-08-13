use criterion::{criterion_group, criterion_main, Criterion};
use geo_types::Coordinate;
use h3ron::iter::KRingBuilder;
use h3ron::H3Cell;
use std::time::Duration;

fn criterion_benchmark(c: &mut Criterion) {
    let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
    let k_min = 1;
    let k_max = 3;
    let mut group = c.benchmark_group("k-ring");
    group.sample_size(100);
    group.warm_up_time(Duration::from_secs(1));
    group.bench_function("KRingBuilder", |bencher| {
        let builder = &mut KRingBuilder::new(k_min, k_max);
        bencher.iter(|| {
            let _ = builder.build_k_ring(&cell).collect::<Vec<_>>();
        });
    });
    group.bench_function("H3Cell::k_ring_distances", |bencher| {
        bencher.iter(|| {
            let _ = cell.k_ring_distances(k_min, k_max);
        });
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

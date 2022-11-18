use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use geo_types::Coord;

use h3ron::iter::GridDiskBuilder;
use h3ron::H3Cell;

fn criterion_benchmark(c: &mut Criterion) {
    let cell = H3Cell::from_coordinate(Coord::from((12.3, 45.4)), 6).unwrap();
    let k_min = 1;
    let k_max = 3;
    let mut group = c.benchmark_group("k-ring");
    group.sample_size(100);
    group.warm_up_time(Duration::from_secs(1));
    group.bench_function("GridDiskBuilder", |bencher| {
        let builder = &mut GridDiskBuilder::create(k_min, k_max).unwrap();
        bencher.iter(|| {
            let _ = builder.build_grid_disk(&cell).unwrap().collect::<Vec<_>>();
        });
    });
    group.bench_function("H3Cell::grid_disk_distances", |bencher| {
        bencher.iter(|| {
            let _ = cell.grid_disk_distances(k_min, k_max);
        });
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

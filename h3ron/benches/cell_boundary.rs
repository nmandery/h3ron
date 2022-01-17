use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use geo_types::Coordinate;

use h3ron::{H3Cell, ToPolygon};
use h3ron::iter::GeoBoundaryBuilder;

fn criterion_benchmark(c: &mut Criterion) {
    let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 10)
        .unwrap();

    let mut group = c.benchmark_group("cell_boundary");
    group.sample_size(1000);
    group.warm_up_time(Duration::from_secs(2));
    group.bench_function("iter boundary poly vertices", |bencher| {
        bencher.iter(|| {
            let _cnt = cell.to_polygon().exterior().0.iter().count();
        });
    });

    group.bench_function(
        "iter boundary builder iter",
        |bencher| {
            let mut builder = GeoBoundaryBuilder::new();
            bencher.iter(|| {
                let _cnt = builder.iter_cell_boundary_vertices(&cell, true).count();
            });
        },
    );
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

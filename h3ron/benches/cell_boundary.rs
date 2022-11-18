use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use geo_types::Coord;

use h3ron::iter::CellBoundaryBuilder;
use h3ron::{H3Cell, ToPolygon};

fn criterion_benchmark(c: &mut Criterion) {
    let cell = H3Cell::from_coordinate(Coord::from((12.3, 45.4)), 10).unwrap();

    let mut group = c.benchmark_group("cell_boundary");
    group.sample_size(1000);
    group.warm_up_time(Duration::from_secs(2));
    group.bench_function("iter boundary poly vertices", |bencher| {
        bencher.iter(|| {
            #[allow(clippy::iter_count)]
            let _cnt = cell.to_polygon().unwrap().exterior().0.iter().count();
        });
    });

    group.bench_function("iter boundary builder iter", |bencher| {
        let mut builder = CellBoundaryBuilder::new();
        bencher.iter(|| {
            let _cnt = builder
                .iter_cell_boundary_vertices(&cell, true)
                .unwrap()
                .count();
        });
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

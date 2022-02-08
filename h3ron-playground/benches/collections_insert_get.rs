use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use geo_types::Coordinate;
use h3ron::H3Cell;
use h3ron_playground::collections::cellhierarchy::H3CellHierarchyMap;

fn criterion_benchmark(c: &mut Criterion) {
    let cells = H3Cell::from_coordinate(Coordinate::from((12.3, 45.4)), 10)
        .unwrap()
        .grid_disk(1000)
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    let value = 78u8;

    let mut group = c.benchmark_group("collections");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(3));
    group.bench_function(
        format!("H3CellHierarchyMap::from_iter (n={})", cells.len()),
        |bencher| {
            bencher.iter(|| {
                H3CellHierarchyMap::from_iter(cells.iter().map(|cell| (*cell, value)));
            });
        },
    );
    group.bench_function(
        format!("H3CellHierarchyMap.get (len={})", cells.len()),
        |bencher| {
            let map = H3CellHierarchyMap::from_iter(cells.iter().map(|cell| (*cell, value)));
            bencher.iter(|| map.get(&cells[0]).unwrap());
        },
    );
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

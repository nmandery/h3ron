use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use geo_types::Coordinate;
use h3ron::collections::compressed::IndexBlock;

use h3ron::H3Cell;

fn criterion_benchmark(c: &mut Criterion) {
    let cells = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 10)
        .unwrap()
        .k_ring(200)
        .iter()
        .collect::<Vec<_>>();

    let mut group = c.benchmark_group("indexblock");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.bench_function(format!("compress {} cells", cells.len()), |bencher| {
        bencher.iter(|| {
            let _ib = IndexBlock::from(cells.clone());
        });
    });

    let ib = IndexBlock::from(cells.clone());
    dbg!((ib.size_of_uncompressed(), ib.size_of_compressed()));
    group.bench_function(
        format!("decompress {} cells to vec", cells.len()),
        |bencher| {
            bencher.iter(|| {
                let _cells2: Vec<_> = ib.iter_uncompressed().unwrap().collect();
            });
        },
    );
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

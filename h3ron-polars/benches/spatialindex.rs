use criterion::{criterion_group, criterion_main, Criterion};
use geo_types::{Coordinate, Rect};
use polars_core::prelude::UInt64Chunked;

use h3ron::H3Cell;
use h3ron_polars::spatial_index::{
    BuildKDTreeIndex, BuildPackedHilbertRTreeIndex, BuildRTreeIndex, SIKind, SpatialIndex,
    SpatialIndexGeomOp,
};
use h3ron_polars::{AsH3CellChunked, FromIndexIterator, IndexChunked, IndexValue};

fn bench_spatialindex<Builder, SI, IX, Kind>(
    c: &mut Criterion,
    builder: Builder,
    ic: &IndexChunked<IX>,
    aoi: &Rect,
    name: &str,
) where
    SI: SpatialIndex<IX, Kind> + SpatialIndexGeomOp<IX, Kind>,
    IX: IndexValue,
    Kind: SIKind,
    Builder: Fn(&IndexChunked<IX>) -> SI,
{
    let mut group = c.benchmark_group(format!("{}-{}-cells", name, ic.len()));
    group.bench_with_input("build", ic, |bencher, ic| {
        bencher.iter(|| {
            let _ = builder(ic);
        });
    });

    let si = builder(ic);
    group.bench_with_input("envelopes_intersect", &si, |bencher, si| {
        bencher.iter(|| {
            let _ = si.envelopes_intersect(aoi);
        });
    });

    group.bench_with_input("geometries_intersect", &si, |bencher, si| {
        bencher.iter(|| {
            let _ = si.geometries_intersect(aoi);
        });
    });
    group.finish();
}

fn criterion_benchmark(c: &mut Criterion) {
    let disk = UInt64Chunked::from_index_iter(
        H3Cell::from_coordinate(Coordinate::from((12.3, 45.4)), 8)
            .unwrap()
            .grid_disk(100)
            .unwrap()
            .iter(),
    );
    let aoi = Rect::new((12.28, 45.35), (12.35, 45.45));
    let cellchunked = disk.h3cell();

    bench_spatialindex(c, |ic| ic.kdtree_index(), &cellchunked, &aoi, "kdtree");
    bench_spatialindex(
        c,
        |ic| ic.packed_hilbert_rtree_index().unwrap(),
        &cellchunked,
        &aoi,
        "packed_hilbert_rtree",
    );
    bench_spatialindex(c, |ic| ic.rtree_index(), &cellchunked, &aoi, "rtree");
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

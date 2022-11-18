use criterion::{criterion_group, criterion_main, Criterion};
use geo_types::{Coord, Rect};
use polars_core::prelude::UInt64Chunked;

use h3ron::H3Cell;
use h3ron_polars::spatial_index::{
    BuildKDTreeIndex, BuildPackedHilbertRTreeIndex, BuildRTreeIndex, SIKind, SpatialIndex,
    SpatialIndexGeomOp,
};
use h3ron_polars::{AsH3CellChunked, FromIndexIterator, IndexChunked};

fn bench_spatialindex<Builder, SI, Kind>(c: &mut Criterion, builder: Builder, name: &str)
where
    SI: SpatialIndex<H3Cell, Kind> + SpatialIndexGeomOp<H3Cell, Kind>,
    Kind: SIKind,
    Builder: Fn(&IndexChunked<H3Cell>) -> SI,
{
    let (disk, aoi) = build_inputs();
    let cellchunked = disk.h3cell();

    let mut group = c.benchmark_group(format!("{}-{}-cells", name, cellchunked.len()));
    group.bench_with_input("build", &cellchunked, |bencher, cellchunked| {
        bencher.iter(|| {
            let _ = builder(cellchunked);
        });
    });

    let si = builder(&cellchunked);
    group.bench_with_input("envelopes_intersect", &si, |bencher, si| {
        bencher.iter(|| {
            let _ = si.envelopes_intersect(&aoi);
        });
    });

    group.bench_with_input("geometries_intersect", &si, |bencher, si| {
        bencher.iter(|| {
            let _ = si.geometries_intersect(&aoi);
        });
    });
    group.finish();
}

fn build_inputs() -> (UInt64Chunked, Rect) {
    let disk = UInt64Chunked::from_index_iter(
        H3Cell::from_coordinate(Coord::from((12.3, 45.4)), 8)
            .unwrap()
            .grid_disk(100)
            .unwrap()
            .iter(),
    );
    let aoi = Rect::new((12.28, 45.35), (12.35, 45.45));

    (disk, aoi)
}

fn bench_kdtree(c: &mut Criterion) {
    bench_spatialindex(c, |ic| ic.kdtree_index(), "kdtree")
}

fn bench_rtree(c: &mut Criterion) {
    bench_spatialindex(c, |ic| ic.rtree_index(), "rtree")
}

fn bench_packed_hilbert_rtree(c: &mut Criterion) {
    bench_spatialindex(
        c,
        |ic| ic.packed_hilbert_rtree_index().unwrap(),
        "packed_hilbert_rtree",
    )
}

criterion_group!(
    benches,
    bench_kdtree,
    bench_rtree,
    bench_packed_hilbert_rtree
);
criterion_main!(benches);

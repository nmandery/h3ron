use std::path::Path;

use criterion::{black_box, Criterion, criterion_group, criterion_main};
use gdal::raster::Dataset;

use h3_raster::input::{ClassifiedBand, NoData};
use h3_raster::input::Value::Uint8;
use h3_raster::rasterconverter::RasterConverter;
use h3_raster::tile::Dimensions;

fn convert_r_dataset(h3_res: u8, n_threads: usize) {
    let filename = format!("{}/../data/r.tiff", env!("CARGO_MANIFEST_DIR"));
    let ds = Dataset::open(Path::new(&filename)).unwrap();
    let band_idx = 1;
    let tile_size = Dimensions {width: 250, height: 250};

    let inputs = vec![
        ClassifiedBand {
            source_band: band_idx as u8,
            classifier: Box::new(NoData::new(Uint8(0))),
        }
    ];
    let converter = RasterConverter::new(ds, inputs, h3_res).unwrap();
    let _ = converter.convert(n_threads as u32, &tile_size, true).unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("raster conversion");
    group.sample_size(10);
    //group.measurement_time(Duration::new(60 * 5, 0));
    for h3_res in [4, 8, 10, 13].iter() {
        for n_threads in [/*1,*/ 4].iter() {
            group.bench_function(
                format!("convert_r_dataset_h3_res_{}_n_threads_{}", h3_res, n_threads),
                |b| b.iter(|| convert_r_dataset(black_box(*h3_res), *n_threads))
            );
        }
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
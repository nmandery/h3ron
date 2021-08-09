use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gdal::Dataset;
use ndarray::{Array2, ArrayView, Ix2};

use h3ron_ndarray::array::H3Converter;
use h3ron_ndarray::transform::Transform;
use h3ron_ndarray::AxisOrder;

fn load_r_dataset() -> (Array2<u8>, Transform) {
    let filename = format!("{}/../data/r.tiff", env!("CARGO_MANIFEST_DIR"));
    let dataset = Dataset::open((&filename).as_ref()).unwrap();
    let transform = Transform::from_gdal(&dataset.geo_transform().unwrap());
    let band = dataset.rasterband(1).unwrap();
    let band_array = band
        .read_as_array::<u8>((0, 0), band.size(), band.size(), None)
        .unwrap();
    (band_array, transform)
}

fn convert_r_dataset<'a>(
    view: &'a ArrayView<'a, u8, Ix2>,
    transform: &'a Transform,
    h3_resolution: u8,
) {
    let conv = H3Converter::new(view, &Some(0_u8), transform, AxisOrder::XY);
    let _ = conv.to_h3(h3_resolution, true).unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    let (band_array, transform) = load_r_dataset();
    let band_view = band_array.view();
    let mut group = c.benchmark_group("raster conversion");
    group.sample_size(10);
    //group.measurement_time(Duration::new(60 * 5, 0));
    for h3_res in [11].iter() {
        group.bench_function(format!("convert_r_dataset_h3_res_{}", h3_res), |b| {
            b.iter(|| convert_r_dataset(&band_view, &transform, black_box(*h3_res)))
        });
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

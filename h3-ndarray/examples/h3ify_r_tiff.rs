use gdal::{
    Dataset,
};
use h3_ndarray::{
    resolution::{
        nearest_h3_resolution,
        NearestH3ResolutionSearchMode::SmallestAreaDifference,
    },
    transform::Transform,
};

fn main() {
    let filename = format!("{}/../data/r.tiff", env!("CARGO_MANIFEST_DIR"));
    let dataset = Dataset::open((&filename).as_ref()).unwrap();
    let transform = Transform::from_gdal(&dataset.geo_transform().unwrap());
    let band = dataset.rasterband(1).unwrap();
    let band_array = band.read_as_array::<u8>(
        (0, 0),
        band.size(),
        band.size(),
    ).unwrap();

    let h3_resolution = nearest_h3_resolution(band_array.shape(), &transform, SmallestAreaDifference).unwrap();
    println!("selected H3 resolution: {}", h3_resolution);
}
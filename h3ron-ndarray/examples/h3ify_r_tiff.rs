use gdal::{
    vector::{Defn, Feature, FieldDefn, OGRFieldType, ToGdal},
    Dataset, Driver,
};

use h3ron::{H3Cell, Index, ToPolygon};
use h3ron_ndarray::{AxisOrder, H3Converter, ResolutionSearchMode::SmallerThanPixel, Transform};
use std::convert::TryFrom;

fn main() {
    env_logger::init(); // run with the environment variable RUST_LOG set to "debug" for log output

    let filename = format!("{}/../data/r.tiff", env!("CARGO_MANIFEST_DIR"));
    let dataset = Dataset::open((&filename).as_ref()).unwrap();
    let transform = Transform::from_gdal(&dataset.geo_transform().unwrap());
    let band = dataset.rasterband(1).unwrap();
    let band_array = band
        .read_as_array::<u8>((0, 0), band.size(), band.size(), None)
        .unwrap();

    let view = band_array.view();
    let conv = H3Converter::new(&view, &Some(0_u8), &transform, AxisOrder::YX);

    let h3_resolution = conv.nearest_h3_resolution(SmallerThanPixel).unwrap();
    println!("selected H3 resolution: {}", h3_resolution);

    let results = conv.to_h3(h3_resolution, true).unwrap();
    results.iter().for_each(|(value, index_stack)| {
        println!("{} -> {}", value, index_stack.len());
    });

    // write to vector file
    let out_drv = Driver::get("GPKG").unwrap();
    let mut out_dataset = out_drv
        .create_vector_only("h3ify_r_tiff_results.gpkg")
        .unwrap();
    let out_lyr = out_dataset.create_layer_blank().unwrap();

    let h3index_field_defn = FieldDefn::new("h3index", OGRFieldType::OFTString).unwrap();
    h3index_field_defn.set_width(20);
    h3index_field_defn.add_to_layer(&out_lyr).unwrap();

    let h3res_field_defn = FieldDefn::new("h3res", OGRFieldType::OFTInteger).unwrap();
    h3res_field_defn.add_to_layer(&out_lyr).unwrap();

    let defn = Defn::from_layer(&out_lyr);

    results.iter().for_each(|(_value, index_stack)| {
        for h3index in index_stack.iter_compacted_indexes() {
            let index = H3Cell::try_from(h3index).unwrap();
            let mut ft = Feature::new(&defn).unwrap();
            ft.set_geometry(index.to_polygon().to_gdal().unwrap())
                .unwrap();
            ft.set_field_string("h3index", &index.to_string()).unwrap();
            ft.set_field_integer("h3res", index.resolution() as i32)
                .unwrap();
            ft.create(&out_lyr).unwrap();
        }
    });
}

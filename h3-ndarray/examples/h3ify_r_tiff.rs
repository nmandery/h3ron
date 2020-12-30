use gdal::{Dataset, Driver};
use h3_ndarray::{
    resolution::{
        nearest_h3_resolution,
        NearestH3ResolutionSearchMode::SmallestAreaDifference,
    },
    transform::Transform,
};
use h3_ndarray::array::H3Converter;
use gdal::vector::{FieldDefn, OGRFieldType, Defn, Feature, ToGdal};
use h3::index::Index;

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

    let view = band_array.view();
    let conv = H3Converter::new(&view, &0_u8, &transform);
    let results = conv.to_h3(h3_resolution, true).unwrap();
    results.iter().for_each(|(value, index_stack)| {
        println!("{} -> {}", value, index_stack.len());
    });

    // write to vector file
    let out_drv = Driver::get("GeoJSON").unwrap();
    let mut out_dataset = out_drv.create_vector_only("results.json").unwrap();
    let out_lyr = out_dataset.create_layer_blank().unwrap();

    let h3index_field_defn = FieldDefn::new("h3index", OGRFieldType::OFTString).unwrap();
    h3index_field_defn.set_width(20);
    h3index_field_defn.add_to_layer(&out_lyr).unwrap();


    let defn = Defn::from_layer(&out_lyr);

    results.iter().for_each(|(_value, index_stack)| {
        index_stack.indexes_by_resolution.iter().for_each(|(_res, h3indexes)| {
            for h3index in h3indexes {
                let index = Index::from(*h3index);
                let mut ft = Feature::new(&defn).unwrap();
                ft.set_geometry(index.polygon().to_gdal().unwrap()).unwrap();
                ft.set_field_string("h3index", &index.to_string()).unwrap();
                ft.create(&out_lyr).unwrap();
            }
        })
    });
}
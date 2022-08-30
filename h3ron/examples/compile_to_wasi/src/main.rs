use geo_types::Coordinate;
use h3ron::H3Cell;

fn main() {
    println!(
        "Hello world: {:?}",
        H3Cell::from_coordinate(Coordinate::from((4.5, 23.4)), 7).unwrap()
    );
}

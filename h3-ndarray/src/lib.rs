
#[macro_use]
extern crate ndarray;

#[cfg(test)]
#[macro_use]
extern crate approx;

mod algo;
pub mod transform;
pub mod error;
pub mod sphere;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

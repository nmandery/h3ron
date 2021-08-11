use geo_types::{Coordinate, Rect};

use crate::{
    error::Error,
    sphere::{area_squaremeters_linearring, area_squaremeters_rect},
    transform::Transform,
    AxisOrder,
};

use h3ron::{H3Cell, ToPolygon, H3_MAX_RESOLUTION, H3_MIN_RESOLUTION};

pub enum ResolutionSearchMode {
    /// Chose the h3 resolution where the difference in the area of a pixel and the h3index is
    /// as small as possible.
    MinDiff,

    /// Chose the h3 resolution where the area of the h3index is smaller than the area of a pixel.
    SmallerThanPixel,
}

/// Find the h3 resolution closed to the size of a pixel in an array
/// of the given shape with the given transform.
pub fn nearest_h3_resolution(
    shape: &[usize],
    transform: &Transform,
    axis_order: &AxisOrder,
    search_mode: ResolutionSearchMode,
) -> Result<u8, Error> {
    if shape.len() != 2 {
        return Err(Error::UnsupportedArrayShape);
    }
    if shape[0] == 0 || shape[1] == 0 {
        return Err(Error::EmptyArray);
    }
    let bbox_array = Rect::new(
        transform * &Coordinate::from((0.0_f64, 0.0_f64)),
        transform
            * &Coordinate::from((
                (shape[axis_order.x_axis()] - 1) as f64,
                (shape[axis_order.y_axis()] - 1) as f64,
            )),
    );
    let area_pixel = area_squaremeters_rect(&bbox_array)
        / (shape[axis_order.x_axis()] * shape[axis_order.y_axis()]) as f64;
    let center_of_array = bbox_array.center();

    let mut nearest_h3_res = 0;
    let mut area_difference = None;
    for h3_res in H3_MIN_RESOLUTION..=H3_MAX_RESOLUTION {
        // calculate the area of the center index to avoid using the approximate values
        // of the h3ron hexArea functions
        let area_h3_index = area_squaremeters_linearring(
            H3Cell::from_coordinate_unchecked(&center_of_array, h3_res)
                .to_polygon()
                .exterior(),
        );

        match search_mode {
            ResolutionSearchMode::SmallerThanPixel => {
                if area_h3_index <= area_pixel {
                    nearest_h3_res = h3_res;
                    break;
                }
            }

            ResolutionSearchMode::MinDiff => {
                let new_area_difference = if area_h3_index > area_pixel {
                    area_h3_index - area_pixel
                } else {
                    area_pixel - area_h3_index
                };
                if let Some(old_area_difference) = area_difference {
                    if old_area_difference < new_area_difference {
                        nearest_h3_res = h3_res - 1;
                        break;
                    } else {
                        area_difference = Some(new_area_difference);
                    }
                } else {
                    area_difference = Some(new_area_difference);
                }
            }
        }
    }

    Ok(nearest_h3_res)
}

#[cfg(test)]
mod tests {
    use crate::resolution::{nearest_h3_resolution, ResolutionSearchMode};
    use crate::transform::Transform;
    use crate::AxisOrder;

    #[test]
    fn test_nearest_h3_resolution() {
        // transform of the included r.tiff
        let gt = Transform::from_rasterio(&[
            0.0011965049999999992,
            0.0,
            8.11377,
            0.0,
            -0.001215135,
            49.40792,
        ]);
        let h3_res1 = nearest_h3_resolution(
            &[2000_usize, 2000_usize],
            &gt,
            &AxisOrder::YX,
            ResolutionSearchMode::MinDiff,
        )
        .unwrap();
        assert_eq!(h3_res1, 10); // TODO: validate

        let h3_res2 = nearest_h3_resolution(
            &[2000_usize, 2000_usize],
            &gt,
            &AxisOrder::YX,
            ResolutionSearchMode::SmallerThanPixel,
        )
        .unwrap();
        assert_eq!(h3_res2, 11); // TODO: validate
    }
}

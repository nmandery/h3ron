#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

mod test {
    use std::os::raw::c_int;

    #[test]
    fn test_geo_to_h3() {
        unsafe {
            let input = GeoCoord {
                lat: degsToRads(95.12345),
                lon: degsToRads(-21.12345),
            };
            let res: c_int = 5;
            let output = geoToH3(&input, res);
            assert_eq!(output, 599073528557338623);
            assert_eq!(format!("{:x}", output), "85056333fffffff")
        }
    }

    #[test]
    fn test_to_string() {
        unsafe {
            let index: H3Index = 599073528557338623;

            let mut buf = vec![0u8; 17];
            h3ToString(index, buf.as_mut_ptr() as *mut i8, buf.capacity());
            assert_eq!(String::from_utf8(buf).unwrap().trim_end_matches('\0'), "85056333fffffff");
        }
    }
}

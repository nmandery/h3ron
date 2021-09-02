use std::io;

use crate::Error;
use lz4_flex::frame::{FrameDecoder, FrameEncoder};
use serde::Serialize;

/// hide bincode errors in the io error to avoid having bincode in the public api.
impl From<bincode::Error> for Error {
    fn from(b_err: bincode::Error) -> Self {
        Self::IOError(std::io::Error::new(std::io::ErrorKind::Other, b_err))
    }
}

/// hide lz4_flex errors in the io error to avoid having them in the public api.
impl From<lz4_flex::frame::Error> for Error {
    fn from(f_err: lz4_flex::frame::Error) -> Self {
        Self::IOError(std::io::Error::new(std::io::ErrorKind::Other, f_err))
    }
}

pub fn serialize_into<W, T: ?Sized>(writer: W, value: &T, compress: bool) -> Result<(), Error>
where
    W: io::Write,
    T: Serialize,
{
    if compress {
        let mut encoder = FrameEncoder::new(writer);
        bincode::serialize_into(&mut encoder, value)?;
        encoder.finish()?;
    } else {
        bincode::serialize_into(writer, value)?;
    };
    Ok(())
}

pub fn deserialize_from<R, T>(reader: R) -> Result<T, Error>
where
    R: io::Read + io::Seek,
    T: serde::de::DeserializeOwned,
{
    let mut decoder = FrameDecoder::new(reader);
    let deserialized = match bincode::deserialize_from(&mut decoder) {
        Err(_) => {
            let original_reader = decoder.get_mut();
            original_reader.seek(io::SeekFrom::Start(0))?;
            bincode::deserialize_from(original_reader)?
        }
        Ok(des) => des,
    };
    Ok(deserialized)
}

#[cfg(test)]
mod tests {
    use crate::io::{deserialize_from, serialize_into};
    use std::io::Cursor;

    fn roundtrip(compress: bool) {
        let data = vec![1_i32, 2, 3, 4];
        let mut data_bytes: Vec<u8> = vec![];
        serialize_into(Cursor::new(&mut data_bytes), &data, compress).unwrap();
        assert!(data_bytes.len() > 0);
        let data2: Vec<i32> = deserialize_from(Cursor::new(&data_bytes)).unwrap();
        assert_eq!(data, data2);
    }

    #[test]
    fn test_roundtrip_no_compression() {
        roundtrip(false);
    }

    #[test]
    fn test_roundtrip_compression() {
        roundtrip(true);
    }
}

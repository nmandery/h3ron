use std::fmt;
use std::error;

#[derive(Debug)]
pub enum Error {
    InvalidSRS,
    BandOutOfRange,
    H3ResolutionOutOfRange,
    NoGeotransformFound,
    GeotransformFailed,
    BandNotReadable(u8),
    GDAL(gdal::errors::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidSRS => write!(f, "Dataset has to be WGS:84"),
            Error::BandOutOfRange => write!(f, "Band out of range"),
            Error::H3ResolutionOutOfRange => write!(f, "H3 resolution must be between 0 and 15"),
            Error::NoGeotransformFound => write!(f, "Dataset has no geotransform"),
            Error::GeotransformFailed => write!(f, "Geotransform failed"),
            Error::BandNotReadable(band_idx) => write!(f, "Band {} can not be read", band_idx),
            Error::GDAL(gdal_err) => write!(f, "GDAL: {:?}", gdal_err)
        }
    }
}


impl error::Error for Error {
    /*
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::GDAL(gdal_err) => Some(gdal_err),
            _ => None
        }
    }

     */
}

impl From<gdal::errors::Error> for Error {
    fn from(gdal_err: gdal::errors::Error) -> Self {
        Error::GDAL(gdal_err)
    }
}
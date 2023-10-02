use std::{path::{Path, PathBuf}, ffi::OsStr};

use chrono::{DateTime, FixedOffset};

#[derive(Debug, thiserror::Error)]
pub enum CoordinateError {
    #[error("Cannot read coordinate file {0}: {1}")]
    CannotReadFile(PathBuf, std::io::Error),
    #[error("Error deserializing {0}: {1}")]
    DeserializationError(PathBuf, serde_json::Error),
    #[error("Received a coordinate file with an unimplemented file extension: {0}")]
    UnknownExtension(PathBuf),
    #[error("Received a coordinate file with invalid UTF-8 in its extension: {0}")]
    InvalidExtension(PathBuf),
}


/// An enum representing a source for geographic coordinates where the EM27 was located
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum CoordinateSource {
    /// This indicates the EM27 was at a fixed location for all of the associated measurements.
    /// It will return the same coordinates no matter what time is queried.
    Fixed{latitude: f32, longitude: f32, altitude: f32}
}

impl CoordinateSource {
    /// Load coordinates from a file. It will try to detect what format the file
    /// is from the extension and to infer which `CoordinateSource` variant the
    /// file represents from its contents.
    /// 
    /// Supported file formats:
    /// - `.json`
    pub fn load_file(coord_file: &Path) -> Result<Self, CoordinateError> {
        match CoordinateFileType::try_from(coord_file)? {
            CoordinateFileType::Json => Self::load_json(coord_file),
        }
    }

    fn load_json(coord_json_file: &Path) -> Result<Self, CoordinateError> {
        let reader = std::fs::File::open(coord_json_file)
            .map_err(|e| CoordinateError::CannotReadFile(coord_json_file.to_path_buf(), e))?;
        serde_json::from_reader(reader)
            .map_err(|e| CoordinateError::DeserializationError(coord_json_file.to_path_buf(), e))
    }

    /// Return the coordinates where the EM27 was for a given datetime.
    /// The return values are latitude (south is negative), longitude (west is negative),
    /// and altitude (in meters).
    pub fn get_coords_for_datetime(&self, _datetime: DateTime<FixedOffset>) -> (f32, f32, f32) {
        match self {
            CoordinateSource::Fixed { latitude, longitude, altitude } => (*latitude, *longitude, *altitude),
        }
    }

}


#[derive(Debug, Clone)]
enum CoordinateFileType {
    Json,
}

impl TryFrom<&Path> for CoordinateFileType {
    type Error = CoordinateError;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        let extension = value.extension()
            .unwrap_or(OsStr::new(""))
            .to_str()
            .ok_or_else(|| CoordinateError::InvalidExtension(value.to_path_buf()))?;

        match extension {
            "json" => Ok(Self::Json),
            _ => Err(CoordinateError::UnknownExtension(value.to_path_buf()))
        }
    }
}
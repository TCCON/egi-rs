use std::{path::{Path, PathBuf}, fmt::Display};

use chrono::{FixedOffset, DateTime};
use error_stack::Context;
use serde::Deserialize;

use ggg_rs::utils::EncodingError;

use crate::path_relative_to_config;
mod jpl_vaisala;
mod cit_csv;


/// This struct indicates an error while reading input met data and interpolating it to 
/// the ZPD time of EM27 interferograms.
#[derive(Debug)]
pub struct MetError {
    met_source_type: MetSource,
    /// An enum describing the reason for the error.
    pub reason: MetErrorType
}

impl Display for MetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error reading met data from {}: {}", self.met_source_type.long_string(), self.reason)
    }
}

impl Context for MetError {}

/// An enum describing the reason interpolating met data to interferogram ZPDs failed
#[derive(Debug, thiserror::Error)]
pub enum MetErrorType {
    /// This represents a problem reading the contents of the met file. This might include
    /// the file not existing or not being readable (due to permissions).
    #[error("Could not open/read file due to: {0}")]
    IoError(#[from] EncodingError),

    #[error("Could not deserialize met config file: {0}")]
    DeserializationError(#[from] serde_json::Error),

    #[error("Problem with the met config file: {0}")]
    ConfigError(String),

    /// This represents a problem parsing the values stored in the met file
    #[error("Error during parsing: {0}")]
    ParsingError(String),

    /// This error indicates that a met file cannot be matched up with a given batch of interferograms
    /// because the interferograms do not have a consistent time zone. This error arises when a met input
    /// file includes timestamps without a time zone identified and the GMT offsets in the interferograms'
    /// header are not all the same. When a met input type does not include a timezone, we must assume that
    /// the met file has the same GMT offset as the interferograms it is being matched up with. When the
    /// interferograms have differing GMT offsets, this assumption is not straightforward.
    #[error("This met type requires that all interferograms being matched with it have the same time zone.")]
    BadTimezoneError
}

impl From<jpl_vaisala::JplMetError> for MetErrorType {
    fn from(value: jpl_vaisala::JplMetError) -> Self {
        match value {
            jpl_vaisala::JplMetError::IoError(e) => MetErrorType::IoError(e.into()),
            jpl_vaisala::JplMetError::EncodingError(e) => MetErrorType::IoError(e),
            jpl_vaisala::JplMetError::HeaderLineMissing => MetErrorType::ParsingError(value.to_string()),
            jpl_vaisala::JplMetError::HeaderMissingFields(_) => MetErrorType::ParsingError(value.to_string()),
            jpl_vaisala::JplMetError::LineTooShort(_) => MetErrorType::ParsingError(value.to_string()),
            jpl_vaisala::JplMetError::ParsingError(_, _) => MetErrorType::ParsingError(value.to_string()),
            jpl_vaisala::JplMetError::InvalidTime(_, _, _) => MetErrorType::ParsingError(value.to_string()),
        }
    }
}

impl From<cit_csv::CitMetError> for MetErrorType {
    fn from(value: cit_csv::CitMetError) -> Self {
        match value {
            cit_csv::CitMetError::IoError(e) => MetErrorType::IoError(e.into()),
            cit_csv::CitMetError::UnknownSite(_) => MetErrorType::ConfigError(value.to_string()),
            cit_csv::CitMetError::HeaderLineMissing(_) => MetErrorType::ParsingError(value.to_string()),
            cit_csv::CitMetError::UnexpectedColumn { file: _, col_index: _, expected: _ } => MetErrorType::ParsingError(value.to_string()),
            cit_csv::CitMetError::LineTooShort(_, _) => MetErrorType::ParsingError(value.to_string()),
            cit_csv::CitMetError::ParsingError { file: _, line: _, col: _, reason: _ } => MetErrorType::ParsingError(value.to_string()),
            cit_csv::CitMetError::TimeMismatch { file1: _, file2: _, cause: _ } => MetErrorType::ParsingError(value.to_string()),
            cit_csv::CitMetError::TimezoneError(_) => MetErrorType::ParsingError(value.to_string()),
        }
    }
}

/// A structure represting a single set of meteorology measurements for one time
#[derive(Debug)]
pub struct MetEntry {
    /// The time & date (with time zone) of the met data, note that it is assumed that
    /// the measurements are instantaneous at this time.
    pub datetime: chrono::DateTime<chrono::FixedOffset>,

    /// Temperature in degrees Celsius
    pub temperature: f64,

    /// Pressure in hPa
    pub pressure: f64,

    /// Relative humidity in percent (i.e. values should be in the range 0 to 100)
    pub humidity: f64
}


/// An enum representing different possible met sources
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum MetSource {
    /// Met data was recorded using the original version of the JPL Powershell script.
    /// The JSON file corresponding to this variant would look like:
    /// ```json
    /// {
    ///   "type": "JplVaisalaV1",
    ///   "file": "./20230826_vaisala.txt"
    /// }
    /// ```
    /// 
    /// The value of "type" must be *exactly* "JplVaisalaV1". The value of "file" must
    /// be a path that points to a file written by the JPL Powershell script. The expected
    /// format of that file is:
    /// 
    /// ```text
    /// YYYYMMDD,HH:MM,Data,Temperature,Humidity,Pressure
    /// 20230826,16:14,0R2,Ta=0.0#,Ua=0.0#,Pa=0.0#
    /// 20230826,16:15,0R2,Ta=26.8C,Ua=39.3P,Pa=972.7H
    /// 20230826,16:16,0R2,Ta=26.8C,Ua=40.3P,Pa=972.7H
    /// ```
    /// 
    /// If the path for "file" is relative, it is interpreted as relative to the location
    /// of the met source file. That is, the example above means that the file
    /// `20230826_vaisala.txt` must be in the same directory as the JSON file.
    JplVaisalaV1{file: PathBuf},

    /// Met data download from a Caltech weather station through http://tccon-weather.caltech.edu/index.php.
    /// The JSON file corresponding to this variant would look like:
    /// ```json
    /// {
    ///   "type": "CitCsvV1",
    ///   "site": "ci",
    ///   "pres_file": "./2023-06-23-Pressure.csv",
    ///   "temp_file": "./2023-06-23-Temp.csv",
    ///   "humid_file": "./2023-06-23-Humidity.csv"
    /// }
    /// ```
    /// 
    /// The value of "type" must be *exactly* "CitCsvV1". The value of "site" must be one
    /// of "ci", "oc", "df", or "pa" and is the TCCON site from which the met data was
    /// taken. The value of "pres_file" must be a path to a file downloaded from the above
    /// URL with pressures for the day(s) you are making a catalog for. Its contents will be
    /// similar to:
    /// 
    /// ```text
    /// Time,"Pressure (mb)"
    /// "2023-06-23 00:00:14",986.9
    /// "2023-06-23 00:05:14",986.9
    /// "2023-06-23 00:10:14",986.9
    /// ```
    /// 
    /// "temp_file" and "humid_file" are optional (but highly recommended) and would point
    /// to the files for temperature and humidity, respectively. If any of these paths are
    /// relative, they are interpreted as relative to the configuration JSON file.
    CitCsvV1{pres_file: PathBuf, site: String, temp_file: Option<PathBuf>, humid_file: Option<PathBuf>},
}

impl MetSource {
    /// Create a `MetSource` instance from a JSON file.
    /// 
    /// Because different sources of met data may have different numbers and types of inputs
    /// (e.g. one or many files, database URLs, etc.), these must be defined by a configuration.
    /// Any paths in the configuration file (e.g. pointing to input files) can be absolute or
    /// relative. If relative, they are converted into absolute paths in the returned structure
    /// and those absolute paths are calculated by considering the relative paths relative to
    /// the directory containing the configuration file. That is, if the file is in `/home/data/config`
    /// and the path is `../met`, then the returned structure will contain the absolute path
    /// `/home/data/config/../met` i.e. `/home/data/met`.
    /// 
    /// # File examples
    /// 
    /// A valid JSON for the `JplVaisalaV1` met source is:
    /// 
    /// ```json
    /// {
    ///   "type": "JplVaisalaV1",
    ///   "file": "./20230826_vaisala.txt"
    /// }
    /// ```
    /// 
    /// A valid JSON for the `CitCsvV1` met source is:
    /// 
    /// ```json
    /// {
    ///   "type": "CitCsvV1",
    ///   "site": "ci",
    ///   "pres_file": "./2023-06-23-Pressure.csv",
    ///   "temp_file": "./2023-06-23-Temp.csv",
    ///   "humid_file": "./2023-06-23-Humidity.csv"
    /// }
    /// ```
    pub fn from_config_json(config_file: &Path) -> Result<Self, MetErrorType> {
        let reader = std::fs::File::open(config_file)
            .map_err(|e| EncodingError::IoError(e))?;
        let this: Self = serde_json::from_reader(reader)?;
        match this {
            MetSource::JplVaisalaV1{file} => {
                let file = path_relative_to_config(config_file, file);
                Ok(Self::JplVaisalaV1{file})
            },
            MetSource::CitCsvV1 { pres_file, site, temp_file, humid_file } => {
                let pres_file = path_relative_to_config(config_file, pres_file);
                let temp_file = temp_file.map(|p| path_relative_to_config(config_file, p));
                let humid_file = humid_file.map(|p| path_relative_to_config(config_file, p));
                Ok(Self::CitCsvV1 { pres_file, site, temp_file, humid_file })
            }
        }
    }

    /// This is a wrapper around `from_config_json` needed for parsing command line arguments.
    /// It can be used as the `value_parser` argument in a [`clap::Arg`], e.g.:
    /// 
    /// ```
    /// #[derive(Debug, clap::Parser)]
    /// struct Cli {
    ///     #[clap(long, value_parser = MetSource::from_clarg)]
    ///     met_source: MetSource
    /// }
    /// ```
    /// 
    /// Otherwise, you should prefer `from_config_json` whenever possible.
    pub fn from_clarg(config_file: &str) -> Result<Self, MetErrorType> {
        let p = PathBuf::from(config_file);
        Self::from_config_json(&p)
    }

    /// Return a string including input paths suitable for display in error messages.
    fn long_string(&self) -> String {
        match self {
            MetSource::JplVaisalaV1{file} => format!("JPL Vaisala V1 (file {})", file.display()),
            MetSource::CitCsvV1 { pres_file, site, temp_file: _, humid_file: _ } => format!("CIT CSV V1 ({site}, pres_file = {})", pres_file.display()),
        }
    }
}

impl Display for MetSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetSource::JplVaisalaV1{file: _} => write!(f, "JplVaisalaV1"),
            MetSource::CitCsvV1 { pres_file: _, site: _, temp_file: _, humid_file: _ } => write!(f, "CitCsvV1"),
        }
    }
}

/// This enum represents the distribution of timezones (i.e UTC offsets) in a collection of data.
/// It is mainly used to check if a met file without an explicit timezone defined for its timestamps
/// can be matched up with a set of interferograms.
pub enum Timezones {
    /// This variant represents either (a) no timezones defined or (b) no available datetimes
    None,
    /// This variant indicates that a collection of datetimes all have the same time zone. That
    /// time zone is carried as the inner value of this variant.
    One(FixedOffset),

    /// This variant indicates that a collection of datetimes have 2 or more time zones among them.
    Multiple
}

impl Timezones {
    /// Given an iterator over datetimes, return the appropriate `Timezones` instance to represent that collection
    /// of datetimes.
    pub fn check_consistent_timezones<I: Iterator<Item = DateTime<FixedOffset>>>(datetimes: I) -> Self {
        let mut offset = None;
        for dt in datetimes {
            let this_offset = dt.offset();
            if let Some(o) = offset {
                if &o != this_offset {
                    return Self::Multiple
                }
            } else {
                offset = Some(this_offset.to_owned());
            }
        }

        if let Some(o) = offset {
            Self::One(o.to_owned())
        } else {
            Self::None
        }
    }

    /// If this is an instance of `Timezones::One`, return the contained timezone. Otherwise return a `BadTimezoneError`.
    fn try_unwrap_one(self) -> Result<FixedOffset, MetErrorType> {
        if let Self::One(tz) = self {
            Ok(tz)
        } else {
            Err(MetErrorType::BadTimezoneError)
        }
    }
}

/// Read a met file or a given type. 
/// 
/// # Inputs
/// - `met_file`: path to the file to be read
pub fn read_met_file(met_type: &MetSource, em27_tz_offset: Timezones) -> Result<Vec<MetEntry>, MetError> {
    
    match met_type {
        MetSource::JplVaisalaV1{file} => {
            let tz = get_em27_tz(em27_tz_offset, met_type)?;
            jpl_vaisala::read_jpl_vaisala_met(file, tz)
                .map_err(|e| {
                    MetError{met_source_type: met_type.to_owned(), reason: e.into()}
                })
        },
        MetSource::CitCsvV1 { pres_file, site, temp_file, humid_file } => {
            cit_csv::read_cit_csv_met(pres_file, site, temp_file.as_deref(), humid_file.as_deref())
                .map_err(|e| {
                    MetError{met_source_type: met_type.to_owned(), reason: e.into()}
                })
        }
    }
}

fn get_em27_tz(em27_tz_offset: Timezones, met_type: &MetSource) -> Result<FixedOffset, MetError> {
    em27_tz_offset.try_unwrap_one()
        .map_err(|reason| MetError{ met_source_type: met_type.to_owned(), reason})
}
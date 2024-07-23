use std::{path::{Path, PathBuf}, str::FromStr};

use chrono::Timelike;

use ggg_rs::error::DateTimeError;
use ggg_rs::utils::{is_usa_dst, read_unknown_encoding_file};
use itertools::Itertools;

use super::MetEntry;

use crate::CATALOG_FILL_FLOAT_F64;

#[derive(Debug, thiserror::Error)]
pub(super) enum CitMetError {
    #[error("Could not open file: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Unknown TCCON site: {0}")]
    UnknownSite(String),
    #[error("CIT .csv file {} missing header line", .0.display())]
    HeaderLineMissing(PathBuf),
    #[error("CIT .csv. file {} column {col_index} did not contain the expected substring '{expected}'", .file.display())]
    UnexpectedColumn{file: PathBuf, col_index: u8, expected: String},
    #[error("CIT .csv file {} line {1} has fewer than 2 values", .0.display())]
    LineTooShort(PathBuf, usize),
    #[error("Could not parse CIT .csv file {} line {line} column {col}: {reason}", .file.display())]
    ParsingError{file: PathBuf, line: usize, col: u8, reason: String},
    #[error("Times in {} and {} do not match exactly: {cause}", .file1.display(), .file2.display())]
    TimeMismatch{file1: PathBuf, file2: PathBuf, cause: String},
    #[error("Problem computing timezone: {0}")]
    TimezoneError(#[from] DateTimeError),
}

enum TcconMetSite {
    ParkFalls,
    Lamont,
    Caltech,
}

impl FromStr for TcconMetSite {
    type Err = CitMetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ci" => Ok(Self::Caltech),
            "oc" => Ok(Self::Lamont),
            "pa" => Ok(Self::ParkFalls),
            _ => Err(CitMetError::UnknownSite(s.to_string()))
        }
    }
}

impl TcconMetSite {
    fn add_timezone(&self, datetime: chrono::NaiveDateTime) -> Result<chrono::DateTime<chrono::FixedOffset>, DateTimeError> {
        let is_dst = is_usa_dst(datetime)?;
        let utc_offset = match (self, is_dst) {
            (Self::ParkFalls | Self::Lamont, false) => -6,
            (Self::ParkFalls | Self::Lamont, true) => -5,
            (Self::Caltech, false) => -8,
            (Self::Caltech, true) => -7,
        };

        let tz = chrono::FixedOffset::east_opt(utc_offset * 3600).unwrap();
        match datetime.and_local_timezone(tz) {
            chrono::LocalResult::None => Err(
                DateTimeError::InvalidTimezone(
                    format!("{datetime} does not exist in time zone with UTC offset {utc_offset}")
                )
            ),
            chrono::LocalResult::Single(dt) => Ok(dt),
            chrono::LocalResult::Ambiguous(_, _) => Err(
                DateTimeError::InvalidTimezone(
                    format!("{datetime} has multiple representation in time zone with UTC offset {utc_offset}")
                )
            ),
        }
    }
}

pub(super) fn read_cit_csv_met(
    pres_file: &Path,
    site: &str,
    temp_file: Option<&Path>,
    humid_file: Option<&Path>,
) -> Result<Vec<MetEntry>, CitMetError> {
    let site = TcconMetSite::from_str(site)?;

    let (times, pressure) = read_cit_csv(pres_file, "Pressure (mb)")?;
    
    let temperature = if let Some(file) = temp_file {
        let (ttime, temp) = read_cit_csv(file, "Temperature")?;
        check_times(&times, &ttime, pres_file, file)?;
        temp
    } else {
        std::iter::repeat(CATALOG_FILL_FLOAT_F64).take(pressure.len()).collect_vec()
    };

    let humidity = if let Some(file) = humid_file {
        let (htime, humid) = read_cit_csv(file, "Relative Humidity (%)")?;
        check_times(&times, &htime, pres_file, file)?;
        humid
    } else {
        std::iter::repeat(CATALOG_FILL_FLOAT_F64).take(pressure.len()).collect_vec()
    };

    let mut met_entries = vec![];
    for (i, time) in times.into_iter().enumerate() {
        let datetime = chrono::NaiveDateTime::parse_from_str(&time, "%Y-%m-%d %H:%M:%S")
            .map_err(|e| CitMetError::ParsingError { 
                file: pres_file.to_path_buf(), line: i+2, col: 1, reason: e.to_string()
            })?;

        // Skip times between midnight and 3a local. We never take data during those times anyway,
        // and daylight savings time makes them a mess.
        if datetime.hour() < 3 {
            continue;
        }

        let datetime = site.add_timezone(datetime)?;

        let p = pressure[i];
        let t = temperature[i];
        let h = humidity[i];

        met_entries.push(MetEntry{ datetime, temperature: Some(t), pressure: p, humidity: Some(h)})
    }

    Ok(met_entries)
}

fn read_cit_csv(csv_file: &Path, second_colname: &str) -> Result<(Vec<String>, Vec<f64>), CitMetError> {
    let contents = read_unknown_encoding_file(csv_file)
        .map_err(|e| CitMetError::IoError(std::io::Error::other(e)))?;

    let mut lines = contents.as_str().lines();

    // Check that the header has the columns we expect
    if let Some((col1, col2)) = lines.next()
    .ok_or_else(|| CitMetError::HeaderLineMissing(csv_file.to_path_buf()))?
    .split(',')
    .collect_tuple() {
        if !col1.contains("Time") {
            return Err(CitMetError::UnexpectedColumn { file: csv_file.to_path_buf(), col_index: 1, expected: "Time".to_string() });
        }

        if !col2.contains(second_colname) {
            return Err(CitMetError::UnexpectedColumn { file: csv_file.to_path_buf(), col_index: 2, expected: second_colname.to_string() });
        }
    };

    let mut times = vec![];
    let mut met_values = vec![];

    for (iline, line) in lines.enumerate() {
        let values = line.split(',').collect_vec();
        if values.len() < 2 {
            return Err(CitMetError::LineTooShort(csv_file.to_path_buf(), iline + 2));
        }
        let v = values[1].parse::<f64>()
            .map_err(|e| CitMetError::ParsingError { 
                file: csv_file.to_path_buf(), line: iline+2, col: 2, reason: e.to_string()
            })?;
        times.push(values[0].trim_matches('"').to_string());
        met_values.push(v);
    }
    
    Ok((times, met_values))
}

fn check_times(main_times: &[String], new_times: &[String], file1: &Path, file2: &Path) -> Result<(), CitMetError> {
    if main_times.len() != new_times.len() {
        return Err(CitMetError::TimeMismatch { file1: file1.to_path_buf(), file2: file2.to_path_buf(), cause: "different numbers of times".to_string() });
    }

    let mut line_num = 2; // skip the header line and use 1-based index for user messages
    for (t1, t2) in main_times.iter().zip(new_times) {
        if t1 != t2 {
            return Err(CitMetError::TimeMismatch { 
                file1: file1.to_path_buf(), 
                file2: file2.to_path_buf(),
                cause: format!("times on line {line_num} are different")
            });
        }

        line_num += 1;
    }
    
    Ok(())
}
use std::{path::Path, fmt::Display};

use chrono::{FixedOffset, DateTime, NaiveDate, NaiveTime, TimeZone};
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;

use ggg_rs::utils::{read_unknown_encoding_file, EncodingError};

use super::MetEntry;

#[derive(Debug, thiserror::Error)]
pub(super) enum JplMetError {
    #[error("Could not open file: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Could not decode contents of file: {0}")]
    EncodingError(#[from] EncodingError),
    #[error("JPL Vaisala file missing header line")]
    HeaderLineMissing,
    #[error("JPL Vaisala file missing fields from the header: {}", .0.join(", "))]
    HeaderMissingFields(Vec<&'static str>),
    #[error("JPL Vaisala file has a line missing the {0} column")]
    LineTooShort(Col),
    #[error("JPL Vaisala file has a line with a malformed {0} column: {1}")]
    ParsingError(Col, String),
    #[error("JPL Vaisala file has a line with date/time {0} {1} that cannot be converted to {2} as it is either an invalid or ambiguous time for that timezone")]
    InvalidTime(NaiveDate, NaiveTime, FixedOffset),
}

pub(super) fn read_jpl_vaisala_met(met_file: &Path, tz_offset: FixedOffset) -> Result<Vec<MetEntry>, JplMetError> {
    let contents = read_unknown_encoding_file(met_file)?;
    let mut lines = contents.as_str().lines();

    // Identify the indices for the quantities we care about
    let header_line = lines.next()
        .ok_or_else(|| JplMetError::HeaderLineMissing)?;
    let header_line = header_line.split(',').collect_vec();
    let column_inds = header_to_inds(&header_line)?;

    // Convert each line into a met entry. Skip lines that look like "20230826,16:14,0R2,Ta=0.0#,Ua=0.0#,Pa=0.0#",
    // those are weird junk lines that happen usually at the start of the recording.
    let mut met_data = vec![];
    
    for line in lines {
        if line.contains('#') {
            // this is one of those junk lines
            continue;
        }
        let parts = line.split(',').collect_vec();

        let temperature = parse_line_numeric_part(&parts, Col::Temp, &column_inds)?;
        let pressure = parse_line_numeric_part(&parts, Col::Pres, &column_inds)?;
        let humidity = parse_line_numeric_part(&parts, Col::RH, &column_inds)?;
        let datetime = parse_line_datetime(&parts, &column_inds, tz_offset)?;

        met_data.push(MetEntry { datetime, temperature: Some(temperature), pressure, humidity: Some(humidity) })
    }

    Ok(met_data)
}


fn parse_line_datetime(parts: &[&str], inds: &ColInds, offset: FixedOffset) -> Result<DateTime<FixedOffset>, JplMetError> {
    let yyyymmdd_str = parts.get(inds.date)
        .ok_or_else(|| JplMetError::LineTooShort(Col::Date))?;
    let hhmm_str = parts.get(inds.time)
        .ok_or_else(|| JplMetError::LineTooShort(Col::Time))?;

    let date = NaiveDate::parse_from_str(&yyyymmdd_str, "%Y%m%d")
        .map_err(|e| JplMetError::ParsingError(Col::Date, format!("expected YYYYMMDD, got {yyyymmdd_str}. Parsing error was {e}")))?;
    let time = NaiveTime::parse_from_str(&hhmm_str, "%H:%M")
        .map_err(|e| JplMetError::ParsingError(Col::Time, format!("expected HH:MM, got {hhmm_str}. Parsing error was {e}")))?;

    match offset.from_local_datetime(&date.and_time(time)) {
        chrono::LocalResult::Single(t) => Ok(t),
        chrono::LocalResult::None | chrono::LocalResult::Ambiguous(_, _) => Err(JplMetError::InvalidTime(date, time, offset)),
    }
}

fn parse_line_numeric_part(parts: &[&str], col: Col, inds: &ColInds) -> Result<f64, JplMetError> {
    static HUM_PAT: &str = r"Ua=(\d+\.\d+)P";
    static PRES_PAT: &str = r"Pa=(\d+\.\d+)H";
    static TEMP_PAT: &str = r"Ta=(\d+\.\d+)C";
    static RE_HUM: Lazy<Regex> = Lazy::new(|| Regex::new(HUM_PAT).unwrap());
    static RE_PRES: Lazy<Regex> = Lazy::new(|| Regex::new(PRES_PAT).unwrap());
    static RE_TEMP: Lazy<Regex> = Lazy::new(|| Regex::new(TEMP_PAT).unwrap());

    let (i, re, pat) = match col {
        Col::Pres => (inds.pres, &RE_PRES, PRES_PAT),
        Col::Temp => (inds.temp, &RE_TEMP, TEMP_PAT),
        Col::RH => (inds.rh, &RE_HUM, HUM_PAT),
        _ => panic!("Tried to call parse_line_numeric_part with col = {col}, a non-numeric column"),
    };

    let s = parts.get(i)
        .ok_or_else(|| JplMetError::LineTooShort(col))?;

    let s = re.captures(s)
        .map(|c| c.get(1))
        .flatten()
        .ok_or_else(|| JplMetError::ParsingError(col, format!("does not match pattern {pat}")))?
        .as_str();
        

    let v = s.parse::<f64>()
        .map_err(|e| JplMetError::ParsingError(col, e.to_string()))?;

    Ok(v)
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Col {
    Date,
    Time,
    Pres,
    Temp,
    RH
}

impl Display for Col {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Col::Pres => write!(f, "Pressure"),
            Col::Temp => write!(f, "Temperature"),
            Col::RH => write!(f, "Humidity"),
            Col::Date => write!(f, "YYYYMMDD"),
            Col::Time => write!(f, "HH:MM")
        }
    }
}


#[derive(Debug, Default)]
struct ColInds {
    date: usize,
    time: usize,
    pres: usize,
    temp: usize,
    rh: usize
}

fn header_to_inds(header: &[&str]) -> Result<ColInds, JplMetError> {
    let mut inds = ColInds::default();
    let mut missing = vec![];

    if let Some(i) = header.iter().position(|&s| s == "YYYYMMDD") {
        inds.date = i;
    } else {
        missing.push("YYYYMMDD");
    }

    if let Some(i) = header.iter().position(|&s| s == "HH:MM") {
        inds.time = i;
    } else {
        missing.push("HH:MM");
    }

    if let Some(i) = header.iter().position(|&s| s == "Temperature") {
        inds.temp = i;
    } else {
        missing.push("Temperature");
    }

    if let Some(i) = header.iter().position(|&s| s == "Humidity") {
        inds.rh = i;
    } else {
        missing.push("Humidity");
    }

    if let Some(i) = header.iter().position(|&s| s == "Pressure") {
        inds.pres = i;
    } else {
        missing.push("Pressure");
    }

    if missing.is_empty() {
        Ok(inds)
    } else {
        Err(JplMetError::HeaderMissingFields(missing))
    }
}
use std::path::{Path, PathBuf};

use chrono::FixedOffset;
use error_stack::ResultExt;
use serde::Deserialize;

use super::MetEntry;

const MATLAB_UNIX_EPOCH: f64 = 719529.0;

#[derive(Debug, thiserror::Error)]
pub(super) enum LegacyMetError {
    #[error("Invalid time format: {0}")]
    InvalidTimeFormat(String),
    #[error("Invalid time: {0}")]
    InvalidTime(String),
    #[error("Cannot read {}", .0.display())]
    ReadError(PathBuf),
    #[error("Error parsing data line #{0}")]
    CsvError(usize),
}

pub(super) fn read_legacy_met_csv(
    csv_file: &Path,
    em27_tz: FixedOffset,
) -> error_stack::Result<Vec<MetEntry>, LegacyMetError> {
    let f = std::fs::File::open(csv_file)
        .change_context_lazy(|| LegacyMetError::ReadError(csv_file.to_path_buf()))?;

    read_legacy_inner(f, em27_tz)
}

fn read_legacy_inner<R: std::io::Read>(
    input: R,
    em27_tz: FixedOffset,
) -> error_stack::Result<Vec<MetEntry>, LegacyMetError> {
    // A limitation of the CSV crate is that it can only take one comment character
    // We'll use # since that is more standard outside of GGG
    let mut rdr = csv::ReaderBuilder::new()
        .comment(Some(b'#'))
        .trim(csv::Trim::All)
        .from_reader(input);

    let mut entries = vec![];
    for (idx, row) in rdr.deserialize().enumerate() {
        let raw: RawLegacyMetRow = row.change_context_lazy(|| LegacyMetError::CsvError(idx + 1))?;
        let entry = raw
            .to_met_entry(em27_tz)
            .change_context_lazy(|| LegacyMetError::CsvError(idx + 1))?;
        entries.push(entry);
    }

    Ok(entries)
}

fn matlab_to_chrono(mdatenum: f64) -> chrono::NaiveDateTime {
    // 00:00 1 Jan 1970 is 719529.0 as a Matlab date number
    // Date numbers are a number of days since a reference time
    let nsec = ((mdatenum - MATLAB_UNIX_EPOCH) * 24.0 * 3600.0) as i64;
    chrono::DateTime::from_timestamp(nsec, 0)
        .expect("mdatenum is out of the allowed range")
        .naive_utc()
}

#[allow(non_snake_case, unused)]
#[derive(Debug, Deserialize)]
struct RawLegacyMetRow {
    CompSrlDate: Option<f64>,
    CompDate: Option<String>,
    CompTime: Option<String>,
    UTCDate: Option<String>,
    UTCTime: Option<String>,
    Pout: f64,
    Tout: Option<f64>,
    RH: Option<f64>,
    WSPD: Option<f64>,
    WDIR: Option<f64>,
}

impl RawLegacyMetRow {
    fn to_met_entry(self, em27_tz: FixedOffset) -> Result<MetEntry, LegacyMetError> {
        let datetime = if let Some(timestamp) = self.CompSrlDate {
            // Convert a Matlab-style date number and assign it the same timezone as the EM27 interferograms
            let dt = matlab_to_chrono(timestamp);
            dt.and_local_timezone(em27_tz).single().ok_or_else(|| {
                LegacyMetError::InvalidTime(format!(
                    "Matlab-style date number {timestamp} cannot be assigned time zone {em27_tz}"
                ))
            })?
        } else if let (Some(datestr), Some(timestr)) = (&self.CompDate, &self.CompTime) {
            // Convert separate date and time strings and assign them the same timezone as the EM27 interferograms
            let full_datestr = format!("{datestr} {timestr}");
            let dt = chrono::NaiveDateTime::parse_from_str(&full_datestr, "%Y/%m/%d %H:%M:%S")
                .map_err(|_| LegacyMetError::InvalidTimeFormat(
                    format!("computer date and time {datestr} {timestr} does not have the proper format of %Y/%m/%d and %H:%M:%S, respectively")
                ))?;
            dt.and_local_timezone(em27_tz).single().ok_or_else(|| {
                LegacyMetError::InvalidTime(format!("Compute date {datestr} and time {timestr} cannot be assigned time zone {em27_tz}"))
            })?
        } else if let (Some(datestr), Some(timestr)) = (self.UTCDate, self.UTCTime) {
            if self.CompDate.is_some() || self.CompTime.is_some() {
                return Err(LegacyMetError::InvalidTimeFormat(
                    "one of CompDate and CompTime was given, but not both.".to_string(),
                ));
            }

            // Convert separate date and time strings and assign them the UTC timezone
            let full_datestr = format!("{datestr} {timestr}");
            let dt = chrono::NaiveDateTime::parse_from_str(&full_datestr, "%Y/%m/%d %H:%M:%S")
                .map_err(|_| LegacyMetError::InvalidTimeFormat(
                    format!("computer date and time {datestr} {timestr} does not have the proper format of %Y/%m/%d and %H:%M:%S, respectively")
                ))?;
            dt.and_utc().into()
        } else {
            return Err(LegacyMetError::InvalidTimeFormat(
                "none of CompSrlDate, CompDate + CompTime, or UTCDate + UTCTime were given"
                    .to_string(),
            ));
        };

        Ok(MetEntry {
            datetime,
            temperature: self.Tout,
            pressure: self.Pout,
            humidity: self.RH,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matlab_datenum() {
        // This is the example Jacob H. gives on the TCCON wiki
        let dt = matlab_to_chrono(735854.84046);
        assert_eq!(
            dt,
            chrono::NaiveDateTime::parse_from_str("2014-09-12 20:10:15", "%Y-%m-%d %H:%M:%S")
                .unwrap()
        );
    }

    #[test]
    fn test_compsrl_file() {
        let wiki_example = r#"# This file was acquired in Pasadena, CA, USA on February 2, 2015
        CompSrlDate,  Unit,  UTCDate,   UTCTime, WSPD, WDIR, SigTheta, Gust, Tout, RH, SFlux,  Pout, Precip, LeafWet, Battery, Bit,
        736005.73038, 4449, 2015/02/10, 18:04:46, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      15,    13.7,   0,
        736005.73041, 4449, 2015/02/10, 18:04:48, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      19,    13.7,   0,
        736005.73043, 4449, 2015/02/10, 18:04:50, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      19,    13.7,   0,
        736005.73045, 4449, 2015/02/10, 18:04:52, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      15,    13.7,   0,"#;

        let mut entries = read_legacy_inner(
            wiki_example.as_bytes(),
            FixedOffset::west_opt(7 * 3600).unwrap(),
        )
        .unwrap()
        .into_iter();
        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T17:31:44-07:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));

        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T17:31:47-07:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));

        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T17:31:49-07:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));

        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T17:31:50-07:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));
    }

    #[test]
    fn test_compdatetime_file() {
        let wiki_example = r#"# This file was acquired in Pasadena, CA, USA on February 2, 2015
        CompDate,  CompTime,  UTCDate,   UTCTime, WSPD, WDIR, SigTheta, Gust, Tout, RH, SFlux,  Pout, Precip, LeafWet, Battery, Bit,
        2015/02/10, 17:31:44, 2015/02/10, 18:04:46, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      15,    13.7,   0,
        2015/02/10, 17:31:47, 2015/02/10, 18:04:48, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      19,    13.7,   0,
        2015/02/10, 17:31:49, 2015/02/10, 18:04:50, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      19,    13.7,   0,
        2015/02/10, 17:31:50, 2015/02/10, 18:04:52, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      15,    13.7,   0,"#;

        let mut entries = read_legacy_inner(
            wiki_example.as_bytes(),
            FixedOffset::west_opt(7 * 3600).unwrap(),
        )
        .unwrap()
        .into_iter();
        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T17:31:44-07:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));

        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T17:31:47-07:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));

        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T17:31:49-07:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));

        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T17:31:50-07:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));
    }

    #[test]
    fn test_utcdatetime_file() {
        let wiki_example = r#"# This file was acquired in Pasadena, CA, USA on February 2, 2015
        UTCDate,   UTCTime, WSPD, WDIR, SigTheta, Gust, Tout, RH, SFlux,  Pout, Precip, LeafWet, Battery, Bit,
        2015/02/10, 18:04:46, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      15,    13.7,   0,
        2015/02/10, 18:04:48, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      19,    13.7,   0,
        2015/02/10, 18:04:50, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      19,    13.7,   0,
        2015/02/10, 18:04:52, 0.0,    0,     0.0,   0.0, 19.9, 46,   0.0, 985.9,   0,      15,    13.7,   0,"#;

        let mut entries = read_legacy_inner(
            wiki_example.as_bytes(),
            FixedOffset::west_opt(7 * 3600).unwrap(),
        )
        .unwrap()
        .into_iter();
        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T18:04:46-00:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));

        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T18:04:48-00:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));

        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T18:04:50-00:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));

        let entry = entries.next().unwrap();
        let dtime = chrono::DateTime::parse_from_rfc3339("2015-02-10T18:04:52-00:00").unwrap();
        assert!(entry.is_close(&MetEntry {
            datetime: dtime,
            temperature: Some(19.9),
            pressure: 985.9,
            humidity: Some(46.0)
        }));
    }
}

use std::{collections::HashSet, fmt::Display, path::{Path, PathBuf}};

use chrono::{NaiveDate, NaiveTime, DateTime, FixedOffset, TimeZone, Datelike};
use error_stack::ResultExt;
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;

use ggg_rs::{i2s::{self, OpusCatalogueEntry}, interpolation::{ConstantValueInterp, InterpolationError, InterpolationMethod}, opus::{self, constants::bruker::BrukerBlockType, IgramHeader, MissingOpusParameterError}};
use crate::{CATALOGUE_FILL_FLOAT_F32, coordinates::CoordinateSource, meteorology::{read_met_file, MetSource, Timezones, MetEntry}};

type CatalogueResult<T> = error_stack::Result<T, CatalogueError>;


pub fn make_catalogue_entries<P: AsRef<Path>>(coordinate_file: &Path, surface_met_source_file: &Path, interferograms: &[P], keep_if_missing_met: bool)
-> error_stack::Result<Vec<OpusCatalogueEntry>, MainCatalogError> {
    let coords = CoordinateSource::load_file(coordinate_file)
        .change_context_lazy(|| MainCatalogError::Coordinates)?;
    let surf_met_source = MetSource::from_config_json(surface_met_source_file)
        .change_context_lazy(|| MainCatalogError::Met)?;
    let met = load_met(interferograms, surf_met_source)
        .change_context_lazy(|| MainCatalogError::Met)?;

    let mut run_num = 1;
    let catalogue_entries: Vec<i2s::OpusCatalogueEntry> = interferograms
        .into_iter()
        .filter_map(|igm| {
            // Three cases. (1) Successfully made a catalogue entry, add it to the list. (2) Should skip this entry,
            // log than and do not add it to the list. (3) Other error, put it in the list so that try_collect() can
            // return that error at the end.
            match create_catalogue_entry_for_igram(igm.as_ref(), run_num, &coords, &met, keep_if_missing_met) {
                Ok(entry) => {
                    // Only advance the run number if we successfully added the interferogram. We're assuming that there's
                    // forward and reverse scans, so each interferogram should have two runs.
                    run_num += 2;
                    Some(Ok(entry))
                },
                Err(e) => {
                    if let CatalogueError::SkippingIgram(igm, reason) = e.current_context() {
                        log::warn!("Skipping {} because {}", igm.display(), reason);
                        None
                    } else {
                        Some(Err(e))
                    }
                }
            }
        })
        .try_collect()
        .change_context_lazy(|| MainCatalogError::Catalog)?;

    Ok(catalogue_entries)
}

#[derive(Debug, thiserror::Error)]
pub enum MainCatalogError {
    #[error("Error loading EM27 coordinate file")]
    Coordinates,
    #[error("Error loading EM27 meteorology information")]
    Met,
    #[error("Error creating an EM27 catalog entry or writing the catalog")]
    Catalog,
}

#[derive(Debug, thiserror::Error)]
enum CatalogueError {
    #[error("Could not create catalogue entry for interferogram {0}")]
    EntryCreationError(PathBuf),
    #[error("Could not read met file")]
    MetError,
    #[error("Skipping interferogram {0} because {1}")]
    SkippingIgram(PathBuf, IgramSkipReason),
    #[error("Could not get the file name part of {0}")]
    PathMissingFileName(PathBuf),
    #[error("The path {0} contains invalid UTF-8 characters")]
    PathInvalidUnicode(PathBuf),
    #[error("{0}")]
    MissingHeaderParameter(#[from] MissingOpusParameterError),
    #[error("Parameter {1} from block {0:?} had an unexpected type")]
    UnexpectedParameterType(BrukerBlockType, String),
    #[error("Parameter {param} from block {block:?} had an unexpected format: {cause}")]
    UnexpectedParameterFormat{block: BrukerBlockType, param: String, cause: String}
}

#[derive(Debug, thiserror::Error)]
enum IgramSkipReason {
    #[error("surface met data could not be interpolated to the ZPD time")]
    MetUnavailable
}

fn create_catalogue_entry_for_igram(igram: &Path, run: u32, coords: &CoordinateSource, met: &[MetEntry], keep_if_missing_met: bool) -> CatalogueResult<i2s::OpusCatalogueEntry> {
    let igram_header = opus::IgramHeader::read_full_igram_header(igram)
        .change_context_lazy(|| CatalogueError::EntryCreationError(igram.to_path_buf()))?;
    let zpd_time = get_zpd_time(&igram_header)
        .change_context_lazy(|| CatalogueError::EntryCreationError(igram.to_path_buf()))?;

    let (lat, lon, alt) = coords.get_coords_for_datetime(zpd_time);

    // EM27s only seem to record their instrument temperature, not humidity or pressure
    // (which I guess can reasonably be assumed to match the exterior conditions)
    let tins: f64 = igram_header.get_value(BrukerBlockType::InstrumentStatus, "TSC")
        .map_err(|e| CatalogueError::from(e))?
        .as_float()
        .map_err(|_| CatalogueError::UnexpectedParameterType(BrukerBlockType::InstrumentStatus, "TSC".to_string()))?;

    let igram_name = igram.file_name()
        .ok_or_else(|| CatalogueError::PathMissingFileName(igram.to_path_buf()))?
        .to_str()
        .ok_or_else(|| CatalogueError::PathInvalidUnicode(igram.to_path_buf()))?
        .to_string();

    // Interpolate met values to the interferograms
    // TODO: these interpolation calls right now assume that an error is an out-of-bounds error, which should get a fill value. 
    //  Really we should verify that is the case and log it; other errors should not result in fill values.
    let interpolator = ConstantValueInterp::new(false);

    let met_times = met.iter()
        .map(|m| m.datetime)
        .collect_vec();

    let met_pres = met.iter()
        .map(|m| m.pressure as f32)
        .collect_vec();
    let met_pres_res = interpolator.interp1d_to_time(met_times.as_slice(), met_pres.as_slice(), zpd_time);
    let met_pres = match met_pres_res {
        Ok(v) => v,
        Err(InterpolationError::OutOfDomain { left: _, right: _, out: _ }) => {
            if keep_if_missing_met {
                CATALOGUE_FILL_FLOAT_F32
            } else {
                return Err(CatalogueError::SkippingIgram(igram.to_path_buf(), IgramSkipReason::MetUnavailable).into())
            }
        }
        Err(e) => {
            return Err(CatalogueError::EntryCreationError(igram.to_path_buf()))
                .attach_printable_lazy(|| e);
        }
    };

    let met_temp = met.iter()
        .map(|m| m.temperature.map(|t| t as f32).unwrap_or(CATALOGUE_FILL_FLOAT_F32))
        .collect_vec();
    let met_temp = interpolator.interp1d_to_time(met_times.as_slice(), met_temp.as_slice(), zpd_time)
        .unwrap_or(CATALOGUE_FILL_FLOAT_F32);

    let met_rh = met.iter()
        .map(|m| m.humidity.map(|rh| rh as f32).unwrap_or(CATALOGUE_FILL_FLOAT_F32))
        .collect_vec();
    let met_rh = interpolator.interp1d_to_time(met_times.as_slice(), met_rh.as_slice(), zpd_time)
        .unwrap_or(CATALOGUE_FILL_FLOAT_F32);

    let entry = i2s::OpusCatalogueEntry::build(igram_name)
        .with_time(zpd_time.year(), zpd_time.month(), zpd_time.day(), run)
        .change_context_lazy(|| CatalogueError::EntryCreationError(igram.to_path_buf()))?
        .with_coordinates(lat, lon, alt)
        .change_context_lazy(|| CatalogueError::EntryCreationError(igram.to_path_buf()))?
        .with_instrument(tins as f32, CATALOGUE_FILL_FLOAT_F32, CATALOGUE_FILL_FLOAT_F32)
        .with_outside_met(met_temp, met_pres, met_rh)
        .finalize(CATALOGUE_FILL_FLOAT_F32)
        .change_context_lazy(|| CatalogueError::EntryCreationError(igram.to_path_buf()))?;

    Ok(entry)
}

/// Load the meteorology from the given file.
fn load_met<P: AsRef<Path>>(igrams: &[P], met_source: MetSource) -> CatalogueResult<Vec<MetEntry>> {
    // First check that all our interferograms have consistent timezones, since some met sources don't
    // record the time zone for their timestamps.
    let mut zpd_times = vec![];
    for igm in igrams {
        let header = IgramHeader::read_full_igram_header(igm.as_ref())
            .map_err(|_| CatalogueError::EntryCreationError(igm.as_ref().to_path_buf()))?;
        let dt = get_zpd_time(&header)?;
        zpd_times.push(dt);
    }

    let timezones = Timezones::check_consistent_timezones(zpd_times.into_iter());
    let met = read_met_file(&met_source, timezones)
        .change_context_lazy(|| CatalogueError::MetError)?;

    // For now, I'm using interpolators that don't care if the input is ordered. If they get slow, we can change this.
    // met.sort_by_key(|m| m.datetime);

    Ok(met)

}

fn get_zpd_time(header: &IgramHeader) -> error_stack::Result<DateTime<FixedOffset>, CatalogueError> {
    // let header = opus::IgramHeader::read_full_igram_header(igram)
    //     .map_err(|e| ZpdTimeError::from(e))?;

    let datestr = header.get_value(BrukerBlockType::IgramPrimaryStatus, "DAT")
        .map_err(|e| CatalogueError::from(e))?
        .as_str()
        .change_context_lazy(|| CatalogueError::UnexpectedParameterType(BrukerBlockType::IgramPrimaryData, "DAT".to_string()))?;

    let timestr = header.get_value(BrukerBlockType::IgramPrimaryStatus, "TIM")
        .map_err(|e: MissingOpusParameterError| CatalogueError::from(e))?
        .as_str()
        .change_context_lazy(|| CatalogueError::UnexpectedParameterType(BrukerBlockType::IgramPrimaryData, "TIM".to_string()))?;

    // The date string is easy to parse: it's dd/mm/yyyy. The time string is more a pain: "HH:MM:SS.fff (GMT+X)" or "-X" if the offset is negative.
    let mut timestr_split = timestr.split_ascii_whitespace();
    let hhmmss_str = timestr_split.next()
        .ok_or_else(|| CatalogueError::UnexpectedParameterFormat { 
            block: BrukerBlockType::IgramPrimaryData, param: "TIM".to_string(),
            cause: "Expected a time string with at least one group of ASCII whitespace, got no whitespace".to_string()
        })?;
    let offset_str = timestr_split.next()
        .ok_or_else(|| CatalogueError::UnexpectedParameterFormat { 
            block: BrukerBlockType::IgramPrimaryData, param: "TIM".to_string(),
            cause: "Expected a time string with at least one group of ASCII whitespace, got no whitespace".to_string()
        })?;

    let date = NaiveDate::parse_from_str(datestr, "%d/%m/%Y")
        .change_context_lazy(|| CatalogueError::UnexpectedParameterFormat { 
            block: BrukerBlockType::IgramPrimaryData, param: "DAT".to_string(), 
            cause: format!("Expected a date string in format DD/MM/YYYY, got '{datestr}'")
        })?;
    let time = NaiveTime::parse_from_str(hhmmss_str, "%H:%M:%S.%3f")
        .change_context_lazy(|| CatalogueError::UnexpectedParameterFormat { 
            block: BrukerBlockType::IgramPrimaryData, param: "TIM".to_string(),
            cause: format!("Expected a time string starting with 'HH:MM:SS.fff', got '{hhmmss_str}' instead")
        })?;

    // TODO: check how this works with non-integer hour timezones
    static OFFSET_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\(GMT([+\-]\d+)\)").unwrap());
    let offset_hours: i32 = OFFSET_RE.captures(offset_str)
        .map(|c| c.get(1))
        .flatten()
        .ok_or_else(|| CatalogueError::UnexpectedParameterFormat { 
            block: BrukerBlockType::IgramPrimaryData, param: "TIM".to_string(),
            cause: format!("Expected a time string ending with '(GMT+X)' or '(GMT-X)', got '{offset_str}' instead")
        })?.as_str()
        .parse()
        .unwrap(); // should be okay to unwrap, we've constructed our regex to find valid integers

    let offset = FixedOffset::east_opt(offset_hours * 3600)
        .ok_or_else(|| CatalogueError::UnexpectedParameterFormat { 
            block: BrukerBlockType::IgramPrimaryData, param: "TIM".to_string(),
            cause: format!("GMT offset ({offset_hours}) was out of bounds")
        })?;
    
    // Finally we can construct the darn time!
    Ok(offset.from_local_datetime(&date.and_time(time))
        .single()
        .ok_or_else(|| CatalogueError::UnexpectedParameterFormat { 
            block: BrukerBlockType::IgramPrimaryData, param: "TIM".to_string(),
            cause: format!("Date/time {date} {time} is invalid or ambiguous for offset {offset}")
        })?)
    
}

/// An error type for possible failures when getting a common timezone for multiple interferograms.
/// (e.g. with [`get_common_igram_timezone`]).
#[derive(Debug, thiserror::Error)]
pub enum IgramTimezoneError {
    /// Indicates no interferograms were provided (usually the input was an empty list)
    NoIgrams,

    /// Indicates that multiple time zones were found in the interferograms; all time zones
    /// found are in the contained set.
    Multiple(HashSet<FixedOffset>),

    /// Indicates that an error occurred while reading the interferograms. This error type
    /// is expected to be used inside an [`error_stack::Report`] so that the specific error
    /// is carried as part of the report.
    Error(PathBuf)
}

impl Display for IgramTimezoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IgramTimezoneError::NoIgrams => {
                write!(f, "No interferograms provided")
            }
            IgramTimezoneError::Multiple(tzs) => {
                write!(f, "Multiple timezones found in given interferograms: ")?;
                for (idx, tz) in tzs.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{tz}")?;
                }
                write!(f, "")
            },
            IgramTimezoneError::Error(p) => write!(f, "An error occurred while reading {}", p.display()),
        }
    }
}


/// Given a list of paths to interferograms, identify the timezone shared by them.
/// 
/// Errors if:
/// - the interferogram header cannot be read,
/// - the interferogram's time could not be parsed from the header,
/// - the list of interferograms is empty, or
/// - different interferograms had different timezones.
pub fn get_common_igram_timezone<P: AsRef<Path>>(igrams: &[P]) -> error_stack::Result<FixedOffset, IgramTimezoneError> {
    let mut timezones = HashSet::new();
    for igm in igrams {
        let igram_header = opus::IgramHeader::read_full_igram_header(igm.as_ref())
            .change_context_lazy(|| IgramTimezoneError::Error(igm.as_ref().to_owned()))?;
        let this_tz = get_zpd_time(&igram_header)
            .map(|t| t.timezone())
            .change_context_lazy(|| IgramTimezoneError::Error(igm.as_ref().to_owned()))?;
        timezones.insert(this_tz);
    }

    if timezones.is_empty() {
        Err(IgramTimezoneError::NoIgrams.into())
    } else if timezones.len() > 1 {
        Err(IgramTimezoneError::Multiple(timezones).into())
    } else {
        let tz = timezones.into_iter().next().unwrap();
        Ok(tz)
    }
}
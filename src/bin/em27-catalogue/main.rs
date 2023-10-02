use std::path::{Path, PathBuf};

use chrono::{NaiveDate, NaiveTime, DateTime, FixedOffset, TimeZone, Datelike};
use clap::Parser;
use error_stack::ResultExt;
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;

use ggg_rs::{i2s, opus::{self, constants::bruker::BrukerBlockType, IgramHeader, MissingOpusParameterError}};
use egi_rs::{coordinates::CoordinateSource, meteorology::{read_met_file, MetSource, Timezones, MetEntry}};

type CatalogueResult<T> = error_stack::Result<T, CatalogueError>;

const CATALOGUE_FILL_FLOAT: f32 = -999.0;

fn main() {
    let clargs = Cli::parse();
    let coords = CoordinateSource::load_file(&clargs.coordinate_file).unwrap();
    let met = load_met(&clargs.interferograms, clargs.met_source).unwrap();
    let catalogue_entries: Vec<i2s::OpusCatalogueEntry> = clargs.interferograms
        .into_iter()
        .enumerate()
        .map(|(i, igm)| {
            let run = 2*i as u32 + 1;
            create_catalogue_entry_for_igram(&igm, run, &coords, &met)
        })
        .try_collect()
        .unwrap();

    let mut stdout = std::io::stdout();
    i2s::write_opus_catalogue_table(&mut stdout, &catalogue_entries, false).unwrap();
}

#[derive(Debug, clap::Parser)]
struct Cli {
    #[clap(long="coords")]
    coordinate_file: PathBuf,
    #[clap(long, value_parser = MetSource::from_clarg)]
    met_source: MetSource,
    interferograms: Vec<PathBuf>
}

#[derive(Debug, thiserror::Error)]
enum CatalogueError {
    #[error("Could not create catalogue entry for interferogram {0}")]
    EntryCreationError(PathBuf),
    #[error("Could not read met file")]
    MetError,
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

fn create_catalogue_entry_for_igram(igram: &Path, run: u32, coords: &CoordinateSource, met: &[MetEntry]) -> CatalogueResult<i2s::OpusCatalogueEntry> {
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

    // TODO: matchup met. Met is time ordered by load_met, so pick the closest met in time or interpolate (and error/skip if met data not available within required time).
    let entry = i2s::OpusCatalogueEntry::build(igram_name)
        .with_time(zpd_time.year(), zpd_time.month(), zpd_time.day(), run)
        .change_context_lazy(|| CatalogueError::EntryCreationError(igram.to_path_buf()))?
        .with_coordinates(lat, lon, alt)
        .change_context_lazy(|| CatalogueError::EntryCreationError(igram.to_path_buf()))?
        .with_instrument(tins as f32, CATALOGUE_FILL_FLOAT, CATALOGUE_FILL_FLOAT)
        .finalize(CATALOGUE_FILL_FLOAT)
        .change_context_lazy(|| CatalogueError::EntryCreationError(igram.to_path_buf()))?;



        
    Ok(entry)
}

/// Load the meteorology from the give file. The returned vector will be ordered by time.
fn load_met(igrams: &[PathBuf], met_source: MetSource) -> CatalogueResult<Vec<MetEntry>> {
    // First check that all our interferograms have consistent timezones, since some met sources don't
    // record the time zone for their timestamps.
    let mut zpd_times = vec![];
    for igm in igrams {
        let header = IgramHeader::read_full_igram_header(igm)
            .map_err(|_| CatalogueError::EntryCreationError(igm.to_path_buf()))?;
        let dt = get_zpd_time(&header)?;
        zpd_times.push(dt);
    }

    let timezones = Timezones::check_consistent_timezones(zpd_times.into_iter());
    let mut met = read_met_file(&met_source, timezones)
        .change_context_lazy(|| CatalogueError::MetError)?;

    met.sort_by_key(|m| m.datetime);

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
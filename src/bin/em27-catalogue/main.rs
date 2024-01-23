use std::path::{Path, PathBuf};

use chrono::{NaiveDate, NaiveTime, DateTime, FixedOffset, TimeZone, Datelike};
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use error_stack::ResultExt;
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;

use ggg_rs::{i2s, opus::{self, constants::bruker::BrukerBlockType, IgramHeader, MissingOpusParameterError}, interpolation::{ConstantValueInterp, InterpolationMethod, InterpolationError}};
use egi_rs::{coordinates::CoordinateSource, meteorology::{read_met_file, MetSource, Timezones, MetEntry}};

type CatalogueResult<T> = error_stack::Result<T, CatalogueError>;

const CATALOGUE_FILL_FLOAT: f32 = -99.0;

fn main() {
    let clargs = Cli::parse();

    env_logger::Builder::new()
    .filter_level(clargs.verbose.log_level_filter())
    .init();

    let coords = CoordinateSource::load_file(&clargs.coordinate_file).unwrap();
    let surf_met_source = MetSource::from_config_json(&clargs.surface_met_source_file).unwrap();
    let met = load_met(&clargs.interferograms, surf_met_source).unwrap();

    let mut run_num = 1;
    let catalogue_entries: Vec<i2s::OpusCatalogueEntry> = clargs.interferograms
        .into_iter()
        .filter_map(|igm| {
            // Three cases. (1) Successfully made a catalogue entry, add it to the list. (2) Should skip this entry,
            // log than and do not add it to the list. (3) Other error, put it in the list so that try_collect() can
            // return that error at the end.
            match create_catalogue_entry_for_igram(&igm, run_num, &coords, &met, clargs.keep_if_missing_met) {
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
        .unwrap();

    let mut stdout = std::io::stdout();
    i2s::write_opus_catalogue_table(&mut stdout, &catalogue_entries, false).unwrap();
}


/// Generate an I2S catalogue for EM27 interferograms
#[derive(Debug, clap::Parser)]
struct Cli {
    #[command(flatten)]
    verbose: Verbosity<WarnLevel>,
    
    /// Set this flag to include an interferogram even if there isn't surface met data available to match up with it.
    /// The default is to skip it, since GGG requires surface pressure to perform the retrieval.
    #[clap(long)]
    keep_if_missing_met: bool,

    /// Path to a coordinates JSON file (required). See the documentation for [`CoordinateSource`] for allowed formats.
    #[clap(long="coords")]
    coordinate_file: PathBuf,

    /// Path to a surface met source description file (required). See the documentation for [`MetSource`] for allowed formats.
    #[clap(long="surf-met",)]
    surface_met_source_file: PathBuf,

    /// Paths to the interferograms to add to the catalogue.
    interferograms: Vec<PathBuf>
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
                CATALOGUE_FILL_FLOAT
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
        .map(|m| m.temperature as f32)
        .collect_vec();
    let met_temp = interpolator.interp1d_to_time(met_times.as_slice(), met_temp.as_slice(), zpd_time)
        .unwrap_or(CATALOGUE_FILL_FLOAT);

    let met_rh = met.iter()
        .map(|m| m.humidity as f32)
        .collect_vec();
    let met_rh = interpolator.interp1d_to_time(met_times.as_slice(), met_rh.as_slice(), zpd_time)
        .unwrap_or(CATALOGUE_FILL_FLOAT);

    let entry = i2s::OpusCatalogueEntry::build(igram_name)
        .with_time(zpd_time.year(), zpd_time.month(), zpd_time.day(), run)
        .change_context_lazy(|| CatalogueError::EntryCreationError(igram.to_path_buf()))?
        .with_coordinates(lat, lon, alt)
        .change_context_lazy(|| CatalogueError::EntryCreationError(igram.to_path_buf()))?
        .with_instrument(tins as f32, CATALOGUE_FILL_FLOAT, CATALOGUE_FILL_FLOAT)
        .with_outside_met(met_temp, met_pres, met_rh)
        .finalize(CATALOGUE_FILL_FLOAT)
        .change_context_lazy(|| CatalogueError::EntryCreationError(igram.to_path_buf()))?;

    Ok(entry)
}

/// Load the meteorology from the given file.
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
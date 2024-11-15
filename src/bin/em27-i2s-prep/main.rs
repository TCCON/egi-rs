use std::{path::PathBuf, process::ExitCode, str::FromStr};

use clap::{Parser, Args, Subcommand};
use clap_verbosity_flag::{Verbosity, WarnLevel};

use error_stack::ResultExt;
use ggg_rs::i2s::{I2SHeaderEdit, I2SInputModifcations};
use serde::{de, Deserialize};

mod default_files;
mod patterns;
mod run_daily;

fn main() -> ExitCode {
    let clargs = Cli::parse();

    env_logger::Builder::new()
    .filter_level(clargs.verbose.log_level_filter())
    .init();

    let res = match clargs.command {
        PrepActions::Daily(args) => run_daily::prep_daily_i2s(args),
        PrepActions::DailyJson(json_args) => run_daily::prep_daily_i2s_json(json_args)
    };

    if let Err(e) = res {
        eprintln!("An error occurred:\n{e:?}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    BadInput(String),
    #[error("I/O error: {0}")]
    IoError(String),
    #[error("The interferogram directory {} does not exist", .0.display())]
    MissingIgramDir(PathBuf),
    #[error("There was an error preparing the catalogue of interferograms.")]
    CatalogueError,
    #[error("{0} (this was unexpected)")]
    UnexpectedError(String),
}

// ---------------------- //
// Command line interface //
// ---------------------- //

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: PrepActions,

    #[command(flatten)]
    verbose: Verbosity<WarnLevel>,
}

#[derive(Debug, Subcommand)]
enum PrepActions {
    Daily(DailyCli),
    DailyJson(DailyJsonCli)
}

#[derive(Debug, Args)]
struct DailyCli {
    #[command(flatten)]
    pub(crate) common: DailyCommonArgs,

    /// The two-letter site ID to use in spectrum names.
    pub(crate) site_id: String,

    /// The first date to process, in YYYY-MM-DD format.
    pub(crate) start_date: chrono::NaiveDate,

    /// The last date to process, in YYYY-MM-DD format.
    pub(crate) end_date: chrono::NaiveDate,

    /// Where to write the file to drive the `parallel` utility to run I2S.
    /// If not given, the default is to write to "multii2s.sh" in the current
    /// directory.
    #[clap(short='p', long, default_value = "multii2s.in")]
    pub(crate) parallel_file: PathBuf,

    /// If a run directory already exists, it is deleted and recreated. Use with care!
    #[clap(long)]
    pub(crate) clear: bool,

    /// If a date in the date range does not have an interferogram directory,
    /// skip it rather than erroring. Useful when running a range where data
    /// is not available every day.
    #[clap(short='s', long)]
    pub(crate) no_skip_missing_dates: bool,
}

impl TryFrom<DailyJsonCli> for DailyCli {
    type Error = error_stack::Report<CliError>;

    fn try_from(value: DailyJsonCli) -> Result<Self, Self::Error> {
        let rdr = std::fs::File::open(&value.json_file)
            .change_context_lazy(|| CliError::IoError(
                format!("Error opening the JSON file {}", value.json_file.display())
            ))?;
        let common: DailyCommonArgs = serde_json::from_reader(rdr)
            .change_context_lazy(|| CliError::BadInput(
                format!("The JSON file {} is not correct.", value.json_file.display())
            ))?;
        Ok(DailyCli {
            common,
            site_id: value.site_id,
            start_date: value.start_date,
            end_date: value.end_date,
            parallel_file: value.parallel_file,
            clear: value.clear,
            no_skip_missing_dates: value.no_skip_missing_dates
        })
    }
}

#[derive(Debug, Args)]
struct DailyJsonCli {
    json_file: PathBuf,

    /// The two-letter site ID to use in spectrum names.
    pub(crate) site_id: String,

    /// The first date to process, in YYYY-MM-DD format.
    pub(crate) start_date: chrono::NaiveDate,

    /// The last date to process, in YYYY-MM-DD format.
    pub(crate) end_date: chrono::NaiveDate,

    /// Where to write the file to drive the `parallel` utility to run I2S.
    /// If not given, the default is to write to "multii2s.sh" in the current
    /// directory.
    #[clap(short='p', long, default_value = "multii2s.in")]
    pub(crate) parallel_file: PathBuf,

    /// If a run directory already exists, it is deleted and recreated. Use with care!
    #[clap(long)]
    pub(crate) clear: bool,

    /// If a date in the date range does not have an interferogram directory,
    /// skip it rather than erroring. Useful when running a range where data
    /// is not available every day.
    #[clap(short='s', long)]
    pub(crate) no_skip_missing_dates: bool,
}

#[derive(Debug, Args, Deserialize)]
struct DailyCommonArgs {
    /// A path with a date placeholder where interferograms are stored.
    /// 
    /// This uses curly braces to indicate a placeholder. The current date
    /// and the site ID can be inserted, replacing instances of {DATE}
    /// and {SITE_ID}, respectively. A format can also be given after a colon
    /// for DATE, e.g. {DATE:%Y%j} would be replaced with the four
    /// digit year and three digit day of year. If no format is given,
    /// as in {DATE}, it defaults to YYYY-MM-DD format.
    /// 
    /// Two examples, assuming that we are processing 1 Apr 2024 with site ID "xx",
    /// "/data/{DATE}/igms" would resolve to "/data/2024-04-01/igms",
    /// while "/data/{SITE_ID}/{DATE:%Y}/{DATE:%m}/{DATE:%d}/igms" would 
    /// resolve to "/data/xx/2024/04/01/igms".
    #[clap(short='i', long)]
    pub(crate) igram_pattern: String,

    /// A path with a date placeholder where I2S should be set up to run (required).
    /// 
    /// These paths can substitute in value using the same sort of patterns
    /// as IGRAM_PATTERN.
    #[clap(short='o', long)]
    pub(crate) run_dir_pattern: String,

    /// A path with an optional date placeholder pointing to the coordinates JSON file (required).
    /// 
    /// These paths can substitute in values using the same sort of patterns
    /// as IGRAM_PATTERN.
    #[clap(short='c', long)]
    pub(crate) coord_file_pattern: String,

    /// A path with a date placeholder pointing to the meteorology JSON file (required).
    /// 
    /// These paths can substitute in values using the same sort of patterns
    /// as IGRAM_PATTERN.
    #[clap(short='m', long)]
    pub(crate) met_file_pattern: String,

    /// A glob pattern to append to IGRAM_PATTERN that should return all interferograms
    /// for a given date (required). The same placeholder patterns as allowed in 
    /// IGRAM_PATTERN can be included, e.g. "ifg_{DATE:%Y%m%d}*" would search for files
    /// starting with "ifg_20240401" for 1 Apr 2024.
    #[clap(short='g', long, default_value_t = String::from("*"))]
    pub(crate) igram_glob_pattern: String,

    /// Which detector configuration the EM27 data used (required)
    /// 
    /// Options are "single" (for a standard InGaAs detector only)
    /// and "dual" (for a standard InGaAs plus an extended InGaAs
    /// to cover the CO band).
    /// TODO: make this optional, get from the interferogram headers
    /// if not given.
    #[clap(short='d', long)]
    #[serde(deserialize_with = "deserialize_detector_set")]
    pub(crate) detectors: DetectorSet,

    /// A file containing the top part of an I2S input file (i.e. 
    /// the header parameters) to use as a template (optional). Note that
    /// some parameters will always be overwritten to handle the file
    /// structure and detectors. If omitted, the recommended top will
    /// be used.
    #[clap(short='t', long)]
    pub(crate) top_file: Option<PathBuf>,

    /// If given, the UTC offset to insert in the I2S input file header (optional).
    /// The default is "0.0", which assumes your interferograms were
    /// collected by a computer with the time set to UTC. Negative values
    /// are permitted.
    #[clap(short='u', long, allow_negative_numbers = true)]
    pub(crate) utc_offset: Option<String>,
}


#[derive(Debug, Clone, Copy)]
enum DetectorSet {
    Single,
    Dual,
}

impl DetectorSet {
    fn get_changes(&self) -> I2SInputModifcations {
        let changes = match self {
            DetectorSet::Single => {
                vec![I2SHeaderEdit{parameter: 7, value: "2 2".to_string()}, 
                     I2SHeaderEdit{parameter: 11, value: "AA".to_string()},
                     I2SHeaderEdit{parameter: 12, value: "aa".to_string()}]
            },
            DetectorSet::Dual => {
                vec![I2SHeaderEdit{parameter: 7, value: "1 2".to_string()}, 
                     I2SHeaderEdit{parameter: 11, value: "DA".to_string()},
                     I2SHeaderEdit{parameter: 12, value: "da".to_string()}]
            },
        };

        I2SInputModifcations::from(changes)
    }

    fn get_flimit(&self) -> &'static str {
        match self {
            DetectorSet::Single => default_files::FLIMIT_SINGLE,
            DetectorSet::Dual => default_files::FLIMIT_DUAL,
        }
    }
}

impl FromStr for DetectorSet {
    type Err = CliError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "s" | "single" => Ok(Self::Single),
            "d" | "dual" => Ok(Self::Dual),
            _ => Err(CliError::BadInput(
                format!("'{s}' is not a valid detector set")
            ))
        }
    }
}

fn deserialize_detector_set<'de, D>(deserializer: D) -> Result<DetectorSet, D::Error>
where D: serde::Deserializer<'de>
{
    let s = String::deserialize(deserializer)?;
    match DetectorSet::from_str(&s) {
        Ok(dset) => Ok(dset),
        Err(e) => Err(de::Error::custom(format!("{e}"))),
    }
}

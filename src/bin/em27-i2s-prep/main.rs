use std::{path::PathBuf, process::ExitCode, str::FromStr};

use clap::{Parser, Args, Subcommand};

use ggg_rs::i2s::{I2SHeaderEdit, I2SInputModifcations};

mod default_files;
mod patterns;
mod run_daily;

fn main() -> ExitCode {
    let clargs = Cli::parse();
    let res = match clargs.command {
        PrepActions::Daily(args) => run_daily::prep_daily_i2s(args),
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
    #[error("There was an error preparing the catalogue of interferograms.")]
    CatalogueError,
}

// ---------------------- //
// Command line interface //
// ---------------------- //

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: PrepActions
}

#[derive(Debug, Subcommand)]
enum PrepActions {
    Daily(DailyCli)
    // TODO: add prep daily v2 std subcommand that follows the JPL recommended structure
}

#[derive(Debug, Args)]
struct DailyCli {
    /// A path with a date placeholder where interferograms are stored.
    /// 
    /// This uses curly braces to indicate a placeholder. The only value
    /// that can be inserted is the current date, and it will replace
    /// instances of {DATE} in the pattern. A format can also be given
    /// after a colon, e.g. {DATE:%Y%j} would be replaced with the four
    /// digit year and three digit day of year. If no format is given,
    /// as in {DATE}, it defaults to YYYYMMDD format.
    /// 
    /// Two examples, assuming that we a processing 1 Apr 2024,
    /// "/data/{DATE}/igms" would resolve to "/data/20240401/igms",
    /// while "/data/{DATE:%Y}/{DATE:%m}/{DATE:%d}/igms" would resolve to
    /// "/data/2024/04/01/igms".
    #[clap(short='i', long)]
    pub(crate) igram_pattern: String,

    /// A path with a date placeholder where I2S should be set up to run.
    /// 
    /// These paths can substitute in the date using the same sort of patterns
    /// as IGRAM_PATTERN.
    #[clap(short='o', long)]
    pub(crate) run_dir_pattern: String,

    /// A path with an optional date placeholder pointing to the coordinates JSON file.
    /// 
    /// These paths can substitute in the date using the same sort of patterns
    /// as IGRAM_PATTERN.
    pub(crate) coord_file_pattern: String,

    /// A path with a date placeholder pointing to the meteorology JSON file.
    /// 
    /// These paths can substitute in the date using the same sort of patterns
    /// as IGRAM_PATTERN.
    pub(crate) met_file_pattern: String,

    /// A glob pattern to append to IGRAM_PATTERN that should return all interferograms
    /// for a given date. The same date placeholder pattern as allowed in IGRAM_PATTERN
    /// can be included if the date needs to be part of the glob pattern, e.g.
    /// "ifg_{DATE}*" would search for files starting with "ifg_20240401" for 1 Apr 2024.
    #[clap(short='g', long, default_value_t = String::from("*"))]
    pub(crate) igram_glob_pattern: String,

    #[command(flatten)]
    pub(crate) common_args: CommonCli,
}

#[derive(Debug, Args)]
struct CommonCli {
    /// Which detector configuration the EM27 data used (required)
    /// 
    /// Options are "single" (for a standard InGaAs detector only)
    /// and "dual" (for a standard InGaAs plus an extended InGaAs
    /// to cover the CO band).
    #[clap(short='d', long)]
    pub(crate) detectors: DetectorSet,

    /// The first date to process, in YYYY-MM-DD format.
    pub(crate) start_date: chrono::NaiveDate,

    /// The last date to process, in YYYY-MM-DD format.
    pub(crate) end_date: chrono::NaiveDate,

    /// If given, a file containing the top part of an I2S input file
    /// (i.e. the header parameters) to use as a template. Note that
    /// some parameters will always be overwritten to handle the file
    /// structure and detectors. If omitted, the recommended top will
    /// be used.
    pub(crate) top_file: Option<PathBuf>,
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
                format!("Unknown detector set '{s}'")
            ))
        }
    }
}

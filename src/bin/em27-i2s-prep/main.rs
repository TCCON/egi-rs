use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};
use clap_verbosity_flag::{Verbosity, WarnLevel};

use error_stack::ResultExt;

use egi_rs::config::DailyCommonArgs;

mod run_daily;

fn main() -> ExitCode {
    let clargs = Cli::parse();

    env_logger::Builder::new()
        .filter_level(clargs.verbose.log_level_filter())
        .init();

    let res = match clargs.command {
        PrepActions::Daily(args) => run_daily::prep_daily_i2s(args),
        PrepActions::DailyJson(json_args) => run_daily::prep_daily_i2s_json(json_args),
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
    #[error("There was an error preparing the catalog of interferograms.")]
    CatalogError,
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
    DailyJson(DailyJsonCli),
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
    #[clap(short = 'p', long, default_value = "multii2s.in")]
    pub(crate) parallel_file: PathBuf,

    /// If a run directory already exists, it is deleted and recreated. Use with care!
    #[clap(long)]
    pub(crate) clear: bool,

    /// If a date in the date range does not have an interferogram directory,
    /// raise an error rather than continuing.
    #[clap(short = 's', long)]
    pub(crate) no_skip_missing_dates: bool,
}

impl TryFrom<DailyJsonCli> for DailyCli {
    type Error = error_stack::Report<CliError>;

    fn try_from(value: DailyJsonCli) -> Result<Self, Self::Error> {
        let common =
            DailyCommonArgs::read_from_path(&value.json_file).change_context_lazy(|| {
                CliError::BadInput("Error opening the configuration JSON file".to_string())
            })?;

        Ok(DailyCli {
            common,
            site_id: value.site_id,
            start_date: value.start_date,
            end_date: value.end_date,
            parallel_file: value.parallel_file,
            clear: value.clear,
            no_skip_missing_dates: value.no_skip_missing_dates,
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
    #[clap(short = 'p', long, default_value = "multii2s.in")]
    pub(crate) parallel_file: PathBuf,

    /// If a run directory already exists, it is deleted and recreated. Use with care!
    #[clap(long)]
    pub(crate) clear: bool,

    /// If a date in the date range does not have an interferogram directory,
    /// raise an error rather than continuing.
    #[clap(short = 's', long)]
    pub(crate) no_skip_missing_dates: bool,
}

use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};
use clap_verbosity_flag::{Verbosity, WarnLevel};

mod list_spectra;

fn main() -> ExitCode {
    let clargs = Cli::parse();

    env_logger::Builder::new()
    .filter_level(clargs.verbose.log_level_filter())
    .init();

    let res = match clargs.command {
        PrepActions::ListDataPartitionsDaily(clargs) => {
            list_spectra::print_daily_spec_dirs(
                &clargs.site_id,
                clargs.start_date,
                clargs.end_date,
                &clargs.i2s_dir_pattern,
                !clargs.no_skip_missing_dates
            )
        }
        PrepActions::ListDataPartitionsDailyJson(clargs) => {
            list_spectra::print_daily_spec_dirs_json(
                &clargs.site_id,
                clargs.start_date,
                clargs.end_date,
                &clargs.json_file,
                !clargs.no_skip_missing_dates
            )
        },
        PrepActions::ListSpectraDaily(clargs) => {
            list_spectra::print_daily_ordered_spectra(
                &clargs.site_id,
                clargs.start_date,
                clargs.end_date,
                &clargs.i2s_dir_pattern,
                !clargs.no_skip_missing_dates
            )
        },
        
    };

    if let Err(e) = res {
        eprintln!("An error occurred:\n{e:?}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: PrepActions,

    #[command(flatten)]
    verbose: Verbosity<WarnLevel>,
}

#[derive(Debug, Subcommand)]
enum PrepActions {
    ListDataPartitionsDaily(DailyCli),
    ListDataPartitionsDailyJson(DailyJsonCli),
    ListSpectraDaily(DailyCli),
}

#[derive(Debug, Args)]
pub(crate) struct DailyCli {
    /// The two-letter site ID to use in spectrum names.
    pub(crate) site_id: String,

    /// The first date to process, in YYYY-MM-DD format.
    pub(crate) start_date: chrono::NaiveDate,

    /// The last date to process, in YYYY-MM-DD format.
    pub(crate) end_date: chrono::NaiveDate,

    /// A path, potentially with placeholders, where I2S was run.
    /// 
    /// This uses curly braces to indicate a placeholder. The current date
    /// and the site ID can be inserted, replacing instances of {DATE}
    /// and {SITE_ID}, respectively. A format can also be given after a colon
    /// for DATE, e.g. {DATE:%Y%j} would be replaced with the four
    /// digit year and three digit day of year. If no format is given,
    /// as in {DATE}, it defaults to YYYY-MM-DD format.
    pub(crate) i2s_dir_pattern: String,

    /// If a date in the date range does not have an interferogram directory,
    /// raise an error rather than continuing. 
    #[clap(short='s', long)]
    pub(crate) no_skip_missing_dates: bool,
}

#[derive(Debug, Args)]
pub(crate) struct DailyJsonCli {
    json_file: PathBuf,

    /// The two-letter site ID to use in spectrum names.
    pub(crate) site_id: String,

    /// The first date to process, in YYYY-MM-DD format.
    pub(crate) start_date: chrono::NaiveDate,

    /// The last date to process, in YYYY-MM-DD format.
    pub(crate) end_date: chrono::NaiveDate,

    /// If a date in the date range does not have an interferogram directory,
    /// raise an error rather than continuing. 
    #[clap(short='s', long)]
    pub(crate) no_skip_missing_dates: bool,
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    BadInput(String),
    #[error("{0}")]
    MissingInput(String),
}


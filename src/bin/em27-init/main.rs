//! This program handles initial steps needed to integrate EGI
//! into GGG, e.g. adding new windows or correction files, creating
//! a directory for EGI user data, ensuring the sqlite3 database
//! is up to date (using the [sqlx migrate
//! macro](https://docs.rs/sqlx/latest/sqlx/macro.migrate.html), and so on.
//!
//! Each step should be designed so that if this program is run multiple times,
//! the step will only be done once (unless it somehow gets reverted in a way
//! that the program can't detect).
use std::{io::{Read, Write}, path::PathBuf, process::ExitCode};
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use ggg_rs::utils::{get_ggg_path, GggError};
use egi_rs::default_files::EM27_WINDOWS;


fn main() -> ExitCode {
    let clargs = Cli::parse();

    env_logger::Builder::new()
    .filter_level(clargs.verbose.log_level_filter())
    .init();

    let res = driver(clargs.yes);
    if let Err(e) = res {
        eprintln!("Error initializing EGI:\n{e}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Generate an I2S catalogue for EM27 interferograms
#[derive(Debug, clap::Parser)]
struct Cli {
    #[command(flatten)]
    verbose: Verbosity<WarnLevel>,

    /// Automatically answer "yes" to any prompts.
    #[clap(short = 'y', long)]
    yes: bool,
}

fn driver(always_yes: bool) -> Result<(), SetupError> {
    let ggg_path = get_ggg_path()?;

    let steps = [
        CreateFileStep::new_boxed(EM27_WINDOWS, ggg_path.join("windows").join("gnd").join("em27.gnd"))
    ];

    for step in steps {
        step.describe();
        let outcome = step.execute(always_yes)?;
        match outcome {
            SetupOutcome::Executed => step.tell_completion(),
            SetupOutcome::NotNeeded => step.tell_not_needed(),
            SetupOutcome::UserSkipped => println!("Skipped as requested"),
        }
    }

    Ok(())
}

type SetupResult = Result<SetupOutcome, SetupError>;

enum SetupOutcome {
    /// Indicates that the step was executed successfully
    Executed,

    /// Indicates that the step was not run because it had
    /// been completed previously.
    NotNeeded,

    /// Indicates that the step did not complete because the
    /// user cancelled it at some point.
    UserSkipped,

}

#[derive(Debug, thiserror::Error)]
enum SetupError {
    #[error("Aborted initialization")]
    UserAbort,
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    GggError(#[from] GggError),
}

trait SetupStep {
    fn describe(&self);
    fn tell_completion(&self);
    fn tell_not_needed(&self);
    fn execute(&self, always_yes: bool) -> SetupResult;
}

/// Initialization step to create a file with fixed contents.
struct CreateFileStep {
    source: &'static str,
    dest: PathBuf
}

/// Used to indicate whether a file to create exists, needs created,
/// or needs overwritten.
#[derive(Debug, Clone)]
enum FileStatus {
    /// The file does not exist
    Missing,

    /// The file exists, but has different content than expected.
    /// (The current content is returned as the contained `String`.)
    ContentDiffers(String),

    /// The file exists with the expected content.
    Extant
}

impl CreateFileStep {
    fn new_boxed(source: &'static str, dest: PathBuf) -> Box<dyn SetupStep> {
        let me = Self { source, dest };
        Box::new(me)
    }

    fn file_status(&self) -> std::io::Result<FileStatus> {
        if !self.dest.exists() {
            return Ok(FileStatus::Missing);
        }

        let mut f = std::fs::File::open(&self.dest)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf)?;
        if buf == self.source {
            Ok(FileStatus::Extant)
        } else {
            Ok(FileStatus::ContentDiffers(buf))
        }
    }

    /// Ask the user whether to overwrite an existing file with different
    /// content than expected. Returns `Some(true)` if they answer "yes",
    /// `Some(false)` if "no", and `None` if they want to abort initialization.
    fn ask_to_overwrite(&self, current_content: &str, always_yes: bool) -> Option<bool> {
        // Should show the diff (with https://docs.rs/difflib/latest/difflib/ or similar)
        // then ask if it is okay to overwrite.
        todo!()
    }
}

impl SetupStep for CreateFileStep {
    fn describe(&self) {
        println!("Creating EM27 window file...");
    }

    fn tell_completion(&self) {
        println!("File created.");
    }

    fn tell_not_needed(&self) {
        println!("File already exists, not re-creating.");
    }

    fn execute(&self, always_yes: bool) -> SetupResult {
        match self.file_status()? {
            FileStatus::Extant => return Ok(SetupOutcome::NotNeeded),
            FileStatus::ContentDiffers(curr_content) => {
                match self.ask_to_overwrite(&curr_content, always_yes) {
                    Some(true) => (),
                    Some(false) => return Ok(SetupOutcome::UserSkipped),
                    None => return Err(SetupError::UserAbort)
                }
            },
            FileStatus::Missing => (),
        }

        let mut f = std::fs::File::create(&self.dest)?;
        f.write_all(self.source.as_bytes())?;
        Ok(SetupOutcome::Executed)
    }
}

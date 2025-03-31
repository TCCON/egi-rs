//! This program handles initial steps needed to integrate EGI
//! into GGG, e.g. adding new windows or correction files, creating
//! a directory for EGI user data, ensuring the sqlite3 database
//! is up to date (using the [sqlx migrate
//! macro](https://docs.rs/sqlx/latest/sqlx/macro.migrate.html), and so on.
//!
//! Each step should be designed so that if this program is run multiple times,
//! the step will only be done once (unless it somehow gets reverted in a way
//! that the program can't detect).
use std::{borrow::Cow, io::{Read, Write}, path::PathBuf, process::ExitCode};
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use colored::Colorize;
use ggg_rs::utils::{get_ggg_path, GggError};
use egi_rs::{default_files::{default_core_config_toml, EM27_ADCFS, EM27_AICFS, EM27_WINDOWS}, utils};
use inquire::{prompt_confirmation, InquireError};
use itertools::Itertools;


fn main() -> ExitCode {
    let clargs = Cli::parse();

    env_logger::Builder::new()
    .filter_level(clargs.verbose.log_level_filter())
    .init();

    let res = driver(clargs.yes);
    match res {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(2),
        Err(e) => {
            eprintln!("Error initializing EGI:\n{e}");
            ExitCode::FAILURE
        }
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

fn driver(always_yes: bool) -> Result<bool, SetupError> {
    let ggg_path = get_ggg_path()?;

    let steps = [
        MakeDirStep::new_boxed(ggg_path.join("egi"), false),
        CreateFileStep::new_owned_boxed(default_core_config_toml(), ggg_path.join("egi").join("egi_config.toml")),
        CreateFileStep::new_boxed(EM27_WINDOWS, ggg_path.join("windows").join("gnd").join("em27.gnd")),
        CreateFileStep::new_boxed(EM27_ADCFS, ggg_path.join("tccon").join("corrections_airmass_postavg.em27.dat")),
        CreateFileStep::new_boxed(EM27_AICFS, ggg_path.join("tccon").join("corrections_insitu_postavg.em27.dat")),
        AddMenuEntryStep::new_boxed(
            ggg_path.join("windows").join("gnd").join("windows.men"), 
            "em27.gnd",
            Some("Subset of standard windows for an EM27 with an extended InGaAs detector")
        )
    ];

    let mut n_skipped = 0;
    let mut outcomes = vec![];
    for step in steps {
        step.describe();
        let outcome = step.execute(always_yes)?;
        match outcome {
            SetupOutcome::Executed => {
                step.tell_completion();
                outcomes.push((true, step.name()));
            },
            SetupOutcome::NotNeeded => {
                step.tell_not_needed();
                outcomes.push((true, step.name()));
            },
            SetupOutcome::UserSkipped => {
                println!("Skipped as requested");
                n_skipped += 1;
                outcomes.push((false, step.name()));
            },
            SetupOutcome::OtherSkip(reason) => {
                println!("Step skipped: {reason}");
                n_skipped += 1;
                outcomes.push((false, step.name()));
            }
        }
    }

    println!("\nSummary:");
    for (step_ok, step_name) in outcomes {
        let status_text = if step_ok {
            "OK".on_green().black().bold()
        } else {
            "SKIPPED".on_red().black().bold()
        };
        println!("{status_text:^8} {step_name}");
    }

    if n_skipped == 0 {
        println!("\nEGI initialization complete.");
        Ok(true)
    } else {
        println!("\n{n_skipped} steps were skipped, your EGI integration may be incomplete. Review the steps skipped and rerun this program if needed.");
        Ok(false)
    }
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

    /// The step was skipped for another reason
    OtherSkip(String),
}

#[derive(Debug, thiserror::Error)]
enum SetupError {
    #[error("Aborted initialization")]
    UserAbort,
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    GggError(#[from] GggError),
    #[error("{0}")]
    Other(String),
}

trait SetupStep {
    fn name(&self) -> Cow<'static, str>;
    fn describe(&self);
    fn tell_completion(&self);
    fn tell_not_needed(&self);
    fn execute(&self, always_yes: bool) -> SetupResult;
}

/// Initialization step to create a file with fixed contents.
struct CreateFileStep {
    source: Cow<'static, str>,
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
        let source = Cow::Borrowed(source);
        let me = Self { source, dest };
        Box::new(me)
    }

    fn new_owned_boxed(source: String, dest: PathBuf) -> Box<dyn SetupStep> {
        let source = Cow::Owned(source);
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
    fn ask_to_overwrite(&self, current_content: &str, always_yes: bool) -> Result<bool, InquireError> {
        if always_yes {
            return Ok(true);
        }

        // Show the diff (with https://docs.rs/difflib/latest/difflib/ or similar)
        // then ask if it is okay to overwrite.
        let current_lines = current_content.split('\n').collect_vec();
        let wanted_lines = self.source.split('\n').collect_vec();
        let diff = difflib::unified_diff(
            &current_lines,
            &wanted_lines,
            &format!("On disk ({})", self.dest.display()),
            "To write",
            "",
            "",
            3);
        
        for line in diff {
            println!("{line}");
        }

        prompt_confirmation("Okay to overwrite?")
    }
}

impl SetupStep for CreateFileStep {
    fn name(&self) -> Cow<'static, str> {
        let name = self.dest.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_else(|| self.dest.to_string_lossy());
        format!("Create '{name}' file").into()
    }

    fn describe(&self) {
        println!("Creating file {}", self.dest.display());
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
                    Ok(true) => (),
                    Ok(false) => return Ok(SetupOutcome::UserSkipped),
                    Err(InquireError::OperationCanceled) => return Err(SetupError::UserAbort),
                    Err(InquireError::OperationInterrupted) => panic!("Ctrl+C received, aborting"),
                    Err(InquireError::IO(e)) => return Err(SetupError::IoError(e)),
                    Err(InquireError::NotTTY) => return Ok(SetupOutcome::OtherSkip("input required but program is not running interactively".to_string())),
                    Err(InquireError::InvalidConfiguration(e)) => return Err(SetupError::Other(e)),
                    Err(InquireError::Custom(e)) => return Err(SetupError::Other(e.to_string())),
                }
            },
            FileStatus::Missing => (),
        }

        let mut f = std::fs::File::create(&self.dest)?;
        f.write_all(self.source.as_bytes())?;
        Ok(SetupOutcome::Executed)
    }
}


struct MakeDirStep {
    target_dir: PathBuf,
    create_parents: bool
}

impl MakeDirStep {
    fn new_boxed(target_dir: PathBuf, create_parents: bool) -> Box<dyn SetupStep> {
        let me = Self { target_dir, create_parents };
        Box::new(me)
    }
}

impl SetupStep for MakeDirStep {
    fn name(&self) -> Cow<'static, str> {
        // Abbreviate the path if it is inside GGGPATH
        let dir_name = if let Ok(ggg_path) = get_ggg_path() {
            if let Ok(subdir) = self.target_dir.strip_prefix(&ggg_path) {
                format!("$GGGPATH/{}", subdir.display())
            } else {
                format!("{}", self.target_dir.display())
            }
        } else {
            format!("{}", self.target_dir.display())
        };
        
        format!("Make directory {dir_name}").into()
    }

    fn describe(&self) {
        println!("Creating directory {}", self.target_dir.display());
    }

    fn tell_completion(&self) {
        println!("Directory created.");
    }

    fn tell_not_needed(&self) {
        println!("Directory already exists");
    }

    fn execute(&self, _always_yes: bool) -> SetupResult {
        if self.target_dir.is_dir() {
            return Ok(SetupOutcome::NotNeeded);
        } else if self.target_dir.is_file() {
            return Ok(SetupOutcome::OtherSkip("Target directory exists as a file, which is not expected".to_string()));
        }

        if !self.create_parents {
            // check that the parent directory exists
            let parent_exists = self.target_dir.parent().map(|p| p.exists())
                .expect("Cannot get target directory parent; this is a bug.");
            if !parent_exists {
                return Ok(SetupOutcome::OtherSkip("Could not create directory; parent directory does not exist.".to_string()));
            }

            std::fs::create_dir(&self.target_dir)?;
            Ok(SetupOutcome::Executed)
        } else {
            std::fs::create_dir_all(&self.target_dir)?;
            Ok(SetupOutcome::Executed)
        }
    }
}


struct AddMenuEntryStep {
    menu_file: PathBuf,
    value: &'static str,
    description: Option<&'static str>
}

impl AddMenuEntryStep {
    fn new_boxed(menu_file: PathBuf, value: &'static str, description: Option<&'static str>) -> Box<dyn SetupStep> {
        let me = Self { menu_file, value, description };
        Box::new(me)
    }
}

impl SetupStep for AddMenuEntryStep {
    fn name(&self) -> Cow<'static, str> {
        let file_name = self.menu_file.file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_else(|| self.menu_file.to_string_lossy());
        format!("Add {} entry to {}", self.value, file_name).into()
    }

    fn describe(&self) {
        println!("Adding new entry '{}' to menu {}", self.value, self.menu_file.display());
    }

    fn tell_completion(&self) {
        println!("Added entry.");
    }

    fn tell_not_needed(&self) {
        println!("Did not add entry (already present).");
    }

    fn execute(&self, _always_yes: bool) -> SetupResult {
        let current_entries = utils::read_menu_file(&self.menu_file)?;
        for entry in current_entries {
            if entry.value == self.value {
                return Ok(SetupOutcome::NotNeeded);
            }
        }

        utils::add_menu_entry(&self.menu_file, self.value, self.description)?;
        Ok(SetupOutcome::Executed)
    }
}
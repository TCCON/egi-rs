use std::path::{Path, PathBuf};

use error_stack::ResultExt;

use egi_rs::{
    config::DailyCommonArgs,
    utils::{ensure_trailing_path_sep, pattern_replacement::render_daily_pattern},
};
use ggg_rs::{tccon::sort_spectra::sort_spectra_in_dirs, utils::iter_dates};
use log::{debug, info};

use crate::CliError;

pub(crate) fn print_daily_spec_dirs(
    site_id: &str,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
    run_dir_pattern: &str,
    allow_missing: bool,
) -> error_stack::Result<(), CliError> {
    let spec_dirs = vec![];
    let spec_dirs = add_spectrum_dirs_to_list(
        spec_dirs,
        site_id,
        start_date,
        end_date,
        run_dir_pattern,
        allow_missing,
    )?;
    for dir in spec_dirs {
        println!("{dir}");
    }
    Ok(())
}

pub(crate) fn print_daily_spec_dirs_json(
    site_id: &str,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
    json_file: &Path,
    allow_missing: bool,
) -> error_stack::Result<(), CliError> {
    let common = DailyCommonArgs::read_from_path(json_file)
        .change_context_lazy(|| CliError::BadInput("Could not read JSON file".to_string()))?;
    print_daily_spec_dirs(
        site_id,
        start_date,
        end_date,
        &common.run_dir_pattern,
        allow_missing,
    )
}

fn add_spectrum_dirs_to_list(
    mut data_partition: Vec<String>,
    site_id: &str,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
    run_dir_pattern: &str,
    allow_missing: bool,
) -> error_stack::Result<Vec<String>, CliError> {
    for curr_date in iter_dates(start_date, end_date) {
        let spec_dir = render_daily_pattern(run_dir_pattern, curr_date, site_id)
            .map(|s| PathBuf::from(s))
            .change_context_lazy(|| {
                CliError::BadInput("The RUN_DIR_PATTERN was not valid".to_string())
            })?
            .join("spectra");

        if !spec_dir.exists() {
            if allow_missing {
                continue;
            } else {
                return Err(CliError::MissingInput(format!(
                    "The directory {} does not exist",
                    spec_dir.display()
                )))?;
            }
        }

        let spec_dir_str = ensure_trailing_path_sep(&spec_dir).ok_or_else(|| {
            CliError::BadInput(format!(
                "Could not encode {} to valid UTF-8",
                spec_dir.display()
            ))
        })?;

        if data_partition.contains(&spec_dir_str) {
            // already present; do nothing
        } else if let Some(idx) = dir_in_commented_line(&data_partition, &spec_dir_str) {
            // directory was present previously but commented out - remove the commenting colon
            data_partition[idx] = data_partition[idx]
                .trim_start_matches(':')
                .trim_start()
                .to_string();
        } else {
            data_partition.push(spec_dir_str);
        }
    }

    Ok(data_partition)
}

fn dir_in_commented_line(data_part: &[String], dir_str: &str) -> Option<usize> {
    for (i, s) in data_part.iter().enumerate() {
        if s.starts_with(':') && s.contains(dir_str) {
            return Some(i);
        }
    }

    return None;
}

pub(crate) fn print_daily_ordered_spectra(
    site_id: &str,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
    run_dir_pattern: &str,
    allow_missing: bool,
) -> error_stack::Result<(), CliError> {
    let spectra = list_ordered_spectra_daily(
        site_id,
        start_date,
        end_date,
        run_dir_pattern,
        allow_missing,
    )?;
    for spec in spectra {
        println!("{spec}");
    }
    Ok(())
}

fn list_ordered_spectra_daily(
    site_id: &str,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
    run_dir_pattern: &str,
    allow_missing: bool,
) -> error_stack::Result<Vec<String>, CliError> {
    let mut spec_dirs = vec![];
    info!("Searching for spectra between {start_date} and {end_date}");
    for curr_date in iter_dates(start_date, end_date) {
        let spec_dir = render_daily_pattern(run_dir_pattern, curr_date, site_id)
            .map(|s| PathBuf::from(s))
            .change_context_lazy(|| {
                CliError::BadInput("The RUN_DIR_PATTERN was not valid".to_string())
            })?
            .join("spectra");

        if !spec_dir.exists() {
            if allow_missing {
                continue;
            } else {
                return Err(CliError::MissingInput(format!(
                    "The directory {} does not exist",
                    spec_dir.display()
                )))?;
            }
        }

        debug!("Found {}", spec_dir.display());
        spec_dirs.push(spec_dir);
    }

    debug!("Sorting...");
    let sorted_spec = sort_spectra_in_dirs(&spec_dirs).change_context_lazy(|| {
        CliError::BadInput("There was a problem listing the spectra in order".to_string())
    })?;
    Ok(sorted_spec)
}

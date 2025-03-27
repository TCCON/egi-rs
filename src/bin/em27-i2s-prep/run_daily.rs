use std::{
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

use egi_rs::{
    config::DetectorSet,
    default_files,
    i2s_catalog::{self, make_catalog_entries},
    utils::{ensure_trailing_path_sep, pattern_replacement::render_daily_pattern},
};
use error_stack::ResultExt;
use ggg_rs::i2s::{self, I2SInputModifcations, I2SLineIter, I2SVersion};
use log::{debug, info, warn};

use crate::{CliError, DailyCli, DailyJsonCli};

pub(crate) fn prep_daily_i2s_json(args: DailyJsonCli) -> error_stack::Result<(), CliError> {
    let args: DailyCli = args.try_into()?;
    prep_daily_i2s(args)
}

pub(crate) fn prep_daily_i2s(args: DailyCli) -> error_stack::Result<(), CliError> {
    let mut glob_error_counts = vec![];
    let mut input_files = vec![];

    let mut curr_date = args.start_date;
    if args.end_date < curr_date {
        warn!("Warning: end date is before start date, no days will be prepared.");
    }

    while curr_date <= args.end_date {
        info!("Preparing I2S run for {curr_date}");

        // Set up the run directory with a spectrum output directory and the correct flimit file
        let res = setup_dirs(
            &args.common.igram_pattern,
            &args.common.run_dir_pattern,
            &args.site_id,
            curr_date,
            args.clear,
        );

        // A bit messy, but this unpacks the directories if everything worked, otherwise it checks
        // if the reason it failed is because there is no input data for that day and we are allowed
        // to just skip in that case, advance the loop.
        let (run_dir_path, igram_dir, spec_dir) = match res {
            Ok(dirs) => dirs,
            Err(e) => match (e.current_context(), args.no_skip_missing_dates) {
                (CliError::MissingIgramDir(_), false) => {
                    info!("Interferogram directory for {curr_date} missing, assuming no data");
                    curr_date += chrono::Duration::days(1);
                    continue;
                }
                _ => {
                    return Err(e.change_context(CliError::IoError(format!(
                        "Error setting up I2S run directory for date {curr_date}"
                    ))))
                }
            },
        };

        // Get the paths to the interferograms, as we'll need them if a UTC offset and/or detector set wasn't specified.
        let igram_glob =
            render_daily_pattern(&args.common.igram_glob_pattern, curr_date, &args.site_id)
                .change_context_lazy(|| {
                    CliError::BadInput("IGRAM_GLOB_PATTERN is not valid".to_string())
                })?;
        let (interferograms, n_glob_errs) = glob_igrams(&igram_dir, &igram_glob)?;

        if n_glob_errs > 0 {
            glob_error_counts.push((curr_date, n_glob_errs));
        }

        let (mut i2s_input_file, i2s_input_path) = create_i2s_top(
            &igram_dir,
            &run_dir_path,
            &spec_dir,
            &interferograms,
            args.common.detectors,
            &args.site_id,
            args.common.utc_offset.as_deref(),
            args.common.top_file.as_deref(),
            curr_date,
        )?;
        debug!("I2S input top written to {}", i2s_input_path.display());

        let n_entries = add_catalog_to_top(
            &mut i2s_input_file,
            &interferograms,
            &args.site_id,
            &args.common.coord_file_pattern,
            &args.common.met_file_pattern,
            curr_date,
        )
        .change_context_lazy(|| {
            CliError::IoError(format!(
                "Error occurred while adding catalog to {}",
                i2s_input_path.display()
            ))
        })?;
        debug!(
            "{} interferograms written to the catalog in {}",
            n_entries,
            i2s_input_path.display()
        );

        input_files.push(i2s_input_path);

        curr_date += chrono::Duration::days(1);
    }

    write_parallel_file(&input_files, args.parallel_file)?;

    for (date, n) in glob_error_counts {
        warn!("Warning: there were {n} files on {date} that could not be checked against the glob pattern, double check the catalog for {date}");
    }

    Ok(())
}

// ---------------------------------------------------------- //
//                     MAIN HELPER FUNCTIONS                  //
//  The functions in this section handle parts of the overall //
//           task of setting up an I2S run directory          //
// ---------------------------------------------------------- //

/// Setup the run directory and the necessary modifications for the I2S head parameters
///
/// # Inputs
/// - igram_pattern: template for paths where the interferograms are stored
/// - run_dir_pattern: template for paths where I2S should set up to run
/// - detectors: which set of detector(s) the EM27 has for this date
/// - curr_date: which date is being processed
///
/// # Returns
/// Three [`PathBuf`] instances
/// - path to the run directory,
/// - path to the directory containing the interferograms for this day, and
/// - path within the run directory where the spectra will be written.
///
/// # Errors
/// - if `igram_pattern` or `run_dir_pattern` are invalid (e.g. have an unknown substitution key), or
/// - if there is an I/O error creating the needed output directories or flimit file
fn setup_dirs(
    igram_pattern: &str,
    run_dir_pattern: &str,
    site_id: &str,
    curr_date: chrono::NaiveDate,
    clear_existing: bool,
) -> error_stack::Result<(PathBuf, PathBuf, PathBuf), CliError> {
    // Set up and create paths
    let igram_dir = render_daily_pattern(igram_pattern, curr_date, site_id)
        .change_context_lazy(|| CliError::BadInput("IGRAM_PATTERN is not valid".to_string()))?;
    let igram_path = PathBuf::from(&igram_dir);

    if !igram_path.is_dir() {
        return Err(CliError::MissingIgramDir(igram_path).into());
    }

    let run_dir = render_daily_pattern(run_dir_pattern, curr_date, site_id)
        .change_context_lazy(|| CliError::BadInput("RUN_DIR_PATTERN is not valid".to_string()))?;

    let run_dir_path = PathBuf::from(&run_dir);
    if clear_existing && run_dir_path.exists() {
        std::fs::remove_dir_all(&run_dir_path)
            .map(|_| info!("Deleted existing run directory {}", run_dir_path.display()))
            .unwrap_or_else(|e| {
                warn!(
                    "Failed to delete existing run directory {}, error was: {e}",
                    run_dir_path.display()
                )
            });
    }

    if !run_dir_path.exists() {
        std::fs::create_dir_all(&run_dir_path).change_context_lazy(|| {
            CliError::IoError(format!("could not create run directory {run_dir}"))
        })?;
    }

    let spec_dir_path = run_dir_path.join("spectra");
    if !spec_dir_path.exists() {
        std::fs::create_dir(&spec_dir_path).change_context_lazy(|| {
            CliError::IoError(format!(
                "could not create spectrum output directory {}",
                spec_dir_path.display()
            ))
        })?;
    }

    Ok((run_dir_path, igram_path, spec_dir_path))
}

/// Writes the first part of the I2S input files: the top containing I2S settings and the flimit file
///
/// # Inputs
/// - `igram_dir`: path to where the interferograms can be found
/// - `run_dir`: path to where I2S will be run
/// - `interferograms`: a slice of paths to all the interferograms to be processed on this date
/// - `detectors`: which detector set the instrument has; if `None`, this function will try to infer that
///   from the interferogram headers.
/// - `site_id`: the two-character site ID to use for this instrument
/// - `user_utc_offset`: the UTC offset value to enter into the I2S top file to convert interferogram timestamps
///   to UTC. If `None`, this function will try to infer that from the interferogram headers.
/// - `top_file_template`: a path to an I2S input top template to base the input on. If not given, the default
///   one bundled with EGI will be used. Note that parameters 1 (interferogram path), 2 (spectrum path), 7 (channel
///   to process), 8 (flimit file path), 9 (spectrum name patter), 11 (interferogram detector characters),
///   12 (spectrum detector characters) and 19 (UTC offset) will be overridden.
/// - `curr_date`: the data date for which this input file is being created.
///
/// # Returns
/// - [`std::fs::File`]: a writable file handle to the I2S input file
/// - [`PathBuf`]: the path to the input file
///
/// # Errors
/// - If the detector set must be inferred and the interferogram have different detectors or their
///   headers cannot be read.
/// - If the UTC offset must be inferred ard the inteferograms have different UTC offsets or their
///   headers cannot be read.
/// - If the interferogram or spectrum directory paths cannot be encoded as UTF-8.
/// - If writing the I2S input top or flimit file fails.
fn create_i2s_top(
    igram_dir: &Path,
    run_dir: &Path,
    spec_dir: &Path,
    interferograms: &[PathBuf],
    detectors: Option<DetectorSet>,
    site_id: &str,
    user_utc_offset: Option<&str>,
    top_file_template: Option<&Path>,
    curr_date: chrono::NaiveDate,
) -> error_stack::Result<(std::fs::File, PathBuf), CliError> {
    // Determine what detector(s) this instrument has if that wasn't included in the config.
    let detectors = if let Some(det) = detectors {
        det
    } else {
        let dtmp =
            DetectorSet::infer_from_multi_headers(&interferograms).change_context_lazy(|| {
                CliError::BadInput(format!("Unable to infer detector set for {curr_date}"))
            })?;
        log::info!("Interferograms on {curr_date} appear to use {dtmp} detector(s)");
        dtmp
    };

    let utc_offset = get_utc_offset(user_utc_offset, interferograms).change_context_lazy(|| {
        CliError::BadInput(format!(
            "Could not determine a consistent timezone for interferograms on date {curr_date}"
        ))
    })?;

    let igm_dir_param = ensure_trailing_path_sep(igram_dir).ok_or_else(|| {
        CliError::BadInput(format!("Could not encode {} as UTF-8", igram_dir.display()))
    })?;
    // Since our multii2s file ensures we CD into the run directory, it's better to make this relative
    // so that if we move this directory later, the path still works.
    let rel_spec_dir = spec_dir
        .strip_prefix(run_dir)
        .expect("spec_dir should be a subdirectory of run_dir");
    let spec_dir_param = ensure_trailing_path_sep(rel_spec_dir).ok_or_else(|| {
        CliError::BadInput(format!("Could not encode {} as UTF-8", spec_dir.display()))
    })?;
    let mut i2s_changes = detectors.get_changes();
    i2s_changes.set_parameter_change(1, igm_dir_param);
    i2s_changes.set_parameter_change(2, spec_dir_param);
    i2s_changes.set_parameter_change(8, "./flimit.i2s".to_string());
    i2s_changes.set_parameter_change(9, format!("{}YYYYMMDDS0e00C.RRRR", site_id));
    i2s_changes.set_parameter_change(19, utc_offset);

    debug!("Interferograms will be read from {}", igram_dir.display());
    debug!("Run directory will be {}", run_dir.display());

    // Create the input files in two parts. First we write the top of the I2S input file (with all of the options) plus
    // the flimit file. Then we add the catalog of interferograms to the input file.
    let i2s_input_path = run_dir.join("opus-i2s.in");
    let mut i2s_input_file = std::fs::File::create(&i2s_input_path).change_context_lazy(|| {
        CliError::IoError(format!(
            "Could not create the I2S input file at {}",
            i2s_input_path.display()
        ))
    })?;
    write_input_top(&mut i2s_input_file, &i2s_changes, top_file_template)?;
    write_flimit_file(run_dir, detectors)?;

    Ok((i2s_input_file, i2s_input_path))
}

/// Add the catalog of interferograms to the I2S input file
///
/// # Inputs
/// - `i2s_input_file`: a writeable handle to the input file; it should have the top parameters
///   already written and be ready to write the catalog header as the next line.
/// - `interferograms`: a slice of paths to all the interferograms to be processed on this date
/// - `site_id`: the two-character site ID to use for this instrument
/// - `coord_file_pattern`: a string, optionally with substitutions (e.g. date and site ID), that
///   can be rendered to produce the path to the coordinate input file for this date.
/// - `met_file_pattern`: like `coord_file_pattern`, except for the input file specifying the met
///   type and necessary options to access the met information.
/// - `curr_date`: the data date for which this input file is being created.
///
/// # Returns
/// - [`usize`] - the number of catalog entries added
///
/// # Errors
/// - If the coordinate or met file pattern is not valid.
/// - If assembling the catalog entries fails (see [`make_catalog_entries`] for why this might happen).
/// - If writing to the input file fails.
fn add_catalog_to_top(
    i2s_input_file: &mut std::fs::File,
    interferograms: &[PathBuf],
    site_id: &str,
    coord_file_pattern: &str,
    met_file_pattern: &str,
    curr_date: chrono::NaiveDate,
) -> error_stack::Result<usize, CliError> {
    let coordinate_file = render_daily_pattern(coord_file_pattern, curr_date, site_id)
        .map(PathBuf::from)
        .change_context_lazy(|| {
            CliError::BadInput("COORD_FILE_PATTERN is not valid".to_string())
        })?;
    let met_source_file = render_daily_pattern(met_file_pattern, curr_date, site_id)
        .map(PathBuf::from)
        .change_context_lazy(|| CliError::BadInput("MET_FILE_PATTERN is not valid".to_string()))?;

    let catalog_entries =
        make_catalog_entries(&coordinate_file, &met_source_file, &interferograms, false)
            .change_context_lazy(|| CliError::CatalogError)?;

    // Write the catalog
    i2s::write_opus_catalogue_table(i2s_input_file, &catalog_entries, false)
        .map_err(|e| CliError::IoError(e.to_string()))?;
    Ok(catalog_entries.len())
}

/// Get the list of interferograms matching a glob pattern
fn glob_igrams(
    igram_path: &Path,
    igram_glob: &str,
) -> error_stack::Result<(Vec<PathBuf>, u64), CliError> {
    let mut igrams = vec![];
    let mut n_glob_err = 0;

    let full_igram_pattern = igram_path.join(igram_glob);
    let full_igram_pattern = full_igram_pattern.to_str().ok_or_else(|| {
        CliError::BadInput(format!(
            "Could not convert the interferogram pattern '{}' into a valid UTF-8 string",
            full_igram_pattern.display()
        ))
    })?;

    let glob_iter = glob::glob(full_igram_pattern).change_context_lazy(|| {
        CliError::BadInput("The IGRAM_GLOB_PATTERN produced an invalid glob pattern".to_string())
    })?;

    for entry in glob_iter {
        match entry {
            Ok(p) => igrams.push(p),
            Err(_) => n_glob_err += 1,
        }
    }

    Ok((igrams, n_glob_err))
}

// ------------------------------------------------- //
//               ADDITIONAL HELPER FUNCTIONS         //
//    The functions in this section perform smaller, //
//                   individual tasks.               //
// ------------------------------------------------- //

/// Get the UTC offset string for a set of interferograms
fn get_utc_offset(
    user_utc_offset: Option<&str>,
    igram_paths: &[PathBuf],
) -> error_stack::Result<String, i2s_catalog::IgramTimezoneError> {
    if let Some(offset) = user_utc_offset {
        return Ok(offset.to_string());
    }

    let igram_tz = i2s_catalog::get_common_igram_timezone(igram_paths)?;
    let offset_hour = -igram_tz.local_minus_utc() as f32 / 3600.0;
    Ok(format!("{offset_hour:.2}"))
}

fn write_flimit_file(
    run_dir_path: &Path,
    detectors: DetectorSet,
) -> error_stack::Result<(), CliError> {
    let flimit_path = run_dir_path.join("flimit.i2s");
    let flimit_contents = detectors.get_flimit();
    let mut f = std::fs::File::create(&flimit_path).change_context_lazy(|| {
        CliError::IoError(format!(
            "Error creating flimit file at {}",
            flimit_path.display()
        ))
    })?;
    f.write_all(flimit_contents.as_bytes())
        .change_context_lazy(|| {
            CliError::IoError(format!(
                "Error writing flimit file at {}",
                flimit_path.display()
            ))
        })?;

    Ok(())
}

/// Write the top part of the I2S input file
///
/// # Inputs
/// - `input_file` - handle to write the top to
/// - `top_edits` - collection of parameters that should be set
/// - `source_top_path` - path pointing to an existing I2S top file to use as a template,
///   if `None`, the default EM27 template is used.
///
/// # Errors
/// - if cannot open/read the source top file (if given), or
/// - if cannot write the output file successfully
fn write_input_top(
    input_file: &mut std::fs::File,
    top_edits: &I2SInputModifcations,
    source_top_path: Option<&Path>,
) -> error_stack::Result<(), CliError> {
    let top_contents = if let Some(p) = source_top_path {
        let mut f = std::fs::File::open(p).change_context_lazy(|| {
            CliError::IoError(format!(
                "Error opening source I2S top file at {}",
                p.display()
            ))
        })?;

        let mut buf = String::new();
        f.read_to_string(&mut buf).change_context_lazy(|| {
            CliError::IoError(format!(
                "Error reading source I2S top file at {}",
                p.display()
            ))
        })?;

        buf
    } else {
        default_files::I2S_TOP.to_string()
    };

    let reader = BufReader::new(top_contents.as_bytes());
    modify_i2s_head(reader, top_edits, input_file)?;
    Ok(())
}

/// Write a version of the I2S header with specific changes made
///
/// # Inputs
/// - `top`: the template for the I2S header to modify. Can be anything that implements
///   the [`Read`] trait, typically a [`std::fs::File`] instance or a `&[u8]`.
/// - `edits`: collection of parameters in the I2S header to set.
/// - `writer`: handle to write the changes to, e.g. a mutable [`std::fs::File`] instance.
///
/// # Errors
/// - if reading a line from `top` fails, or
/// - if writing a line to `writer` fails
fn modify_i2s_head<R: Read, W: Write>(
    top: R,
    edits: &I2SInputModifcations,
    mut writer: W,
) -> error_stack::Result<(), CliError> {
    // TODO: this should go into ggg_rs::i2s once error types in ggg_rs are cleaned up
    let rdr = BufReader::new(top);
    let iterator = I2SLineIter::new(rdr, I2SVersion::I2S2020);
    for head_line in iterator {
        let (line_type, head_line) = head_line
            .change_context_lazy(|| CliError::IoError("Error reading I2S top file".to_string()))?;

        if let Some(new_line) = edits.change_line_opt(line_type) {
            writeln!(writer, "{}", new_line).change_context_lazy(|| {
                CliError::IoError("Error writing new line to I2S input file".to_string())
            })?;
        } else {
            write!(writer, "{}", head_line).change_context_lazy(|| {
                CliError::IoError("Error writing existing line to I2S input file".to_string())
            })?;
        }
    }
    Ok(())
}

fn write_parallel_file(
    input_files: &[PathBuf],
    parallel_file: PathBuf,
) -> error_stack::Result<(), CliError> {
    let gggpath = ggg_rs::utils::get_ggg_path().change_context_lazy(|| {
        CliError::BadInput(
            "Could not get GGGPATH, ensure the environmental variable is set".to_string(),
        )
    })?;
    let gggpath = gggpath.to_str().ok_or_else(|| {
        CliError::IoError("Could not convert GGGPATH value to valid UTF-8".to_string())
    })?;

    let mut writer = std::fs::File::create(&parallel_file).change_context_lazy(|| {
        CliError::IoError(format!(
            "Could not create parallel input file at {}",
            parallel_file.display()
        ))
    })?;

    for file in input_files {
        let run_dir = file
            .parent()
            .ok_or_else(|| {
                CliError::UnexpectedError(format!(
                    "Could not get parent of input file {}",
                    file.display()
                ))
            })?
            .to_str()
            .ok_or_else(|| {
                CliError::IoError("Could not convert run directory path to valid UTF-8".to_string())
            })?;

        let input_file = file
            .file_name()
            .ok_or_else(|| {
                CliError::UnexpectedError(format!(
                    "Could not get base name of input file {}",
                    file.display()
                ))
            })?
            .to_str()
            .ok_or_else(|| {
                CliError::IoError(
                    "Could not convert base name of input file to valid UTF-8".to_string(),
                )
            })?;

        writeln!(
            &mut writer,
            "cd {run_dir} && {gggpath}/bin/i2s {input_file} > i2s.log"
        )
        .change_context_lazy(|| {
            CliError::IoError(format!(
                "Error occurred writing line for run directory {run_dir} to {}",
                parallel_file.display()
            ))
        })?;
    }

    Ok(())
}


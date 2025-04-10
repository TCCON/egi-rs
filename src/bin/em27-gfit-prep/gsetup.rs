use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use egi_rs::utils::{get_user_menu_selection, read_menu_file};
use egi_rs::{default_files, utils::pattern_replacement::render_postproc_script_pattern};
use error_stack::ResultExt;
use ggg_rs::utils::get_ggg_path;

use crate::CliError;

pub(super) fn run_gsetup(
    run_dir: &Path,
    runlog_name: Option<&str>,
) -> error_stack::Result<(), CliError> {
    // TODO: check that the priors are ready, download if needed, abort if some of the priors are
    // not available yet.

    if !run_dir.exists() {
        std::fs::create_dir(run_dir).change_context_lazy(|| {
            CliError::other(format!(
                "Could not create run directory, {} (does the parent directory exist?)",
                run_dir.display()
            ))
        })?;
    }

    // Other than the priors check, this basically wraps gsetup to ensure that the
    // right standard options get selected then overwrites the post_processing.sh file
    // with the EM27 version.

    let ggg_path = get_ggg_path().change_context_lazy(|| {
        CliError::BadInput("Could not get GGGPATH environmental variable.".to_string())
    })?;

    // We will need the window menu to find the em27 window file; get that now so we don't
    // prompt the user if we can't finish the rest of the setup
    let win_menu_file = ggg_path.join("windows").join("gnd").join("windows.men");
    let window_options = read_menu_file(&win_menu_file).change_context_lazy(|| {
        CliError::missing_input(format!("Could not read {}", win_menu_file.display()))
    })?;
    let em27_win_index = window_options
        .iter()
        .find_map(|entry| {
            // TODO: ensure that this window file is added to the windows/gnd directory and menu by
            // a first-time setup function.
            if entry.value == "em27.gnd" {
                Some(entry.index)
            } else {
                None
            }
        })
        .ok_or_else(|| CliError::bad_input("Could not find 'em27.gnd' in the windows menu file; have you run the EGI initialization on the current GGG installation?"))?;

    // We need to read the runlog menu to determine what value to pass to gsetup.
    let menu_file = ggg_path.join("runlogs").join("gnd").join("runlogs.men");
    let runlog_options = read_menu_file(&menu_file).change_context_lazy(|| {
        CliError::MissingInput(format!("Could not read {}", menu_file.display()))
    })?;

    let runlog_index = if let Some(rn) = runlog_name {
        runlog_options
            .iter()
            .find_map(|entry| {
                if entry.value == rn {
                    Some(entry.index)
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                CliError::BadInput(format!(
                    "Could not find runlog '{rn}' in the ground runlogs.men file"
                ))
            })?
    } else {
        get_user_menu_selection(&runlog_options).change_context_lazy(|| {
            CliError::BadInput("Could not get user selection for runlog".to_string())
        })?
    };

    let gsetup = ggg_path.join("bin").join("gsetup");
    let mut child = Command::new(gsetup)
        .current_dir(run_dir)
        .stdin(Stdio::piped())
        .spawn()
        .change_context_lazy(|| CliError::program_error("Error occurred while calling gsetup"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| CliError::program_error("Failed to connect to stdin for gsetup"))?;

    // The example (https://doc.rust-lang.org/std/process/struct.Stdio.html) spawns a thread
    // to write to stdin, I assume this is to prevent a deadlock, or possibly to move the stdin
    // handle out of the parent so that wait_with_output doesn't close it.
    let gsetup_input = format!("g\n{runlog_index}\n5\n{em27_win_index}\ny\n");
    std::thread::spawn(move || {
        stdin
            .write_all(gsetup_input.as_bytes())
            .expect("Unable to write to stdin for gsetup");
    });

    let output = child.wait_with_output().change_context_lazy(|| {
        CliError::program_error("Error occurred while waiting for gsetup to finish")
    })?;

    if !output.status.success() {
        // TODO: should get the gsetup output and print so the user knows what happened.
        return Err(CliError::program_error("gsetup did not run successfully").into());
    }

    // Finally we can overwrite the existing post_processing.sh in our run directory with the EM27
    // specific one. We need the runlog and site ID to substitute in, as well as the GGGPATH as a
    // string.
    let ggg_path_str = ggg_path.to_string_lossy();
    let runlog_name = runlog_options
        .iter()
        .find_map(|entry| {
            if entry.index != runlog_index {
                return None;
            }

            // Get the runlog name as the part before the period. If no period (which shouldn't
            // happen), assume that the full value is the runlog name.
            if let Some((stem, _)) = entry.value.rsplit_once('.') {
                Some(stem)
            } else {
                Some(&entry.value)
            }
        })
        .expect("Failed to get the runlog with our previously found index");

    let (i, _) = runlog_name.char_indices().nth(1)
        .ok_or_else(|| CliError::bad_input(format!("Runlog name ({runlog_name}) too short; it did not have at least the two character site ID at the start")))?;
    let site_id = &runlog_name[..i + 1];

    let postproc_script_contents = render_postproc_script_pattern(
        default_files::POSTPROC_SCRIPT,
        &ggg_path_str,
        runlog_name,
        site_id,
    )
    .change_context_lazy(|| {
        CliError::bad_input("Could not generate the EGI post processing script.")
    })?;

    let postproc_script = run_dir.join("post_processing.sh");
    let mut f = std::fs::File::create(&postproc_script).change_context_lazy(|| {
        CliError::other(format!(
            "Could not open {} for writing",
            postproc_script.display()
        ))
    })?;
    f.write_all(postproc_script_contents.as_bytes())
        .change_context_lazy(|| {
            CliError::other(format!("Failed to write to {}", postproc_script.display()))
        })?;
    Ok(())
}

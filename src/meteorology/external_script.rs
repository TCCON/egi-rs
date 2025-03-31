use std::{
    path::Path,
    process::Command,
};

use itertools::Itertools;
use log::info;

use super::MetEntry;
use crate::utils::pattern_replacement::{render_met_script_arg_pattern, PatternError};

#[derive(Debug, thiserror::Error)]
pub(super) enum ScriptMetError {
    #[error(transparent)]
    ArgPatternError(#[from] PatternError),
    #[error("Error while getting met data: setting up to call {script} produced the following error: {error}")]
    ScriptRunError {
        script: String,
        error: std::io::Error,
    },
    #[error("Error while getting met data: calling {script} with arguments {args} returned non-zero exit code {exit_code}")]
    ScriptFailedError {
        script: String,
        args: String,
        exit_code: i32,
    },
    #[error("Error while getting met data: could not parse entry {entry_num}, error was: {error}. (Entry value was: '{entry_str}')")]
    EntryParseError {
        entry_num: u32,
        error: serde_json::Error,
        entry_str: String,
    },
}

impl ScriptMetError {
    fn script_run_error<S: ToString>(script: S, error: std::io::Error) -> Self {
        Self::ScriptRunError {
            script: script.to_string(),
            error,
        }
    }

    fn script_failed_error<S: ToString>(
        script: S,
        args: &[String],
        exit_code: Option<i32>,
    ) -> Self {
        let args = args.join(" ");
        // If terminated by a signal, the exit code will apparently be none.
        // For simplicity, we'll just give that a clearly unusual exit code.
        let exit_code = exit_code.unwrap_or(-999);
        Self::ScriptFailedError {
            script: script.to_string(),
            args,
            exit_code,
        }
    }

    fn entry_parse_error(entry_num: u32, error: serde_json::Error, entry_bytes: &[u8]) -> Self {
        let entry_str = String::from_utf8_lossy(entry_bytes).to_string();
        Self::EntryParseError {
            entry_num,
            error,
            entry_str,
        }
    }
}

/// Get meteorology for an I2S catalog by calling an external script or program
///
/// # Arguments
/// - `script`: path (preferably absolute) to the script to call. Note that the script
///   must be executable.
/// - `args`: a list of arguments to pass to the program. Any paths must be absolute or
///   relative to the working directory.
/// - `working_dir`: path (preferably absolute) in which to execute this script.

pub(super) fn read_met_with_script<S: AsRef<str>>(
    script: &str,
    args: &[S],
    working_dir: &Path,
    first_igram_time: chrono::DateTime<chrono::FixedOffset>,
    last_igram_time: chrono::DateTime<chrono::FixedOffset>,
) -> Result<Vec<MetEntry>, ScriptMetError> {
    let args: Vec<String> = args
        .iter()
        .map(|a| render_met_script_arg_pattern(a.as_ref(), first_igram_time, last_igram_time))
        .try_collect()?;

    info!(
        "Calling script '{script}' in directory '{}' to get met entries",
        working_dir.display()
    );
    let output = Command::new(script)
        .args(&args)
        .current_dir(working_dir)
        .output()
        .map_err(|e| ScriptMetError::script_run_error(script, e))?;

    if !output.status.success() {
        return Err(ScriptMetError::script_failed_error(
            script,
            &args,
            output.status.code(),
        ));
    }

    let mut met_entries = vec![];

    // In principle, this should handle OSes that LF, CR+LF, or CR only newlines.
    // By skipping empty lines, if we get a CR+LF, the LF on its own created by
    // splitting on the CR should be skipped.

    let mut ientry = 0;
    for line in output.stdout.split(|b| *b == b'\n' || *b == b'\r') {
        let line = line.trim_ascii();
        if !line.is_empty() {
            ientry += 1;
            let entry: MetEntry = serde_json::from_slice(&line)
                .map_err(|e| ScriptMetError::entry_parse_error(ientry, e, line))?;
            met_entries.push(entry);
        }
    }

    Ok(met_entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_ext_met_script() {
        let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let t1 = chrono::DateTime::parse_from_rfc3339("2025-03-01T06:00:00Z").unwrap();
        let t2 = chrono::DateTime::parse_from_rfc3339("2025-03-02T00:00:00Z").unwrap();
        let wd = crate_root.join("test_inputs");
        let entries = read_met_with_script::<String>("./dummy_met.py", &[], &wd, t1, t2).unwrap();
        let expected = vec![
            MetEntry {
                datetime: chrono::DateTime::parse_from_rfc3339("2025-03-01T12:00:00Z").unwrap(),
                pressure: 1013.25,
                temperature: None,
                humidity: None,
            },
            MetEntry {
                datetime: chrono::DateTime::parse_from_rfc3339("2025-03-01T15:00:00Z").unwrap(),
                pressure: 1013.25,
                temperature: Some(25.0),
                humidity: None,
            },
            MetEntry {
                datetime: chrono::DateTime::parse_from_rfc3339("2025-03-01T18:00:00Z").unwrap(),
                pressure: 1013.25,
                temperature: None,
                humidity: Some(50.0),
            },
            MetEntry {
                datetime: chrono::DateTime::parse_from_rfc3339("2025-03-01T21:00:00Z").unwrap(),
                pressure: 1013.25,
                temperature: Some(-10.0),
                humidity: Some(0.0),
            },
        ];

        for (a, b) in entries.into_iter().zip_eq(expected) {
            assert_eq!(a, b);
        }
    }
}

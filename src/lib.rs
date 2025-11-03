use std::{
    env,
    path::{Path, PathBuf},
};

pub mod config;
pub mod coordinates;
pub mod default_files;
pub mod i2s_catalog;
pub mod meteorology;
pub mod utils;

pub const CATALOG_FILL_FLOAT_F32: f32 = -99.0;
pub const CATALOG_FILL_FLOAT_F64: f64 = -99.0;

/// If `p` is already an absolute path, return it unchanged. Otherwise, make it relative to
/// the parent directory of `config_file`.
///
/// # Panics
/// Panics if it cannot get the parent directory of `config_file`, which should only happen
/// if a root directory was given instead of a file, so this is considered an internal mistake.
pub(crate) fn path_relative_to_config(config_file: &Path, p: PathBuf) -> PathBuf {
    if p.is_absolute() {
        p
    } else if let Some(parent_dir) = config_file.parent() {
        parent_dir.join(p)
    } else {
        panic!("Could not get parent from path {}", config_file.display());
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum EgiPathError {
    /// Indicates that no GGGPATH environmental variable was set in the current environment.
    #[error("Neither EGIPATH nor egipath environmental variable set")]
    NotSet,
    /// Indicates that the path taken from the environment points to a directory that
    /// doesn't exist at all. The contained [`PathBuf`] will be the path it expected.
    #[error("Current EGIPATH ({}) does not exist", .0.display())]
    DoesNotExist(PathBuf),
    /// Indicated that the path taken from the environment points to *something* but that
    /// something is not a directory. The contained [`PathBuf`] will be the path it checked.
    #[error("Current EGIPATH ({}) is not a directory", .0.display())]
    IsNotDir(PathBuf),
}

pub(crate) fn get_egi_path() -> Result<PathBuf, EgiPathError> {
    let env_path = env::var_os("GGGPATH")
        .or_else(|| env::var_os("gggpath"))
        .ok_or_else(|| EgiPathError::NotSet)
        .and_then(|p| Ok(PathBuf::from(p)))?;

    if !env_path.exists() {
        return Err(EgiPathError::DoesNotExist(env_path));
    }

    if !env_path.is_dir() {
        return Err(EgiPathError::IsNotDir(env_path));
    }

    Ok(env_path)
}

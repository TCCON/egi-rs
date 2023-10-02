use std::path::{PathBuf, Path};

pub mod coordinates;
pub mod meteorology;


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
use std::path::Path;

pub mod pattern_replacement;

pub fn ensure_trailing_path_sep(p: &Path) -> Option<String> {
    let mut s = p.to_str()?.to_string();
    if !s.ends_with(std::path::MAIN_SEPARATOR_STR) {
        s.push(std::path::MAIN_SEPARATOR);
    }
    Some(s)
}

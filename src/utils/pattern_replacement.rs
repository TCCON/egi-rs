use std::borrow::Cow;

use once_cell::sync::Lazy;
use regex::Regex;

pub use daily::render_daily_pattern;
pub use gsetup::render_postproc_script_pattern;
pub use met_ext_script::render_met_script_arg_pattern;
mod daily;
mod gsetup;
mod met_ext_script;

#[derive(Debug, thiserror::Error)]
pub enum PatternError {
    #[error("Unknown key '{0}' in pattern string")]
    UnknownKey(String),
}

pub(super) trait PatternReplacer {
    fn get_replacement_value(
        &self,
        key: &str,
        fmt: Option<&str>,
    ) -> Result<Cow<'_, str>, PatternError>;

    fn render_pattern(&self, pattern: &str) -> Result<String, PatternError> {
        static SUB_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{([^\}]+)\}").unwrap());
        let mut rendered = String::with_capacity(pattern.len());
        let mut last_match = 0;
        for caps in SUB_RE.captures_iter(pattern) {
            let m = caps.get(0).unwrap();
            let inner = &caps[1];
            rendered.push_str(&pattern[last_match..m.start()]);
            rendered.push_str(&self.do_pattern_replacement(inner)?);
            last_match = m.end();
        }
        rendered.push_str(&pattern[last_match..]);
        Ok(rendered)
    }

    fn do_pattern_replacement(&self, fmt_str: &str) -> Result<Cow<'_, str>, PatternError> {
        let mut split = fmt_str.splitn(2, ":");
        let key = split
            .next()
            .expect("Should always be able to get at least one substring out of a format string");
        let fmt = split.next();
        self.get_replacement_value(key, fmt)
    }
}

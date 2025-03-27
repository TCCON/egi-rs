use std::borrow::Cow;

use super::{PatternError, PatternReplacer};

pub fn render_postproc_script_pattern(
    postproc_script: &str,
    ggg_path_str: &str,
    runlog_name: &str,
    site_id: &str,
) -> Result<String, PatternError> {
    let rep = PostProcScriptReplacer {
        ggg_path_str,
        runlog_name,
        site_id,
    };
    rep.render_pattern(postproc_script)
}

struct PostProcScriptReplacer<'a> {
    ggg_path_str: &'a str,
    runlog_name: &'a str,
    site_id: &'a str,
}

impl<'a> PatternReplacer for PostProcScriptReplacer<'a> {
    fn get_replacement_value(
        &self,
        key: &str,
        _fmt: Option<&str>,
    ) -> Result<Cow<'a, str>, PatternError> {
        match key {
            "GGGPATH" => Ok(self.ggg_path_str.into()),
            "RUNLOG" => Ok(self.runlog_name.into()),
            "SITE_ID" => Ok(self.site_id.into()),
            _ => Err(PatternError::UnknownKey(key.to_string())),
        }
    }
}

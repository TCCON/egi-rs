use std::borrow::Cow;

use chrono::{DateTime, FixedOffset};

use super::{PatternError, PatternReplacer};

const DEFAULT_TIME_FMT: &'static str = "%Y-%m-%dT%H:%M:%S%z";

pub fn render_met_script_arg_pattern(
    pattern: &str,
    first_igram_time: DateTime<FixedOffset>,
    last_igram_time: DateTime<FixedOffset>,
) -> Result<String, PatternError> {
    let rep = MetArgReplacer {
        first_igram_time,
        last_igram_time,
    };
    rep.render_pattern(pattern)
}

struct MetArgReplacer {
    first_igram_time: DateTime<FixedOffset>,
    last_igram_time: DateTime<FixedOffset>,
}

impl PatternReplacer for MetArgReplacer {
    fn get_replacement_value(
        &self,
        key: &str,
        fmt: Option<&str>,
    ) -> Result<Cow<'_, str>, PatternError> {
        match key {
            "FIRST_IGRAM_TIME" => {
                let fmt = fmt.unwrap_or(DEFAULT_TIME_FMT);
                let timestr = self.first_igram_time.format(fmt).to_string();
                Ok(timestr.into())
            }
            "LAST_IGRAM_TIME" => {
                let fmt = fmt.unwrap_or(DEFAULT_TIME_FMT);
                let timestr = self.last_igram_time.format(fmt).to_string();
                Ok(timestr.into())
            }
            _ => Err(PatternError::UnknownKey(key.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_time_patterns() {
        let t1 = DateTime::parse_from_rfc3339("2025-03-01T06:00:00Z").unwrap();
        let t2 = DateTime::parse_from_rfc3339("2025-03-01T18:00:00Z").unwrap();

        let p1 = render_met_script_arg_pattern("-s{FIRST_IGRAM_TIME}", t1, t2).unwrap();
        assert_eq!(p1, "-s2025-03-01T06:00:00+0000");
        let p2 = render_met_script_arg_pattern("-e{LAST_IGRAM_TIME}", t1, t2).unwrap();
        assert_eq!(p2, "-e2025-03-01T18:00:00+0000");
    }

    #[test]
    fn test_custom_time_patterns() {
        let t1 = DateTime::parse_from_rfc3339("2025-03-01T06:00:00Z").unwrap();
        let t2 = DateTime::parse_from_rfc3339("2025-03-01T18:00:00Z").unwrap();

        let p1 = render_met_script_arg_pattern(
            "{FIRST_IGRAM_TIME:%y/%m/%d/%H/%M%:::z},{LAST_IGRAM_TIME:%y/%m/%d/%H/%M%:::z}",
            t1,
            t2,
        )
        .unwrap();
        assert_eq!(p1, "25/03/01/06/00+00,25/03/01/18/00+00");
    }
}

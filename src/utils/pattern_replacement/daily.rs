use super::{PatternError, PatternReplacer};

pub fn render_daily_pattern(
    pattern: &str,
    date: chrono::NaiveDate,
    site_id: &str,
) -> Result<String, PatternError> {
    let rep = DailyPatternReplacer { date, site_id };
    rep.render_pattern(pattern)
}

struct DailyPatternReplacer<'a> {
    date: chrono::NaiveDate,
    site_id: &'a str,
}

impl<'a> PatternReplacer for DailyPatternReplacer<'a> {
    fn get_replacement_value(&self, key: &str, fmt: Option<&str>) -> Result<String, PatternError> {
        match key {
            "DATE" => {
                let fmt = fmt.unwrap_or("%Y-%m-%d");
                let datestr = self.date.format(fmt).to_string();
                Ok(datestr)
            }
            "SITE_ID" => Ok(self.site_id.to_string()),
            _ => Err(PatternError::UnknownKey(key.to_string()).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_pattern() {
        let date = chrono::NaiveDate::from_ymd_opt(2024, 4, 1).unwrap();
        let p1 = "/data/{DATE}";
        let p1 = render_daily_pattern(p1, date, "").unwrap();
        assert_eq!(p1, "/data/2024-04-01");

        let p2 = "/data/{DATE:%Y}/{DATE:%m}/{DATE:%d}";
        let p2 = render_daily_pattern(p2, date, "").unwrap();
        assert_eq!(p2, "/data/2024/04/01");

        let p3 = "/data/{date}";
        let e = render_daily_pattern(p3, date, "");
        assert!(e.is_err());

        let p4 = "/data/{DATE}/igms/";
        let p4 = render_daily_pattern(p4, date, "").unwrap();
        assert_eq!(p4, "/data/2024-04-01/igms/");

        // This is not really desired behavior, 4.1 should not be an appropriate format for a date,
        // but there's no way to distinguish that from other aux characters (i.e. the dashes in "%Y-%m-%d"),
        // so we have to keep this behavior.
        let p5 = "/data/{DATE:4.1}";
        let p5 = render_daily_pattern(p5, date, "").unwrap();
        assert_eq!(p5, "/data/4.1");
    }

    #[test]
    fn test_site_id_pattern() {
        let date = chrono::NaiveDate::from_ymd_opt(2024, 4, 1).unwrap();
        let sid = "xx";
        let p1 = "/data/{SITE_ID}";
        let p1 = render_daily_pattern(p1, date, sid).unwrap();
        assert_eq!(p1, "/data/xx");

        let p2 = "/data/{SITE_ID}/originals/";
        let p2 = render_daily_pattern(p2, date, sid).unwrap();
        assert_eq!(p2, "/data/xx/originals/");
    }
}

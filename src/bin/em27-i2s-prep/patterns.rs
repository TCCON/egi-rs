use once_cell::sync::Lazy;
use regex::Regex;
use crate::CliError;

pub(crate) fn render_daily_pattern(pattern: &str, date: chrono::NaiveDate) -> error_stack::Result<String, CliError> {
    static SUB_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{([^\}]+)\}").unwrap());
    let mut rendered = String::with_capacity(pattern.len());
    let mut last_match = 0;
    for caps in SUB_RE.captures_iter(pattern) {
        let m = caps.get(0).unwrap();
        let inner = &caps[1];
        rendered.push_str(&pattern[last_match..m.start()]);
        rendered.push_str(&do_pattern_replacement(inner, date)?);
        last_match = m.end();
    }
    rendered.push_str(&pattern[last_match..]);
    Ok(rendered)
}

fn do_pattern_replacement(fmt_str: &str, date: chrono::NaiveDate) -> error_stack::Result<String, CliError> {
    let mut split = fmt_str.splitn(2, ":");
    let key = split.next().expect("Should always be able to get at least one substring out of a format string");
    let fmt = split.next();

    match key {
        "DATE" => {
            let fmt = fmt.unwrap_or("%Y-%m-%d");
            let datestr = date.format(fmt)
                .to_string();
            Ok(datestr)
        }
        _ => {
            Err(CliError::BadInput(
                format!("Unknown key '{key}' in format placeholder")
            ).into())
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
        let p1 = render_daily_pattern(p1, date).unwrap();
        assert_eq!(p1, "/data/2024-04-01");

        let p2 = "/data/{DATE:%Y}/{DATE:%m}/{DATE:%d}";
        let p2 = render_daily_pattern(p2, date).unwrap();
        assert_eq!(p2, "/data/2024/04/01");

        let p3 = "/data/{date}";
        let e = render_daily_pattern(p3, date);
        assert!(e.is_err());

        let p4 = "/data/{DATE}/igms/";
        let p4 = render_daily_pattern(p4, date).unwrap();
        assert_eq!(p4, "/data/2024-04-01/igms/");

        // This is not really desired behavior, 4.1 should not be an appropriate format for a date,
        // but there's no way to distinguish that from other aux characters (i.e. the dashes in "%Y-%m-%d"),
        // so we have to keep this behavior.
        let p5 = "/data/{DATE:4.1}";
        let p5 = render_daily_pattern(p5, date).unwrap();
        assert_eq!(p5, "/data/4.1");
    }
}
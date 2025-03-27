use std::{
    io::{BufRead, BufReader},
    path::Path,
};

pub mod pattern_replacement;

pub fn ensure_trailing_path_sep(p: &Path) -> Option<String> {
    let mut s = p.to_str()?.to_string();
    if !s.ends_with(std::path::MAIN_SEPARATOR_STR) {
        s.push(std::path::MAIN_SEPARATOR);
    }
    Some(s)
}

pub fn read_menu_file(menu_file: &Path) -> Result<Vec<MenuEntry>, std::io::Error> {
    let f = std::fs::File::open(menu_file)?;
    let mut rdr = BufReader::new(f);
    // skip the first line, which should always be the header
    let mut buf = String::new();
    rdr.read_line(&mut buf)?;

    let mut entries = vec![];
    let mut index = 0;
    for line in rdr.lines() {
        let line = line?;
        let mut parts = line.splitn(2, |c: char| c.is_whitespace());
        let value = if let Some(p) = parts.next() {
            p.trim().to_string()
        } else {
            // probably a blank line, just skip it.
            continue;
        };

        // also probably a blank line, just skip it
        if value.is_empty() {
            continue;
        }

        let description = parts.next().map(|p| p.trim().to_string());

        index += 1;
        entries.push(MenuEntry {
            index,
            value,
            description,
        });
    }
    Ok(entries)
}

pub fn get_user_menu_selection(entries: &[MenuEntry]) -> Result<usize, inquire::InquireError> {
    let mut min_index = 1;
    let mut max_index = 1;
    for entry in entries.iter() {
        if let Some(desc) = &entry.description {
            println!("{:3} - {} ({desc})", entry.index, entry.value);
        } else {
            println!("{:3} - {}", entry.index, entry.value);
        }

        min_index = min_index.min(entry.index);
        max_index = max_index.max(entry.index);
    }

    let prompt = format!("Select an option ({min_index}-{max_index})");
    loop {
        let i = inquire::prompt_usize(&prompt)?;
        if i >= min_index && i <= max_index {
            return Ok(i);
        }
        println!("Error: value must be between {min_index} and {max_index}");
    }
}

pub struct MenuEntry {
    pub index: usize,
    pub value: String,
    pub description: Option<String>,
}

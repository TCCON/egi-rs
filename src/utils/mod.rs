use std::{
    io::{BufRead, BufReader, Read, Write}, path::Path
};

use itertools::Itertools;

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


pub fn add_menu_entry(file: &Path, value: &str, description: Option<&str>) -> std::io::Result<()> {
    let mut current_contents = String::new();
    {
        let mut f = std::fs::File::open(file)?;
        f.read_to_string(&mut current_contents)?;
    }

    // Try to figure out where the description should start - assume that the first line
    // is the header and that it has description or similar as the second word
    let desc_start_index = find_nth_word_index(&current_contents, 1);
    let mut new_line = String::from(value);
    if let Some(desc) = description {
        if let Some(idesc) = desc_start_index {
            // Add spaces up until we only need to add one more
            while new_line.len() < idesc - 1 {
                new_line.push(' ');
            }
            // Always add at least one space to separate the value and description
            new_line.push(' ');
            new_line.push_str(desc);
        } else {
            // We didn't find where the description column starts, so just put a
            // space between the value and description.
            new_line.push(' ');
            new_line.push_str(desc);
        }
    }


    // Since all OSes other than pre-OSX Mac use a line feed in their newline, check if the
    // last character is a newline. If so, we can just append to the end of the current contents.
    // Otherwise, check if the last line is all whitespace. If so, we can just replace that line,
    // since the user must have just accidentally added a blank line. Otherwise, we actually need to
    // add an entirely new line.
    let mut lines = current_contents.split('\n').collect_vec();
    let last_line = lines.last();
    if let Some(last_line) = last_line {
        if last_line.is_empty() || last_line.chars().all(|c| c.is_whitespace()) {
            lines.pop();
        }
        lines.push(&new_line);
    } else {
        log::warn!("Adding entry to empty menu file, {}", file.display());
    }
    
    let mut ext = file.extension()
        .map(|ext| ext.to_string_lossy().to_string())
        .unwrap_or_else(|| String::new());
    ext.push_str(".bak");
    let backup = file.with_extension(ext);
    std::fs::rename(&file, &backup)?;
    let mut f = std::fs::File::create(&file)?;
    for line in lines {
        writeln!(&mut f, "{line}")?;
    }

    Ok(())
}

fn find_nth_word_index(s: &str, n: usize) -> Option<usize> {
    let mut iword = 0;
    let mut last_char_was_space = true;
    for (ichar, c) in s.char_indices() {
        if c.is_whitespace() {
            last_char_was_space = true;
        } else if last_char_was_space && !c.is_whitespace() {
            last_char_was_space = false;
            iword += 1;
        }

        if iword == n + 1 {
            return Some(ichar)
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::find_nth_word_index;

    #[test]
    fn test_nth_word_index() {
        let s = " one two  three";
        let i1 = find_nth_word_index(s, 0);
        assert_eq!(i1, Some(1));
        let i2 = find_nth_word_index(s, 1);
        assert_eq!(i2, Some(5));
        let i3 = find_nth_word_index(s, 2);
        assert_eq!(i3, Some(10));
        let i4 = find_nth_word_index(s, 3);
        assert_eq!(i4, None);
    }
}

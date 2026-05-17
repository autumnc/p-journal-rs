use crate::config;
use chrono::{DateTime, Datelike, Local, NaiveDate, TimeDelta, Utc};
use std::fs;
use std::io;

pub fn ensure_journal_dir() {
    fs::create_dir_all(config::journal_dir()).ok();
}

pub fn today_str() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

pub fn entry_exists(date_str: &str) -> bool {
    ensure_journal_dir();
    let ext = config::file_ext();
    fs::read_dir(config::journal_dir())
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.starts_with(date_str) && n.ends_with(ext))
                        .unwrap_or(false)
                })
        })
        .unwrap_or(false)
}

pub fn entry_count_today() -> usize {
    ensure_journal_dir();
    let ds = today_str();
    let ext = config::file_ext();
    fs::read_dir(config::journal_dir())
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.starts_with(&ds) && n.ends_with(ext))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

pub fn list_entries() -> Vec<String> {
    ensure_journal_dir();
    let ext = config::file_ext();
    let mut files: Vec<String> = fs::read_dir(config::journal_dir())
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.ends_with(ext) && !n.starts_with('.'))
                        .unwrap_or(false)
                })
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    files.sort_by(|a, b| b.cmp(a));
    files
}

pub fn read_entry(filename: &str) -> Option<String> {
    fs::read_to_string(config::journal_dir().join(filename)).ok()
}

pub fn get_week_dates() -> Vec<NaiveDate> {
    let today = Local::now().date_naive();
    let monday = today - TimeDelta::try_days(today.weekday().num_days_from_monday() as i64).unwrap();
    (0..7)
        .map(|i| monday + TimeDelta::try_days(i).unwrap())
        .collect()
}

pub fn get_streak() -> usize {
    let today = Local::now().date_naive();
    let mut streak: usize = 0;
    let mut day = today;
    loop {
        if entry_exists(&day.format("%Y-%m-%d").to_string()) {
            streak += 1;
            day -= TimeDelta::try_days(1).unwrap();
        } else {
            break;
        }
    }
    streak
}

pub fn get_total_entries() -> usize {
    ensure_journal_dir();
    let ext = config::file_ext();
    fs::read_dir(config::journal_dir())
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.ends_with(ext) && !n.starts_with('.'))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

pub fn save_entry(text: &str) -> io::Result<String> {
    ensure_journal_dir();
    let timestamp = Local::now().format("%Y-%m-%d_%H%M%S").to_string();
    let filename = format!("{}{}", timestamp, config::file_ext());
    let filepath = config::journal_dir().join(&filename);
    fs::write(&filepath, text)?;
    Ok(filepath.to_string_lossy().to_string())
}

pub fn extract_body_from_entry(content: &str) -> String {
    if content.is_empty() {
        return String::new();
    }
    let mut in_metadata = true;
    let mut body_lines: Vec<&str> = Vec::new();
    for line in content.lines() {
        let stripped = line.trim();
        if in_metadata {
            if stripped.starts_with("日期:")
                || stripped.starts_with("字数:")
                || stripped.starts_with("提示词:")
                || stripped == "自由写作"
                || stripped.is_empty()
            {
                continue;
            } else {
                in_metadata = false;
                body_lines.push(line);
            }
        } else {
            body_lines.push(line);
        }
    }
    body_lines.join("\n").trim().to_string()
}

pub fn get_local_mtime(filename: &str) -> Option<DateTime<Utc>> {
    let filepath = config::journal_dir().join(filename);
    match fs::metadata(&filepath) {
        Ok(meta) => match meta.modified() {
            Ok(time) => {
                let dt: DateTime<Utc> = time.into();
                Some(dt)
            }
            Err(_) => None,
        },
        Err(_) => None,
    }
}

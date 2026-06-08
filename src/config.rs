use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

const JOURNAL_DIR: &str = "journal";
const CONFIG_FILE: &str = ".pjournal";

fn home_dir() -> PathBuf {
    dirs::home_dir().expect("无法获取用户主目录")
}

pub fn journal_dir() -> PathBuf {
    home_dir().join(JOURNAL_DIR)
}

pub fn config_path() -> PathBuf {
    home_dir().join(CONFIG_FILE)
}

pub fn sync_state_path() -> PathBuf {
    home_dir().join(".pjournal_sync_state")
}

pub fn is_journal_ext(name: &str) -> bool {
    name.ends_with(".txt") || name.ends_with(".md")
}

pub fn strip_journal_ext(name: &str) -> &str {
    if name.ends_with(".md") {
        name.trim_end_matches(".md")
    } else {
        name.trim_end_matches(".txt")
    }
}

pub fn format_to_ext(format: &str) -> &str {
    match format {
        "md" => ".md",
        _ => ".txt",
    }
}

pub fn tab_width() -> usize {
    4
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub deepseek_api_key: String,
    #[serde(default)]
    pub flomo_email: String,
    #[serde(default)]
    pub flomo_password: String,
    #[serde(default)]
    pub flomo_token: String,
    #[serde(default)]
    pub webdav_url: String,
    #[serde(default)]
    pub webdav_username: String,
    #[serde(default)]
    pub webdav_password: String,
    #[serde(default)]
    pub personal_experience: String,
    #[serde(default)]
    pub personal_hobbies: String,
    #[serde(default)]
    pub personal_recent_status: String,
    #[serde(default)]
    pub markdown_enabled: bool,
    #[serde(default = "default_file_format")]
    pub file_format: String,
}

fn default_file_format() -> String {
    "txt".to_string()
}

pub fn load_config() -> Config {
    match fs::read_to_string(config_path()) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save_config(config: &Config) -> io::Result<()> {
    let json = serde_json::to_string_pretty(config).unwrap();
    fs::write(config_path(), json)?;
    let mut perms = fs::metadata(config_path())?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(config_path(), perms)?;
    Ok(())
}

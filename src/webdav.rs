use crate::config::{self, Config};
use crate::journal::{get_local_mtime, list_entries, read_entry};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, NaiveDateTime, Utc};
use quick_xml::de::from_str;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::time::Duration;

fn is_journal_ext(name: &str) -> bool {
    name.ends_with(".txt") || name.ends_with(".md")
}

#[derive(Debug, Deserialize)]
#[serde(rename = "multistatus")]
struct MultiStatus {
    #[serde(rename = "response", default)]
    responses: Vec<Response>,
}

#[derive(Debug, Deserialize)]
struct Response {
    href: Option<String>,
    #[serde(rename = "propstat", default)]
    propstats: Vec<Propstat>,
}

#[derive(Debug, Deserialize)]
struct Propstat {
    prop: Option<Prop>,
}

#[derive(Debug, Deserialize)]
struct Prop {
    #[serde(rename = "getlastmodified", default)]
    last_modified: Option<String>,
    #[serde(rename = "resourcetype", default)]
    resource_type: Option<ResourceType>,
}

#[derive(Debug, Deserialize)]
struct ResourceType {
    collection: Option<Collection>,
}

#[derive(Debug, Deserialize)]
struct Collection {}

fn make_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap()
}

fn auth_header(username: &str, password: &str) -> String {
    let creds = format!("{}:{}", username, password);
    let b64 = general_purpose::STANDARD.encode(creds.as_bytes());
    format!("Basic {}", b64)
}

fn webdav_mkdir(url: &str, username: &str, password: &str) -> bool {
    let url = if url.ends_with('/') {
        url.to_string()
    } else {
        format!("{}/", url)
    };

    let client = make_client();
    match client
        .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &url)
        .header("Authorization", auth_header(username, password))
        .header("User-Agent", "pjournal/1.0")
        .send()
    {
        Ok(resp) => resp.status().as_u16() == 201
            || resp.status().as_u16() == 200
            || resp.status().as_u16() == 405
            || resp.status().as_u16() == 301
            || resp.status().as_u16() == 302,
        Err(_) => false,
    }
}

fn webdav_upload(url: &str, username: &str, password: &str, content: &str, filename: &str) -> bool {
    let url = if url.ends_with('/') {
        format!("{}{}", url, url_encode_path(filename))
    } else {
        format!("{}/{}", url, url_encode_path(filename))
    };

    let client = make_client();
    match client
        .put(&url)
        .header("Authorization", auth_header(username, password))
        .header("User-Agent", "pjournal/1.0")
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(content.to_string())
        .send()
    {
        Ok(resp) => {
            let code = resp.status().as_u16();
            code == 200 || code == 201 || code == 204
        }
        Err(_) => false,
    }
}

fn webdav_download(url: &str, username: &str, password: &str, filename: &str) -> Option<String> {
    let url = if url.ends_with('/') {
        format!("{}{}", url, url_encode_path(filename))
    } else {
        format!("{}/{}", url, url_encode_path(filename))
    };

    let client = make_client();
    match client
        .get(&url)
        .header("Authorization", auth_header(username, password))
        .header("User-Agent", "pjournal/1.0")
        .send()
    {
        Ok(resp) => {
            if resp.status().as_u16() == 200 || resp.status().as_u16() == 203 {
                resp.text().ok()
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

fn webdav_head(
    url: &str,
    username: &str,
    password: &str,
    filename: &str,
) -> Option<DateTime<Utc>> {
    let url = if url.ends_with('/') {
        format!("{}{}", url, url_encode_path(filename))
    } else {
        format!("{}/{}", url, url_encode_path(filename))
    };

    let client = make_client();
    match client
        .head(&url)
        .header("Authorization", auth_header(username, password))
        .header("User-Agent", "pjournal/1.0")
        .send()
    {
        Ok(resp) => {
            if resp.status().as_u16() == 200 || resp.status().as_u16() == 203 {
                resp.headers()
                    .get("last-modified")
                    .and_then(|v| v.to_str().ok())
                    .and_then(parse_http_date)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

fn webdav_delete(url: &str, username: &str, password: &str, filename: &str) -> bool {
    let url = if url.ends_with('/') {
        format!("{}{}", url, url_encode_path(filename))
    } else {
        format!("{}/{}", url, url_encode_path(filename))
    };

    let client = make_client();
    match client
        .delete(&url)
        .header("Authorization", auth_header(username, password))
        .header("User-Agent", "pjournal/1.0")
        .send()
    {
        Ok(resp) => {
            let code = resp.status().as_u16();
            code == 200 || code == 204 || code == 404
        }
        Err(_) => false,
    }
}

fn webdav_propfind(
    url: &str,
    username: &str,
    password: &str,
) -> Option<HashMap<String, Option<DateTime<Utc>>>> {
    let url = if url.ends_with('/') {
        url.to_string()
    } else {
        format!("{}/", url)
    };

    let propfind_body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:getlastmodified/>
    <d:resourcetype/>
  </d:prop>
</d:propfind>"#;

    let client = make_client();
    match client
        .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
        .header("Authorization", auth_header(username, password))
        .header("User-Agent", "pjournal/1.0")
        .header("Content-Type", "application/xml; charset=utf-8")
        .header("Depth", "1")
        .body(propfind_body.to_string())
        .send()
    {
        Ok(resp) => {
            let code = resp.status().as_u16();
            if code != 207 && code != 200 {
                return None;
            }
            let body = resp.text().ok()?;

            // Parse multistatus XML
            let ms: MultiStatus = match from_str(&body) {
                Ok(ms) => ms,
                Err(_) => return None,
            };

            let mut result = HashMap::new();
            for resp_elem in ms.responses {
                let href = resp_elem.href.unwrap_or_default();
                let filename =
                    url_decode_path(&href.trim_end_matches('/').split('/').last().unwrap_or(""));

                if filename.is_empty() || !is_journal_ext(&filename) {
                    continue;
                }

                for propstat in resp_elem.propstats {
                    if let Some(prop) = propstat.prop {
                        // Skip collections (directories)
                        if let Some(rt) = &prop.resource_type {
                            if rt.collection.is_some() {
                                continue;
                            }
                        }

                        let mtime = prop
                            .last_modified
                            .as_deref()
                            .and_then(|s| parse_webdav_date(s));
                        result.insert(filename.clone(), mtime);
                    }
                }
            }
            Some(result)
        }
        Err(_) => None,
    }
}

fn parse_webdav_date(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    let fmts = [
        "%a, %d %b %Y %H:%M:%S GMT",
        "%a, %d %b %Y %H:%M:%S %Z",
        "%Y-%m-%dT%H:%M:%SZ",
        "%Y-%m-%dT%H:%M:%S%z",
        "%Y-%m-%dT%H:%M:%S",
    ];
    for fmt in &fmts {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(dt.and_utc());
        }
        if let Ok(dt) = DateTime::parse_from_str(s, fmt) {
            return Some(dt.with_timezone(&Utc));
        }
    }
    None
}

fn parse_http_date(s: &str) -> Option<DateTime<Utc>> {
    parse_webdav_date(s)
}

/// URL-encode a path component
fn url_encode_path(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' | '/' => c.to_string(),
            ' ' => "%20".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

/// URL-decode a path component
fn url_decode_path(s: &str) -> String {
    let mut result = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(
                &std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                result.push(hex as char);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

fn load_sync_state() -> HashMap<String, String> {
    let path = config::sync_state_path();
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_sync_state(state: &HashMap<String, String>) -> io::Result<()> {
    let json = serde_json::to_string_pretty(state).unwrap();
    fs::write(config::sync_state_path(), json)?;
    let mut perms = fs::metadata(config::sync_state_path())?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(config::sync_state_path(), perms)?;
    Ok(())
}

pub fn sync_to_webdav(config: &Config) -> (bool, String) {
    let url = config.webdav_url.trim();
    let username = config.webdav_username.trim();
    let password = config.webdav_password.trim();

    if url.is_empty() || username.is_empty() || password.is_empty() {
        return (false, "请先在设置中配置 WebDAV".to_string());
    }

    let _ = crate::journal::ensure_journal_dir();

    // Ensure remote directory exists
    let remote_dir = if url.ends_with('/') {
        format!("{}journal/", url)
    } else {
        format!("{}/journal/", url)
    };

    let remote_dir = if !webdav_mkdir(&remote_dir, username, password) {
        let alt_dir = if url.ends_with('/') {
            url.to_string()
        } else {
            format!("{}/", url)
        };
        if !webdav_mkdir(&alt_dir, username, password) {
            return (false, "无法创建 WebDAV 远程目录".to_string());
        }
        alt_dir
    } else {
        remote_dir
    };

    // Get local file list
    let mut local_files: HashMap<String, Option<DateTime<Utc>>> = HashMap::new();
    for fname in list_entries() {
        let mtime = get_local_mtime(&fname);
        local_files.insert(fname, mtime);
    }

    // Get remote file list
    let remote_files =
        webdav_propfind(&remote_dir, username, password).unwrap_or_default();

    // Load previous sync state
    let prev_state = load_sync_state();

    let mut uploaded = 0;
    let mut downloaded = 0;
    let mut skipped = 0;
    let mut deleted_local = 0;
    let mut deleted_remote = 0;
    let mut failed = 0;

    let mut new_state: HashMap<String, String> = HashMap::new();

    // Union of all filenames
    let mut all_filenames: Vec<String> = local_files
        .keys()
        .chain(remote_files.keys())
        .cloned()
        .collect();
    all_filenames.sort();
    all_filenames.dedup();

    for fname in &all_filenames {
        let local_mtime = local_files.get(fname).cloned().flatten();
        let mut remote_mtime = remote_files.get(fname).cloned().flatten();
        let in_prev = prev_state.contains_key(fname);

        // If PROPFIND didn't return mtime, try HEAD
        if remote_files.contains_key(fname) && remote_mtime.is_none() {
            remote_mtime = webdav_head(&remote_dir, username, password, fname);
        }

        let local_exists = local_files.contains_key(fname);
        let remote_exists = remote_files.contains_key(fname);

        if !local_exists && !remote_exists {
            continue;
        } else if !local_exists && remote_exists {
            if in_prev {
                // Local deleted -> delete remote
                if webdav_delete(&remote_dir, username, password, fname) {
                    deleted_remote += 1;
                } else {
                    failed += 1;
                }
            } else {
                // Remote new -> download
                if let Some(content) = webdav_download(&remote_dir, username, password, fname) {
                    let filepath = config::journal_dir().join(fname);
                    if fs::write(&filepath, content).is_ok() {
                        downloaded += 1;
                        if let Some(ref mt) = remote_mtime {
                            new_state.insert(fname.clone(), mt.to_rfc3339());
                        } else {
                            new_state
                                .insert(fname.clone(), Utc::now().to_rfc3339());
                        }
                    } else {
                        failed += 1;
                    }
                } else {
                    failed += 1;
                }
            }
        } else if local_exists && !remote_exists {
            if in_prev {
                // Remote deleted -> delete local
                if fs::remove_file(config::journal_dir().join(fname)).is_ok() {
                    deleted_local += 1;
                } else {
                    failed += 1;
                }
            } else {
                // Local new -> upload
                let _filepath = config::journal_dir().join(fname);
                if let Some(content) = read_entry(fname) {
                    if webdav_upload(&remote_dir, username, password, &content, fname) {
                        uploaded += 1;
                        if let Some(ref mt) = local_mtime {
                            new_state.insert(fname.clone(), mt.to_rfc3339());
                        } else {
                            new_state
                                .insert(fname.clone(), Utc::now().to_rfc3339());
                        }
                    } else {
                        failed += 1;
                    }
                } else {
                    failed += 1;
                }
            }
        } else {
            // Both exist, compare mtimes
            let lm = local_mtime.unwrap_or(Utc::now());
            let rm = remote_mtime.unwrap_or(Utc::now());

            let diff = (lm - rm).num_seconds();

            if diff.abs() <= 1 {
                skipped += 1;
                new_state.insert(fname.clone(), lm.to_rfc3339());
            } else if diff > 1 {
                // Local newer -> upload
                let _filepath = config::journal_dir().join(fname);
                if let Some(content) = read_entry(fname) {
                    if webdav_upload(&remote_dir, username, password, &content, fname) {
                        uploaded += 1;
                        new_state.insert(fname.clone(), lm.to_rfc3339());
                    } else {
                        failed += 1;
                        new_state.insert(
                            fname.clone(),
                            prev_state
                                .get(fname)
                                .cloned()
                                .unwrap_or_else(|| lm.to_rfc3339()),
                        );
                    }
                } else {
                    failed += 1;
                }
            } else {
                // Remote newer -> download
                if let Some(content) = webdav_download(&remote_dir, username, password, fname) {
                    let filepath = config::journal_dir().join(fname);
                    if fs::write(&filepath, content).is_ok() {
                        downloaded += 1;
                        new_state.insert(fname.clone(), rm.to_rfc3339());
                    } else {
                        failed += 1;
                    }
                } else {
                    failed += 1;
                }
            }
        }
    }

    save_sync_state(&new_state).ok();

    let mut parts: Vec<String> = Vec::new();
    if uploaded > 0 {
        parts.push(format!("上传 {} 篇", uploaded));
    }
    if downloaded > 0 {
        parts.push(format!("下载 {} 篇", downloaded));
    }
    if deleted_local > 0 {
        parts.push(format!("本地删除 {} 篇", deleted_local));
    }
    if deleted_remote > 0 {
        parts.push(format!("远程删除 {} 篇", deleted_remote));
    }
    if skipped > 0 {
        parts.push(format!("跳过 {} 篇", skipped));
    }
    if failed > 0 {
        parts.push(format!("失败 {} 篇", failed));
    }

    if parts.is_empty() {
        return (true, "无需同步，本地和远程一致 ✓".to_string());
    }

    if failed == 0 {
        (true, format!("同步完成: {} ✓", parts.join(" · ")))
    } else if uploaded + downloaded == 0 && deleted_local + deleted_remote == 0 {
        (false, format!("同步失败: {}", parts.join(" · ")))
    } else {
        (true, format!("部分同步: {}", parts.join(" · ")))
    }
}

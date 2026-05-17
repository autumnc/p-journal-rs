use crate::config::{save_config, Config};
use md5;
use reqwest::blocking::Client;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

const FLOMO_API_BASE: &str = "https://flomoapp.com/api/v1";
const FLOMO_API_KEY: &str = "flomo_web";
const FLOMO_APP_VERSION: &str = "4.0";
const FLOMO_PLATFORM: &str = "web";
const FLOMO_SIGN_SECRET: &str = "dbbc3dd73364b4084c3a69346e0ce2b2";
const FLOMO_TIMEZONE: &str = "8:0";

fn now_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string()
}

fn generate_flomo_sign(params: &BTreeMap<String, String>) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (key, value) in params {
        if value.is_empty() {
            continue;
        }
        parts.push(format!("{}={}", key, value));
    }
    let raw = parts.join("&") + FLOMO_SIGN_SECRET;
    format!("{:x}", md5::compute(raw.as_bytes()))
}

fn flomo_login(email: &str, password: &str) -> Option<String> {
    let mut params = BTreeMap::new();
    params.insert("email".to_string(), email.to_string());
    params.insert("password".to_string(), password.to_string());
    params.insert("wechat_union_id".to_string(), String::new());
    params.insert("wechat_oa_open_id".to_string(), String::new());
    params.insert("timestamp".to_string(), now_timestamp());
    params.insert("api_key".to_string(), FLOMO_API_KEY.to_string());
    params.insert("app_version".to_string(), FLOMO_APP_VERSION.to_string());
    params.insert("platform".to_string(), FLOMO_PLATFORM.to_string());
    params.insert("webp".to_string(), "1".to_string());

    let sign = generate_flomo_sign(&params);
    params.insert("sign".to_string(), sign);

    let client = Client::new();
    match client
        .post(format!("{}/user/login_by_email", FLOMO_API_BASE))
        .json(&params)
        .header("User-Agent", "pjournal/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .send()
    {
        Ok(resp) => {
            let result: Value = resp.json().ok()?;
            if result.get("code").and_then(|c| c.as_i64()) == Some(0) {
                result
                    .get("data")
                    .and_then(|d| d.get("access_token"))
                    .and_then(|t| t.as_str())
                    .map(String::from)
                    .or_else(|| {
                        result
                            .get("access_token")
                            .and_then(|t| t.as_str())
                            .map(String::from)
                    })
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

fn flomo_create_memo(token: &str, content: &str) -> bool {
    let mut params = BTreeMap::new();
    params.insert("timestamp".to_string(), now_timestamp());
    params.insert("api_key".to_string(), FLOMO_API_KEY.to_string());
    params.insert("app_version".to_string(), FLOMO_APP_VERSION.to_string());
    params.insert("platform".to_string(), FLOMO_PLATFORM.to_string());
    params.insert("webp".to_string(), "1".to_string());
    params.insert("content".to_string(), content.to_string());
    params.insert("source".to_string(), "web".to_string());
    params.insert("tz".to_string(), FLOMO_TIMEZONE.to_string());

    let sign = generate_flomo_sign(&params);
    params.insert("sign".to_string(), sign);

    let client = Client::new();
    match client
        .put(format!("{}/memo", FLOMO_API_BASE))
        .json(&params)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "pjournal/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .send()
    {
        Ok(resp) => {
            if let Ok(result) = resp.json::<Value>() {
                result.get("code").and_then(|c| c.as_i64()) == Some(0)
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

pub fn send_to_flomo(text: &str, config: &mut Config) -> (bool, String) {
    let email = &config.flomo_email;
    let password = &config.flomo_password;

    if email.is_empty() || password.is_empty() {
        return (false, "请先在设置中配置Flomo账号".to_string());
    }

    let content = format!("<p>{}\n\n#日记</p>", text);

    // Try cached token
    if !config.flomo_token.is_empty() {
        if flomo_create_memo(&config.flomo_token, &content) {
            return (true, "已发送到Flomo ✓".to_string());
        }
    }

    // Re-login
    if let Some(token) = flomo_login(email, password) {
        config.flomo_token = token;
        save_config(config).ok();

        if flomo_create_memo(&config.flomo_token, &content) {
            return (true, "已发送到Flomo ✓".to_string());
        } else {
            return (false, "发送到Flomo失败".to_string());
        }
    } else {
        config.flomo_token.clear();
        save_config(config).ok();
        return (false, "Flomo登录失败，请检查账号密码".to_string());
    }
}

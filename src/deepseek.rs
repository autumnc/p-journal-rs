use reqwest::blocking::Client;
use serde_json::Value;

const DEEPSEEK_API_URL: &str = "https://api.deepseek.com/v1/chat/completions";

pub fn generate_ai_prompt(api_key: &str, experience: &str, hobbies: &str) -> Option<String> {
    let system_prompt = "你是一个日记写作助手。根据用户的个人信息，随机生成一个日记提示词，\
        以问题的形式呈现。提示词应该与个人的经历和爱好相关，帮助用户深入思考。\
        只生成一个问题，不要其他内容，不要加引号。";

    let mut user_parts = Vec::new();
    if !experience.is_empty() {
        user_parts.push(format!("个人经历：{}", experience));
    }
    if !hobbies.is_empty() {
        user_parts.push(format!("个人爱好：{}", hobbies));
    }
    user_parts.push("请生成一个日记提示词：".to_string());
    let user_message = user_parts.join("\n");

    let payload = serde_json::json!({
        "model": "deepseek-chat",
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_message}
        ],
        "max_tokens": 100,
        "temperature": 0.9
    });

    let client = Client::new();
    match client
        .post(DEEPSEEK_API_URL)
        .json(&payload)
        .header("Authorization", format!("Bearer {}", api_key))
        .timeout(std::time::Duration::from_secs(30))
        .send()
    {
        Ok(resp) => {
            let result: Value = resp.json().ok()?;
            let content = result["choices"][0]["message"]["content"]
                .as_str()?
                .trim()
                .to_string();
            // Strip possible quote wrapping
            let content = content
                .trim_matches('"')
                .trim_matches('\'')
                .trim_matches('\u{201c}')
                .trim_matches('\u{201d}')
                .trim_matches('\u{2018}')
                .trim_matches('\u{2019}')
                .to_string();
            Some(content)
        }
        Err(_) => None,
    }
}

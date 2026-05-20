use reqwest::blocking::Client;
use serde_json::Value;

const DEEPSEEK_API_URL: &str = "https://api.deepseek.com/v1/chat/completions";

pub fn generate_ai_prompt(
    api_key: &str,
    experience: &str,
    hobbies: &str,
    recent_status: &str,
    seed_prompt: Option<&str>,
) -> Option<String> {
    let system_prompt = "你是一个日记写作助手。用户会给你一个种子提示词以及个人信息。\
        你需要基于种子提示词进行发散联想，生成一个全新的、更加开阔的日记提示词。\
        不要直接回答种子提示词的问题，而是借题发挥，把话题引向更深处或更广处，\
        帮助用户跳出思维惯性，写出有深度的日记。\
        只输出提示词本身，不要加「提示词：」等前缀，不要加引号，控制在三句话以内。";

    let mut user_parts = Vec::new();
    if !experience.is_empty() {
        user_parts.push(format!("个人经历：{}", experience));
    }
    if !hobbies.is_empty() {
        user_parts.push(format!("个人爱好：{}", hobbies));
    }
    if !recent_status.is_empty() {
        user_parts.push(format!("最近状态：{}", recent_status));
    }
    if let Some(seed) = seed_prompt {
        user_parts.push(format!("种子提示词：{}", seed));
    }
    user_parts.push("请生成一个新的日记提示词：".to_string());
    let user_message = user_parts.join("\n");

    let payload = serde_json::json!({
        "model": "deepseek-chat",
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_message}
        ],
        "max_tokens": 200,
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

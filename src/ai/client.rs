use crate::ai::config::AiConfig;
use anyhow::{bail, Result};
use serde_json::json;

const SQL_SYSTEM: &str =
    "You are a SQL query generator. Output ONLY valid DuckDB SQL. No markdown, no explanation.";

const SCHEMA_SYSTEM: &str =
    "You design spreadsheet/tracker schemas. Output ONLY a JSON object. No markdown, no prose.";

/// Natural-language → SQL (the AI bar).
pub async fn query_ai(config: &AiConfig, prompt: &str) -> Result<String> {
    query_with_system(config, SQL_SYSTEM, prompt).await
}

/// Natural-language → table schema JSON (the create flow).
pub async fn query_schema(config: &AiConfig, prompt: &str) -> Result<String> {
    query_with_system(config, SCHEMA_SYSTEM, prompt).await
}

async fn query_with_system(config: &AiConfig, system: &str, prompt: &str) -> Result<String> {
    match config.provider.as_str() {
        "openai" => query_openai(config, system, prompt).await,
        "anthropic" => query_anthropic(config, system, prompt).await,
        _ => bail!("Unknown AI provider: {}", config.provider),
    }
}

async fn query_openai(config: &AiConfig, system: &str, prompt: &str) -> Result<String> {
    let api_key = config
        .openai_api_key
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No OpenAI API key. Set OPENAI_API_KEY or run: xeli config set-key openai <key>"))?;

    let model = config
        .model
        .as_deref()
        .unwrap_or("gpt-4o-mini");

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": system
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.0,
            "max_tokens": 700
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("OpenAI API error {}: {}", status, body);
    }

    let body: serde_json::Value = response.json().await?;
    let sql = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(sql)
}

async fn query_anthropic(config: &AiConfig, system: &str, prompt: &str) -> Result<String> {
    let api_key = config
        .anthropic_api_key
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No Anthropic API key. Set ANTHROPIC_API_KEY or run: xeli config set-key anthropic <key>"))?;

    let model = config
        .model
        .as_deref()
        .unwrap_or("claude-sonnet-4-5-20250929");

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "max_tokens": 700,
            "system": system,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("Anthropic API error {}: {}", status, body);
    }

    let body: serde_json::Value = response.json().await?;
    let sql = body["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(sql)
}

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

pub mod anthropic;
pub mod ollama;
pub mod openai;

pub use anthropic::AnthropicProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAIProvider;

/// Messages for LLM API requests
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// LLM Provider trait - implement this for new providers
pub trait LlmProvider: Send + Sync {
    /// Provider name (e.g., "ollama", "openai", "anthropic")
    fn name(&self) -> &str;

    /// API endpoint URL
    fn endpoint(&self) -> String;

    /// Additional HTTP headers required for this provider
    fn headers(&self) -> HashMap<String, String>;

    /// Build the HTTP request body for the API
    fn build_request_body(&self, model: &str, system: &str, user: &str, stream: bool, no_think: bool) -> String;

    /// Extract content from a streaming JSON line
    fn extract_content(&self, line: &str) -> Option<String>;
}

/// Create a provider from a string name
pub fn create_provider(name: &str) -> Result<Box<dyn LlmProvider>> {
    match name {
        "ollama" => Ok(Box::new(OllamaProvider::new())),
        "openai" => {
            let api_key = env::var("OPENAI_API_KEY").context("OPENAI_API_KEY not set")?;
            Ok(Box::new(OpenAIProvider::new(api_key)))
        }
        "anthropic" => {
            let api_key = env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY not set")?;
            Ok(Box::new(AnthropicProvider::new(api_key)))
        }
        _ => bail!("Unknown provider: {}. Use: ollama, openai, or anthropic", name),
    }
}

/// Stream a response from the LLM
pub async fn stream_response<P: LlmProvider + ?Sized>(
    provider: &P,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    no_think: bool,
) -> Result<String> {
    use std::io::Write;

    let client = Client::new();

    let body = provider.build_request_body(model, system_prompt, user_prompt, true, no_think);

    let mut headers = reqwest::header::HeaderMap::new();
    for (key, value) in provider.headers() {
        let name = key
            .parse::<reqwest::header::HeaderName>()
            .with_context(|| format!("Invalid header name: {key}"))?;
        let val = value
            .parse::<reqwest::header::HeaderValue>()
            .with_context(|| format!("Invalid header value for {key}"))?;
        headers.insert(name, val);
    }

    let response = client
        .post(provider.endpoint())
        .headers(headers)
        .body(body)
        .send()
        .await
        .context("Failed to send request")?;

    // HTTP status check to avoid silently ignoring errors
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("Provider '{}' returned HTTP {}: {}", provider.name(), status, body);
    }

    let mut stream = response.bytes_stream();
    let mut full_response = String::new();

    // Buffer across chunk boundaries to ensure we split on newlines only
    let mut buffer = String::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.context("stream chunk")?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].to_string();
            buffer.drain(..=pos);
            if let Some(content) = provider.extract_content(&line) {
                print!("{}", content);
                let _ = std::io::stdout().flush();
                full_response.push_str(&content);
            }
        }
    }
    // flush remaining buffer
    if let Some(content) = provider.extract_content(&buffer) {
        print!("{}", content);
        let _ = std::io::stdout().flush();
        full_response.push_str(&content);
    }

    Ok(full_response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_ollama_provider() {
        let provider = create_provider("ollama").unwrap();
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn test_create_provider_invalid() {
        let result = create_provider("invalid");
        assert!(result.is_err());
    }
}

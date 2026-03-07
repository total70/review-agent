use crate::providers::LlmProvider;
use std::collections::HashMap;
use std::fmt;

/// Anthropic API provider
pub struct AnthropicProvider {
    api_key: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

impl fmt::Debug for AnthropicProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnthropicProvider")
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn endpoint(&self) -> String {
        "https://api.anthropic.com/v1/messages".to_string()
    }

    fn headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("x-api-key".to_string(), self.api_key.clone());
        headers.insert("anthropic-version".to_string(), "2023-06-01".to_string());
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers
    }

    fn build_request_body(&self, model: &str, system: &str, user: &str, stream: bool) -> String {
        // Map common model names to full Anthropic model names
        let model = match model {
            "claude-sonnet-4-6" | "sonnet" => "claude-sonnet-4-6-20250514",
            "claude-opus-4-5" | "opus" => "claude-opus-4-5-20250514",
            "claude-3-5-sonnet" => "claude-3-5-sonnet-20241022",
            "claude-3-opus" => "claude-3-opus-20240229",
            "claude-3-sonnet" => "claude-3-sonnet-20240229",
            "claude-3-haiku" => "claude-3-haiku-20240307",
            m => m,
        };

        serde_json::json!({
            "model": model,
            "system": system,
            "messages": [
                { "role": "user", "content": user }
            ],
            "stream": stream,
            "max_tokens": 4096
        })
        .to_string()
    }

    fn extract_content(&self, line: &str) -> Option<String> {
        // Skip empty lines and "[DONE]"
        let line = line.trim();
        if line.is_empty() || line == "data: [DONE]" {
            return None;
        }

        // Remove "data: " prefix if present
        let json = line.strip_prefix("data: ")?;

        // Parse JSON
        let value: serde_json::Value = serde_json::from_str(json).ok()?;

        // Extract content from content_block (streaming response)
        value["content_block"]["text"].as_str().map(String::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_name() {
        let provider = AnthropicProvider::new("sk-ant-test".to_string());
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_anthropic_endpoint() {
        let provider = AnthropicProvider::new("sk-ant-test".to_string());
        let endpoint = provider.endpoint();
        assert!(endpoint.contains("anthropic.com"), "endpoint should contain 'anthropic.com': {}", endpoint);
    }

    #[test]
    fn test_anthropic_headers() {
        let provider = AnthropicProvider::new("sk-ant-test123".to_string());
        let headers = provider.headers();
        assert!(headers.contains_key("x-api-key"));
        assert!(headers.contains_key("anthropic-version"));
    }

    #[test]
    fn test_extract_content_valid() {
        let provider = AnthropicProvider::new("sk-ant-test".to_string());
        let line = r#"data: {"type":"content_block","index":0,"content_block":{"type":"text","text":"Hello"}}"#;
        let content = provider.extract_content(line);
        assert_eq!(content, Some("Hello".to_string()));
    }

    #[test]
    fn test_extract_content_done() {
        let provider = AnthropicProvider::new("sk-ant-test".to_string());
        let line = "data: [DONE]";
        let content = provider.extract_content(line);
        assert!(content.is_none());
    }

    #[test]
    fn test_build_request_body() {
        let provider = AnthropicProvider::new("sk-ant-test".to_string());
        let body = provider.build_request_body("claude-sonnet-4-6", "You are helpful.", "Hi", true);
        assert!(body.contains("claude-sonnet-4-6-20250514"));
        assert!(body.contains("You are helpful."));
    }

    #[test]
    fn test_model_mapping() {
        let provider = AnthropicProvider::new("sk-ant-test".to_string());
        
        // Test sonnet mapping
        let body = provider.build_request_body("sonnet", "sys", "user", true);
        assert!(body.contains("claude-sonnet"));
    }
}

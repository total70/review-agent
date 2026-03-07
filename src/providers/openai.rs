use crate::providers::LlmProvider;
use std::collections::HashMap;
use std::fmt;

/// OpenAI API provider
pub struct OpenAIProvider {
    api_key: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

impl fmt::Debug for OpenAIProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAIProvider")
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

impl LlmProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn endpoint(&self) -> String {
        "https://api.openai.com/v1/chat/completions".to_string()
    }

    fn headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", self.api_key),
        );
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers
    }

    fn build_request_body(
        &self,
        model: &str,
        system: &str,
        user: &str,
        stream: bool,
        _no_think: bool,
    ) -> String {
        serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ],
            "stream": stream
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

        // Extract content from delta (streaming response)
        value["choices"][0]["delta"]["content"]
            .as_str()
            .map(String::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_name() {
        let provider = OpenAIProvider::new("sk-test".to_string());
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_openai_endpoint() {
        let provider = OpenAIProvider::new("sk-test".to_string());
        let endpoint = provider.endpoint();
        assert!(
            endpoint.contains("openai.com"),
            "endpoint should contain 'openai.com': {}",
            endpoint
        );
    }

    #[test]
    fn test_openai_headers() {
        let provider = OpenAIProvider::new("sk-test123".to_string());
        let headers = provider.headers();
        assert!(headers.get("Authorization").unwrap().contains("Bearer"));
    }

    #[test]
    fn test_extract_content_valid() {
        let provider = OpenAIProvider::new("sk-test".to_string());
        let line = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        let content = provider.extract_content(line);
        assert_eq!(content, Some("Hello".to_string()));
    }

    #[test]
    fn test_extract_content_done() {
        let provider = OpenAIProvider::new("sk-test".to_string());
        let line = "data: [DONE]";
        let content = provider.extract_content(line);
        assert!(content.is_none());
    }

    #[test]
    fn test_build_request_body() {
        let provider = OpenAIProvider::new("sk-test".to_string());
        let body = provider.build_request_body("gpt-4o", "You are helpful.", "Hi", true, false);
        assert!(body.contains("gpt-4o"));
        assert!(body.contains("You are helpful."));
    }
}

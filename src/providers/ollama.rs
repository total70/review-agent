use crate::providers::LlmProvider;
use std::collections::HashMap;
use std::fmt;

/// Ollama local provider
pub struct OllamaProvider {
    base_url: String,
}

impl OllamaProvider {
    pub fn new() -> Self {
        let base_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        Self { base_url }
    }
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for OllamaProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OllamaProvider")
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn endpoint(&self) -> String {
        format!("{}/api/chat", self.base_url)
    }

    fn headers(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    fn build_request_body(&self, model: &str, system: &str, user: &str, stream: bool, no_think: bool) -> String {
        let model_name = model.split(':').next().unwrap_or(model);
        let mut body = serde_json::json!({
            "model": model_name,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ],
            "stream": stream
        });
        // Only set think: false when no_think is requested; otherwise omit to allow model defaults
        if no_think {
            if let Some(map) = body.as_object_mut() {
                map.insert("think".to_string(), serde_json::json!(false));
            }
        }
        body.to_string()
    }

    fn extract_content(&self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        if value["done"].as_bool().unwrap_or(false) {
            return None;
        }
        value["message"]["content"].as_str().map(String::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_name() {
        let provider = OllamaProvider::new();
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn test_ollama_endpoint() {
        let provider = OllamaProvider::new();
        let endpoint = provider.endpoint();
        assert!(endpoint.contains("11434"), "endpoint should contain '11434': {}", endpoint);
    }

    #[test]
    fn test_extract_content_valid() {
        let provider = OllamaProvider::new();
        let line = r#"{"message":{"content":"Hello"},"done":false}"#;
        let content = provider.extract_content(line);
        assert_eq!(content, Some("Hello".to_string()));
    }

    #[test]
    fn test_extract_content_done() {
        let provider = OllamaProvider::new();
        let line = r#"{"done":true}"#;
        let content = provider.extract_content(line);
        assert!(content.is_none());
    }

    #[test]
    fn test_extract_content_empty() {
        let provider = OllamaProvider::new();
        let content = provider.extract_content("");
        assert!(content.is_none());
    }

    #[test]
    fn test_build_request_body() {
        let provider = OllamaProvider::new();
        let body = provider.build_request_body("qwen3.5", "You are a helpful assistant.", "Hello", true, true);
        assert!(body.contains("qwen3.5"));
        assert!(body.contains("You are a helpful assistant."));
        assert!(body.contains("Hello"));
        assert!(body.contains("\"think\":false"));
        // when no_think is false, omit think
        let body2 = provider.build_request_body("qwen3.5", "sys", "Hi", true, false);
        assert!(!body2.contains("\"think\""));
    }
}

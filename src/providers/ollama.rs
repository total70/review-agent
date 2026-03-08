use crate::providers::LlmProvider;
use std::collections::HashMap;
use std::fmt;

/// Ollama local provider
pub struct OllamaProvider {
    base_url: String,
}

impl OllamaProvider {
    pub fn new(host: Option<&str>) -> Self {
        let base_url = host
            .map(str::to_owned)
            .or_else(|| std::env::var("OLLAMA_BASE_URL").ok())
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let base_url = normalize_base_url(&base_url);
        Self { base_url }
    }
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new(None)
    }
}

fn normalize_base_url(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
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
        let mut h = HashMap::new();
        h.insert("Content-Type".to_string(), "application/json".to_string());
        h
    }

    fn build_request_body(
        &self,
        model: &str,
        system: &str,
        user: &str,
        stream: bool,
        no_think: bool,
    ) -> String {
        let model_name = model; // use model directly; don't strip tags
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
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_ollama_name() {
        let provider = OllamaProvider::new(None);
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn test_ollama_endpoint() {
        let provider = OllamaProvider::new(None);
        let endpoint = provider.endpoint();
        assert_eq!(endpoint, "http://localhost:11434/api/chat");
    }

    #[test]
    fn test_extract_content_valid() {
        let provider = OllamaProvider::new(None);
        let line = r#"{"message":{"content":"Hello"},"done":false}"#;
        let content = provider.extract_content(line);
        assert_eq!(content, Some("Hello".to_string()));
    }

    #[test]
    fn test_extract_content_done() {
        let provider = OllamaProvider::new(None);
        let line = r#"{"done":true}"#;
        let content = provider.extract_content(line);
        assert!(content.is_none());
    }

    #[test]
    fn test_extract_content_empty() {
        let provider = OllamaProvider::new(None);
        let content = provider.extract_content("");
        assert!(content.is_none());
    }

    #[test]
    fn test_build_request_body() {
        let provider = OllamaProvider::new(None);
        let body = provider.build_request_body(
            "qwen3.5",
            "You are a helpful assistant.",
            "Hello",
            true,
            true,
        );
        assert!(body.contains("qwen3.5"));
        assert!(body.contains("You are a helpful assistant."));
        assert!(body.contains("Hello"));
        assert!(body.contains("\"think\":false"));
        // when no_think is false, omit think
        let body2 = provider.build_request_body("qwen3.5", "sys", "Hi", true, false);
        assert!(!body2.contains("\"think\""));
    }

    #[test]
    fn test_ollama_host_from_env() {
        let _guard = env_lock().lock().unwrap();
        std::env::remove_var("OLLAMA_BASE_URL");
        std::env::set_var("OLLAMA_BASE_URL", "http://192.168.1.100:11434");

        let provider = OllamaProvider::new(None);

        std::env::remove_var("OLLAMA_BASE_URL");
        assert_eq!(provider.endpoint(), "http://192.168.1.100:11434/api/chat");
    }

    #[test]
    fn test_ollama_host_from_arg() {
        let provider = OllamaProvider::new(Some("192.168.1.100:11434"));
        assert_eq!(provider.endpoint(), "http://192.168.1.100:11434/api/chat");
    }

    #[test]
    fn test_ollama_host_arg_overrides_env() {
        let _guard = env_lock().lock().unwrap();
        std::env::remove_var("OLLAMA_BASE_URL");
        std::env::set_var("OLLAMA_BASE_URL", "http://localhost:11434");

        let provider = OllamaProvider::new(Some("https://192.168.1.100:11434"));

        std::env::remove_var("OLLAMA_BASE_URL");
        assert_eq!(provider.endpoint(), "https://192.168.1.100:11434/api/chat");
    }

    #[test]
    fn test_ollama_host_adds_scheme() {
        let provider = OllamaProvider::new(Some("192.168.1.100"));
        assert_eq!(provider.endpoint(), "http://192.168.1.100/api/chat");
    }
}

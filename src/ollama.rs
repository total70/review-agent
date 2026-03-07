use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::time::Duration;

const OLLAMA_URL: &str = "http://localhost:11434";

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    stream: bool,
    think: bool,
    messages: [Message<'a>; 2],
}

#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    done: bool,
    message: Option<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: String,
}

pub async fn ensure_ollama_running(client: &Client) -> Result<()> {
    match client.get(format!("{OLLAMA_URL}/api/tags")).send().await {
        Ok(response) if response.status().is_success() => Ok(()),
        Ok(response) => bail!(
            "Ollama returned {}. Start it with: ollama serve",
            response.status()
        ),
        Err(_) => bail!("Ollama is not running. Start it with: ollama serve"),
    }
}

pub async fn stream_review(
    client: &Client,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    no_think: bool,
) -> Result<String> {
    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(120));
    spinner.set_style(
        ProgressStyle::with_template("{spinner} Waiting for Ollama response...")
            .unwrap()
            .tick_strings(&[".", "..", "...", "...."]),
    );

    let request = ChatRequest {
        model,
        stream: true,
        think: !no_think && false,
        messages: [
            Message {
                role: "system",
                content: system_prompt,
            },
            Message {
                role: "user",
                content: user_prompt,
            },
        ],
    };

    let response = client
        .post(format!("{OLLAMA_URL}/api/chat"))
        .json(&request)
        .send()
        .await
        .context("failed to contact Ollama chat API")?
        .error_for_status()
        .context("Ollama chat API returned an error")?;

    let mut stream = response.bytes_stream();
    let mut pending = String::new();
    let mut review = String::new();
    let mut saw_token = false;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("failed to read Ollama stream")?;
        let text = std::str::from_utf8(&chunk).context("Ollama returned non-UTF-8 data")?;
        pending.push_str(text);

        while let Some(newline) = pending.find('\n') {
            let line = pending[..newline].trim().to_owned();
            let rest = pending[newline + 1..].to_owned();
            pending = rest;
            let line = line.as_str();

            if line.is_empty() {
                continue;
            }

            process_line(line, &spinner, &mut saw_token, &mut review)?;
        }
    }

    let trailing = pending.trim();
    if !trailing.is_empty() {
        process_line(trailing, &spinner, &mut saw_token, &mut review)?;
    }

    spinner.finish_and_clear();
    if !review.ends_with('\n') {
        println!();
    }
    io::stdout().flush().ok();

    if review.trim().is_empty() {
        return Err(anyhow!(
            "Ollama completed without returning any review text"
        ));
    }

    Ok(review)
}

fn process_line(
    line: &str,
    spinner: &ProgressBar,
    saw_token: &mut bool,
    review: &mut String,
) -> Result<()> {
    let chunk: ChatChunk = serde_json::from_str(line)
        .with_context(|| format!("failed to parse NDJSON line: {line}"))?;

    if let Some(message) = chunk.message {
        if !message.content.is_empty() {
            if !*saw_token {
                spinner.finish_and_clear();
                *saw_token = true;
            }
            print!("{}", message.content);
            io::stdout().flush().ok();
            review.push_str(&message.content);
        }
    }

    if chunk.done && !*saw_token {
        spinner.finish_and_clear();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use indicatif::ProgressDrawTarget;
    use serde_json::json;

    fn hidden_spinner() -> ProgressBar {
        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(ProgressDrawTarget::hidden());
        pb
    }

    // 1. Unit tests for process_line
    #[test]
    fn process_line_parses_valid_chunk_with_message() {
        let spinner = hidden_spinner();
        let mut saw_token = false;
        let mut review = String::new();
        let line = r#"{"done": false, "message": {"content": "Hello"}}"#;

        let res = process_line(line, &spinner, &mut saw_token, &mut review);
        assert!(res.is_ok());
        assert!(saw_token, "saw_token should be set after first token");
        assert_eq!(review, "Hello");
    }

    #[test]
    fn process_line_with_empty_message_does_not_update_review() {
        let spinner = hidden_spinner();
        let mut saw_token = false;
        let mut review = String::new();
        let line = r#"{"done": false, "message": {"content": ""}}"#;

        let res = process_line(line, &spinner, &mut saw_token, &mut review);
        assert!(res.is_ok());
        assert!(!saw_token, "saw_token should remain false with empty content");
        assert!(review.is_empty());
    }

    #[test]
    fn process_line_handles_done_true_without_message() {
        let spinner = hidden_spinner();
        let mut saw_token = false;
        let mut review = String::new();
        let line = r#"{"done": true}"#;

        let res = process_line(line, &spinner, &mut saw_token, &mut review);
        assert!(res.is_ok());
        assert!(!saw_token);
        assert!(review.is_empty());
    }

    #[test]
    fn process_line_json_parse_error() {
        let spinner = hidden_spinner();
        let mut saw_token = false;
        let mut review = String::new();
        let line = "not json at all";

        let err = process_line(line, &spinner, &mut saw_token, &mut review).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("failed to parse NDJSON line"), "unexpected error: {msg}");
        assert!(!saw_token);
        assert!(review.is_empty());
    }

    // 2. Unit tests for ChatRequest/Message serialization
    #[test]
    fn chat_request_serialization_think_true() {
        let req = ChatRequest {
            model: "llama3",
            stream: true,
            think: true,
            messages: [
                Message { role: "system", content: "sys" },
                Message { role: "user", content: "usr" },
            ],
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["model"], json!("llama3"));
        assert_eq!(v["stream"], json!(true));
        assert_eq!(v["think"], json!(true));
        assert_eq!(v["messages"][0]["role"], json!("system"));
        assert_eq!(v["messages"][0]["content"], json!("sys"));
        assert_eq!(v["messages"][1]["role"], json!("user"));
        assert_eq!(v["messages"][1]["content"], json!("usr"));
    }

    #[test]
    fn chat_request_serialization_think_false_and_different_model() {
        let req = ChatRequest {
            model: "mistral",
            stream: true,
            think: false,
            messages: [
                Message { role: "system", content: "a" },
                Message { role: "user", content: "b" },
            ],
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["model"], json!("mistral"));
        assert_eq!(v["stream"], json!(true));
        assert_eq!(v["think"], json!(false));
        assert_eq!(v["messages"].as_array().unwrap().len(), 2);
    }

    // 3. Edge cases for streaming logic (tested indirectly via process_line)
    #[test]
    fn streaming_multiple_json_lines_accumulate_review() {
        let spinner = hidden_spinner();
        let mut saw_token = false;
        let mut review = String::new();
        let line1 = r#"{"done": false, "message": {"content": "A"}}"#;
        let line2 = r#"{"done": false, "message": {"content": "B"}}"#;

        process_line(line1, &spinner, &mut saw_token, &mut review).unwrap();
        process_line(line2, &spinner, &mut saw_token, &mut review).unwrap();

        assert!(saw_token);
        assert_eq!(review, "AB");
    }

    #[test]
    fn streaming_multiple_json_in_single_chunk_split_on_newlines() {
        // Simulate a single network chunk that contains two NDJSON lines
        let spinner = hidden_spinner();
        let mut saw_token = false;
        let mut review = String::new();
        let mut pending = String::new();

        let combined = format!(
            "{}\n{}\n",
            r#"{"done": false, "message": {"content": "X"}}"#,
            r#"{"done": false, "message": {"content": "Y"}}"#
        );
        pending.push_str(&combined);

        while let Some(newline) = pending.find('\n') {
            let line = pending[..newline].trim().to_owned();
            let rest = pending[newline + 1..].to_owned();
            pending = rest;
            if line.is_empty() {
                continue;
            }
            process_line(&line, &spinner, &mut saw_token, &mut review).unwrap();
        }

        assert!(saw_token);
        assert_eq!(review, "XY");
    }

    #[test]
    fn streaming_trailing_partial_json_results_in_error() {
        let spinner = hidden_spinner();
        let mut saw_token = false;
        let mut review = String::new();
        // First a valid line to simulate earlier complete chunk
        let valid = r#"{"done": false, "message": {"content": "Hi"}}"#;
        process_line(valid, &spinner, &mut saw_token, &mut review).unwrap();
        assert_eq!(review, "Hi");

        // Then a trailing partial JSON (what stream_review would keep in `pending`)
        let partial = r#"{"done": false, "message": {"content": "Unfinished"}"#; // missing closing }
        let res = process_line(partial, &spinner, &mut saw_token, &mut review);
        assert!(res.is_err());
        // Previously accumulated review remains
        assert_eq!(review, "Hi");
    }
}

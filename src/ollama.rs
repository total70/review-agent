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

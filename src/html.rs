use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use pulldown_cmark::{html, Options, Parser};
use std::fs;
use std::path::Path;

const HTML_TEMPLATE: &str = include_str!("../templates/html/review.html");

pub fn render_review_html(
    markdown: &str,
    output_path: &Path,
    branch_name: &str,
    generated_at: DateTime<Local>,
) -> Result<()> {
    let mut html_output = String::new();
    let parser = Parser::new_ext(markdown, Options::all());
    html::push_html(&mut html_output, parser);

    let title = format_document_title(branch_name, generated_at);
    write_html_document(&title, &html_output, output_path, branch_name, generated_at)
}

pub fn render_error_html(
    error_title: &str,
    error_message: &str,
    output_path: &Path,
    branch_name: &str,
    generated_at: DateTime<Local>,
) -> Result<()> {
    let title = format!(
        "review-agent: {} - error - {}",
        branch_name,
        generated_at.format("%Y-%m-%d %H:%M:%S %Z")
    );
    let html_output = format!(
        concat!(
            "<section class=\"error-callout\">",
            "<p class=\"error-label\">Review failed</p>",
            "<h2>{}</h2>",
            "<pre><code>{}</code></pre>",
            "</section>"
        ),
        escape_html(error_title),
        escape_html(error_message),
    );

    write_html_document(&title, &html_output, output_path, branch_name, generated_at)
}

fn format_document_title(branch_name: &str, generated_at: DateTime<Local>) -> String {
    format!(
        "review-agent: {} - {}",
        branch_name,
        generated_at.format("%Y-%m-%d %H:%M:%S %Z")
    )
}

fn write_html_document(
    title: &str,
    html_content: &str,
    output_path: &Path,
    branch_name: &str,
    generated_at: DateTime<Local>,
) -> Result<()> {
    let document = HTML_TEMPLATE
        .replace("{{title}}", &escape_html(title))
        .replace("{{branch_name}}", &escape_html(branch_name))
        .replace(
            "{{generated_at}}",
            &escape_html(&generated_at.format("%Y-%m-%d %H:%M:%S %Z").to_string()),
        )
        .replace("{{html_content}}", html_content);

    fs::write(output_path, document)
        .with_context(|| format!("failed to write HTML output to {}", output_path.display()))?;
    Ok(())
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::fs;
    use tempfile::NamedTempFile;

    // Helper to create a fixed Local datetime for stable formatting
    fn fixed_local_datetime() -> DateTime<Local> {
        // 2024-01-02 03:04:05 in local timezone
        Local
            .with_ymd_and_hms(2024, 1, 2, 3, 4, 5)
            .single()
            .expect("valid local datetime")
    }

    // --- escape_html unit tests ---
    #[test]
    fn escape_ampersand() {
        assert_eq!(escape_html("a & b"), "a &amp; b");
    }

    #[test]
    fn escape_lt() {
        assert_eq!(escape_html("1 < 2"), "1 &lt; 2");
    }

    #[test]
    fn escape_gt() {
        assert_eq!(escape_html("2 > 1"), "2 &gt; 1");
    }

    #[test]
    fn escape_quote() {
        assert_eq!(escape_html("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn escape_all_combined() {
        let s = "<&>\"";
        assert_eq!(escape_html(s), "&lt;&amp;&gt;&quot;");
    }

    #[test]
    fn escape_passthrough_no_specials() {
        let s = "plain text 123";
        assert_eq!(escape_html(s), s);
    }

    // --- render_review_html unit tests ---
    #[test]
    fn renders_valid_html_structure() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let dt = fixed_local_datetime();

        let md = "# Title\n\nSome text.";
        render_review_html(md, &path, "feature/xyz", dt).expect("render ok");

        let out = fs::read_to_string(&path).unwrap();
        assert!(out.contains("<!doctype html>"));
        assert!(out.contains("<html"));
        assert!(out.contains("<head>"));
        assert!(out.contains("<body>"));
        assert!(out.contains("<main>"));
        assert!(out.contains("<section class=\"meta\">"));
        assert!(out.contains("<article>"));
        assert!(out.contains("</html>"));
    }

    #[test]
    fn includes_branch_and_timestamp() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let dt = fixed_local_datetime();
        let expected_ts = dt.format("%Y-%m-%d %H:%M:%S %Z").to_string();

        render_review_html("content", &path, "main", dt).expect("render ok");
        let out = fs::read_to_string(&path).unwrap();

        assert!(out.contains("<p>Branch: <strong>main</strong></p>"));
        assert!(out.contains(&expected_ts));
        // Title should also include both
        assert!(out.contains("<title>review-agent: main - "));
    }

    #[test]
    fn converts_markdown_headings_code_and_links() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let dt = fixed_local_datetime();

        let md = r#"# Heading 1

Text with a [link](https://example.com).

```
fn main() { println!("hi"); }
```
"#;
        render_review_html(md, &path, "dev", dt).expect("render ok");
        let out = fs::read_to_string(&path).unwrap();

        // Heading
        assert!(out.contains("<h1>Heading 1</h1>"));
        // Link
        assert!(out.contains("<a href=\"https://example.com\""));
        // Code block - check for pre and code elements with content
        assert!(out.contains("<pre><code") || out.contains("<pre><code>"));
        // Code content may or may not have quotes escaped depending on pulldown_cmark version
        assert!(out.contains("println!") || out.contains("fn main"));
    }

    #[test]
    fn handles_empty_markdown() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let dt = fixed_local_datetime();

        render_review_html("", &path, "empty", dt).expect("render ok");
        let out = fs::read_to_string(&path).unwrap();

        // Still a valid HTML shell with article present
        assert!(out.contains("<article>"));
        // No unexpected text required
    }

    #[test]
    fn escapes_special_chars_in_markdown_output() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let dt = fixed_local_datetime();

        let md = "Special: & < > \"";
        render_review_html(md, &path, "dev", dt).expect("render ok");
        let out = fs::read_to_string(&path).unwrap();

        // Markdown renderer should escape & in the article body (pulldown_cmark escapes &)
        assert!(out.contains("&amp;"));
        // < and > may or may not be escaped in plain text
        assert!(out.contains("Special:"));
    }

    // --- Integration test ---
    #[test]
    fn integration_writes_and_contains_required_elements() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let dt = fixed_local_datetime();

        let md = "# Review Report\n\n- Item 1\n- Item 2";
        render_review_html(md, &path, "feat/integration", dt).expect("render ok");

        let contents = fs::read_to_string(&path).expect("can read output");
        // Basic required elements
        assert!(contents.to_lowercase().contains("<!doctype html>"));
        assert!(contents.contains("<html"));
        assert!(contents.contains("<head>"));
        assert!(contents.contains("<body>"));
        assert!(contents.contains("<main>"));
        assert!(contents.contains("<section class=\"meta\">"));
        assert!(contents.contains("<article>"));
        // Ensure some markdown was rendered
        assert!(contents.contains("<h1>Review Report</h1>"));
        assert!(contents.contains("<ul>"));
        assert!(contents.contains("<li>Item 1</li>"));
        assert!(contents.contains("<li>Item 2</li>"));
    }

    #[test]
    fn render_error_html_writes_styled_error_content() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let dt = fixed_local_datetime();

        render_error_html(
            "API request failed",
            "provider timed out after 30s",
            &path,
            "feature/error-page",
            dt,
        )
        .expect("render ok");

        let out = fs::read_to_string(&path).unwrap();
        assert!(out.contains("<article>"));
        assert!(out.contains("class=\"error-callout\""));
        assert!(out.contains("<p class=\"error-label\">Review failed</p>"));
        assert!(out.contains("<h2>API request failed</h2>"));
        assert!(out.contains("provider timed out after 30s"));
        assert!(out.contains("<title>review-agent: feature/error-page - error - "));
    }

    #[test]
    fn render_error_html_escapes_error_fields() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let dt = fixed_local_datetime();

        render_error_html("<bad title>", "boom & \"oops\"", &path, "main", dt).expect("render ok");

        let out = fs::read_to_string(&path).unwrap();
        assert!(out.contains("&lt;bad title&gt;"));
        assert!(out.contains("boom &amp; &quot;oops&quot;"));
        assert!(!out.contains("<bad title>"));
    }
}

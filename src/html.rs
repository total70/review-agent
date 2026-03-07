use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use pulldown_cmark::{html, Options, Parser};
use std::fs;
use std::path::Path;

pub fn render_review_html(
    markdown: &str,
    output_path: &Path,
    branch_name: &str,
    generated_at: DateTime<Local>,
) -> Result<()> {
    let mut html_output = String::new();
    let parser = Parser::new_ext(markdown, Options::all());
    html::push_html(&mut html_output, parser);

    let title = format!(
        "review-agent: {} - {}",
        branch_name,
        generated_at.format("%Y-%m-%d %H:%M:%S %Z")
    );

    let document = format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <style>
    :root {{
      color-scheme: dark;
      --bg: #0f1115;
      --panel: #171a21;
      --panel-alt: #1e2430;
      --border: #31394a;
      --text: #edf2f7;
      --muted: #a9b4c5;
      --accent: #6cc0ff;
      --code-bg: #11161f;
      --inline-code: #ffd580;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      font-family: ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      background:
        radial-gradient(circle at top, rgba(108, 192, 255, 0.16), transparent 32rem),
        linear-gradient(180deg, #11151c 0%, var(--bg) 100%);
      color: var(--text);
      line-height: 1.7;
    }}
    main {{
      max-width: 960px;
      margin: 0 auto;
      padding: 3rem 1.25rem 4rem;
    }}
    .meta {{
      margin-bottom: 2rem;
      padding: 1rem 1.25rem;
      border: 1px solid var(--border);
      border-radius: 14px;
      background: rgba(23, 26, 33, 0.92);
      box-shadow: 0 12px 40px rgba(0, 0, 0, 0.25);
    }}
    .meta p {{
      margin: 0.2rem 0;
      color: var(--muted);
    }}
    article {{
      padding: 2rem;
      border: 1px solid var(--border);
      border-radius: 18px;
      background: rgba(23, 26, 33, 0.94);
      box-shadow: 0 18px 48px rgba(0, 0, 0, 0.28);
    }}
    h1, h2, h3, h4 {{ line-height: 1.25; }}
    h1, h2 {{ margin-top: 1.8rem; }}
    h1:first-child {{ margin-top: 0; }}
    a {{ color: var(--accent); }}
    code {{
      padding: 0.12rem 0.35rem;
      border-radius: 6px;
      background: rgba(255, 213, 128, 0.08);
      color: var(--inline-code);
      font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
      font-size: 0.95em;
    }}
    pre {{
      overflow-x: auto;
      padding: 1rem;
      border: 1px solid rgba(108, 192, 255, 0.18);
      border-radius: 12px;
      background: linear-gradient(180deg, var(--panel-alt), var(--code-bg));
    }}
    pre code {{
      padding: 0;
      background: transparent;
      color: #d7e3f4;
      font-size: 0.92rem;
    }}
    blockquote {{
      margin: 1.5rem 0;
      padding: 0.25rem 1rem;
      border-left: 4px solid var(--accent);
      color: var(--muted);
      background: rgba(108, 192, 255, 0.06);
    }}
    table {{
      width: 100%;
      border-collapse: collapse;
      overflow: hidden;
      border-radius: 12px;
    }}
    th, td {{
      padding: 0.75rem;
      border: 1px solid var(--border);
      text-align: left;
    }}
    th {{ background: rgba(108, 192, 255, 0.08); }}
  </style>
</head>
<body>
  <main>
    <section class="meta">
      <h1>{title}</h1>
      <p>Branch: <strong>{branch_name}</strong></p>
      <p>Generated: <strong>{generated_at}</strong></p>
    </section>
    <article>
      {html_output}
    </article>
  </main>
</body>
</html>
"#,
        title = escape_html(&title),
        branch_name = escape_html(branch_name),
        generated_at = escape_html(&generated_at.format("%Y-%m-%d %H:%M:%S %Z").to_string()),
        html_output = html_output,
    );

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
}

use crate::html::render_review_html;
use crate::ollama::{ensure_ollama_running, stream_review};
use anyhow::{Context, Result};
use chrono::Local;
use reqwest::Client;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;
use zip::read::ZipArchive;

pub struct RunOptions<'a> {
    pub model: &'a str,
    pub no_open: bool,
    pub no_think: bool,
}

pub async fn run_review(input: &Path, options: &RunOptions<'_>) -> Result<PathBuf> {
    let prepared = PreparedInput::load(input)?;
    let system_prompt = fs::read_to_string(prepared.root.join("AGENTS.md")).with_context(|| {
        format!(
            "failed to read {}",
            prepared.root.join("AGENTS.md").display()
        )
    })?;
    let summary = fs::read_to_string(prepared.root.join("summary.md")).with_context(|| {
        format!(
            "failed to read {}",
            prepared.root.join("summary.md").display()
        )
    })?;
    let user_prompt = build_user_prompt(&prepared.root, &summary)?;

    let client = Client::new();
    ensure_ollama_running(&client).await?;
    let review = stream_review(
        &client,
        options.model,
        &system_prompt,
        &user_prompt,
        options.no_think,
    )
    .await?;

    let review_path = prepared.root.join("review.md");
    fs::write(&review_path, &review)
        .with_context(|| format!("failed to write {}", review_path.display()))?;

    let html_path = prepared.root.join("review.html");
    let branch_name =
        extract_branch_name(&summary).unwrap_or_else(|| prepared.display_name.clone());
    render_review_html(&review, &html_path, &branch_name, Local::now())?;

    if !options.no_open {
        open::that(&html_path)
            .with_context(|| format!("failed to open {}", html_path.display()))?;
    }

    println!("review.md: {}", review_path.display());
    println!("review.html: {}", html_path.display());
    Ok(prepared.root)
}

struct PreparedInput {
    root: PathBuf,
    display_name: String,
}

impl PreparedInput {
    fn load(input: &Path) -> Result<Self> {
        if input.is_dir() {
            let root = input.canonicalize().with_context(|| {
                format!("failed to resolve input directory {}", input.display())
            })?;
            validate_review_root(&root)?;
            return Ok(Self {
                root,
                display_name: input
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("review")
                    .to_string(),
            });
        }

        let extension = input
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default();
        if extension.eq_ignore_ascii_case("zip") {
            let extracted = extract_zip(input)?;
            let display_name = input
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("review")
                .to_string();
            validate_review_root(&extracted)?;
            return Ok(Self {
                root: extracted,
                display_name,
            });
        }

        anyhow::bail!(
            "input must be a review directory or a .zip file: {}",
            input.display()
        )
    }
}

fn build_user_prompt(root: &Path, summary: &str) -> Result<String> {
    let patches_dir = root.join("patches");
    let mut patch_files = Vec::new();
    for entry in WalkDir::new(&patches_dir) {
        let entry = entry.with_context(|| format!("failed to walk {}", patches_dir.display()))?;
        if entry.file_type().is_file() {
            patch_files.push(entry.into_path());
        }
    }
    patch_files.sort();

    let mut prompt = String::new();
    prompt.push_str("Review this branch package.\n\n");
    prompt.push_str("## summary.md\n");
    prompt.push_str(summary.trim());
    prompt.push_str("\n\n");

    for path in patch_files {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path.as_path())
            .display()
            .to_string();
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read patch file {}", path.display()))?;
        prompt.push_str(&format!("## {relative}\n```diff\n{content}\n```\n\n"));
    }

    Ok(prompt)
}

fn extract_zip(input: &Path) -> Result<PathBuf> {
    let file =
        fs::File::open(input).with_context(|| format!("failed to open zip {}", input.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("failed to read zip archive {}", input.display()))?;
    let target = unique_temp_dir("review-agent-zip")?;
    fs::create_dir_all(&target)
        .with_context(|| format!("failed to create temp dir {}", target.display()))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .context("failed to read zip entry")?;
        let Some(safe_name) = entry.enclosed_name().map(|path| path.to_path_buf()) else {
            continue;
        };
        let destination = target.join(safe_name);

        if entry.name().ends_with('/') {
            fs::create_dir_all(&destination).with_context(|| {
                format!("failed to create extracted dir {}", destination.display())
            })?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let mut output = fs::File::create(&destination)
            .with_context(|| format!("failed to create {}", destination.display()))?;
        io::copy(&mut entry, &mut output)
            .with_context(|| format!("failed to extract {}", destination.display()))?;
    }

    Ok(find_review_root(&target))
}

fn find_review_root(base: &Path) -> PathBuf {
    if base.join("summary.md").is_file() && base.join("AGENTS.md").is_file() {
        return base.to_path_buf();
    }

    let mut candidates = fs::read_dir(base)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    candidates.sort();

    for candidate in candidates {
        if candidate.join("summary.md").is_file() && candidate.join("AGENTS.md").is_file() {
            return candidate;
        }
    }

    base.to_path_buf()
}

fn unique_temp_dir(prefix: &str) -> Result<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX_EPOCH")?
        .as_millis();
    Ok(std::env::temp_dir().join(format!("{prefix}-{nonce}")))
}

fn validate_review_root(root: &Path) -> Result<()> {
    for required in ["AGENTS.md", "summary.md"] {
        let path = root.join(required);
        if !path.is_file() {
            anyhow::bail!("missing required file: {}", path.display());
        }
    }

    let patches = root.join("patches");
    if !patches.is_dir() {
        anyhow::bail!("missing required directory: {}", patches.display());
    }

    Ok(())
}

fn extract_branch_name(summary: &str) -> Option<String> {
    summary.lines().find_map(|line| {
        let prefix = "**Branch:**";
        line.strip_prefix(prefix)
            .map(str::trim)
            .map(|value| value.trim_matches('`').to_string())
            .filter(|value| !value.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // Note: these tests use the tempfile crate. Add to Cargo.toml:
    // [dev-dependencies]
    // tempfile = "3"
    use tempfile::tempdir;

    #[test]
    fn test_extract_branch_name_variants() {
        // exact format
        let s = "Title\n**Branch:** feature/cool\nOther";
        assert_eq!(extract_branch_name(s), Some("feature/cool".to_string()));

        // backticks + spaces
        let s = "**Branch:**   `bugfix/issue-123`  ";
        assert_eq!(extract_branch_name(s), Some("bugfix/issue-123".to_string()));

        // empty value
        let s = "**Branch:**   ";
        assert_eq!(extract_branch_name(s), None);

        // missing prefix
        let s = "Branch: main";
        assert_eq!(extract_branch_name(s), None);

        // multiple lines, first irrelevant
        let s = "foo\nbar\n**Branch:** release/v1.2.3\nend";
        assert_eq!(extract_branch_name(s), Some("release/v1.2.3".to_string()));
    }

    fn make_valid_review_dir() -> tempfile::TempDir {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("AGENTS.md"), "agents").unwrap();
        fs::write(
            dir.path().join("summary.md"),
            "Summary here\n**Branch:** `feat/test`",
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("patches/sub"))
            .expect("create patches/sub");
        fs::write(dir.path().join("patches/a.diff"), "+add a\n- remove a").unwrap();
        fs::write(dir.path().join("patches/sub/b.diff"), "+add b").unwrap();
        dir
    }

    #[test]
    fn test_validate_review_root_valid() {
        let dir = make_valid_review_dir();
        assert!(validate_review_root(dir.path()).is_ok());
    }

    #[test]
    fn test_validate_review_root_missing_files() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("patches")).unwrap();
        // Missing AGENTS.md and summary.md
        let err = validate_review_root(dir.path()).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("missing required file"));

        // Now add AGENTS.md only
        fs::write(dir.path().join("AGENTS.md"), "agents").unwrap();
        let err = validate_review_root(dir.path()).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("missing required file"));

        // Add summary.md but remove patches dir
        fs::write(dir.path().join("summary.md"), "sum").unwrap();
        fs::remove_dir(dir.path().join("patches")).unwrap();
        let err = validate_review_root(dir.path()).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("missing required directory"));
    }

    #[test]
    fn test_build_user_prompt_contents() {
        let dir = make_valid_review_dir();
        let summary = fs::read_to_string(dir.path().join("summary.md")).unwrap();
        let prompt = build_user_prompt(dir.path(), &summary).expect("prompt");

        // Contains summary header and content (trimmed)
        assert!(prompt.contains("## summary.md"));
        assert!(prompt.contains("Summary here"));

        // Contains both patch files, with relative paths inside the temp dir
        assert!(prompt.contains("## patches/a.diff"));
        assert!(prompt.contains("## patches/sub/b.diff"));

        // Diff code fences
        assert!(prompt.contains("```diff"));
        assert!(prompt.contains("+add a"));
        assert!(prompt.contains("+add b"));
    }

    #[test]
    fn test_find_review_root_at_base() {
        // find_review_root returns base if review files are at root level
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("patches")).unwrap();
        fs::write(dir.path().join("AGENTS.md"), "agents").unwrap();
        fs::write(dir.path().join("summary.md"), "sum").unwrap();

        let found = find_review_root(dir.path());
        assert_eq!(found, dir.path());
    }

    #[test]
    fn test_unique_temp_dir_format() {
        let a = unique_temp_dir("ra").unwrap();
        assert!(a.starts_with(std::env::temp_dir()));
        assert!(a.to_string_lossy().contains("ra-"));
    }

    fn write_zip_from_dir(src: &Path, zip_path: &Path) {
        let file = fs::File::create(zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::<()>::default();

        // Walk source and write files
        for entry in WalkDir::new(src) {
            let entry = entry.unwrap();
            let path = entry.path();
            let name = path.strip_prefix(src).unwrap();
            if entry.file_type().is_dir() {
                if !name.as_os_str().is_empty() {
                    zip.add_directory(name.to_string_lossy(), options)
                        .unwrap();
                }
            } else {
                zip.start_file(name.to_string_lossy(), options).unwrap();
                let mut f = fs::File::open(path).unwrap();
                std::io::copy(&mut f, &mut zip).unwrap();
            }
        }
        zip.finish().unwrap();
    }

    #[test]
    fn test_extract_zip_and_find_root() {
        // Create a temp directory with valid review structure inside a subdir
        let src = tempdir().unwrap();
        let inner = src.path().join("myreview");
        fs::create_dir_all(inner.join("patches")).unwrap();
        fs::write(inner.join("AGENTS.md"), "agents").unwrap();
        fs::write(inner.join("summary.md"), "sum").unwrap();
        fs::write(inner.join("patches/x.diff"), "+x").unwrap();

        // Zip it up
        let zipdir = tempdir().unwrap();
        let zip_path = zipdir.path().join("pkg.zip");
        write_zip_from_dir(src.path(), &zip_path);

        // Extract via the function under test
        let extracted_root = extract_zip(&zip_path).expect("extract");
        // It should point to the inner review root
        assert!(extracted_root.join("AGENTS.md").is_file());
        assert!(extracted_root.join("summary.md").is_file());
        assert!(extracted_root.join("patches").is_dir());
    }

    #[test]
    fn test_prepared_input_load_dir_and_zip() {
        // Directory case
        let review = make_valid_review_dir();
        let pi = PreparedInput::load(review.path()).expect("load dir");
        assert_eq!(pi.root, review.path().canonicalize().unwrap());
        let expected_name = review
            .path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap();
        assert_eq!(pi.display_name, expected_name);

        // Zip case
        let src = tempdir().unwrap();
        fs::create_dir_all(src.path().join("patches")).unwrap();
        fs::write(src.path().join("AGENTS.md"), "agents").unwrap();
        fs::write(src.path().join("summary.md"), "sum").unwrap();
        fs::write(src.path().join("patches/x.diff"), "+x").unwrap();

        let zip_path = src.path().join("review.zip");
        // Write zip from the review root itself so files are top-level
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::<()>::default();
        zip.add_directory("patches/", options).unwrap();
        zip.start_file("AGENTS.md", options).unwrap();
        write!(zip, "agents").unwrap();
        zip.start_file("summary.md", options).unwrap();
        write!(zip, "sum").unwrap();
        zip.start_file("patches/x.diff", options).unwrap();
        write!(zip, "+x").unwrap();
        zip.finish().unwrap();

        let pi_zip = PreparedInput::load(&zip_path).expect("load zip");
        assert!(pi_zip.root.join("AGENTS.md").is_file());
        assert!(pi_zip.root.join("summary.md").is_file());
        assert!(pi_zip.root.join("patches/x.diff").is_file());
        assert_eq!(pi_zip.display_name, "review"); // stem of review.zip
    }
}

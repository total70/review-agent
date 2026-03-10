use anyhow::{bail, Context, Result};
use std::borrow::Cow;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// Embed the script from scripts/ folder instead of src/
const REVIEW_BRANCH_SCRIPT: &str = include_str!("../scripts/review-branch.sh");

/// Move a folder to /tmp with a unique timestamp prefix
pub fn move_to_tmp(source: &PathBuf) -> Result<PathBuf> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

    let folder_name = source
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid folder name"))?;

    let tmp_name = format!("review-{}-{}", timestamp, folder_name.to_string_lossy());
    let tmp_path = PathBuf::from("/tmp").join(tmp_name);

    // Use mv command for cross-filesystem compatibility (macOS /tmp is separate)
    let status = Command::new("mv")
        .arg(source)
        .arg(&tmp_path)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to execute mv: {}", e))?;

    if !status.success() {
        return Err(anyhow::anyhow!("mv command failed with status: {}", status));
    }

    Ok(tmp_path)
}

/// Restore folder from /tmp to original location
pub fn restore_from_tmp(tmp_path: &PathBuf, original_path: &PathBuf) -> Result<()> {
    let status = Command::new("mv")
        .arg(tmp_path)
        .arg(original_path)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to execute mv: {}", e))?;

    if !status.success() {
        return Err(anyhow::anyhow!("mv command failed with status: {}", status));
    }

    Ok(())
}

// Embed the agent templates
const TEMPLATE_GENERAL: &str = include_str!("../templates/agents/general.md");
const TEMPLATE_RUST: &str = include_str!("../templates/agents/rust.md");
const TEMPLATE_ANGULAR: &str = include_str!("../templates/agents/angular.md");

/// Get template content by name or file path
pub fn get_template(name: &str) -> Result<Cow<'static, str>> {
    // First check if it's a built-in template
    match name {
        "general" => Ok(Cow::Borrowed(TEMPLATE_GENERAL)),
        "rust" => Ok(Cow::Borrowed(TEMPLATE_RUST)),
        "angular" => Ok(Cow::Borrowed(TEMPLATE_ANGULAR)),
        _ => {
            // Check if it looks like a file path (contains / or starts with . or contains . for extension)
            let path = Path::new(name);
            let looks_like_path =
                path.components().count() > 1 || name.starts_with('.') || name.contains('.');

            // Try to read as a file if it looks like a path, OR if the file actually exists
            if looks_like_path || path.exists() {
                let content = fs::read_to_string(path)
                    .with_context(|| format!("failed to read template file: {}", name))?;
                return Ok(Cow::Owned(content));
            }

            bail!(
                "unknown template '{}'; built-ins are: general, rust, angular",
                name
            )
        }
    }
}

pub fn run_pack(
    base_branch: Option<&str>,
    output_dir: Option<&Path>,
    template: &str,
) -> Result<PathBuf> {
    let current_dir = env::current_dir().context("failed to determine current directory")?;
    let git_root = git_output(&current_dir, &["rev-parse", "--show-toplevel"])?;
    let branch_name = git_output(&current_dir, &["rev-parse", "--abbrev-ref", "HEAD"])?;

    let script_path = write_temp_script()?;
    let mut command = Command::new("bash");
    command.arg(&script_path).current_dir(&current_dir);

    if let Some(base_branch) = base_branch {
        command.arg(base_branch);
    }
    if let Some(output_dir) = output_dir {
        command.arg(output_dir);
    }

    let status = command
        .status()
        .context("failed to start review-branch.sh")?;
    let _ = fs::remove_file(&script_path);
    if !status.success() {
        bail!("review-branch.sh exited with status {status}");
    }

    let resolved_output = resolve_output_dir(&git_root, &branch_name, output_dir);
    warn_if_overwriting_output_dir(&resolved_output);
    write_agents_template(&resolved_output, template)?;

    println!("{}", resolved_output.display());
    Ok(resolved_output)
}

pub fn run_pack_uncommitted(
    base_branch: &str,
    output_dir: Option<&Path>,
    template: &str,
) -> Result<PathBuf> {
    let current_dir = env::current_dir().context("failed to determine current directory")?;
    let git_root = git_output(&current_dir, &["rev-parse", "--show-toplevel"])?;
    let branch_name = git_output(&current_dir, &["rev-parse", "--abbrev-ref", "HEAD"])?;

    git_output(&current_dir, &["rev-parse", "--verify", "HEAD"])
        .context("HEAD does not resolve to a commit")?;

    let status = git_output(&current_dir, &["status", "--porcelain"])?;
    if status.is_empty() {
        bail!("no uncommitted changes to review");
    }

    let resolved_output = resolve_output_dir(&git_root, &branch_name, output_dir);
    warn_if_overwriting_output_dir(&resolved_output);

    // For uncommitted mode, diff against HEAD to capture only working tree and staged changes,
    // not commits from other branches that may have been pulled recently.
    create_review_package_from_diff(
        &current_dir,
        &resolved_output,
        &branch_name,
        base_branch,
        "HEAD", // Use HEAD instead of merge-base to only show uncommitted changes
        true,
    )?;
    write_agents_template(&resolved_output, template)?;

    println!("{}", resolved_output.display());
    Ok(resolved_output)
}

pub fn detect_default_base_branch() -> Result<String> {
    let current_dir = env::current_dir().context("failed to determine current directory")?;
    detect_default_base_branch_for_dir(&current_dir)
}

fn write_temp_script() -> Result<PathBuf> {
    let mut path = env::temp_dir();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX_EPOCH")?
        .as_millis();
    path.push(format!("review-agent-pack-{nonce}.sh"));
    fs::write(&path, REVIEW_BRANCH_SCRIPT)
        .with_context(|| format!("failed to write temp script to {}", path.display()))?;
    Ok(path)
}

fn resolve_output_dir(git_root: &str, branch_name: &str, output_dir: Option<&Path>) -> PathBuf {
    match output_dir {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => PathBuf::from(git_root).join(path),
        None => PathBuf::from(git_root).join(format!("review-{branch_name}")),
    }
}

fn detect_default_base_branch_for_dir(current_dir: &Path) -> Result<String> {
    if let Ok(origin_head) = git_output(
        current_dir,
        &["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"],
    ) {
        let branch = origin_head
            .strip_prefix("refs/remotes/")
            .unwrap_or(origin_head.as_str())
            .to_string();
        return Ok(branch);
    }

    if let Ok(default_branch) = git_output(current_dir, &["config", "--get", "init.defaultBranch"])
    {
        if !default_branch.is_empty() {
            return Ok(format!("origin/{default_branch}"));
        }
    }

    Ok("origin/master".to_string())
}

fn existing_output_warning(output_dir: &Path) -> Option<String> {
    if output_dir.exists() {
        Some(format!(
            "warning: overwriting existing output directory {}",
            output_dir.display()
        ))
    } else {
        None
    }
}

fn warn_if_overwriting_output_dir(output_dir: &Path) {
    if let Some(message) = existing_output_warning(output_dir) {
        eprintln!("{message}");
    }
}

fn write_agents_template(output_dir: &Path, template: &str) -> Result<()> {
    let agents_path = output_dir.join("AGENTS.md");
    let template_content = get_template(template)?;
    fs::write(&agents_path, template_content.as_ref())
        .with_context(|| format!("failed to write AGENTS.md to {}", agents_path.display()))
}

fn create_review_package_from_diff(
    current_dir: &Path,
    output_dir: &Path,
    branch_name: &str,
    base_branch: &str,
    merge_base: &str,
    include_uncommitted_note: bool,
) -> Result<()> {
    let changed_files = git_diff_name_only(current_dir, merge_base, "ACMRT")?;
    let deleted_files = git_diff_name_only(current_dir, merge_base, "D")?;
    let full_patch = git_diff(current_dir, merge_base, None)?;
    let commit_log = git_output(
        current_dir,
        &["log", "--oneline", &format!("{merge_base}..HEAD")],
    )?;

    fs::create_dir_all(output_dir.join("patches"))
        .with_context(|| format!("failed to create {}", output_dir.join("patches").display()))?;
    fs::create_dir_all(output_dir.join("files"))
        .with_context(|| format!("failed to create {}", output_dir.join("files").display()))?;

    for file in &changed_files {
        let patch_path = output_dir.join("patches").join(format!("{file}.patch"));
        if let Some(parent) = patch_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let patch = git_diff(current_dir, merge_base, Some(file))?;
        fs::write(&patch_path, patch)
            .with_context(|| format!("failed to write {}", patch_path.display()))?;

        let source_path = current_dir.join(file);
        if source_path.is_file() {
            let copied_path = output_dir.join("files").join(file);
            if let Some(parent) = copied_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::copy(&source_path, &copied_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    copied_path.display()
                )
            })?;
        }
    }

    fs::write(output_dir.join("full.patch"), full_patch).with_context(|| {
        format!(
            "failed to write {}",
            output_dir.join("full.patch").display()
        )
    })?;
    fs::write(
        output_dir.join("summary.md"),
        build_summary(
            branch_name,
            base_branch,
            merge_base,
            &commit_log,
            &changed_files,
            &deleted_files,
            include_uncommitted_note,
        ),
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            output_dir.join("summary.md").display()
        )
    })?;

    Ok(())
}

fn git_diff_name_only(
    current_dir: &Path,
    merge_base: &str,
    diff_filter: &str,
) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args([
            "diff",
            merge_base,
            "--name-only",
            "--diff-filter",
            diff_filter,
        ])
        .current_dir(current_dir)
        .output()
        .with_context(|| {
            format!(
                "failed to run git diff {} --name-only --diff-filter={}",
                merge_base, diff_filter
            )
        })?;

    if !output.status.success() {
        bail!(
            "git diff {} --name-only --diff-filter={} failed: {}",
            merge_base,
            diff_filter,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8(output.stdout)?;
    let mut files = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn git_diff(current_dir: &Path, merge_base: &str, file: Option<&str>) -> Result<String> {
    let mut command = Command::new("git");
    command.arg("diff").arg(merge_base);
    if let Some(file) = file {
        command.arg("--").arg(file);
    }

    let output = command
        .current_dir(current_dir)
        .output()
        .with_context(|| match file {
            Some(file) => format!("failed to run git diff {} -- {}", merge_base, file),
            None => format!("failed to run git diff {}", merge_base),
        })?;

    if !output.status.success() {
        bail!(
            "{} failed: {}",
            match file {
                Some(file) => format!("git diff {} -- {}", merge_base, file),
                None => format!("git diff {}", merge_base),
            },
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8(output.stdout)?)
}

fn build_summary(
    branch_name: &str,
    base_branch: &str,
    merge_base: &str,
    commit_log: &str,
    changed_files: &[String],
    deleted_files: &[String],
    include_uncommitted_note: bool,
) -> String {
    let mut summary = String::new();
    summary.push_str("# Branch Review Summary\n\n");
    summary.push_str(&format!("**Branch:** `{branch_name}`\n"));
    summary.push_str(&format!("**Base:** `{base_branch}`\n"));
    summary.push_str(&format!("**Merge base:** `{merge_base}`\n"));
    if include_uncommitted_note {
        summary.push_str(
            "**Includes:** current branch commits and uncommitted working tree changes\n",
        );
    }
    summary.push_str("\n## Commits\n```\n");
    if commit_log.trim().is_empty() {
        summary.push('\n');
    } else {
        summary.push_str(commit_log.trim());
        summary.push('\n');
    }
    summary.push_str("```\n\n## Changed Files\n");
    append_file_list(&mut summary, changed_files);
    summary.push_str("\n## Deleted Files\n");
    append_file_list(&mut summary, deleted_files);
    summary
}

fn append_file_list(summary: &mut String, files: &[String]) {
    if files.is_empty() {
        summary.push_str("_None_\n");
        return;
    }

    for file in files {
        summary.push_str("- ");
        summary.push_str(file);
        summary.push('\n');
    }
}

fn git_output(current_dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(current_dir)
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;

    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self};
    use std::process::Command;
    use tempfile::TempDir;

    fn has_git() -> bool {
        Command::new("git")
            .arg("--version")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn init_bare_and_clone() -> anyhow::Result<(TempDir, PathBuf)> {
        // Create a temp workspace
        let tmp = TempDir::new()?;
        let origin_dir = tmp.path().join("origin.git");
        let work_dir = tmp.path().join("work");

        // git init --bare origin.git
        Command::new("git")
            .args(["init", "--bare", origin_dir.to_str().unwrap()])
            .status()?;

        // git clone origin.git work
        Command::new("git")
            .args([
                "clone",
                origin_dir.to_str().unwrap(),
                work_dir.to_str().unwrap(),
            ])
            .status()?;

        // Configure user
        Command::new("git")
            .args([
                "-C",
                work_dir.to_str().unwrap(),
                "config",
                "user.email",
                "tester@example.com",
            ])
            .status()?;
        Command::new("git")
            .args([
                "-C",
                work_dir.to_str().unwrap(),
                "config",
                "user.name",
                "Test User",
            ])
            .status()?;

        // Ensure master branch exists
        Command::new("git")
            .args(["-C", work_dir.to_str().unwrap(), "checkout", "-b", "master"])
            .status()?;

        // Initial commit
        let readme = work_dir.join("README.md");
        fs::write(&readme, "hello\n")?;
        Command::new("git")
            .args(["-C", work_dir.to_str().unwrap(), "add", "."])
            .status()?;
        Command::new("git")
            .args(["-C", work_dir.to_str().unwrap(), "commit", "-m", "init"])
            .status()?;

        // Push to origin master and set upstream
        Command::new("git")
            .args([
                "-C",
                work_dir.to_str().unwrap(),
                "push",
                "-u",
                "origin",
                "master",
            ])
            .status()?;

        Ok((tmp, work_dir))
    }

    #[test]
    fn test_git_output_version() -> anyhow::Result<()> {
        if !has_git() {
            return Ok(());
        }
        let td = TempDir::new()?;
        let out = git_output(td.path(), &["--version"])?;
        assert!(out.to_lowercase().contains("git version"));
        Ok(())
    }

    #[test]
    fn test_git_output_in_repo_rev_parse() -> anyhow::Result<()> {
        if !has_git() {
            return Ok(());
        }
        let (_tmp, work) = init_bare_and_clone()?;
        let short = git_output(&work, &["rev-parse", "--short", "HEAD"])?; // Should succeed and be hex
        assert!(!short.is_empty());
        assert!(short.chars().all(|c| c.is_ascii_hexdigit()));
        Ok(())
    }

    #[test]
    fn test_git_output_failure() {
        let td = TempDir::new().unwrap();
        let res = git_output(td.path(), &["definitely-not-a-command"]); // invalid subcommand
        assert!(res.is_err());
    }

    #[test]
    fn test_write_temp_script_creates_file_and_contents() -> anyhow::Result<()> {
        let path = write_temp_script()?;
        assert!(path.exists(), "temp script should exist");
        // Should be created under system temp dir
        let sys_tmp = env::temp_dir();
        assert!(path.starts_with(&sys_tmp));
        let contents = fs::read_to_string(&path)?;
        assert_eq!(contents, REVIEW_BRANCH_SCRIPT);
        // cleanup
        let _ = fs::remove_file(&path);
        Ok(())
    }

    #[test]
    fn test_run_pack_integration_with_origin_master() -> anyhow::Result<()> {
        if !has_git() {
            return Ok(());
        }
        let (_tmp, work) = init_bare_and_clone()?;

        // Create feature branch and a change
        Command::new("git")
            .args([
                "-C",
                work.to_str().unwrap(),
                "checkout",
                "-b",
                "feature/test",
            ])
            .status()?;
        let src = work.join("src.txt");
        fs::write(&src, "line1\n")?;
        Command::new("git")
            .args(["-C", work.to_str().unwrap(), "add", "."])
            .status()?;
        Command::new("git")
            .args([
                "-C",
                work.to_str().unwrap(),
                "commit",
                "-m",
                "feat: add src.txt",
            ])
            .status()?;

        // Run inside the repo directory
        let cwd_before = env::current_dir()?;
        env::set_current_dir(&work)?;

        // Output directory (absolute) inside temp
        let out_abs = work.join("out-review");
        if out_abs.exists() {
            fs::remove_dir_all(&out_abs).ok();
        }
        let result_path = run_pack(Some("origin/master"), Some(out_abs.as_path()), "general")?;

        // Restore cwd
        env::set_current_dir(cwd_before)?;

        assert_eq!(result_path, out_abs);
        // Expected structure
        assert!(out_abs.join("patches").exists());
        assert!(out_abs.join("files").exists());
        assert!(out_abs.join("summary.md").exists());
        assert!(out_abs.join("full.patch").exists());
        assert!(out_abs.join("AGENTS.md").exists());
        // Per-file artifacts for src.txt
        assert!(out_abs.join("patches/src.txt.patch").exists());
        assert!(out_abs.join("files/src.txt").exists());
        Ok(())
    }

    #[test]
    fn test_run_pack_integration_default_output_dir() -> anyhow::Result<()> {
        if !has_git() {
            return Ok(());
        }
        let (_tmp, work) = init_bare_and_clone()?;

        // Make a new change on a branch
        Command::new("git")
            .args([
                "-C",
                work.to_str().unwrap(),
                "checkout",
                "-b",
                "chore/change",
            ])
            .status()?;
        let f = work.join("data.log");
        fs::write(&f, "x\n")?;
        Command::new("git")
            .args(["-C", work.to_str().unwrap(), "add", "."])
            .status()?;
        Command::new("git")
            .args([
                "-C",
                work.to_str().unwrap(),
                "commit",
                "-m",
                "chore: add data.log",
            ])
            .status()?;

        // Run within repo, no output_dir to use default
        let cwd_before = env::current_dir()?;
        env::set_current_dir(&work)?;

        let out_path = run_pack(Some("origin/master"), None, "general")?;

        env::set_current_dir(cwd_before)?;

        // Directory name should be review-<branch>
        let branch = git_output(&work, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        assert!(out_path.ends_with(&format!("review-{}", branch)));
        assert!(out_path.join("patches").exists());
        assert!(out_path.join("files").exists());
        Ok(())
    }

    #[test]
    fn test_detect_default_base_branch_prefers_origin_head() -> anyhow::Result<()> {
        if !has_git() {
            return Ok(());
        }
        let (_tmp, work) = init_bare_and_clone()?;

        Command::new("git")
            .args(["-C", work.to_str().unwrap(), "checkout", "-b", "main"])
            .status()?;
        Command::new("git")
            .args(["-C", work.to_str().unwrap(), "push", "-u", "origin", "main"])
            .status()?;
        Command::new("git")
            .args([
                "-C",
                work.to_str().unwrap(),
                "remote",
                "set-head",
                "origin",
                "main",
            ])
            .status()?;

        let detected = detect_default_base_branch_for_dir(&work)?;
        assert_eq!(detected, "origin/main");
        Ok(())
    }

    #[test]
    fn test_detect_default_base_branch_falls_back_to_git_config() -> anyhow::Result<()> {
        if !has_git() {
            return Ok(());
        }
        let tmp = TempDir::new()?;
        Command::new("git")
            .args(["init", tmp.path().to_str().unwrap()])
            .status()?;
        Command::new("git")
            .args([
                "-C",
                tmp.path().to_str().unwrap(),
                "config",
                "init.defaultBranch",
                "main",
            ])
            .status()?;

        let detected = detect_default_base_branch_for_dir(tmp.path())?;
        assert_eq!(detected, "origin/main");
        Ok(())
    }

    #[test]
    fn test_existing_output_warning_only_for_existing_directory() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let existing = temp.path().join("review-existing");
        fs::create_dir_all(&existing)?;
        let missing = temp.path().join("review-missing");

        let warning = existing_output_warning(&existing);
        let expected_warning = format!(
            "warning: overwriting existing output directory {}",
            existing.display()
        );
        assert_eq!(warning.as_deref(), Some(expected_warning.as_str()));
        assert!(existing_output_warning(&missing).is_none());
        Ok(())
    }

    #[test]
    fn test_run_pack_uncommitted_rejects_clean_worktree() -> anyhow::Result<()> {
        if !has_git() {
            return Ok(());
        }
        let (_tmp, work) = init_bare_and_clone()?;

        let cwd_before = env::current_dir()?;
        env::set_current_dir(&work)?;

        let result = run_pack_uncommitted("origin/master", None, "general");

        env::set_current_dir(cwd_before)?;

        let err = result.expect_err("clean worktree should fail");
        assert!(err.to_string().contains("no uncommitted changes to review"));
        Ok(())
    }

    #[test]
    fn test_run_pack_uncommitted_creates_review_package_from_worktree() -> anyhow::Result<()> {
        if !has_git() {
            return Ok(());
        }
        let (_tmp, work) = init_bare_and_clone()?;

        Command::new("git")
            .args([
                "-C",
                work.to_str().unwrap(),
                "checkout",
                "-b",
                "feature/uncommitted",
            ])
            .status()?;
        let committed = work.join("committed.txt");
        fs::write(&committed, "first\n")?;
        Command::new("git")
            .args(["-C", work.to_str().unwrap(), "add", "."])
            .status()?;
        Command::new("git")
            .args([
                "-C",
                work.to_str().unwrap(),
                "commit",
                "-m",
                "feat: add committed file",
            ])
            .status()?;

        fs::write(&committed, "first\nsecond\n")?;
        let staged_new = work.join("nested").join("draft.txt");
        fs::create_dir_all(staged_new.parent().unwrap())?;
        fs::write(&staged_new, "draft\n")?;
        Command::new("git")
            .args(["-C", work.to_str().unwrap(), "add", "nested/draft.txt"])
            .status()?;

        let cwd_before = env::current_dir()?;
        env::set_current_dir(&work)?;

        let out_abs = work.join("out-uncommitted");
        if out_abs.exists() {
            fs::remove_dir_all(&out_abs).ok();
        }
        let result_path =
            run_pack_uncommitted("origin/master", Some(out_abs.as_path()), "general")?;

        env::set_current_dir(cwd_before)?;

        assert_eq!(result_path, out_abs);
        assert!(out_abs.join("patches").exists());
        assert!(out_abs.join("files").exists());
        assert!(out_abs.join("summary.md").exists());
        assert!(out_abs.join("full.patch").exists());
        assert!(out_abs.join("AGENTS.md").exists());
        assert!(out_abs.join("patches/committed.txt.patch").exists());
        assert!(out_abs.join("patches/nested/draft.txt.patch").exists());
        assert!(out_abs.join("files/committed.txt").exists());
        assert!(out_abs.join("files/nested/draft.txt").exists());

        let summary = fs::read_to_string(out_abs.join("summary.md"))?;
        assert!(summary.contains("current branch commits and uncommitted working tree changes"));
        assert!(summary.contains("- committed.txt"));
        assert!(summary.contains("- nested/draft.txt"));

        let patch = fs::read_to_string(out_abs.join("patches/nested/draft.txt.patch"))?;
        assert!(patch.contains("draft"));
        Ok(())
    }

    #[test]
    fn test_get_template_unknown() {
        let result = get_template("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown template"));
    }

    #[test]
    fn test_get_template_custom_file() -> anyhow::Result<()> {
        let td = tempfile::TempDir::new()?;
        let custom_path = td.path().join("custom.md");
        fs::write(&custom_path, "# Custom Template\nHello world")?;

        let content = get_template(custom_path.to_str().unwrap())?;
        assert!(content.contains("Custom Template"));
        Ok(())
    }

    #[test]
    fn test_get_template_missing_file() {
        let result = get_template("/tmp/__definitely_does_not_exist__.md");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("failed to read template file"), "got: {msg}");
    }

    #[test]
    fn test_get_template_directory_path_errors() -> anyhow::Result<()> {
        let td = tempfile::TempDir::new()?;
        let result = get_template(td.path().to_str().unwrap());
        // Reading a directory should fail
        assert!(result.is_err());
        Ok(())
    }
}

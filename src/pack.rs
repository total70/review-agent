use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// Embed the script from scripts/ folder instead of src/
const REVIEW_BRANCH_SCRIPT: &str = include_str!("../scripts/review-branch.sh");

// Embed the agent templates
const TEMPLATE_GENERAL: &str = include_str!("../templates/agents/general.md");
const TEMPLATE_RUST: &str = include_str!("../templates/agents/rust.md");
const TEMPLATE_ANGULAR: &str = include_str!("../templates/agents/angular.md");

/// Get template content by name
pub fn get_template(name: &str) -> Result<&'static str> {
    match name {
        "general" => Ok(TEMPLATE_GENERAL),
        "rust" => Ok(TEMPLATE_RUST),
        "angular" => Ok(TEMPLATE_ANGULAR),
        _ => bail!("unknown template: {}", name),
    }
}

pub fn run_pack(base_branch: Option<&str>, output_dir: Option<&Path>, template: &str) -> Result<PathBuf> {
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

    let resolved_output = match output_dir {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => PathBuf::from(&git_root).join(path),
        None => PathBuf::from(&git_root).join(format!("review-{branch_name}")),
    };

    // Copy the selected template to AGENTS.md in output directory
    let agents_path = resolved_output.join("AGENTS.md");
    let template_content = get_template(template)?;
    fs::write(&agents_path, template_content)
        .with_context(|| format!("failed to write AGENTS.md to {}", agents_path.display()))?;

    println!("{}", resolved_output.display());
    Ok(resolved_output)
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
        Command::new("git").arg("--version").status().map(|s| s.success()).unwrap_or(false)
    }

    fn init_bare_and_clone() -> anyhow::Result<(TempDir, PathBuf)> {
        // Create a temp workspace
        let tmp = TempDir::new()?;
        let origin_dir = tmp.path().join("origin.git");
        let work_dir = tmp.path().join("work");

        // git init --bare origin.git
        Command::new("git").args(["init", "--bare", origin_dir.to_str().unwrap()]).status()?;

        // git clone origin.git work
        Command::new("git")
            .args(["clone", origin_dir.to_str().unwrap(), work_dir.to_str().unwrap()])
            .status()?;

        // Configure user
        Command::new("git").args(["-C", work_dir.to_str().unwrap(), "config", "user.email", "tester@example.com"]).status()?;
        Command::new("git").args(["-C", work_dir.to_str().unwrap(), "config", "user.name", "Test User"]).status()?;

        // Ensure master branch exists
        Command::new("git").args(["-C", work_dir.to_str().unwrap(), "checkout", "-b", "master"]).status()?;

        // Initial commit
        let readme = work_dir.join("README.md");
        fs::write(&readme, "hello\n")?;
        Command::new("git").args(["-C", work_dir.to_str().unwrap(), "add", "."]).status()?;
        Command::new("git").args(["-C", work_dir.to_str().unwrap(), "commit", "-m", "init"]).status()?;

        // Push to origin master and set upstream
        Command::new("git").args(["-C", work_dir.to_str().unwrap(), "push", "-u", "origin", "master"]).status()?;

        Ok((tmp, work_dir))
    }

    #[test]
    fn test_git_output_version() -> anyhow::Result<()> {
        if !has_git() { return Ok(()); }
        let td = TempDir::new()?;
        let out = git_output(td.path(), &["--version"])?;
        assert!(out.to_lowercase().contains("git version"));
        Ok(())
    }

    #[test]
    fn test_git_output_in_repo_rev_parse() -> anyhow::Result<()> {
        if !has_git() { return Ok(()); }
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
        if !has_git() { return Ok(()); }
        let (_tmp, work) = init_bare_and_clone()?;

        // Create feature branch and a change
        Command::new("git").args(["-C", work.to_str().unwrap(), "checkout", "-b", "feature/test"]).status()?;
        let src = work.join("src.txt");
        fs::write(&src, "line1\n")?;
        Command::new("git").args(["-C", work.to_str().unwrap(), "add", "."]).status()?;
        Command::new("git").args(["-C", work.to_str().unwrap(), "commit", "-m", "feat: add src.txt"]).status()?;

        // Run inside the repo directory
        let cwd_before = env::current_dir()?;
        env::set_current_dir(&work)?;

        // Output directory (absolute) inside temp
        let out_abs = work.join("out-review");
        if out_abs.exists() { fs::remove_dir_all(&out_abs).ok(); }
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
        if !has_git() { return Ok(()); }
        let (_tmp, work) = init_bare_and_clone()?;

        // Make a new change on a branch
        Command::new("git").args(["-C", work.to_str().unwrap(), "checkout", "-b", "chore/change"]).status()?;
        let f = work.join("data.log");
        fs::write(&f, "x\n")?;
        Command::new("git").args(["-C", work.to_str().unwrap(), "add", "."]).status()?;
        Command::new("git").args(["-C", work.to_str().unwrap(), "commit", "-m", "chore: add data.log"]).status()?;

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
}

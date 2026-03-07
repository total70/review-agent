use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const REVIEW_BRANCH_SCRIPT: &str = include_str!("review-branch.sh");

pub fn run_pack(base_branch: Option<&str>, output_dir: Option<&Path>) -> Result<PathBuf> {
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

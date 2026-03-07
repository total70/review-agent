use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "review-agent",
    version,
    about = "Review git diffs with a local Ollama model"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Pack(PackCommand),
    Run(RunCommand),
    Review(ReviewCommand),
}

#[derive(Debug, Args)]
pub struct PackCommand {
    pub base_branch: Option<String>,
    pub output_dir: Option<PathBuf>,
}

#[derive(Debug, Args, Clone)]
pub struct SharedRunArgs {
    #[arg(long, default_value = "qwen3.5")]
    pub model: String,
    #[arg(long)]
    pub no_open: bool,
    #[arg(long)]
    pub no_think: bool,
}

#[derive(Debug, Args)]
pub struct RunCommand {
    pub input: PathBuf,
    #[command(flatten)]
    pub shared: SharedRunArgs,
}

#[derive(Debug, Args)]
pub struct ReviewCommand {
    #[arg(long)]
    pub base_branch: Option<String>,
    #[command(flatten)]
    pub shared: SharedRunArgs,
}

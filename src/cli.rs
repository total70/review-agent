use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, ValueEnum)]
pub enum Provider {
    Ollama,
    Openai,
    Anthropic,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Ollama => "ollama",
            Provider::Openai => "openai",
            Provider::Anthropic => "anthropic",
        }
    }
}

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
    #[arg(long, default_value = "general")]
    pub template: String,
    /// Review working tree changes without creating a temporary branch
    #[arg(long, default_value_t = false)]
    pub uncommitted: bool,
}

#[derive(Debug, Args, Clone)]
pub struct SharedRunArgs {
    #[arg(long, value_enum, default_value_t = Provider::Ollama)]
    pub provider: Provider,

    #[arg(long, default_value = "qwen3.5")]
    pub model: String,

    /// Override the Ollama host (e.g. 192.168.1.100:11434)
    #[arg(long)]
    pub host: Option<String>,

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
    #[arg(long, default_value = "general")]
    pub template: String,
    #[command(flatten)]
    pub shared: SharedRunArgs,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // Helper to parse a Cli from provided args (with a fake binary name)
    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        let mut full = vec!["review-agent"]; // binary name placeholder
        full.extend_from_slice(args);
        Cli::try_parse_from(full)
    }

    #[test]
    fn defaults_shared_run_args_in_run() {
        let cli = parse(&["run", "input.zip"]).expect("should parse");
        match cli.command {
            Commands::Run(run) => {
                assert!(matches!(run.shared.provider, Provider::Ollama));
                assert_eq!(run.shared.model, "qwen3.5");
                assert_eq!(run.shared.host, None);
                assert_eq!(run.shared.no_open, false);
                assert_eq!(run.shared.no_think, false);
                assert_eq!(run.input, PathBuf::from("input.zip"));
            }
            _ => panic!("expected Commands::Run"),
        }
    }

    #[test]
    fn command_parsing_pack() {
        let cli = parse(&["pack"]).expect("should parse pack with no args");
        match cli.command {
            Commands::Pack(pack) => {
                assert!(pack.base_branch.is_none());
                assert!(pack.output_dir.is_none());
                assert!(!pack.uncommitted);
            }
            _ => panic!("expected Commands::Pack"),
        }
    }

    #[test]
    fn command_parsing_run_with_path() {
        let cli = parse(&["run", "./some/path.zip"]).expect("should parse run");
        match cli.command {
            Commands::Run(run) => {
                assert_eq!(run.input, PathBuf::from("./some/path.zip"));
                // defaults already asserted in other test, but double check model presence
                assert_eq!(run.shared.model, "qwen3.5");
            }
            _ => panic!("expected Commands::Run"),
        }
    }

    #[test]
    fn command_parsing_review() {
        let cli = parse(&["review"]).expect("should parse review with defaults");
        match cli.command {
            Commands::Review(review) => {
                assert!(review.base_branch.is_none());
                assert!(matches!(review.shared.provider, Provider::Ollama));
                assert_eq!(review.shared.model, "qwen3.5");
                assert!(!review.shared.no_open);
                assert!(!review.shared.no_think);
            }
            _ => panic!("expected Commands::Review"),
        }
    }

    #[test]
    fn combined_args_run_model_input() {
        let cli = parse(&["run", "--model", "llama3", "input.zip"]).expect("should parse");
        match cli.command {
            Commands::Run(run) => {
                assert_eq!(run.shared.model, "llama3");
                assert_eq!(run.shared.host, None);
                assert_eq!(run.input, PathBuf::from("input.zip"));
            }
            _ => panic!("expected Commands::Run"),
        }
    }

    #[test]
    fn combined_args_review_flags_and_base_branch() {
        let cli = parse(&["review", "--no-open", "--base-branch", "main"]).expect("should parse");
        match cli.command {
            Commands::Review(review) => {
                assert_eq!(review.base_branch.as_deref(), Some("main"));
                assert_eq!(review.shared.host, None);
                assert!(review.shared.no_open);
                assert!(!review.shared.no_think);
            }
            _ => panic!("expected Commands::Review"),
        }
    }

    #[test]
    fn combined_args_pack_with_positionals() {
        let cli = parse(&["pack", "origin/main", "output-dir"]).expect("should parse");
        match cli.command {
            Commands::Pack(pack) => {
                assert_eq!(pack.base_branch.as_deref(), Some("origin/main"));
                assert_eq!(
                    pack.output_dir.as_deref(),
                    Some(PathBuf::from("output-dir").as_path())
                );
                assert!(!pack.uncommitted);
            }
            _ => panic!("expected Commands::Pack"),
        }
    }

    #[test]
    fn combined_args_pack_with_uncommitted_flag() {
        let cli = parse(&[
            "pack",
            "origin/main",
            "--template",
            "general",
            "--uncommitted",
        ])
        .expect("should parse");
        match cli.command {
            Commands::Pack(pack) => {
                assert_eq!(pack.base_branch.as_deref(), Some("origin/main"));
                assert_eq!(pack.template, "general");
                assert!(pack.uncommitted);
            }
            _ => panic!("expected Commands::Pack"),
        }
    }

    #[test]
    fn edge_case_invalid_subcommand_fails() {
        let err = parse(&["frobnicate"]).expect_err("invalid subcommand should error");
        // clap error kind contains useful info, but just ensure it's an error
        let msg = err.to_string();
        assert!(msg.contains("error") || msg.contains("Usage"));
    }

    #[test]
    fn edge_case_missing_required_args_run_input() {
        let err = parse(&["run"]).expect_err("missing input should error");
        let msg = err.to_string();
        // Clap error for missing required arg - be flexible about message content
        assert!(!msg.is_empty(), "should have an error message");
    }

    #[test]
    fn host_flag_parses_for_run() {
        let cli =
            parse(&["run", "--host", "192.168.1.100:11434", "input.zip"]).expect("should parse");
        match cli.command {
            Commands::Run(run) => {
                assert_eq!(run.shared.host.as_deref(), Some("192.168.1.100:11434"));
                assert_eq!(run.input, PathBuf::from("input.zip"));
            }
            _ => panic!("expected Commands::Run"),
        }
    }

    #[test]
    fn host_flag_parses_for_review() {
        let cli = parse(&["review", "--host", "https://ollama.example"]).expect("should parse");
        match cli.command {
            Commands::Review(review) => {
                assert_eq!(
                    review.shared.host.as_deref(),
                    Some("https://ollama.example")
                );
            }
            _ => panic!("expected Commands::Review"),
        }
    }
}

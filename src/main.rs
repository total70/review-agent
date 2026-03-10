mod cli;
mod html;
mod pack;
mod providers;
mod review;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, Provider};
use review::RunOptions;

/// Returns the default model for a given provider.
/// - ollama: qwen3.5
/// - openai: gpt-5.4
/// - anthropic: claude-sonnet-4-6
fn get_default_model(provider: &Provider) -> &'static str {
    match provider {
        Provider::Ollama => "qwen3.5",
        Provider::Openai => "gpt-5.4",
        Provider::Anthropic => "claude-sonnet-4-6",
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Pack(command) => {
            if command.uncommitted {
                pack::run_pack_uncommitted(
                    &command
                        .base_branch
                        .clone()
                        .unwrap_or_else(|| pack::detect_default_base_branch().unwrap()),
                    command.output_dir.as_deref(),
                    &command.template,
                )?;
            } else {
                pack::run_pack(
                    command.base_branch.as_deref(),
                    command.output_dir.as_deref(),
                    &command.template,
                )?;
            }
        }
        Commands::Run(command) => {
            let model = command
                .shared
                .model
                .as_deref()
                .unwrap_or(get_default_model(&command.shared.provider));
            let options = RunOptions {
                provider: command.shared.provider.as_str(),
                model,
                host: command.shared.host.as_deref(),
                no_open: command.shared.no_open,
                no_think: command.shared.no_think,
            };
            review::run_review(&command.input, &options).await?;
        }
        Commands::Review(command) => {
            let base_branch = match command.base_branch.clone() {
                Some(b) => b,
                None => pack::detect_default_base_branch()?,
            };

            let packed = if command.uncommitted {
                pack::run_pack_uncommitted(&base_branch, None, &command.template)?
            } else {
                pack::run_pack(Some(base_branch.as_str()), None, &command.template)?
            };

            // Move packed folder to /tmp before running review
            let tmp_path = pack::move_to_tmp(&packed)?;
            println!("Moved review folder to: {}", tmp_path.display());

            let model = command
                .shared
                .model
                .as_deref()
                .unwrap_or(get_default_model(&command.shared.provider));
            let options = RunOptions {
                provider: command.shared.provider.as_str(),
                model,
                host: command.shared.host.as_deref(),
                no_open: command.shared.no_open,
                no_think: command.shared.no_think,
            };
            // Run review first (opens browser from /tmp)
            let result = review::run_review(&tmp_path, &options).await;

            // Restore from tmp if --restore flag is set (default: keep in /tmp)
            // This ensures browser has time to load the files before they're moved
            if command.restore && result.is_ok() {
                pack::restore_from_tmp(&tmp_path, &packed)?;
                println!("Restored review folder to: {}", packed.display());
            } else {
                println!("Review folder kept at: {}", tmp_path.display());
                println!(
                    "To restore manually: mv {} {}",
                    tmp_path.display(),
                    packed.display()
                );
            }

            result?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests are intentionally lightweight: main.rs only wires modules together.
    // We mainly verify that module declarations and imports link properly.

    #[test]
    fn get_default_model_returns_correct_model_for_each_provider() {
        use crate::cli::Provider;

        assert_eq!(get_default_model(&Provider::Ollama), "qwen3.5");
        assert_eq!(get_default_model(&Provider::Openai), "gpt-5.4");
        assert_eq!(get_default_model(&Provider::Anthropic), "claude-sonnet-4-6");
    }

    #[test]
    fn modules_compile_and_types_are_accessible() {
        use crate::cli::Provider;
        // Ability to name and construct key public types proves modules are wired.
        // Cli/Commands from cli, RunOptions from review.
        let _ = Cli {
            command: Commands::Review(crate::cli::ReviewCommand {
                base_branch: None,
                template: "general".into(),
                uncommitted: false,
                restore: false,
                shared: crate::cli::SharedRunArgs {
                    provider: Provider::Ollama,
                    model: Some("qwen3.5".to_string()),
                    host: None,
                    no_open: false,
                    no_think: false,
                },
            }),
        };

        // Construct each command variant to ensure visibility and correct shapes.
        let _pack = Commands::Pack(crate::cli::PackCommand {
            base_branch: None,
            output_dir: None,
            template: "general".into(),
            uncommitted: false,
        });
        let _run = Commands::Run(crate::cli::RunCommand {
            input: std::path::PathBuf::from("/tmp/dummy.zip"),
            shared: crate::cli::SharedRunArgs {
                provider: Provider::Ollama,
                model: Some("qwen3.5".to_string()),
                host: Some("127.0.0.1:11434".into()),
                no_open: true,
                no_think: false,
            },
        });
        let _review = Commands::Review(crate::cli::ReviewCommand {
            base_branch: Some("main".into()),
            template: "rust".into(),
            uncommitted: true,
            restore: false,
            shared: crate::cli::SharedRunArgs {
                provider: Provider::Ollama,
                model: Some("qwen3.5".to_string()),
                host: None,
                no_open: false,
                no_think: true,
            },
        });

        // Also test the uncommitted=false variant explicitly (for coverage)
        let _review_no_uncommitted = Commands::Review(crate::cli::ReviewCommand {
            base_branch: Some("main".into()),
            template: "rust".into(),
            uncommitted: false,
            restore: true,
            shared: crate::cli::SharedRunArgs {
                provider: Provider::Ollama,
                model: Some("qwen3.5".to_string()),
                host: None,
                no_open: false,
                no_think: true,
            },
        });

        // RunOptions lifetime/fields compile from review module
        let _opts = RunOptions {
            provider: "ollama",
            model: "model",
            host: Some("localhost:11434"),
            no_open: true,
            no_think: false,
        };

        // Touch other modules to ensure they resolve (no calls to external services here)
        let _ = &html::render_review_html as *const _;
        let _ = &pack::run_pack as *const _;
        let _ = &review::run_review as *const _;
    }
}

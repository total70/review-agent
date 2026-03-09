mod cli;
mod html;
mod pack;
mod providers;
mod review;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use review::RunOptions;

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
                        .unwrap_or(pack::detect_default_base_branch()?),
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
            let options = RunOptions {
                provider: command.shared.provider.as_str(),
                model: &command.shared.model,
                host: command.shared.host.as_deref(),
                no_open: command.shared.no_open,
                no_think: command.shared.no_think,
            };
            review::run_review(&command.input, &options).await?;
        }
        Commands::Review(command) => {
            let base_branch = command
                .base_branch
                .clone()
                .unwrap_or(pack::detect_default_base_branch()?);

            let packed = if command.uncommitted {
                pack::run_pack_uncommitted(&base_branch, None, &command.template)?
            } else {
                pack::run_pack(Some(base_branch.as_str()), None, &command.template)?
            };
            let options = RunOptions {
                provider: command.shared.provider.as_str(),
                model: &command.shared.model,
                host: command.shared.host.as_deref(),
                no_open: command.shared.no_open,
                no_think: command.shared.no_think,
            };
            review::run_review(&packed, &options).await?;
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
    fn modules_compile_and_types_are_accessible() {
        use crate::cli::Provider;
        // Ability to name and construct key public types proves modules are wired.
        // Cli/Commands from cli, RunOptions from review.
        let _ = Cli {
            command: Commands::Review(crate::cli::ReviewCommand {
                base_branch: None,
                template: "general".into(),
                uncommitted: false,
                shared: crate::cli::SharedRunArgs {
                    provider: Provider::Ollama,
                    model: "qwen3.5".into(),
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
                model: "qwen3.5".into(),
                host: Some("127.0.0.1:11434".into()),
                no_open: true,
                no_think: false,
            },
        });
        let _review = Commands::Review(crate::cli::ReviewCommand {
            base_branch: Some("main".into()),
            template: "rust".into(),
            uncommitted: true,
            shared: crate::cli::SharedRunArgs {
                provider: Provider::Ollama,
                model: "qwen3.5".into(),
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
            shared: crate::cli::SharedRunArgs {
                provider: Provider::Ollama,
                model: "qwen3.5".into(),
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

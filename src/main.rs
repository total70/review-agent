mod cli;
mod html;
mod ollama;
mod pack;
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
            pack::run_pack(
                command.base_branch.as_deref(),
                command.output_dir.as_deref(),
            )?;
        }
        Commands::Run(command) => {
            let options = RunOptions {
                model: &command.shared.model,
                no_open: command.shared.no_open,
                no_think: command.shared.no_think,
            };
            review::run_review(&command.input, &options).await?;
        }
        Commands::Review(command) => {
            let packed = pack::run_pack(command.base_branch.as_deref(), None)?;
            let options = RunOptions {
                model: &command.shared.model,
                no_open: command.shared.no_open,
                no_think: command.shared.no_think,
            };
            review::run_review(&packed, &options).await?;
        }
    }

    Ok(())
}

pub mod commands;

use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::Command;
use nekoui_agent::runtime::Agent;
use tracing::{error, warn};

fn cli() -> Command {
    Command::new("neko")
        .about("NekoUI")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(Command::new("start").about("Start NekoUI"))
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!(error = %err, "application terminated with error");
            eprintln!("Error: {:#}", err);
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<()> {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("start", sub_matches)) => {
            let start_command = commands::start::StartCommand::new(sub_matches).await?;

            let runtime = Agent::builder(start_command.config)?.build()?;
            Ok(())
        }
        _ => {
            warn!("no command specified");
            println!("Please specify a command. Use --help for more information.");
            bail!("no command specified");
        }
    }
}

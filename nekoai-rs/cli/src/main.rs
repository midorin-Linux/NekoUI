mod chat;
pub mod commands;

use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::Command;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use nekoai_agent::runtime::{AgentRuntime, RuntimeInitProgress};
use tracing::{error, info, warn};

fn cli() -> Command {
    Command::new("neko")
        .about("NekoAI")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(
            Command::new("start")
                .about("Start NekoAI")
                .arg(
                    clap::Arg::new("skip-setup")
                        .long("skip-setup")
                        .help("Skip the setup wizard even if no config file exists")
                        .action(clap::ArgAction::SetTrue),
                )
                .arg(
                    clap::Arg::new("token")
                        .long("token")
                        .help("Discord Bot Token (used with --skip-setup). ⚠️ PREFER the DISCORD_AGENT_TOKEN environment variable instead to avoid exposing secrets in process listings.")
                        .num_args(1),
                )
                .arg(
                    clap::Arg::new("api-key")
                        .long("api-key")
                        .help("API key for the AI provider (used with --skip-setup). ⚠️ PREFER the NEKOAI_API_KEY environment variable instead to avoid exposing secrets in process listings.")
                        .num_args(1),
                )
                .arg(
                    clap::Arg::new("provider")
                        .long("provider")
                        .help("AI provider (openai, anthropic, ollama, custom; used with --skip-setup)")
                        .num_args(1),
                )
                .arg(
                    clap::Arg::new("model")
                        .long("model")
                        .help("Model name (used with --skip-setup)")
                        .num_args(1),
                )
                .arg(
                    clap::Arg::new("base-url")
                        .long("base-url")
                        .help("Base URL for the AI provider API (used with --skip-setup)")
                        .num_args(1),
                )
                .arg(
                    clap::Arg::new("guild-id")
                        .long("guild-id")
                        .help("Discord Guild/Server ID (used with --skip-setup)")
                        .num_args(1),
                )
                .arg(
                    clap::Arg::new("web-search")
                        .long("web-search")
                        .help("Enable web search tool (used with --skip-setup)")
                        .action(clap::ArgAction::SetTrue),
                ),
        )
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

            info!("start command initialized");

            let agent_init_bar = ProgressBar::new(RuntimeInitProgress::TOTAL_STEPS);
            let agent_init_style = match ProgressStyle::with_template(
                "    [{bar:32.cyan/blue}] {pos:>2}/{len:2} {msg}",
            ) {
                Ok(style) => style.progress_chars("=>-"),
                Err(_) => ProgressStyle::default_bar(),
            };
            agent_init_bar.set_style(agent_init_style);
            agent_init_bar.set_message("initializing agent runtime");

            let runtime = match AgentRuntime::new_with_progress(
                start_command.config.clone(),
                start_command.memory_store,
                |progress| {
                    agent_init_bar.set_position(progress.completed_steps);
                    agent_init_bar.set_length(progress.total_steps);
                    agent_init_bar.set_message(progress.message);
                },
            )
            .await
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    agent_init_bar.finish_and_clear();
                    println!(
                        "    {} Failed to initialize agent runtime: {}",
                        "✗".red(),
                        err
                    );
                    return Err(err);
                }
            };

            agent_init_bar.finish_and_clear();
            println!("    {} Agent runtime initialized", "✓".green());

            info!("agent runtime initialized");

            let chat_client = chat::ChatClient::initialize(&start_command.config, runtime).await?;

            info!(
                platform = chat_client.platform_name(),
                "chat client initialized"
            );

            chat_client.run().await?;

            info!("application exited successfully");
            Ok(())
        }
        _ => {
            warn!("no command specified");
            println!("Please specify a command. Use --help for more information.");
            bail!("no command specified");
        }
    }
}

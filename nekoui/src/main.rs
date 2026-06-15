use std::{process::ExitCode, sync::Arc};

use anyhow::{Result, bail};
use clap::{ArgMatches, Command};
use dialoguer::{Input, theme::SimpleTheme};
use nekoui::{
    config::Config,
    repositories::sqlite::SqliteRepository,
    services::Services,
    state::ServerState,
    utils::logging::{State, WorkerGuard, init_tracing, print_log},
    wizard::{config_exists, run_setup_wizard},
};
use tracing::{error, info, warn};

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
    let guard = init_tracing()?;
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("start", sub_matches)) => {
            let _start_command = StartCommand::new(sub_matches, guard).await?;

            Ok(())
        }
        _ => {
            warn!("no command specified");
            println!("Please specify a command. Use --help for more information.");
            bail!("no command specified");
        }
    }
}

pub struct StartCommand {}

impl StartCommand {
    pub async fn new(sub_matches: &ArgMatches, guard: WorkerGuard) -> Result<Self> {
        Self::start(sub_matches, guard).await?;

        Ok(Self {})
    }

    pub async fn start(_sub_matches: &ArgMatches, _guard: WorkerGuard) -> Result<()> {
        println!();
        println!("NekoUI");
        println!("----------------");
        println!();
        println!("Welcome to NekoUI!\n");

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let config = if config_exists() {
            match Config::load() {
                Ok(config) => config,
                Err(e) => {
                    print_log(State::Failed, "Failed to load configuration");
                    bail!("failed to load configuration: {e}");
                }
            }
        } else {
            print_log(State::Warn, "Configuration file not found");
            println!();
            println!("--------------- Setup wizard ----------------");
            println!();
            println!(
                "It's likely that this is the first time the program is running, or the configuration file has been deleted."
            );

            loop {
                let response: String = Input::with_theme(&SimpleTheme)
                    .with_prompt(
                        "Do you want to run the setup wizard to create a new configuration? [y/n]",
                    )
                    .interact_text()?;

                let response = response.trim().to_ascii_lowercase();

                if response == "y" {
                    info!("User chose to run the setup wizard");
                    break;
                } else if response == "n" {
                    println!();
                    print_log(State::Info, "Setup wizard cancelled. Shutting down...");
                    std::process::exit(0);
                } else {
                    println!("\nInvalid input. Please enter 'y' or 'n'.");
                }
            }

            info!("Running setup wizard to create new configuration");
            let config = run_setup_wizard().await?;
            info!("setup wizard completed, configuration saved");
            config
        };
        print_log(State::Ok, "Loaded configuration");

        info!("connecting to database");
        let sqlite_repo = match SqliteRepository::new(&config.server.database_url).await {
            Ok(sqlite) => sqlite,
            Err(e) => {
                print_log(State::Failed, "Failed to connect to database");
                bail!("failed to connect to database: {e}");
            }
        };
        print_log(State::Ok, "Connected to database");

        info!("initializing services");
        let jwt_secret = Arc::new(config.server.jwt_secret.as_ref().to_string().clone());
        let services = Arc::new(Services::new(&sqlite_repo, jwt_secret.clone()).await);
        print_log(State::Ok, "Services initialized");

        let server_state = ServerState {
            services,
            jwt_secret,
        };

        info!("initializing HTTP/WebSocket API server");
        let server = nekoui::api::HttpServer::new(config.server, server_state);
        print_log(State::Ok, "server initialized");

        server.serve().await?;

        Ok(())
    }
}

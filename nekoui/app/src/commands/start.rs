use anyhow::{Result, bail};
use clap::ArgMatches;
use dialoguer::{Input, theme::SimpleTheme};
use nekoui_config::Config;
use nekoui_telemetry::{State, WorkerGuard, init_tracing, print_log};
use nekoui_wizard::{config_exists, run_setup_wizard};
use tracing::info;

pub struct StartCommand {
    pub config: Config,
    _guard: WorkerGuard,
}

impl StartCommand {
    pub async fn new(sub_matches: &ArgMatches) -> Result<Self> {
        let (config, guard) = Self::start(sub_matches).await?;

        Ok(Self {
            config,
            _guard: guard,
        })
    }

    pub async fn start(_sub_matches: &ArgMatches) -> Result<(Config, WorkerGuard)> {
        println!();
        println!("NekoUI");
        println!("----------------");
        println!();
        println!("Welcome to NekoUI!\n");

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let guard = match init_tracing() {
            Ok(guard) => guard,
            Err(e) => {
                print_log(State::Failed, "Failed to initialize tracing");
                bail!("failed to initialize tracing: {e}");
            }
        };
        print_log(State::Ok, "Initialized tracing");

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

        Ok((config, guard))
    }
}

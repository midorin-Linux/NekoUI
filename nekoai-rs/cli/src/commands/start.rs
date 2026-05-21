use anyhow::{Result, bail};
use clap::ArgMatches;
use colored::Colorize;
use dialoguer::{Input, theme::SimpleTheme};
use indicatif::{ProgressBar, ProgressStyle};
use nekoai_config::loader::Config;
use nekoai_infra::logging::{WorkerGuard, init_tracing};
use nekoai_memory::store::MemoryStore;
use nekoai_setup::{
    config_exists, config_from_env, has_env_token, migrate_json_to_toml, run_setup_wizard,
};
use tokio::time::sleep;
use tracing::{error, info, warn};

pub struct StartCommand {
    pub config: Config,
    _guard: WorkerGuard,
    pub memory_store: MemoryStore,
}

impl StartCommand {
    pub async fn new(sub_matches: &ArgMatches) -> Result<Self> {
        let (config, guard, memory_store) = Self::start(sub_matches).await?;

        Ok(Self {
            config,
            _guard: guard,
            memory_store,
        })
    }

    pub async fn start(sub_matches: &ArgMatches) -> Result<(Config, WorkerGuard, MemoryStore)> {
        println!();
        println!("  ███╗   ██╗ ███████╗ ██╗  ██╗  ██████╗       █████╗  ██╗");
        println!("  ████╗  ██║ ██╔════╝ ██║ ██╔╝ ██╔═══██╗     ██╔══██╗ ██║");
        println!("  ██╔██╗ ██║ █████╗   █████╔╝  ██║   ██║     ███████║ ██║");
        println!("  ██║╚██╗██║ ██╔══╝   ██╔═██╗  ██║   ██║     ██╔══██║ ██║");
        println!("  ██║ ╚████║ ███████╗ ██║  ██╗ ╚██████╔╝     ██║  ██║ ██║");
        println!("  ╚═╝  ╚═══╝ ╚══════╝ ╚═╝  ╚═╝  ╚═════╝      ╚═╝  ╚═╝ ╚═╝");
        println!();
        println!("  Welcome to Neko AI!\n");

        sleep(std::time::Duration::from_secs(1)).await;

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("    {spinner} Initializing tracing...")?,
        );
        spinner.enable_steady_tick(std::time::Duration::from_millis(120));

        let guard = match init_tracing() {
            Ok(guard) => guard,
            Err(e) => {
                spinner.finish_and_clear();

                println!("    {} Failed to initialize tracing: {}", "✗".red(), e);

                bail!("failed to initialize tracing: {e}");
            }
        };

        spinner.finish_and_clear();

        info!("Tracing initialized successfully");
        println!("    {} Tracing initialized", "✓".green());

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("    {spinner} Loading configuration...")?,
        );
        spinner.enable_steady_tick(std::time::Duration::from_millis(120));

        info!("Checking for configuration file");

        // Auto-migrate legacy JSON config to TOML if needed
        if let Err(e) = migrate_json_to_toml() {
            warn!(error = %e, "failed to migrate legacy config.json (may not exist)");
        }

        let config = if config_exists() {
            let config_path = if std::path::Path::new(".config/config.toml").exists() {
                ".config/config.toml"
            } else {
                ".config/config.json"
            };

            info!("Configuration file found at {config_path}");
            spinner.finish_and_clear();
            println!(
                "    {} Configuration file found ({config_path})",
                "✓".green()
            );

            // Load existing config
            Config::load()
                .inspect_err(|_e| {
                    spinner.finish_and_clear();
                })
                .inspect(|_| spinner.finish_and_clear())
                .map_err(|e| {
                    error!("Failed to load configuration: {}", e);
                    println!("    {} Failed to load configuration: {}", "✗".red(), e);
                    e
                })
                .inspect(|_| {
                    info!("Configuration loaded successfully");
                    println!("    {} Configuration loaded", "✓".green());
                })?
        } else {
            spinner.finish_and_clear();

            warn!("No configuration file found at .config/config.toml");
            println!("    {} No configuration found\n", "✗".red());
            println!(
                "    It's likely that this is the first time the program is running, or the configuration file has been deleted."
            );

            // Determine setup path
            let skip_setup = sub_matches.get_flag("skip-setup");
            if skip_setup || has_env_token() {
                // CLI fallback mode: skip interactive wizard
                if has_env_token() {
                    info!("DISCORD_AGENT_TOKEN environment variable found, skipping setup wizard");
                    println!(
                        "    {} DISCORD_AGENT_TOKEN detected, using environment",
                        "i".cyan()
                    );

                    if let Some(cfg) = config_from_env() {
                        info!("configuration built from environment variables");
                        cfg
                    } else {
                        bail!("failed to build config from DISCORD_AGENT_TOKEN");
                    }
                } else {
                    info!("--skip-setup flag provided, using CLI arguments");

                    let token = sub_matches
                        .get_one::<String>("token")
                        .cloned()
                        .unwrap_or_else(|| {
                            warn!("no --token provided with --skip-setup");
                            String::new()
                        });

                    if token.is_empty() {
                        bail!("--token is required when using --skip-setup");
                    }

                    // Prefer env vars for optional values to reduce CLI secret exposure
                    let api_key = sub_matches
                        .get_one::<String>("api-key")
                        .cloned()
                        .or_else(|| std::env::var("NEKOAI_API_KEY").ok())
                        .unwrap_or_default();

                    let provider = sub_matches
                        .get_one::<String>("provider")
                        .cloned()
                        .or_else(|| std::env::var("NEKOAI_PROVIDER").ok())
                        .unwrap_or_default();

                    let model = sub_matches
                        .get_one::<String>("model")
                        .cloned()
                        .or_else(|| std::env::var("NEKOAI_MODEL").ok())
                        .unwrap_or_default();

                    let base_url = sub_matches
                        .get_one::<String>("base-url")
                        .cloned()
                        .or_else(|| std::env::var("NEKOAI_BASE_URL").ok())
                        .unwrap_or_default();

                    let guild_id = sub_matches
                        .get_one::<String>("guild-id")
                        .cloned()
                        .or_else(|| std::env::var("NEKOAI_GUILD_ID").ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);

                    let web_search = sub_matches.get_flag("web-search");

                    let cfg = nekoai_setup::cli_fallback::make_config(
                        &token, &api_key, &provider, &model, &base_url, guild_id, web_search,
                    );
                    info!(
                        "configuration built from CLI arguments (provider: {}, model: {})",
                        if provider.is_empty() {
                            "default"
                        } else {
                            &provider
                        },
                        if model.is_empty() { "default" } else { &model }
                    );
                    cfg
                }
            } else {
                // Interactive setup wizard
                loop {
                    let response: String = Input::with_theme(&SimpleTheme)
                        .with_prompt("    Do you want to run the setup wizard to create a new configuration? [y/n]")
                        .interact_text()?;

                    let response = response.trim().to_ascii_lowercase();

                    if response == "y" {
                        info!("User chose to run the setup wizard");
                        println!("\n    Starting setup wizard...");
                        break;
                    } else if response == "n" {
                        info!("User chose to cancel the setup wizard");
                        println!("\n    Setup wizard cancelled. Shutting down...");
                        bail!("setup wizard cancelled by user");
                    } else {
                        println!("\n    Invalid input. Please enter 'y' or 'n'.");
                    }
                }

                info!("Running setup wizard to create new configuration");
                let cfg = run_setup_wizard().await?;
                info!("setup wizard completed, configuration saved");
                cfg
            }
        };

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("    {spinner} Initializing context memory...")?,
        );
        spinner.enable_steady_tick(std::time::Duration::from_millis(120));

        info!("Initializing context memory");

        let memory_store = MemoryStore::new(&config)?;

        memory_store.initialize().await.inspect_err(|e| {
            error!(error = %e, "failed to initialize vector memory collections");
        })?;

        // Start background cleanup job for midterm memory retention
        memory_store.start_cleanup_job();

        spinner.finish_and_clear();

        info!("context memory initialized successfully");
        println!("    {} context memory initialized", "✓".green());

        Ok((config, guard, memory_store))
    }
}

pub mod cli_fallback;
pub mod config_writer;
pub mod wizard;

use std::path::Path;

use anyhow::{Context, Result};
use nekoui_config::{loader::Config, mcp_config};
use tracing::{info, warn};

/// Environment variable names for configuration (preferred over CLI flags).
pub const ENV_API_KEY: &str = "nekoui_API_KEY";
pub const ENV_PROVIDER: &str = "nekoui_PROVIDER";
pub const ENV_MODEL: &str = "nekoui_MODEL";
pub const ENV_BASE_URL: &str = "nekoui_BASE_URL";
pub const ENV_BIND_ADDRESS: &str = "nekoui_BIND_ADDRESS";
pub const ENV_AUTH_TOKEN: &str = "nekoui_AUTH_TOKEN";

/// Run the interactive setup wizard (dialoguer-based).
/// This collects all necessary configuration from the user step by step,
/// saves the config file, and returns the generated Config.
pub async fn run_setup_wizard() -> Result<Config> {
    info!("starting interactive setup wizard");
    let config = wizard::run_wizard()?;
    config_writer::save_config(&config)?;
    mcp_config::save_mcp_servers(&[])?;
    info!("setup wizard completed successfully");
    Ok(config)
}

/// Build a Config from environment variables.
/// Supports: nekoui_API_KEY, nekoui_PROVIDER,
/// nekoui_MODEL, nekoui_BASE_URL, nekoui_BIND_ADDRESS, nekoui_AUTH_TOKEN.
/// Falls back to sensible defaults for missing values.
pub fn config_from_env() -> Option<Config> {
    let api_key = std::env::var(ENV_API_KEY).ok()?;
    let provider = std::env::var(ENV_PROVIDER).unwrap_or_default();
    let model = std::env::var(ENV_MODEL).unwrap_or_default();
    let base_url = std::env::var(ENV_BASE_URL).unwrap_or_default();

    Some(cli_fallback::make_config(
        &api_key, &provider, &model, &base_url, false,
    ))
}

/// Check if the nekoui_API_KEY environment variable is set.
pub fn has_env_token() -> bool {
    std::env::var(ENV_API_KEY).is_ok()
}

/// Check if a configuration file already exists at the expected path.
pub fn config_exists() -> bool {
    let toml_path = Path::new(".config/config.toml");
    let json_path = Path::new(".config/config.json");
    toml_path.exists() || json_path.exists()
}

/// Migrate a legacy JSON config file to TOML format.
/// Reads `.config/config.json`, writes `.config/config.toml`,
/// and renames the JSON file to `.config/config.json.bak`.
pub fn migrate_json_to_toml() -> Result<bool> {
    let json_path = Path::new(".config/config.json");
    let toml_path = Path::new(".config/config.toml");

    if !json_path.exists() || toml_path.exists() {
        return Ok(false);
    }

    info!("migrating legacy config.json to config.toml");

    let json_content =
        std::fs::read_to_string(json_path).context("failed to read legacy config.json")?;
    let config: Config =
        serde_json::from_str(&json_content).context("failed to parse legacy config.json")?;

    let toml_output =
        toml::to_string_pretty(&config).context("failed to serialize config to TOML")?;
    std::fs::write(toml_path, &toml_output).context("failed to write config.toml")?;

    let bak_path = json_path.with_extension("json.bak");
    std::fs::rename(json_path, &bak_path)
        .context("failed to rename config.json to config.json.bak")?;

    warn!(
        "legacy config.json migrated to config.toml; a backup was saved at {}",
        bak_path.display()
    );

    Ok(true)
}

pub mod wizard;
pub mod writer;

use std::path::PathBuf;

use anyhow::Result;
use tracing::info;

pub use super::config::{CONFIG_PATH, Config, ServerConfig};

pub async fn run_setup_wizard() -> Result<Config> {
    info!("starting interactive setup wizard");

    let config = wizard::run_wizard()?;
    writer::save_config(&config)?;

    info!("setup wizard completed successfully");

    Ok(config)
}

pub fn config_exists() -> bool {
    let file_path = PathBuf::from(CONFIG_PATH).join("config.yaml");

    file_path.exists()
}

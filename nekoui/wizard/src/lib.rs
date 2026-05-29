pub mod wizard;
pub mod writer;

use anyhow::Result;
use nekoui_config::Config;
use tracing::info;

pub async fn run_setup_wizard() -> Result<Config> {
    info!("starting interactive setup wizard");

    let config = wizard::run_wizard()?;
    writer::save_config(&config)?;

    info!("setup wizard completed successfully");

    Ok(config)
}

pub fn config_exists() -> bool {
    #[cfg(debug_assertions)]
    let file_path = std::path::Path::new("../config/config.yaml");

    #[cfg(not(debug_assertions))]
    let file_path = std::path::Path::new("config/config.yaml");

    file_path.exists()
}

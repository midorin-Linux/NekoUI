use std::path::Path;

use anyhow::{Context, Result};
use nekoui_config::Config;
use tracing::info;

#[cfg(debug_assertions)]
const CONFIG_PATH: &str = "../config/config.yaml";

#[cfg(not(debug_assertions))]
const CONFIG_PATH: &str = "config/config.yaml";

pub fn save_config(config: &Config) -> Result<()> {
    let config_dir = Path::new(CONFIG_PATH).parent().unwrap();
    if !config_dir.exists() {
        std::fs::create_dir_all(config_dir).with_context(|| {
            format!(
                "failed to create config directory: {}",
                config_dir.display()
            )
        })?;
        info!(path = %config_dir.display(), "created config directory");
    }

    let yaml_output =
        serde_yaml::to_string(config).context("failed to serialize config to YAML")?;

    std::fs::write(CONFIG_PATH, &yaml_output)
        .with_context(|| format!("failed to write config file: {CONFIG_PATH}"))?;

    info!(path = CONFIG_PATH, "configuration saved successfully");
    Ok(())
}

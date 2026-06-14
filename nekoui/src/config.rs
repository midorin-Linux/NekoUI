use std::{fmt, path::PathBuf};

use anyhow::{Context, Result};
use config::{Config as ConfigBuilder, File};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use zeroize::Zeroizing;

#[cfg(debug_assertions)]
pub const CONFIG_PATH: &str = "../config";

#[cfg(not(debug_assertions))]
pub const CONFIG_PATH: &str = "config";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
}

impl Config {
    pub fn load() -> Result<Self> {
        info!("loading configuration file");

        let file_path = PathBuf::from(CONFIG_PATH).join("config.yaml");

        let config = ConfigBuilder::builder()
            .add_source(
                File::from(file_path)
                    .format(config::FileFormat::Yaml)
                    .required(true),
            )
            .build()
            .context("failed to build config")?;

        debug!("configuration source parsed");

        let parsed: Self = config.try_deserialize()?;

        info!("configuration deserialized successfully");

        Ok(parsed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: u16,

    #[serde(default = "default_database_url")]
    pub database_url: String,

    pub allowed_origins: Vec<String>,
}

pub fn default_bind_address() -> u16 {
    3000
}

pub fn default_database_url() -> String {
    "sqlite:nekoui.db".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            database_url: default_database_url(),
            allowed_origins: vec![],
        }
    }
}

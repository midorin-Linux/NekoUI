use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use config::{Config as ConfigBuilder, File};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use crate::utils::secret_key::SecretKey;

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

        parsed.server.validate()?;

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

    pub jwt_secret: SecretKey,

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
            jwt_secret: SecretKey::new("".to_string()),
            allowed_origins: vec![],
        }
    }
}

impl ServerConfig {
    pub fn validate(&self) -> Result<()> {
        if self.jwt_secret.as_ref().trim().is_empty() {
            bail!("server.jwt_secret must not be empty");
        }

        if self.jwt_secret.as_ref().as_bytes().len() < 32 {
            bail!("server.jwt_secret must be at least 32 bytes");
        }

        Ok(())
    }
}

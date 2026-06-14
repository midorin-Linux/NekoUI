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

#[derive(Clone)]
pub struct SecretKey(Zeroizing<SecretString>);

impl SecretKey {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(SecretString::new(value.into())))
    }

    pub fn expose(&self) -> &str {
        (*self.0).expose_secret()
    }
}

impl AsRef<str> for SecretKey {
    fn as_ref(&self) -> &str {
        self.expose()
    }
}

impl Serialize for SecretKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for SecretKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::new(s))
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let visible_length = 4;
        let masked = {
            let inner = (*self.0).expose_secret();
            let length = inner.chars().count();
            let start = length.saturating_sub(visible_length);
            let extracted: String = inner.chars().skip(start).collect();
            format!("{:*>20}", &extracted)
        };
        f.debug_tuple("SecretKey").field(&masked).finish()
    }
}

impl Drop for SecretKey {
    fn drop(&mut self) {}
}

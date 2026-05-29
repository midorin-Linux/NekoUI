pub mod memory;
pub mod provider;
pub mod server;
pub mod tools;

use std::fmt;

use anyhow::{Context, Result};
use config::{Config as ConfigBuilder, File};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use server::ServerConfig;
use tracing::{debug, info};
use zeroize::Zeroizing;

use crate::{memory::MemoryConfig, provider::ProviderConfig, tools::ToolsConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub provider: ProviderConfig,
    pub memory: MemoryConfig,
    pub tools: ToolsConfig,
}

impl Config {
    pub fn load() -> Result<Self> {
        info!("loading configuration file");

        #[cfg(debug_assertions)]
        let file_path = std::path::Path::new("../config/config.yaml");

        #[cfg(not(debug_assertions))]
        let file_path = std::path::Path::new("config/config.yaml");

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

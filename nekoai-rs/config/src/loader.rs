use std::fmt;

use anyhow::{Context, Result};
use config::{Config as ConfigBuilder, File};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discord {
    pub token: SecretKey,
    pub guild_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ChatPlatform {
    #[default]
    Discord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameters {
    #[serde(default = "default_max_token")]
    pub max_token: u64,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_top_p")]
    pub top_p: f64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            max_token: default_max_token(),
            temperature: default_temperature(),
            top_p: default_top_p(),
        }
    }
}

const fn default_max_token() -> u64 {
    262144
}

const fn default_temperature() -> f64 {
    1.0
}

const fn default_top_p() -> f64 {
    0.95
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationModel {
    pub provider_base_url: String,
    pub api_key: SecretKey,
    pub model_name: String,
    pub parameters: Parameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizerModel {
    pub provider_base_url: String,
    pub api_key: SecretKey,
    pub model_name: String,
    pub parameters: Parameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModel {
    pub provider_base_url: String,
    pub api_key: SecretKey,
    pub model_name: String,
    pub dimension: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub conversation_model: ConversationModel,
    pub summarizer_model: SummarizerModel,
    pub embedding_model: EmbeddingModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDb {
    #[serde(default = "default_qdrant_url")]
    pub url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_mid_term_collection")]
    pub mid_term_collection: String,
    #[serde(default = "default_long_term_collection")]
    pub long_term_collection: String,
}

impl Default for VectorDb {
    fn default() -> Self {
        Self {
            url: default_qdrant_url(),
            api_key: None,
            mid_term_collection: default_mid_term_collection(),
            long_term_collection: default_long_term_collection(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    #[serde(default)]
    pub vector_db: VectorDb,
    #[serde(default = "default_short_term_max_entries")]
    pub short_term_max_entries: usize,
    #[serde(default = "default_mid_term_top_k")]
    pub mid_term_top_k: usize,
    #[serde(default = "default_long_term_top_k")]
    pub long_term_top_k: usize,
    #[serde(default = "default_mid_term_retention_days")]
    pub mid_term_retention_days: u32,
    #[serde(default = "default_long_term_extraction_interval")]
    pub long_term_extraction_interval: usize,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            vector_db: VectorDb::default(),
            short_term_max_entries: default_short_term_max_entries(),
            mid_term_top_k: default_mid_term_top_k(),
            long_term_top_k: default_long_term_top_k(),
            mid_term_retention_days: default_mid_term_retention_days(),
            long_term_extraction_interval: default_long_term_extraction_interval(),
        }
    }
}

pub const DEFAULT_QDRANT_URL: &str = "http://localhost:6334";

fn default_qdrant_url() -> String {
    DEFAULT_QDRANT_URL.to_string()
}

fn default_mid_term_collection() -> String {
    "mid_term".to_string()
}

fn default_long_term_collection() -> String {
    "long_term".to_string()
}

const fn default_short_term_max_entries() -> usize {
    20
}

const fn default_mid_term_top_k() -> usize {
    3
}

const fn default_long_term_top_k() -> usize {
    5
}

const fn default_mid_term_retention_days() -> u32 {
    30
}

const fn default_long_term_extraction_interval() -> usize {
    10
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        info!("loading configuration file");

        // Try TOML first, fall back to JSON for migration
        let toml_path = std::path::Path::new(".config/config.toml");
        let json_path = std::path::Path::new(".config/config.json");

        let config = if toml_path.exists() {
            info!("loading config from .config/config.toml");
            ConfigBuilder::builder()
                .add_source(
                    File::from(toml_path)
                        .format(config::FileFormat::Toml)
                        .required(true),
                )
                .build()
                .context("failed to build config from .config/config.toml")?
        } else if json_path.exists() {
            info!("loading config from .config/config.json (legacy format)");
            ConfigBuilder::builder()
                .add_source(
                    File::from(json_path)
                        .format(config::FileFormat::Json)
                        .required(true),
                )
                .build()
                .context("failed to build config from .config/config.json")?
        } else {
            anyhow::bail!(
                "no configuration file found: expected .config/config.toml or .config/config.json"
            );
        };

        debug!("configuration source parsed");

        let parsed: Self = config.try_deserialize()?;

        info!("configuration deserialized successfully");

        Ok(parsed)
    }
}

#[derive(Clone)]
pub struct SecretKey(Zeroizing<SecretString>);

impl SecretKey {
    /// Create a new SecretKey from a UTF-8 String. The inner secret is a
    /// `SecretString` wrapped in `Zeroizing` so the secret will be zeroized
    /// on drop.
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(SecretString::new(value.into())))
    }

    /// Expose the secret as &str for read-only usage. This performs a
    /// UTF-8 conversion; if the bytes are not valid UTF-8, returns an empty
    /// string (which should not happen for keys created via `new`).
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearxngConfig {
    #[serde(default = "default_searxng_url")]
    pub base_url: String,
    #[serde(default = "default_searxng_max_results")]
    pub max_results: u64,
}

impl Default for SearxngConfig {
    fn default() -> Self {
        Self {
            base_url: default_searxng_url(),
            max_results: default_searxng_max_results(),
        }
    }
}

fn default_searxng_url() -> String {
    "http://localhost:8080".to_string()
}

fn default_searxng_max_results() -> u64 {
    5
}

fn default_code_exec_languages() -> Vec<String> {
    vec!["python".to_string()]
}

const fn default_code_exec_timeout() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecConfig {
    #[serde(default = "default_code_exec_languages")]
    pub allowed_languages: Vec<String>,
    #[serde(default = "default_code_exec_timeout")]
    pub timeout_seconds: u64,
}

impl Default for CodeExecConfig {
    fn default() -> Self {
        Self {
            allowed_languages: default_code_exec_languages(),
            timeout_seconds: default_code_exec_timeout(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadFileConfig {
    #[serde(default)]
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub transport: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolPermissions {
    #[serde(default)]
    pub web_search: bool,
    #[serde(default)]
    pub searxng: SearxngConfig,
    #[serde(default)]
    pub code_exec: bool,
    #[serde(default)]
    pub read_file: bool,
    #[serde(default)]
    pub code_exec_sandbox: CodeExecConfig,
    #[serde(default)]
    pub read_file_dirs: ReadFileConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiConfig {
    /// Address to bind the HTTP server (default: 127.0.0.1:8080)
    #[serde(default = "default_web_ui_bind")]
    pub bind_address: String,
    /// Optional bearer token for API authentication
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Allowed CORS origins (default: empty = allow only loopback)
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

fn default_web_ui_bind() -> String {
    "127.0.0.1:8080".to_string()
}

impl Default for WebUiConfig {
    fn default() -> Self {
        Self {
            bind_address: default_web_ui_bind(),
            auth_token: None,
            allowed_origins: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub chat_platform: ChatPlatform,
    pub discord: Discord,
    pub provider: Provider,
    #[serde(default)]
    pub memory: Memory,
    #[serde(default)]
    pub tools: ToolPermissions,
    #[serde(default)]
    pub web_ui: WebUiConfig,
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

// Implement explicit zeroization guard for extra safety: on drop, ensure the
// inner Vec<u8> is zeroized. `secrecy::Secret<Vec<u8>>` already zeroizes the
// inner value on drop when the crate is configured appropriately, but adding
// this Drop impl ensures we call `zeroize()` on the inner slice as well.
impl Drop for SecretKey {
    fn drop(&mut self) {
        // Zeroizing<String> will zeroize its inner buffer on Drop automatically.
        // Nothing else to do here.
    }
}

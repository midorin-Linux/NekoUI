use serde::{Deserialize, Serialize};

use crate::SecretKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qdrant {
    #[serde(default = "default_base_url")]
    pub base_url: String,

    #[serde(default = "default_api_key")]
    pub api_key: SecretKey,

    #[serde(default = "default_mid_term_collection")]
    pub mid_term_collection: String,

    #[serde(default = "default_long_term_collection")]
    pub long_term_collection: String,
}

pub fn default_base_url() -> String {
    "http://localhost:6333".to_string()
}

pub fn default_api_key() -> SecretKey {
    SecretKey::new("".to_string())
}

pub fn default_mid_term_collection() -> String {
    "mid_term".to_string()
}

pub fn default_long_term_collection() -> String {
    "long_term".to_string()
}

impl Default for Qdrant {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            api_key: default_api_key(),
            mid_term_collection: default_mid_term_collection(),
            long_term_collection: default_long_term_collection(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_short_term_max_entries")]
    pub short_term_max_entries: usize,

    #[serde(default = "default_mid_term_top_k")]
    pub mid_term_top_k: usize,

    #[serde(default = "default_long_term_top_k")]
    pub long_term_top_k: usize,

    #[serde(default = "default_mid_term_retention_days")]
    pub mid_term_retention_days: u32,

    #[serde(default = "default_long_term_extraction_interval")]
    pub long_term_extraction_interval: u32,

    #[serde(default)]
    pub qdrant: Qdrant,
}

pub fn default_short_term_max_entries() -> usize {
    20
}

pub fn default_mid_term_top_k() -> usize {
    3
}

pub fn default_long_term_top_k() -> usize {
    5
}

pub fn default_mid_term_retention_days() -> u32 {
    30
}

pub fn default_long_term_extraction_interval() -> u32 {
    10
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            short_term_max_entries: default_short_term_max_entries(),
            mid_term_top_k: default_mid_term_top_k(),
            long_term_top_k: default_long_term_top_k(),
            mid_term_retention_days: default_mid_term_retention_days(),
            long_term_extraction_interval: default_long_term_extraction_interval(),
            qdrant: Qdrant::default(),
        }
    }
}

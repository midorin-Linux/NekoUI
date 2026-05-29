use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::SecretKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextModelProvider {
    pub api_key: SecretKey,
    pub base_url: String,
    pub model_id: String,
    pub parameters: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedModelProvider {
    pub api_key: SecretKey,
    pub base_url: String,
    pub model_id: String,
    pub dimension: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub agent: TextModelProvider,
    pub summarizer: TextModelProvider,
    pub embedder: EmbedModelProvider,
}

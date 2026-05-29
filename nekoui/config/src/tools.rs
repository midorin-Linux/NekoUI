use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Searxng {
    #[serde(default = "default_base_url")]
    pub base_url: String,

    #[serde(default = "default_max_result")]
    pub max_result: usize,
}

pub fn default_base_url() -> String {
    "http://localhost:8080".to_string()
}

pub fn default_max_result() -> usize {
    5
}

impl Default for Searxng {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            max_result: default_max_result(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_web_search")]
    pub web_search: bool,

    #[serde(default)]
    pub searxng: Searxng
}

pub fn default_web_search() -> bool {
    false
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            web_search: default_web_search(),
            searxng: Searxng::default()
        }
    }
}
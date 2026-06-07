use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: u16,

    pub allowed_origins: Vec<String>,
}

pub fn default_bind_address() -> u16 {
    3000
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            allowed_origins: vec![],
        }
    }
}

use anyhow::Result;
use nekoui_config::{
    Config, SecretKey,
    memory::MemoryConfig,
    provider::{EmbedModelProvider, ProviderConfig, TextModelProvider},
    server::ServerConfig,
    tools::{Searxng, ToolsConfig},
};

pub fn run_wizard() -> Result<Config> {
    println!("The wizard is currently being prepared. Default values will be saved.");

    let config = Config {
        server: ServerConfig { bind_address: 3000 },
        provider: ProviderConfig {
            agent: TextModelProvider {
                api_key: SecretKey::new("".to_string()),
                base_url: "".to_string(),
                model_id: "".to_string(),
                parameters: vec![
                    serde_json::json!({"max_token": 262144}),
                    serde_json::json!({"temperature": 1.0}),
                    serde_json::json!({"top_p": 0.95}),
                ],
            },
            summarizer: TextModelProvider {
                api_key: SecretKey::new("".to_string()),
                base_url: "".to_string(),
                model_id: "".to_string(),
                parameters: vec![
                    serde_json::json!({"max_token": 8192}),
                    serde_json::json!({"temperature": 0.2}),
                    serde_json::json!({"top_p": 0.95}),
                ],
            },
            embedder: EmbedModelProvider {
                api_key: SecretKey::new("".to_string()),
                base_url: "".to_string(),
                model_id: "".to_string(),
                dimension: 1536,
            },
        },
        memory: MemoryConfig {
            short_term_max_entries: 20,
            mid_term_top_k: 3,
            long_term_top_k: 5,
            mid_term_retention_days: 30,
            long_term_extraction_interval: 10,
            qdrant: Default::default(),
        },
        tools: ToolsConfig {
            web_search: true,
            searxng: Searxng {
                base_url: "https://localhost:8080".to_string(),
                max_result: 5,
            },
        },
    };

    Ok(config)
}

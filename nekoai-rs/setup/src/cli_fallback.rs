use nekoai_config::loader::{
    ChatPlatform, Config, ConversationModel, Discord, EmbeddingModel, Memory, Parameters, Provider,
    SecretKey, SummarizerModel, ToolPermissions, VectorDb, WebUiConfig,
};
use tracing::warn;

/// Build a Config from explicitly provided CLI arguments.
/// Missing optional fields use sensible defaults.
#[allow(clippy::too_many_arguments)]
pub fn make_config(
    token: &str,
    api_key: &str,
    provider: &str,
    model: &str,
    base_url: &str,
    guild_id: u64,
    web_search: bool,
) -> Config {
    let (provider_base_url, model_name) = resolve_provider(provider, model, base_url);
    let summarizer_model_name = resolve_summarizer(provider, &model_name);

    Config {
        chat_platform: ChatPlatform::Discord,
        discord: Discord {
            token: SecretKey::new(token.to_owned()),
            guild_id,
        },
        provider: Provider {
            conversation_model: ConversationModel {
                provider_base_url: provider_base_url.clone(),
                api_key: SecretKey::new(api_key.to_owned()),
                model_name,
                parameters: Parameters {
                    max_token: 262144,
                    temperature: 1.0,
                    top_p: 0.95,
                },
            },
            summarizer_model: SummarizerModel {
                provider_base_url: provider_base_url.clone(),
                api_key: SecretKey::new(api_key.to_owned()),
                model_name: summarizer_model_name,
                parameters: Parameters {
                    max_token: 262144,
                    temperature: 1.0,
                    top_p: 0.95,
                },
            },
            embedding_model: EmbeddingModel {
                provider_base_url,
                api_key: SecretKey::new(api_key.to_owned()),
                model_name: "text-embedding-3-small".to_owned(),
                dimension: 1536,
            },
        },
        memory: Memory {
            vector_db: VectorDb::default(),
            short_term_max_entries: 20,
            mid_term_top_k: 3,
            long_term_top_k: 5,
            mid_term_retention_days: 30,
            long_term_extraction_interval: 10,
        },
        tools: ToolPermissions {
            web_search,
            searxng: Default::default(),
            code_exec: false,
            read_file: false,
            code_exec_sandbox: Default::default(),
            read_file_dirs: Default::default(),
        },
        web_ui: WebUiConfig::default(),
    }
}

/// Resolve the provider base URL and a default model name.
fn resolve_provider(provider: &str, model: &str, base_url: &str) -> (String, String) {
    // If a custom base_url is provided, use it
    if !base_url.is_empty() {
        let model = if model.is_empty() { "gpt-4o" } else { model };
        return (base_url.to_owned(), model.to_owned());
    }

    // Try to match known provider presets
    let (url, default_model) = match provider.to_ascii_lowercase().as_str() {
        "openai" => ("https://api.openai.com/v1", "gpt-4o"),
        "anthropic" => ("https://api.anthropic.com/v1", "claude-sonnet-4-20250514"),
        "ollama" => ("http://localhost:11434/v1", "llama3.1"),
        _ => {
            warn!(provider = %provider, "unknown AI provider name — falling back to OpenAI defaults");
            let model = if model.is_empty() { "gpt-4o" } else { model };
            return ("https://api.openai.com/v1".to_owned(), model.to_owned());
        }
    };

    let model = if model.is_empty() {
        default_model
    } else {
        model
    };
    (url.to_owned(), model.to_owned())
}

/// Resolve the default summarizer model name based on the provider name.
fn resolve_summarizer(provider: &str, conv_model: &str) -> String {
    match provider.to_ascii_lowercase().as_str() {
        "openai" => "gpt-4o-mini".to_owned(),
        "anthropic" => "claude-haiku-3-5-20241022".to_owned(),
        "ollama" => conv_model.to_owned(),
        _ => conv_model.to_owned(),
    }
}

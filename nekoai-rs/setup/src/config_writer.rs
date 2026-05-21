use std::path::Path;

use anyhow::{Context, Result};
use nekoai_config::loader::{Config, DEFAULT_QDRANT_URL};
use tracing::{info, warn};

const CONFIG_PATH: &str = ".config/config.toml";

/// Save a Config to the config file path.
/// If the file already exists, merge the new config with the existing one
/// (existing values take priority).
pub fn save_config(config: &Config) -> Result<()> {
    let config_dir = Path::new(CONFIG_PATH).parent().unwrap();
    if !config_dir.exists() {
        std::fs::create_dir_all(config_dir).with_context(|| {
            format!(
                "failed to create config directory: {}",
                config_dir.display()
            )
        })?;
        info!(path = %config_dir.display(), "created config directory");
    }

    let final_config = if Path::new(CONFIG_PATH).exists() {
        info!("existing config file found, merging with new values");
        merge_with_existing(config)?
    } else {
        config.clone()
    };

    let toml_output =
        toml::to_string_pretty(&final_config).context("failed to serialize config to TOML")?;

    std::fs::write(CONFIG_PATH, &toml_output)
        .with_context(|| format!("failed to write config file: {CONFIG_PATH}"))?;

    info!(path = CONFIG_PATH, "configuration saved successfully");
    Ok(())
}

/// Merge the wizard-generated config with an existing config file.
/// Existing values take priority over new values — this prevents accidentally
/// overwriting user-customized settings.
fn merge_with_existing(new_config: &Config) -> Result<Config> {
    let existing_content = std::fs::read_to_string(CONFIG_PATH)
        .with_context(|| format!("failed to read existing config: {CONFIG_PATH}"))?;

    let existing: Config = toml::from_str(&existing_content)
        .with_context(|| format!("failed to parse existing config: {CONFIG_PATH}"))?;

    let mut merged = new_config.clone();

    merge_discord(&mut merged, &existing);
    merge_provider(&mut merged, &existing);
    merge_memory(&mut merged, &existing);
    merge_tools(&mut merged, &existing);

    warn!("existing config values were preserved where present");
    Ok(merged)
}

/// Keep the existing Discord settings if they are not placeholders.
fn merge_discord(merged: &mut Config, existing: &Config) {
    if !is_placeholder(existing.discord.token.expose()) {
        merged.discord.token = existing.discord.token.clone();
    }
    if existing.discord.guild_id != 0 {
        merged.discord.guild_id = existing.discord.guild_id;
    }
}

/// Keep the existing provider settings if they are not defaults/placeholders.
fn merge_provider(merged: &mut Config, existing: &Config) {
    // ── Conversation model ──────────────────────────────────
    if !is_placeholder(existing.provider.conversation_model.api_key.expose()) {
        merged.provider.conversation_model.api_key =
            existing.provider.conversation_model.api_key.clone();
    }
    if !existing.provider.conversation_model.model_name.is_empty() {
        merged.provider.conversation_model.model_name =
            existing.provider.conversation_model.model_name.clone();
    }
    if !existing
        .provider
        .conversation_model
        .provider_base_url
        .is_empty()
    {
        merged.provider.conversation_model.provider_base_url = existing
            .provider
            .conversation_model
            .provider_base_url
            .clone();
    }
    merge_parameters(
        &mut merged.provider.conversation_model.parameters,
        &existing.provider.conversation_model.parameters,
    );

    // ── Summarizer model ────────────────────────────────────
    if !is_placeholder(existing.provider.summarizer_model.api_key.expose()) {
        merged.provider.summarizer_model.api_key =
            existing.provider.summarizer_model.api_key.clone();
    }
    if !existing.provider.summarizer_model.model_name.is_empty() {
        merged.provider.summarizer_model.model_name =
            existing.provider.summarizer_model.model_name.clone();
    }
    if !existing
        .provider
        .summarizer_model
        .provider_base_url
        .is_empty()
    {
        merged.provider.summarizer_model.provider_base_url =
            existing.provider.summarizer_model.provider_base_url.clone();
    }
    merge_parameters(
        &mut merged.provider.summarizer_model.parameters,
        &existing.provider.summarizer_model.parameters,
    );

    // ── Embedding model ─────────────────────────────────────
    if !is_placeholder(existing.provider.embedding_model.api_key.expose()) {
        merged.provider.embedding_model.api_key = existing.provider.embedding_model.api_key.clone();
    }
    if !existing.provider.embedding_model.model_name.is_empty() {
        merged.provider.embedding_model.model_name =
            existing.provider.embedding_model.model_name.clone();
    }
    if !existing
        .provider
        .embedding_model
        .provider_base_url
        .is_empty()
    {
        merged.provider.embedding_model.provider_base_url =
            existing.provider.embedding_model.provider_base_url.clone();
    }
    if existing.provider.embedding_model.dimension != 0 {
        merged.provider.embedding_model.dimension = existing.provider.embedding_model.dimension;
    }
}

/// Merge Parameters, preserving values that differ from CLI defaults.
fn merge_parameters(
    merged_params: &mut nekoai_config::loader::Parameters,
    existing: &nekoai_config::loader::Parameters,
) {
    if existing.max_token.abs_diff(262144) > 0 {
        merged_params.max_token = existing.max_token;
    }
    if (existing.temperature - 1.0).abs() > 0.01 {
        merged_params.temperature = existing.temperature;
    }
    if (existing.top_p - 0.95).abs() > 0.01 {
        merged_params.top_p = existing.top_p;
    }
}

/// Keep the existing memory settings if they differ from defaults.
fn merge_memory(merged: &mut Config, existing: &Config) {
    if existing.memory.short_term_max_entries != 20 {
        merged.memory.short_term_max_entries = existing.memory.short_term_max_entries;
    }
    if existing.memory.mid_term_top_k != 3 {
        merged.memory.mid_term_top_k = existing.memory.mid_term_top_k;
    }
    if existing.memory.long_term_top_k != 5 {
        merged.memory.long_term_top_k = existing.memory.long_term_top_k;
    }
    if existing.memory.mid_term_retention_days != 30 {
        merged.memory.mid_term_retention_days = existing.memory.mid_term_retention_days;
    }
    if existing.memory.long_term_extraction_interval != 10 {
        merged.memory.long_term_extraction_interval = existing.memory.long_term_extraction_interval;
    }
    // Vector DB
    if !existing.memory.vector_db.url.is_empty()
        && existing.memory.vector_db.url != DEFAULT_QDRANT_URL
    {
        merged.memory.vector_db.url = existing.memory.vector_db.url.clone();
    }
    if existing.memory.vector_db.api_key.is_some() {
        merged.memory.vector_db.api_key = existing.memory.vector_db.api_key.clone();
    }
}

/// Keep the existing tool permissions.
fn merge_tools(merged: &mut Config, existing: &Config) {
    merged.tools = existing.tools.clone();
}

/// Check if a string looks like a placeholder (e.g. "YOUR_..." or empty).
fn is_placeholder(s: &str) -> bool {
    s.is_empty() || s.starts_with("YOUR_") || s.starts_with("sk-...") || s == "sk-ant-..."
}

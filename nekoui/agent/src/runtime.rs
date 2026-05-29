use std::sync::Arc;

use anyhow::{Context, Result};
use derive_builder::Builder;
use nekoui_config::Config;
use nekoui_telemetry::{State, print_log};
use rig::providers::openai;
use tracing::warn;

use crate::provider::OpenAICompatibleAdapter;

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Agent {
    agent_client: Arc<OpenAICompatibleAdapter>,
    summarizer_client: Arc<OpenAICompatibleAdapter>,
    system_prompt: String,
}

impl Agent {
    pub fn builder(config: Config) -> Result<AgentBuilder> {
        let agent_client = openai::Client::builder()
            .api_key(config.provider.agent.api_key.as_ref())
            .base_url(config.provider.agent.base_url)
            .build()
            .context("failed to build OpenAI compatible responses client")?
            .completions_api();
        print_log(State::Ok, "Agent client initialized");

        let summarizer_client = openai::Client::builder()
            .api_key(config.provider.summarizer.api_key.as_ref())
            .base_url(config.provider.summarizer.base_url)
            .build()
            .context("failed to build OpenAI compatible responses client")?
            .completions_api();
        print_log(State::Ok, "Summarizer client initialized");

        #[cfg(debug_assertions)]
        let system_prompt_path = std::path::Path::new("..config");

        #[cfg(not(debug_assertions))]
        let system_prompt_path = std::path::Path::new("config");

        let system_prompt = std::fs::read_to_string(system_prompt_path.join("INSTRUCTION.md")).unwrap_or_else(|e| {
            warn!(error = %e, path = %system_prompt_path.join("INSTRUCTION.md").display(), "Failed to read system prompt file");
            print_log(State::Warn, "Failed to load INSTRUCTION.md. Using default prompt.");
            "You are helpful assistant.".to_string()
        });

        Ok(AgentBuilder::default()
            .agent_client(Arc::new(OpenAICompatibleAdapter::new(agent_client)))
            .summarizer_client(Arc::new(OpenAICompatibleAdapter::new(summarizer_client)))
            .system_prompt(system_prompt))
    }

    pub fn run(&mut self) -> Result<()> {
        Ok(())
    }
}

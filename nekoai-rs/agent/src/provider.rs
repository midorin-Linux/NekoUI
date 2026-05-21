use nekoai_config::loader::Parameters;
use rig::{
    agent::AgentBuilder,
    client::CompletionClient,
    providers::openai::{self, completion::CompletionModel},
};
use serde_json::json;

pub struct OpenAICompatibleAdapter {
    client: openai::CompletionsClient,
}

impl OpenAICompatibleAdapter {
    pub fn new(client: openai::CompletionsClient) -> Self {
        Self { client }
    }

    pub fn build_agent(
        &self,
        model: &str,
        parameters: Parameters,
    ) -> AgentBuilder<CompletionModel> {
        self.client
            .agent(model)
            .max_tokens(parameters.max_token)
            .temperature(parameters.temperature)
            .default_max_turns(20)
            .additional_params(json!({
                "top_p": parameters.top_p,
            }))
    }

    pub fn provider_name(&self) -> &str {
        "openai-compatible"
    }
}

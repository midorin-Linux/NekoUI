use nekoui_config::provider::TextModelProvider;
use rig::{
    agent::AgentBuilder,
    client::CompletionClient,
    providers::openai::{self, completion::CompletionModel},
};
use serde_json::{Value, json};

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
        config: TextModelProvider,
    ) -> AgentBuilder<CompletionModel> {
        fn extract(entries: &[Value], key: &str) -> Option<Value> {
            entries.iter().find_map(|entry| entry.get(key).cloned())
        }

        let max_token = extract(&config.parameters, "max_token")
            .and_then(|v| v.as_u64())
            .unwrap();
        let temperature = extract(&config.parameters, "temperature")
            .and_then(|v| v.as_f64())
            .unwrap();

        let additional_params = config
            .parameters
            .iter()
            .filter_map(|entry| {
                entry
                    .as_object()?
                    .iter()
                    .find(|(key, _)| *key != "max_token" && *key != "temperature")
            })
            .fold(json!({}), |mut acc, (key, val)| {
                acc[key] = val.clone();
                acc
            });

        self.client
            .agent(model)
            .max_tokens(max_token)
            .temperature(temperature)
            .default_max_turns(20)
            .additional_params(additional_params)
    }

    pub fn provider_name(&self) -> &str {
        "openai-compatible"
    }
}

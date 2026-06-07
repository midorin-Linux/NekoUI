use std::sync::Arc;

use anyhow::{Context, Result};
use derive_builder::Builder;
use nekoui_config::Config;
use nekoui_domain::session::SessionKey;
use nekoui_memory::store::MemoryStore;
use nekoui_telemetry::{State, print_log};
use rig::{
    completion::{Message, Prompt},
    providers::openai,
};
use tracing::{debug, info, warn};

use crate::{context::ContextManager, provider::OpenAICompatibleAdapter, session::SessionManager};

pub struct AgentResponse {
    pub content: String,
}

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Agent {
    agent_client: Arc<OpenAICompatibleAdapter>,
    config: Config,
    context_manager: Arc<ContextManager>,
    memory_store: Arc<MemoryStore>,
    session_manager: Arc<SessionManager>,
    #[allow(unused)]
    summarizer_client: Arc<OpenAICompatibleAdapter>,
}

impl Agent {
    pub fn builder(config: Config) -> Result<AgentBuilder> {
        #[cfg(debug_assertions)]
        let system_prompt_path = std::path::Path::new("../config");

        #[cfg(not(debug_assertions))]
        let system_prompt_path = std::path::Path::new("config");

        let system_prompt = std::fs::read_to_string(system_prompt_path.join("INSTRUCTION.md")).unwrap_or_else(|e| {
            warn!(error = %e, path = %system_prompt_path.join("INSTRUCTION.md").display(), "Failed to read system prompt file");
            print_log(State::Warn, "Failed to load INSTRUCTION.md. Using default prompt.");
            "You are helpful assistant.".to_string()
        });

        let agent_client = openai::Client::builder()
            .api_key(config.provider.agent.api_key.as_ref())
            .base_url(&config.provider.agent.base_url)
            .build()
            .context("failed to build OpenAI compatible responses client")?
            .completions_api();
        print_log(State::Ok, "Agent client initialized");

        let context_manager = Arc::new(ContextManager::new(&system_prompt, 16384));
        print_log(State::Ok, "Context manager initialized");

        let memory_store = Arc::new(MemoryStore::new(&config)?);
        print_log(State::Ok, "Memory store initialized");

        let session_manager = Arc::new(SessionManager::new());
        print_log(State::Ok, "Session manager initialized");

        let summarizer_client = openai::Client::builder()
            .api_key(config.provider.summarizer.api_key.as_ref())
            .base_url(&config.provider.summarizer.base_url)
            .build()
            .context("failed to build OpenAI compatible responses client")?
            .completions_api();
        print_log(State::Ok, "Summarizer client initialized");

        Ok(AgentBuilder::default()
            .agent_client(Arc::new(OpenAICompatibleAdapter::new(agent_client)))
            .context_manager(context_manager)
            .config(config.to_owned())
            .memory_store(memory_store)
            .session_manager(session_manager)
            .summarizer_client(Arc::new(OpenAICompatibleAdapter::new(summarizer_client))))
    }
    pub async fn submit(&self, prompt: String, session_key: SessionKey) -> Result<AgentResponse> {
        info!(prompt = prompt.len(), "submitting user input");

        let session = {
            let session_arc = self.session_manager.get_or_create(&session_key);
            session_arc.lock().await.clone()
        };
        debug!(turn_count = session.turns.len(), "session loaded");

        let context = self
            .context_manager
            .build(
                &session,
                &prompt,
                Some(session_key.conversation_id.to_string()),
            )
            .await;
        debug!(context_turns = context.turns.len(), "context built");

        let mut chat_history = Vec::with_capacity(context.turns.len() * 2);
        for turn in &context.turns {
            chat_history.push(Message::user(&turn.user));
            chat_history.push(Message::assistant(&turn.assistant));
        }

        debug!(
            chat_history_message_count = chat_history.len(),
            context_turn_count = context.turns.len(),
            "prompt composed"
        );

        let agent = self
            .agent_client
            .build_agent(self.config.provider.agent.clone())
            .preamble(context.system_prompt.as_str())
            .build();
        let content = agent
            .prompt(&prompt)
            .max_turns(20)
            .with_history(chat_history)
            .await
            .context("failed to submit prompt")?;

        self.memory_store
            .push_short_term(&session_key, &prompt, &content);
        debug!(session = %session_key.conversation_id, "short-term memory updated after response");

        self.session_manager
            .append(&session_key, &prompt, &content)
            .await;
        debug!(session = %session_key.conversation_id, "session updated after response");

        Ok(AgentResponse { content })
    }

    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.session_manager.clone()
    }
}

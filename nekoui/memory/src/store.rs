use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use nekoui_config::Config;
use nekoui_domain::session::SessionKey;
use serde_json::Value;
use tracing::debug;

use crate::short_term::ShortTermMemory;

pub struct MemoryStore {
    short_term_memory: ShortTermMemory,
}

#[derive(Clone, Debug)]
pub struct MemoryEntry {
    pub content: String,
    pub score: f32,
    pub created_at: DateTime<Utc>,
    pub metadata: HashMap<String, Value>,
}

impl MemoryStore {
    pub fn new(config: &Config) -> Result<Self> {
        let short_term = ShortTermMemory::new(config.memory.short_term_max_entries);

        Ok(Self {
            short_term_memory: short_term,
        })
    }

    pub fn push_short_term(&self, session_key: &SessionKey, user: &str, assistant: &str) {
        debug!(
            session = %session_key.conversation_id,
            user_len = user.len(),
            assistant_len = assistant.len(),
            "pushing conversation turn to short-term memory"
        );
        self.short_term_memory
            .push_turn(session_key, user, assistant);
    }

    pub fn clear_short_term(&self, session_key: &SessionKey) {
        self.short_term_memory.clear(session_key);
        debug!(session = %session_key.conversation_id, "cleared short-term memory");
    }
}

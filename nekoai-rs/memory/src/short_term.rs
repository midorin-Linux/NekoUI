use std::collections::VecDeque;

use chrono::Utc;
use dashmap::DashMap;
use nekoai_domain::agent::session::SessionKey;
use tracing::debug;

#[derive(Debug, Clone)]
pub enum Role {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone)]
pub struct ShortTermEntry {
    pub role: Role,
    pub content: String,
    pub timestamp: i64,
}

pub struct ShortTermMemory {
    pub store: DashMap<SessionKey, VecDeque<ShortTermEntry>>,
    pub max_entry: usize,
}

impl ShortTermMemory {
    pub fn new(max_entry: usize) -> Self {
        Self {
            store: DashMap::new(),
            max_entry,
        }
    }

    pub fn push_turn(&self, session_key: &SessionKey, user: &str, assistant: &str) {
        debug!(
            session = %session_key.channel_id,
            max_entry = self.max_entry,
            "storing short-term conversation turn"
        );

        let timestamp = Utc::now().timestamp();
        let capacity = self.max_entry.max(2);

        let mut queue = self
            .store
            .entry(session_key.clone())
            .or_insert_with(|| VecDeque::with_capacity(capacity));

        queue.push_back(ShortTermEntry {
            role: Role::User,
            content: user.to_string(),
            timestamp,
        });
        queue.push_back(ShortTermEntry {
            role: Role::Assistant,
            content: assistant.to_string(),
            timestamp,
        });

        // Drain excess entries from the front in a single shot.
        let len = queue.len();
        if len > self.max_entry {
            queue.drain(.. len - self.max_entry);
        }

        debug!(session = %session_key.channel_id, entry_count = queue.len(), "short-term memory updated");
    }

    pub fn get_count(&self, session_key: &SessionKey) -> usize {
        self.store.get(session_key).map(|v| v.len()).unwrap_or(0)
    }

    pub fn get_messages(&self, session_key: &SessionKey) -> Vec<ShortTermEntry> {
        self.store
            .get(session_key)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear(&self, session_key: &SessionKey) {
        self.store.remove(session_key);
    }
}

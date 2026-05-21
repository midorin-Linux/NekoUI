use std::{collections::VecDeque, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use nekoai_domain::agent::session::SessionKey;
use rig::completion::Message;
use tokio::sync::Mutex;
use tracing::debug;

#[derive(Clone)]
pub struct ConversationTurn {
    pub user: String,
    pub assistant: String,
}

#[derive(Clone)]
pub struct Session {
    pub key: SessionKey,
    pub messages: VecDeque<Message>,
    pub turns: VecDeque<ConversationTurn>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub token_count: usize,
}

pub struct SessionManager {
    sessions: DashMap<SessionKey, Arc<Mutex<Session>>>,
    max_messages: usize,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
            max_messages: 40,
        }
    }

    pub async fn append(&self, session_key: &SessionKey, user: &str, assistant: &str) {
        let max_messages = self.max_messages;
        let session_arc = self.get_or_create(session_key);
        let mut session = session_arc.lock().await;

        session.turns.push_back(ConversationTurn {
            user: user.to_string(),
            assistant: assistant.to_string(),
        });
        session.messages.push_back(Message::user(user));
        session.messages.push_back(Message::assistant(assistant));

        while session.messages.len() > max_messages {
            session.messages.pop_front();
        }

        while session.turns.len() * 2 > max_messages {
            session.turns.pop_front();
        }

        session.last_active = Utc::now();

        debug!(
            session = %session_key.channel_id,
            message_count = session.messages.len(),
            turn_count = session.turns.len(),
            "session updated"
        );
    }

    pub fn clear(&self, session_key: &SessionKey) -> Result<()> {
        if self.sessions.remove(session_key).is_some() {
            debug!(session = %session_key.channel_id, "session cleared");
        } else {
            debug!(target_session = %session_key.channel_id, "non-existent session");
        }
        Ok(())
    }

    pub fn get_or_create(&self, session_key: &SessionKey) -> Arc<Mutex<Session>> {
        self.sessions
            .entry(session_key.clone())
            .or_insert_with(|| {
                debug!(session = %session_key.channel_id, "created new session");
                let now = Utc::now();
                Arc::new(Mutex::new(Session {
                    key: session_key.clone(),
                    messages: VecDeque::new(),
                    turns: VecDeque::new(),
                    created_at: now,
                    last_active: now,
                    token_count: 0,
                }))
            })
            .clone()
    }

    pub fn all_keys(&self) -> Vec<SessionKey> {
        self.sessions
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub fn get(&self, session_key: &SessionKey) -> Result<Arc<Mutex<Session>>> {
        self.sessions
            .get(session_key)
            .map(|entry| {
                debug!(session = %session_key.channel_id, "found existing session");
                entry.value().clone()
            })
            .ok_or_else(|| {
                debug!(target_session = %session_key.channel_id, "non-existent session");
                anyhow::anyhow!("session not found")
            })
    }
}

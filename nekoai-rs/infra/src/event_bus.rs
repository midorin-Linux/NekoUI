use nekoai_domain::agent::session::SessionKey;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize)]
pub enum AgentEvent {
    MessageReceived {
        session_key: SessionKey,
        content: String,
    },
    ThinkingStarted {
        session_key: SessionKey,
    },
    ToolCalled {
        session_key: SessionKey,
        tool: String,
        args: Value,
    },
    ToolResult {
        session_key: SessionKey,
        tool: String,
        result: Value,
    },
    ResponseChunk {
        session_key: SessionKey,
        chunk: String,
    },
    ResponseCompleted {
        session_key: SessionKey,
        full_response: String,
    },
    MemoryRecalled {
        session_key: SessionKey,
        mid_count: usize,
        long_count: usize,
    },
    MemoryPromoted {
        session_key: SessionKey,
    },
    MemoryExtracted {
        session_key: SessionKey,
        fact: String,
    },
    ErrorOccurred {
        session_key: SessionKey,
        error: String,
    },
}

#[derive(Clone, Debug)]
pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: AgentEvent) {
        // Log debug message if there are no active subscribers
        if let Err(broadcast::error::SendError(_)) = self.sender.send(event) {
            tracing::debug!(target: "event_bus", "no active subscribers for event");
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }
}

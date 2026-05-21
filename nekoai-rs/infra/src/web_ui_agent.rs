use nekoai_domain::agent::session::SessionKey;

use crate::{event_bus::EventBus, metrics::Metrics};

#[async_trait::async_trait]
pub trait WebUiAgent: Send + Sync {
    fn event_bus(&self) -> &EventBus;
    fn metrics(&self) -> &Metrics;
    async fn list_sessions(&self) -> Vec<SessionKey>;
    async fn submit(
        &self,
        session_key: SessionKey,
        user_id: Option<String>,
        content: String,
    ) -> anyhow::Result<String>;
}

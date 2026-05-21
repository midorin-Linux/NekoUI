use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, Utc};
use nekoai_config::loader::Config as AppConfig;
use nekoai_domain::agent::session::SessionKey;
use serde_json::Value;
use tokio::time::{Duration, interval};
use tracing::{debug, info, warn};

use crate::{
    embedding::Embedder,
    long_term::LongTermMemory,
    mid_term::MidTermMemory,
    short_term::{ShortTermEntry, ShortTermMemory},
};

pub struct MemoryStore {
    short_term_memory: ShortTermMemory,
    mid_term: Arc<MidTermMemory>,
    long_term: Arc<LongTermMemory>,
    embedder: Arc<dyn Embedder>,
    mid_term_top_k: usize,
    long_term_top_k: usize,
}

#[derive(Clone)]
pub struct RecalledMemory {
    pub mid_term: Vec<MemoryEntry>,
    pub long_term: Vec<MemoryEntry>,
}

#[derive(Clone, Debug)]
pub struct MemoryEntry {
    pub content: String,
    pub score: f32,
    pub created_at: DateTime<Utc>,
    pub metadata: HashMap<String, Value>,
}

impl MemoryStore {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let short_term_memory = ShortTermMemory::new(config.memory.short_term_max_entries);
        let vector_db = Arc::new(
            crate::vector_db::qdrant::QdrantClient::new(
                config.memory.vector_db.url.clone(),
                config.memory.vector_db.api_key.as_ref().and_then(|value| {
                    if value.is_empty() {
                        None
                    } else {
                        Some(value.clone())
                    }
                }),
            )
            .expect("failed to create Qdrant client"),
        );
        let embedding_dim = config.provider.embedding_model.dimension as usize;
        let embedder: Arc<dyn Embedder> = match crate::embedding::OpenAICompatibleEmbedder::new(
            &config.provider.embedding_model.provider_base_url,
            config.provider.embedding_model.api_key.as_ref(),
            &config.provider.embedding_model.model_name,
            embedding_dim,
        ) {
            Ok(embedder) => {
                info!(
                    embedding_model = %config.provider.embedding_model.model_name,
                    embedding_base_url = %config.provider.embedding_model.provider_base_url,
                    embedding_dim = embedding_dim,
                    "embedding model initialized"
                );
                Arc::new(embedder)
            }
            Err(error) => {
                warn!(
                    error = %error,
                    embedding_model = %config.provider.embedding_model.model_name,
                    "failed to initialize embedding model, falling back to mock embedder"
                );
                Arc::new(crate::embedding::MockEmbedder::new(embedding_dim))
            }
        };

        info!(
            qdrant_url = %config.memory.vector_db.url,
            mid_collection = %config.memory.vector_db.mid_term_collection,
            long_collection = %config.memory.vector_db.long_term_collection,
            "memory store initialized"
        );

        Ok(Self {
            short_term_memory,
            mid_term: Arc::new(MidTermMemory::new(
                vector_db.clone(),
                embedder.clone(),
                config.memory.vector_db.mid_term_collection.clone(),
                config.memory.mid_term_retention_days,
            )),
            long_term: Arc::new(LongTermMemory::new(
                vector_db,
                embedder.clone(),
                config.memory.vector_db.long_term_collection.clone(),
            )),
            embedder,
            mid_term_top_k: config.memory.mid_term_top_k,
            long_term_top_k: config.memory.long_term_top_k,
        })
    }

    pub fn with_components(
        mid_term: Arc<MidTermMemory>,
        long_term: Arc<LongTermMemory>,
        embedder: Arc<dyn Embedder>,
        short_term_max: usize,
        mid_term_top_k: usize,
        long_term_top_k: usize,
    ) -> Self {
        let short_term_memory = ShortTermMemory::new(short_term_max);
        info!(
            short_term_max = short_term_max,
            mid_term_top_k = mid_term_top_k,
            long_term_top_k = long_term_top_k,
            "memory store initialized"
        );

        Self {
            short_term_memory,
            mid_term,
            long_term,
            embedder,
            mid_term_top_k,
            long_term_top_k,
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        let dim = self.embedder.dimension();
        self.mid_term.ensure_collection(dim).await?;
        self.long_term.ensure_collection(dim).await?;
        Ok(())
    }

    pub fn push_short_term(&self, session_key: &SessionKey, user: &str, assistant: &str) {
        debug!(
            session = %session_key.channel_id,
            user_len = user.len(),
            assistant_len = assistant.len(),
            "pushing conversation turn to short-term memory"
        );
        self.short_term_memory
            .push_turn(session_key, user, assistant);
    }

    pub async fn recall(&self, session_key: &SessionKey, query: &str) -> RecalledMemory {
        let query_embedding = self.embedder.embed(query).await;
        let (mid_term_result, long_term_result) = tokio::join!(
            self.mid_term
                .search_with_embedding(session_key, &query_embedding, self.mid_term_top_k,),
            self.long_term.search_with_embedding(
                session_key,
                &query_embedding,
                self.long_term_top_k,
            )
        );

        let mid_term = mid_term_result.unwrap_or_else(|e| {
            warn!(error = %e, "failed to search mid-term memory");
            vec![]
        });

        let long_term = long_term_result.unwrap_or_else(|e| {
            warn!(error = %e, "failed to search long-term memory");
            vec![]
        });

        debug!(
            session = %session_key.channel_id,
            mid_count = mid_term.len(),
            long_count = long_term.len(),
            "recalled memories"
        );

        RecalledMemory {
            mid_term,
            long_term,
        }
    }

    pub fn should_summarize(&self, session_key: &SessionKey) -> bool {
        self.short_term_memory.get_count(session_key) >= self.short_term_memory.max_entry
    }

    pub async fn promote_to_mid_term(
        &self,
        session_key: &SessionKey,
        summary: String,
    ) -> Result<()> {
        let messages = self.short_term_memory.get_messages(session_key);

        self.mid_term
            .store_summary(session_key, &messages, summary)
            .await?;

        self.short_term_memory.clear(session_key);

        debug!(session = %session_key.channel_id, "promoted short-term to mid-term");
        Ok(())
    }

    pub async fn extract_long_term(
        &self,
        session_key: &SessionKey,
        user_id: Option<&str>,
        facts: Vec<(String, Vec<String>)>,
    ) -> Result<()> {
        let fact_count = facts.len();
        for (fact, tags) in facts {
            self.long_term
                .store(session_key, user_id, fact, tags)
                .await?;
        }

        debug!(session = %session_key.channel_id, fact_count = fact_count, "extracted long-term facts");
        Ok(())
    }

    pub fn get_short_term_messages(&self, session_key: &SessionKey) -> Vec<ShortTermEntry> {
        self.short_term_memory.get_messages(session_key)
    }

    pub fn clear_short_term(&self, session_key: &SessionKey) {
        self.short_term_memory.clear(session_key);
        debug!(session = %session_key.channel_id, "cleared short-term memory");
    }

    /// Start a background cleanup job for midterm memory retention.
    /// This runs periodically and deletes entries older than retention_days.
    pub fn start_cleanup_job(&self) {
        let mid_term = self.mid_term.clone();
        let retention_days = self.mid_term.retention_days();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(24 * 60 * 60)); // Run daily

            loop {
                interval.tick().await;
                debug!(
                    retention_days = retention_days,
                    "running mid-term cleanup job"
                );

                match mid_term.delete_old_entries().await {
                    Ok(deleted) => {
                        if deleted > 0 {
                            info!(deleted = deleted, "cleaned up old mid-term entries");
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "failed to run mid-term cleanup job");
                    }
                }
            }
        });

        info!("started mid-term cleanup job (runs daily)");
    }

    pub async fn promote_to_mid_term_with_messages(
        &self,
        session_key: &SessionKey,
        messages: &[ShortTermEntry],
        summary: String,
    ) -> Result<()> {
        self.mid_term
            .store_summary(session_key, messages, summary)
            .await?;
        self.short_term_memory.clear(session_key);
        Ok(())
    }
}

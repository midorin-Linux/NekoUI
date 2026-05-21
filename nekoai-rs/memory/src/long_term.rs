use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, Utc};
use nekoai_domain::agent::session::SessionKey;
use serde_json::json;
use tracing::{debug, info};
use uuid::Uuid;

use crate::{
    embedding::Embedder,
    store::MemoryEntry,
    vector_db::{
        FilterCondition, SearchFilter, SearchResult, VectorDbClient,
        qdrant::{session_kind_value, session_scope_filter},
    },
};

pub struct LongTermMemory {
    db: Arc<dyn VectorDbClient>,
    embedder: Arc<dyn Embedder>,
    collection: String,
}

impl LongTermMemory {
    pub fn new(
        db: Arc<dyn VectorDbClient>,
        embedder: Arc<dyn Embedder>,
        collection: String,
    ) -> Self {
        Self {
            db,
            embedder,
            collection,
        }
    }

    pub async fn ensure_collection(&self, dim: usize) -> Result<()> {
        self.db.ensure_collection(&self.collection, dim).await?;
        info!(collection = %self.collection, "long-term memory initialized");
        Ok(())
    }

    pub async fn store(
        &self,
        session_key: &SessionKey,
        user_id: Option<&str>,
        fact: String,
        tags: Vec<String>,
    ) -> Result<()> {
        let embedding = self.embedder.embed(&fact).await;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();

        let mut payload = HashMap::with_capacity(7);
        payload.insert("content".to_string(), json!(fact));
        payload.insert(
            "guild_id".to_string(),
            json!(session_key.guild_id.map(|g| g.to_string())),
        );
        payload.insert(
            "channel_id".to_string(),
            json!(session_key.channel_id.to_string()),
        );
        payload.insert(
            "kind".to_string(),
            json!(session_kind_value(&session_key.kind)),
        );
        payload.insert("created_at".to_string(), json!(now));
        payload.insert("tags".to_string(), json!(tags));

        if let Some(uid) = user_id {
            payload.insert("user_id".to_string(), json!(uid));
        }

        self.db
            .upsert(crate::vector_db::UpsertRequest {
                collection: &self.collection,
                id: &id,
                vector: embedding,
                payload,
            })
            .await?;

        debug!(id = %id, session = %session_key.channel_id, "stored long-term fact");
        Ok(())
    }

    /// Search long-term memories by guild only (for server-wide facts)
    pub async fn search_by_guild(
        &self,
        guild_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let filter = SearchFilter {
            must: vec![FilterCondition::Match {
                key: "guild_id".to_string(),
                value: json!(guild_id),
            }],
            should: vec![],
        };

        self.search_with_filter(query, filter, top_k).await
    }

    /// Search long-term memories by user_id (for user-specific facts)
    pub async fn search_by_user(
        &self,
        user_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let filter = SearchFilter {
            must: vec![FilterCondition::Match {
                key: "user_id".to_string(),
                value: json!(user_id),
            }],
            should: vec![],
        };

        self.search_with_filter(query, filter, top_k).await
    }

    pub async fn search(
        &self,
        session_key: &SessionKey,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let filter = session_scope_filter(session_key);
        let embedding = self.embedder.embed(query).await;
        self.search_with_filter_embedding(&embedding, filter, top_k)
            .await
    }

    pub async fn search_with_embedding(
        &self,
        session_key: &SessionKey,
        embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let filter = session_scope_filter(session_key);
        self.search_with_filter_embedding(embedding, filter, top_k)
            .await
    }

    async fn search_with_filter(
        &self,
        query: &str,
        filter: SearchFilter,
        top_k: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let embedding = self.embedder.embed(query).await;
        self.search_with_filter_embedding(&embedding, filter, top_k)
            .await
    }

    async fn search_with_filter_embedding(
        &self,
        embedding: &[f32],
        filter: SearchFilter,
        top_k: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let results = self
            .db
            .search(crate::vector_db::SearchRequest {
                collection: &self.collection,
                vector: embedding.to_vec(),
                filter: Some(filter),
                top_k,
            })
            .await?;

        Ok(results.into_iter().map(search_result_to_entry).collect())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.db.delete(&self.collection, id).await?;
        debug!(id = %id, "deleted long-term fact");
        Ok(())
    }

    pub async fn delete_by_channel(&self, channel_id: &str) -> Result<u64> {
        let filter = SearchFilter {
            must: vec![FilterCondition::Match {
                key: "channel_id".to_string(),
                value: json!(channel_id),
            }],
            should: vec![],
        };

        let deleted = self.db.delete_by_filter(&self.collection, filter).await?;

        if deleted > 0 {
            debug!(
                channel_id = channel_id,
                deleted = deleted,
                "cleared long-term memories for channel"
            );
        }

        Ok(deleted)
    }
}

pub(crate) fn search_result_to_entry(r: SearchResult) -> MemoryEntry {
    let content = r
        .payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let created_at = r
        .payload
        .get("created_at")
        .and_then(|v| v.as_i64())
        .and_then(|ts| DateTime::from_timestamp(ts, 0))
        .unwrap_or_default();

    MemoryEntry {
        content,
        score: r.score,
        created_at,
        metadata: r.payload,
    }
}

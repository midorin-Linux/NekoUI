use std::collections::HashMap;

use async_trait::async_trait;

pub mod inmemory;
pub mod qdrant;

#[async_trait]
pub trait VectorDbClient: Send + Sync {
    async fn upsert(&self, req: UpsertRequest<'_>) -> anyhow::Result<()>;
    async fn search(&self, req: SearchRequest<'_>) -> anyhow::Result<Vec<SearchResult>>;
    async fn delete(&self, collection: &str, id: &str) -> anyhow::Result<()>;
    async fn delete_by_filter(&self, collection: &str, filter: SearchFilter)
    -> anyhow::Result<u64>;
    async fn ensure_collection(&self, name: &str, dim: usize) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
pub struct UpsertRequest<'a> {
    pub collection: &'a str,
    pub id: &'a str,
    pub vector: Vec<f32>,
    pub payload: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct SearchRequest<'a> {
    pub collection: &'a str,
    pub vector: Vec<f32>,
    pub filter: Option<SearchFilter>,
    pub top_k: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    pub must: Vec<FilterCondition>,
    pub should: Vec<FilterCondition>,
}

#[derive(Debug, Clone)]
pub enum FilterCondition {
    Match {
        key: String,
        value: serde_json::Value,
    },
    Range {
        key: String,
        lt: Option<f64>,
        gt: Option<f64>,
    },
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub payload: HashMap<String, serde_json::Value>,
}

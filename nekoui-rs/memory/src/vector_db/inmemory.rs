use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::{
    FilterCondition, SearchFilter, SearchRequest, SearchResult, UpsertRequest, VectorDbClient,
};

pub struct InMemoryVectorDb {
    collections: Arc<RwLock<HashMap<String, Vec<Point>>>>,
}

struct Point {
    id: String,
    vector: Vec<f32>,
    norm: f32,
    payload: HashMap<String, serde_json::Value>,
}

// impl Point {
//     fn new(id: String, vector: Vec<f32>, payload: HashMap<String, serde_json::Value>) -> Self {
//         let norm = vector_norm(&vector);
//         Self {
//             id,
//             vector,
//             norm,
//             payload,
//         }
//     }
// }

impl InMemoryVectorDb {
    pub fn new() -> Self {
        Self {
            collections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryVectorDb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VectorDbClient for InMemoryVectorDb {
    async fn upsert(&self, req: UpsertRequest<'_>) -> anyhow::Result<()> {
        let mut collections = self.collections.write().await;
        let points = collections.entry(req.collection.to_string()).or_default();

        let UpsertRequest {
            id,
            vector,
            payload,
            ..
        } = req;
        let norm = vector_norm(&vector);

        if let Some(existing) = points.iter_mut().find(|p| p.id == id) {
            existing.vector = vector;
            existing.norm = norm;
            existing.payload = payload;
        } else {
            points.push(Point {
                id: id.to_string(),
                vector,
                norm,
                payload,
            });
        }

        Ok(())
    }

    async fn search(&self, req: SearchRequest<'_>) -> anyhow::Result<Vec<SearchResult>> {
        let collections = self.collections.read().await;
        let Some(points) = collections.get(req.collection) else {
            return Ok(Vec::new());
        };

        let query_norm = vector_norm(&req.vector);
        if query_norm == 0.0 {
            return Ok(Vec::new());
        }

        let filter_ref = req.filter.as_ref();
        let mut results: Vec<SearchResult> = points
            .iter()
            .filter(|p| filter_ref.is_none_or(|filter| matches_filter(&p.payload, filter)))
            .map(|p| {
                let score =
                    cosine_similarity_with_norms(&req.vector, query_norm, &p.vector, p.norm);
                SearchResult {
                    id: p.id.clone(),
                    score,
                    payload: p.payload.clone(),
                }
            })
            .collect();

        results.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(req.top_k);

        Ok(results)
    }

    async fn delete(&self, collection: &str, id: &str) -> anyhow::Result<()> {
        let mut collections = self.collections.write().await;
        if let Some(points) = collections.get_mut(collection) {
            points.retain(|p| p.id != id);
        }
        Ok(())
    }

    async fn delete_by_filter(
        &self,
        collection: &str,
        filter: SearchFilter,
    ) -> anyhow::Result<u64> {
        let mut collections = self.collections.write().await;

        let Some(points) = collections.get_mut(collection) else {
            return Ok(0);
        };

        let before = points.len();
        points.retain(|p| !matches_filter(&p.payload, &filter));
        let deleted = (before - points.len()) as u64;

        Ok(deleted)
    }

    async fn ensure_collection(&self, name: &str, _dim: usize) -> anyhow::Result<()> {
        let mut collections = self.collections.write().await;
        collections.entry(name.to_string()).or_default();
        Ok(())
    }
}

fn matches_filter(payload: &HashMap<String, serde_json::Value>, filter: &SearchFilter) -> bool {
    let must_ok = filter
        .must
        .iter()
        .all(|condition| matches_condition(payload, condition));

    if !must_ok {
        return false;
    }

    filter.should.is_empty()
        || filter
            .should
            .iter()
            .any(|condition| matches_condition(payload, condition))
}

fn matches_condition(
    payload: &HashMap<String, serde_json::Value>,
    condition: &FilterCondition,
) -> bool {
    match condition {
        FilterCondition::Match { key, value } => {
            let Some(actual) = payload.get(key) else {
                return false;
            };

            value_equals(actual, value)
        }
        FilterCondition::Range { key, lt, gt } => {
            let Some(actual) = payload.get(key).and_then(value_as_f64) else {
                return false;
            };

            if let Some(upper) = lt
                && actual >= *upper
            {
                return false;
            }

            if let Some(lower) = gt
                && actual <= *lower
            {
                return false;
            }

            true
        }
    }
}

fn value_equals(actual: &serde_json::Value, expected: &serde_json::Value) -> bool {
    match (actual, expected) {
        (serde_json::Value::Array(_), serde_json::Value::Array(_)) => actual == expected,
        (serde_json::Value::Array(items), _) => {
            items.iter().any(|item| value_equals(item, expected))
        }
        (serde_json::Value::Number(_), serde_json::Value::Number(_)) => {
            value_as_f64(actual) == value_as_f64(expected)
        }
        _ => actual == expected,
    }
}

fn value_as_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|v| v as f64))
        .or_else(|| value.as_u64().map(|v| v as f64))
}

fn vector_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn cosine_similarity_with_norms(a: &[f32], norm_a: f32, b: &[f32], norm_b: f32) -> f32 {
    if a.len() != b.len() || a.is_empty() || norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    dot / (norm_a * norm_b)
}

#[allow(dead_code)]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    cosine_similarity_with_norms(a, vector_norm(a), b, vector_norm(b))
}

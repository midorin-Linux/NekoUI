use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;
use nekoai_domain::agent::session::{SessionKey, SessionKind};
use serde_json::json;
use tokio_retry::{
    Retry,
    strategy::{ExponentialBackoff, jitter},
};
use tracing::{debug, info};

use super::{
    FilterCondition, SearchFilter, SearchRequest, SearchResult, UpsertRequest, VectorDbClient,
};

pub struct QdrantClient {
    #[allow(dead_code)]
    url: String,
    #[allow(dead_code)]
    api_key: Option<String>,
    client: qdrant_client::Qdrant,
}

impl QdrantClient {
    pub fn new(url: String, api_key: Option<String>) -> anyhow::Result<Self> {
        info!("qdrant client configured for {}", url);

        let builder = qdrant_client::Qdrant::from_url(&url);
        let builder = if let Some(api_key) = api_key.as_deref() {
            builder.api_key(api_key)
        } else {
            builder
        };

        let client = builder.build()?;

        Ok(Self {
            url,
            api_key,
            client,
        })
    }
}

fn qdrant_retry_strategy() -> impl Iterator<Item = Duration> {
    ExponentialBackoff::from_millis(100)
        .max_delay(Duration::from_secs(10))
        .map(jitter)
        .take(5)
}

#[async_trait]
impl VectorDbClient for QdrantClient {
    async fn upsert(&self, req: UpsertRequest<'_>) -> anyhow::Result<()> {
        let collection = req.collection.to_string();
        let point_id = req.id.to_string();
        let vector = qdrant_client::qdrant::Vector::from(req.vector);

        let payload: qdrant_client::Payload = req.payload.into();
        let payload_map: HashMap<String, qdrant_client::qdrant::Value> = payload.into();

        let points = vec![qdrant_client::qdrant::PointStruct {
            id: Some(qdrant_client::qdrant::PointId::from(point_id.clone())),
            vectors: Some(vector.into()),
            payload: payload_map,
        }];

        let client = self.client.clone();
        let col = collection.clone();

        Retry::spawn(qdrant_retry_strategy(), || {
            let client = client.clone();
            let points = points.clone();
            let col = col.clone();
            async move {
                client
                    .upsert_points(qdrant_client::qdrant::UpsertPointsBuilder::new(col, points))
                    .await?;
                Ok::<_, anyhow::Error>(())
            }
        })
        .await?;

        debug!(collection = req.collection, id = %point_id, "upserted point");
        Ok(())
    }

    async fn search(&self, req: SearchRequest<'_>) -> anyhow::Result<Vec<SearchResult>> {
        let filter = req.filter.as_ref().map(build_filter);
        let collection_name = req.collection.to_string();
        let vector = req.vector.to_vec();
        let top_k = req.top_k as u64;

        let client = self.client.clone();

        let results = Retry::spawn(qdrant_retry_strategy(), || {
            let client = client.clone();
            let col = collection_name.clone();
            let vec = vector.clone();
            let filter = filter.clone();
            async move {
                let mut builder = qdrant_client::qdrant::SearchPointsBuilder::new(col, vec, top_k);
                if let Some(f) = filter {
                    builder = builder.filter(f);
                }
                client
                    .search_points(builder)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))
            }
        })
        .await?;

        let search_results = results
            .result
            .into_iter()
            .map(|point| {
                let score = point.score;
                let id = point.id.and_then(point_id_to_string).unwrap_or_default();
                let payload: HashMap<String, serde_json::Value> = point
                    .payload
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect();

                SearchResult { id, score, payload }
            })
            .collect();

        Ok(search_results)
    }

    async fn delete(&self, collection: &str, id: &str) -> anyhow::Result<()> {
        let client = self.client.clone();
        let col = collection.to_string();
        let point_id = point_id_from_str(id);

        Retry::spawn(qdrant_retry_strategy(), || {
            let client = client.clone();
            let col = col.clone();
            let pid = point_id.clone();
            async move {
                client
                    .delete_points(
                        qdrant_client::qdrant::DeletePointsBuilder::new(col)
                            .points(vec![pid])
                            .wait(true),
                    )
                    .await?;
                Ok::<_, anyhow::Error>(())
            }
        })
        .await?;

        debug!(collection = collection, id = %id, "deleted point");
        Ok(())
    }

    async fn delete_by_filter(
        &self,
        collection: &str,
        filter: SearchFilter,
    ) -> anyhow::Result<u64> {
        let qdrant_filter = build_filter(&filter);
        let client = self.client.clone();
        let col = collection.to_string();

        let count_response = Retry::spawn(qdrant_retry_strategy(), || {
            let client = client.clone();
            let col = col.clone();
            let filter = qdrant_filter.clone();
            async move {
                client
                    .count(
                        qdrant_client::qdrant::CountPointsBuilder::new(col)
                            .filter(filter)
                            .exact(true),
                    )
                    .await
                    .map_err(|e| anyhow::anyhow!(e))
            }
        })
        .await?;

        let deleted = count_response.result.map(|r| r.count).unwrap_or(0);
        if deleted == 0 {
            return Ok(0);
        }

        Retry::spawn(qdrant_retry_strategy(), || {
            let client = client.clone();
            let col = col.clone();
            let filter = qdrant_filter.clone();
            async move {
                client
                    .delete_points(
                        qdrant_client::qdrant::DeletePointsBuilder::new(col)
                            .points(filter)
                            .wait(true),
                    )
                    .await?;
                Ok::<_, anyhow::Error>(())
            }
        })
        .await?;

        Ok(deleted)
    }

    async fn ensure_collection(&self, name: &str, dim: usize) -> anyhow::Result<()> {
        let client = self.client.clone();
        let collection_name = name.to_string();

        let exists = Retry::spawn(qdrant_retry_strategy(), || {
            let client = client.clone();
            let name = collection_name.clone();
            async move {
                client
                    .collection_exists(&name)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))
            }
        })
        .await?;

        if !exists {
            info!(collection = name, dim = dim, "creating collection");

            Retry::spawn(qdrant_retry_strategy(), || {
                let client = client.clone();
                let name = collection_name.clone();
                async move {
                    client
                        .create_collection(
                            qdrant_client::qdrant::CreateCollectionBuilder::new(name)
                                .vectors_config(qdrant_client::qdrant::VectorParams {
                                    size: dim as u64,
                                    distance: qdrant_client::qdrant::Distance::Cosine.into(),
                                    ..Default::default()
                                }),
                        )
                        .await?;
                    Ok::<_, anyhow::Error>(())
                }
            })
            .await?;
        }

        Ok(())
    }
}

fn build_filter(filter: &SearchFilter) -> qdrant_client::qdrant::Filter {
    qdrant_client::qdrant::Filter {
        must: filter.must.iter().map(build_condition).collect(),
        should: filter.should.iter().map(build_condition).collect(),
        ..Default::default()
    }
}

fn build_condition(condition: &FilterCondition) -> qdrant_client::qdrant::Condition {
    match condition {
        FilterCondition::Match { key, value } => match value {
            serde_json::Value::Null => qdrant_client::qdrant::Condition::is_null(key.clone()),
            serde_json::Value::String(s) => {
                qdrant_client::qdrant::Condition::matches(key.clone(), s.clone())
            }
            serde_json::Value::Number(n) => {
                if let Some(v) = n.as_i64() {
                    qdrant_client::qdrant::Condition::matches(key.clone(), v)
                } else if let Some(v) = n.as_u64() {
                    match i64::try_from(v) {
                        Ok(v) => qdrant_client::qdrant::Condition::matches(key.clone(), v),
                        Err(_) => {
                            qdrant_client::qdrant::Condition::matches(key.clone(), v.to_string())
                        }
                    }
                } else if let Some(v) = n.as_f64() {
                    qdrant_client::qdrant::Condition::matches(key.clone(), v.to_string())
                } else {
                    qdrant_client::qdrant::Condition::matches(key.clone(), value.to_string())
                }
            }
            serde_json::Value::Bool(b) => {
                qdrant_client::qdrant::Condition::matches(key.clone(), *b)
            }
            serde_json::Value::Array(values) => {
                let string_values: Option<Vec<String>> = values
                    .iter()
                    .map(|value| value.as_str().map(ToOwned::to_owned))
                    .collect();

                match string_values {
                    Some(values) => qdrant_client::qdrant::Condition::matches(key.clone(), values),
                    None => {
                        qdrant_client::qdrant::Condition::matches(key.clone(), value.to_string())
                    }
                }
            }
            serde_json::Value::Object(_) => {
                qdrant_client::qdrant::Condition::matches(key.clone(), value.to_string())
            }
        },
        FilterCondition::Range { key, lt, gt } => {
            let mut range = qdrant_client::qdrant::Range::default();
            if let Some(v) = lt {
                range.lt = Some(*v);
            }
            if let Some(v) = gt {
                range.gt = Some(*v);
            }
            qdrant_client::qdrant::Condition::range(key.clone(), range)
        }
    }
}

fn point_id_to_string(point_id: qdrant_client::qdrant::PointId) -> Option<String> {
    match point_id.point_id_options {
        Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(value)) => {
            Some(value.to_string())
        }
        Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(value)) => Some(value),
        None => None,
    }
}

fn point_id_from_str(id: &str) -> qdrant_client::qdrant::PointId {
    match id.parse::<u64>() {
        Ok(value) => qdrant_client::qdrant::PointId::from(value),
        Err(_) => qdrant_client::qdrant::PointId::from(id),
    }
}

pub(crate) fn session_scope_filter(session_key: &SessionKey) -> SearchFilter {
    SearchFilter {
        must: vec![
            FilterCondition::Match {
                key: "guild_id".to_string(),
                value: json!(session_key.guild_id.map(|g| g.to_string())),
            },
            FilterCondition::Match {
                key: "channel_id".to_string(),
                value: json!(session_key.channel_id.to_string()),
            },
            FilterCondition::Match {
                key: "kind".to_string(),
                value: json!(session_kind_value(&session_key.kind)),
            },
        ],
        should: vec![],
    }
}

pub(crate) fn session_kind_value(kind: &SessionKind) -> &'static str {
    match kind {
        SessionKind::GuildChannel => "guild",
        SessionKind::Thread => "thread",
        SessionKind::DirectMessage => "dm",
    }
}

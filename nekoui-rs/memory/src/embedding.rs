use std::time::Duration;

use async_trait::async_trait;
use rig::{client::EmbeddingsClient as _, embeddings::EmbeddingModel as _, providers::openai};
use tokio_retry::{
    Retry,
    strategy::{ExponentialBackoff, jitter},
};
use tracing::warn;

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Vec<f32>;
    fn dimension(&self) -> usize;
}

pub struct OpenAICompatibleEmbedder {
    model: openai::EmbeddingModel,
    dim: usize,
    fallback: MockEmbedder,
}

impl OpenAICompatibleEmbedder {
    pub fn new(
        base_url: &str,
        api_key: &str,
        model_name: &str,
        dim: usize,
    ) -> anyhow::Result<Self> {
        let client = openai::Client::builder()
            .api_key(api_key)
            .base_url(base_url)
            .build()?;

        let model = client.embedding_model_with_ndims(model_name, dim);

        Ok(Self {
            model,
            dim,
            fallback: MockEmbedder::new(dim),
        })
    }
}

#[async_trait]
impl Embedder for OpenAICompatibleEmbedder {
    async fn embed(&self, text: &str) -> Vec<f32> {
        let strategy = ExponentialBackoff::from_millis(100)
            .max_delay(Duration::from_secs(10))
            .map(jitter)
            .take(5);
        let text = text.to_string();

        match Retry::spawn(strategy, || {
            let model = self.model.clone();
            let text = text.clone();
            async move {
                model
                    .embed_text(&text)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))
            }
        })
        .await
        {
            Ok(embedding) => {
                let mut out = Vec::with_capacity(embedding.vec.len());
                out.extend(embedding.vec.into_iter().map(|value| value as f32));
                out
            }
            Err(error) => {
                warn!(error = %error, "failed to embed text after retries, falling back to mock embedder");
                self.fallback.embed(&text).await
            }
        }
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

pub struct MockEmbedder {
    dim: usize,
}

impl MockEmbedder {
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }
}

#[async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, text: &str) -> Vec<f32> {
        let mut rng = RandSimple(stable_seed(text));
        let mut out = Vec::with_capacity(self.dim);
        for _ in 0 .. self.dim {
            out.push(rng.next_f32() * 2.0 - 1.0);
        }
        out
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

fn stable_seed(text: &str) -> u64 {
    let mut hash = 1469598103934665603u64;
    for &byte in text.as_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

struct RandSimple(u64);

impl RandSimple {
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        (self.0 >> 16) as f32 / 65536.0
    }
}

use axum::{
    extract::State,
    response::{sse::Event, Sse},
    routing::get,
    Router,
};
use tokio_stream::wrappers::BroadcastStream;
use tracing::error;

use super::AppState;

// ── SSE Handler ───────────────────────────────────────────────────────────────

/// GET /api/events - Server-Sent Events endpoint
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.http_state.agent.event_bus().subscribe();
    let stream = tokio_stream::StreamExt::filter_map(BroadcastStream::new(rx), |result| {
        match result {
            Ok(event) => match serde_json::to_string(&event) {
                Ok(json) => Some(Ok(Event::default().data(json))),
                Err(e) => {
                    error!(target: "http_server", error = %e, "failed to serialize event");
                    None
                }
            },
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                tracing::warn!(target: "http_server", skipped = n, "SSE client lagged");
                None
            }
        }
    });
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    )
}

// ── Metrics Handler ──────────────────────────────────────────────────────────

/// GET /api/metrics - Prometheus metrics endpoint
pub async fn metrics_handler(State(state): State<AppState>) -> String {
    state.http_state.agent.metrics().collect_prometheus()
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/events", get(sse_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}

use axum::{Router, response::IntoResponse, routing::get};
use nekoui_agent::session::Message;

use crate::{
    response::ApiResponse,
    routes::{AppState, extractor::ResolvedSession},
};

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_messages))
}

pub async fn get_messages(resolved: ResolvedSession) -> impl IntoResponse {
    let guard = resolved.session.lock().await;
    let messages: Vec<Message> = guard.messages.iter().cloned().collect();
    ApiResponse::success(messages)
}

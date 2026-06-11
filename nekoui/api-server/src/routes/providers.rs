use axum::Router;

use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
}

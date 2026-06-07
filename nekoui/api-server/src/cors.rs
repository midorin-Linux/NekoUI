use axum::http::Method;
use nekoui_config::server::ServerConfig;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

pub fn build_cors_layer(config: &ServerConfig) -> CorsLayer {
    let origins: Vec<_> = config
        .allowed_origins
        .iter()
        .map(|o| {
            o.parse::<axum::http::HeaderValue>()
                .expect("invalid allowed origin")
        })
        .collect();
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers(Any)
        .allow_origin(AllowOrigin::list(origins))
}

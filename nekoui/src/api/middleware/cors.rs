use axum::http::Method;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

pub fn build_cors_layer(allow_origin: &[String]) -> CorsLayer {
    let origins: Vec<_> = allow_origin
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

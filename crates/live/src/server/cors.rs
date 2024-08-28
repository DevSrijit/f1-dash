use axum::http::{HeaderValue, Method};
use tower_http::cors::CorsLayer;

pub fn init() -> CorsLayer {
    let origins = std::env::var("ORIGIN").expect("no origin env found");
    let origins: Vec<HeaderValue> = origins
        .split(',')
        .map(|origin| origin.trim().parse::<HeaderValue>().unwrap())
        .collect();

    let mut cors = CorsLayer::new();
    for origin in origins {
        cors = cors.allow_origin(origin);
    }

    cors.allow_methods([Method::GET, Method::CONNECT])
}
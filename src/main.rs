mod event;

use std::{path::PathBuf, sync::Arc};

use axum::{
    Router,
    http::Method,
    routing::{get, get_service, post},
};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeFile,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::event::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4000")
        .await
        .unwrap();
    let app = app();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

fn app() -> Router {
    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let static_files_service = ServeFile::new(assets_dir.clone().join("index.html"));
    let fallback_service = ServeFile::new(assets_dir.clone().join("fallback.html"));

    let app_state = Arc::new(AppState::new());

    // ref: https://dev.to/amaendeepm/axum-in-rus-flexibility-cors-control-and-tower-power-4ich
    let cors_layer = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST]);

    return Router::new()
        .route("/events", get(event::subscribe))
        .route("/events/send", post(event::send))
        .route("/", get_service(static_files_service))
        .fallback_service(fallback_service)
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer)
        .with_state(app_state);
}

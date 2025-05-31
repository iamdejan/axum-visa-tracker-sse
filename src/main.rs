use std::path::PathBuf;

use axum::{Router, routing::get_service};
use tower_http::{services::ServeFile, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
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
    return Router::new()
        .route("/", get_service(static_files_service))
        .fallback_service(fallback_service)
        .layer(TraceLayer::new_for_http());
}

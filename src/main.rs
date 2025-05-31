mod send_event;

use std::{convert::Infallible, path::PathBuf};

use axum::{
    Router,
    extract::State,
    response::{Sse, sse::Event},
    routing::{get, get_service, post},
};
use axum_extra::TypedHeader;
use futures_util::stream::Stream;
use tokio::sync::broadcast;
use tower_http::{services::ServeFile, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

type AppEvent = String;

#[derive(Clone)]
pub struct AppState {
    tx: broadcast::Sender<AppEvent>,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(800);
        return Self { tx: tx };
    }
}

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

    let app_state = AppState::new();

    return Router::new()
        .route("/events", get(sse_handler))
        .route("/events/send", post(send_event::handler))
        .route("/", get_service(static_files_service))
        .fallback_service(fallback_service)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);
}

async fn sse_handler(
    State(state): State<AppState>,
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    tracing::debug!("{} connected", user_agent.as_str());

    let mut rx = state.tx.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    let event = Event::default().data(msg);
                    yield Ok(event);
                }
                Err(err) => {
                    tracing::error!("Error: {}", err);
                    break;
                }
            }
        }
    };

    return Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default());
}

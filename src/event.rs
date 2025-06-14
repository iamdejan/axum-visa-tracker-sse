use axum::{
    extract::State, http::StatusCode, response::{sse::Event, Sse}, Json
};
use tokio::sync::broadcast;
use axum_extra::TypedHeader;
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppEvent {
    percentage: f64,
}


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

pub async fn send(
    State(state): State<AppState>,
    Json(payload): Json<AppEvent>,
) -> (StatusCode, String) {
    let percentage = payload.percentage;
    if percentage < 0.0 || percentage > 100.0 {
        return (StatusCode::BAD_REQUEST, "Percentage exceeds limit".to_string());
    }

    match state.tx.send(payload.clone()) {
        Ok(num_receivers) => {
            let response_msg = format!("Event sent to {} listeners!", num_receivers);
            return (StatusCode::OK, response_msg);
        }
        Err(_) => {
            let response_msg = "Event accepted, but no listeners".to_string();
            return (StatusCode::ACCEPTED, response_msg);
        }
    }
}


pub async fn subscribe(
    State(state): State<AppState>,
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
) -> Sse<impl Stream<Item = Result<Event, serde_json::Error>>> {
    tracing::debug!("{} connected", user_agent.as_str());

    let mut rx = state.tx.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    let json_data = serde_json::to_string(&msg)?;
                    let event = Event::default().data(json_data.as_str());
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

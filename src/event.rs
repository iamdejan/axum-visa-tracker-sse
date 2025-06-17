use std::sync::Arc;

use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Sse, sse::Event},
};
use axum_extra::{TypedHeader, extract::WithRejection};
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

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

#[derive(Serialize, Debug)]
pub struct EventResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<EventData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorDetail>,
}

#[derive(Serialize, Debug)]
pub struct EventData {
    message: String,
}

#[derive(Serialize, Debug)]
pub struct ErrorDetail {
    code: String,
    message: String,
}

#[derive(Debug)]
pub struct AppError {
    error: ErrorDetail,
    status_code: StatusCode,
}

impl From<JsonRejection> for AppError {
    fn from(value: JsonRejection) -> Self {
        match value {
            JsonRejection::MissingJsonContentType(missing_json_content_type) => AppError {
                error: ErrorDetail {
                    code: "MISSING_JSON_CONTENT_TYPE".to_string(),
                    message: missing_json_content_type.to_string(),
                },
                status_code: StatusCode::BAD_REQUEST,
            },
            JsonRejection::JsonDataError(json_data_error) => AppError {
                error: ErrorDetail {
                    code: "JSON_DESERIALIZATION_ERROR".to_string(),
                    message: json_data_error.body_text(),
                },
                status_code: StatusCode::BAD_REQUEST,
            },
            JsonRejection::JsonSyntaxError(json_syntax_error) => AppError {
                error: ErrorDetail {
                    code: "JSON_VALIDITY_ERROR".to_string(),
                    message: json_syntax_error.body_text(),
                },
                status_code: StatusCode::BAD_REQUEST,
            },
            JsonRejection::BytesRejection(bytes_rejection) => AppError {
                error: ErrorDetail {
                    code: "BUFFER_ERROR".to_string(),
                    message: bytes_rejection.body_text(),
                },
                status_code: StatusCode::BAD_REQUEST,
            },
            _ => AppError {
                error: ErrorDetail {
                    code: "UNKNOWN_ERROR".to_string(),
                    message: "An unexpected error occured".to_string(),
                },
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
            },
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let response = EventResponse {
            data: None,
            error: Some(self.error),
        };
        return (self.status_code, Json(response)).into_response();
    }
}

#[axum::debug_handler]
pub async fn send(
    State(state): State<Arc<AppState>>,
    WithRejection(Json(payload), _): WithRejection<Json<AppEvent>, AppError>,
) -> (StatusCode, Json<EventResponse>) {
    let percentage = payload.percentage;
    if percentage < 0.0 || percentage > 100.0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(EventResponse {
                data: None,
                error: Some(ErrorDetail {
                    code: "RANGE_EXCEEDED_ERROR".to_string(),
                    message: format!(
                        "Percentage range is exceeded. It should be within 0-100, but got {}",
                        percentage
                    )
                    .to_string(),
                }),
            }),
        );
    }

    match state.tx.send(payload.clone()) {
        Ok(num_receivers) => {
            let response_msg = format!("Event sent to {} listeners!", num_receivers);
            return (
                StatusCode::OK,
                Json(EventResponse {
                    data: Some(EventData {
                        message: response_msg,
                    }),
                    error: None,
                }),
            );
        }
        Err(_) => {
            let response_msg = "Event accepted, but no listeners".to_string();
            return (
                StatusCode::ACCEPTED,
                Json(EventResponse {
                    data: Some(EventData {
                        message: response_msg,
                    }),
                    error: None,
                }),
            );
        }
    }
}

pub async fn subscribe(
    State(state): State<Arc<AppState>>,
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    tracing::debug!("{} connected", user_agent.as_str());

    let mut rx = state.tx.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    let event = Event::default().json_data(msg)?;
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

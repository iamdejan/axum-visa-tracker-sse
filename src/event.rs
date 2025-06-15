use axum::{
    extract::{rejection::JsonRejection, State}, http::StatusCode, response::{sse::Event, IntoResponse, Sse}, Json
};
use tokio::sync::broadcast;
use axum_extra::{extract::WithRejection, TypedHeader};
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

#[derive(Serialize, Debug)]
pub struct ErrorResponse {
    error: ErrorDetail
}

#[derive(Serialize, Debug)]
pub struct ErrorDetail {
    code: String,
    message: String
}

#[derive(Debug)]
pub enum AppError {
    Json(JsonRejection)
}

impl From<JsonRejection> for AppError {
    fn from(value: JsonRejection) -> Self {
        return AppError::Json(value);
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_detail) = match self {
            AppError::Json(rejection) => {
                let (status, message, code) = match rejection {
                    JsonRejection::MissingJsonContentType(_) => {
                        (
                            StatusCode::BAD_REQUEST,
                            "Missing or invalid Content-Type header. Expected 'application/json'".to_string(),
                            "MISSING_JSON_CONTENT_TYPE".to_string(),
                        )
                    },
                    JsonRejection::JsonDataError(json_data_error) => {
                        (
                            StatusCode::BAD_REQUEST,
                            json_data_error.body_text(),
                            "JSON_DESERIALIZATION_ERROR".to_string(),
                        )
                    },
                    JsonRejection::JsonSyntaxError(json_syntax_error) => {
                        (
                            StatusCode::BAD_REQUEST,
                            json_syntax_error.body_text(),
                            "JSON_VALIDITY_ERROR".to_string(),
                        )
                    },
                    JsonRejection::BytesRejection(bytes_rejection) => {
                        (
                            StatusCode::BAD_REQUEST,
                            bytes_rejection.body_text(),
                            "BUFFER_ERROR".to_string(),
                        )
                    },
                    _ => {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "An unexpected error occured".to_string(),
                            "UNKNOWN_ERROR".to_string()
                        )
                    },
                };

                (status, ErrorDetail { code: code, message: message })
            }
        };

        let error_respnose = ErrorResponse {
            error: error_detail
        };

        return (status, Json(error_respnose)).into_response();
    }
}

pub async fn send(
    State(state): State<AppState>,
    WithRejection(Json(payload), _): WithRejection<Json<AppEvent>, AppError>,
) -> (StatusCode, String) {
    let percentage = payload.percentage;
    if percentage < 0.0 || percentage > 100.0 {
        return (StatusCode::BAD_REQUEST, String::from("Percentage exceeds limit"));
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

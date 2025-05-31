use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use serde::Deserialize;

use crate::AppState;

#[derive(Deserialize, Debug)]
pub struct PostEventPayload {
    message: String,
}

pub async fn handler(
    State(state): State<AppState>,
    Json(payload): Json<PostEventPayload>,
) -> (StatusCode, String) {
    match state.tx.send(payload.message.clone()) {
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

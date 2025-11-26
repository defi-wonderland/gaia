// HTTP request handlers
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use base64::{engine::general_purpose, Engine as _};
use prost::Message;
use serde_json;
use tracing::{error, info};
use uuid::Uuid;
use wire::pb::grc20::Edit;

use crate::config::get_default_space_id;
use crate::models::{CacheRequest, EditMessage};
use crate::server::state::AppState;

/// Health check endpoint
pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Combined server is running")
}

/// Cache endpoint - receives base64-encoded Edit protobuf and processes it
pub async fn cache_handler(
    State(state): State<AppState>,
    Json(payload): Json<CacheRequest>,
) -> impl IntoResponse {
    info!(
        "Received cache request with {} base64 characters",
        payload.data.len()
    );

    // Decode base64 string to bytes
    let bytes = match general_purpose::STANDARD.decode(&payload.data) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to decode base64 string: {:?}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to decode base64: {}", e)
                })),
            );
        }
    };

    info!("Decoded {} bytes from base64", bytes.len());

    // Decode the protobuf bytes into an Edit
    let edit = match Edit::decode(&bytes[..]) {
        Ok(edit) => edit,
        Err(e) => {
            error!("Failed to decode Edit from protobuf bytes: {:?}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to decode Edit: {}", e)
                })),
            );
        }
    };

    info!("Successfully decoded Edit: {:?}", edit.name);

    // Send edit to decoder via channel
    let message = EditMessage {
        edit: edit.clone(),
        space_id: get_default_space_id(),
        content_uri: format!("ipfs://decoded_{}", Uuid::new_v4()),
    };

    match state.edit_sender.send(message) {
        Ok(receiver_count) => {
            let queued = state.edit_sender.len();
            info!(
                "Edit sent to {} decoder(s), {} messages queued in channel",
                receiver_count, queued
            );
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "status": "success",
                    "message": format!("Edit sent to decoder ({} receivers, {} queued)", receiver_count, queued)
                })),
            )
        }
        Err(e) => {
            error!("Failed to send edit to decoder: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to send edit to decoder: {}", e)
                })),
            )
        }
    }
}

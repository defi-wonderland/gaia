// Shared data types
use serde::Deserialize;
use uuid::Uuid;
use wire::pb::grc20::Edit;

/// Message type to send through channel between HTTP server and decoder
#[derive(Debug, Clone)]
pub struct EditMessage {
    pub edit: Edit,
    pub space_id: Uuid,
    pub content_uri: String,
}

/// HTTP request payload for cache endpoint
#[derive(Debug, Deserialize)]
pub struct CacheRequest {
    pub data: String, // Base64-encoded protobuf bytes
}


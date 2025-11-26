// Configuration constants and environment helpers
use axum::http::{HeaderValue, Method};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

// Property UUIDs - centralized configuration
pub const NAME_PROPERTY_ID: &str = "b5a02028-eb65-4e22-9c48-7cb579a67440";
pub const NAME_PROPERTY_TYPE: &str = "String";

pub const RANK_TYPE_PROPERTY_ID: &str = "bafdbdc9-2576-40e7-beca-06bdbf99a198";
pub const RANK_TYPE_PROPERTY_TYPE: &str = "String";

pub const RANKS_PROPERTY_ID: &str = "b51f6fc8-556a-40d0-ba87-616a92a626ef";
pub const RANKS_PROPERTY_TYPE: &str = "Relation";

pub const SCORE_PROPERTY_ID: &str = "665d731a-ee6f-469d-81d2-11da727ca2cf";
pub const SCORE_PROPERTY_TYPE: &str = "Number";

pub const OTHER_PROPERTY_ID: &str = "a126ca53-0c8e-48d5-b888-82c734c38935";
pub const OTHER_PROPERTY_TYPE: &str = "String";

// Default Space ID
pub const DEFAULT_SPACE_ID: &str = "249195fe-9af9-4b62-be71-454800967cfe";

// Server configuration
pub const SERVER_HOST: [u8; 4] = [127, 0, 0, 1];
pub const SERVER_PORT: u16 = 8080;

/// Get all properties as tuples for PropertiesCache initialization
pub fn get_property_tuples() -> Vec<(String, String)> {
    vec![
        (NAME_PROPERTY_ID.to_string(), NAME_PROPERTY_TYPE.to_string()),
        (RANK_TYPE_PROPERTY_ID.to_string(), RANK_TYPE_PROPERTY_TYPE.to_string()),
        (RANKS_PROPERTY_ID.to_string(), RANKS_PROPERTY_TYPE.to_string()),
        (SCORE_PROPERTY_ID.to_string(), SCORE_PROPERTY_TYPE.to_string()),
        (OTHER_PROPERTY_ID.to_string(), OTHER_PROPERTY_TYPE.to_string()),
    ]
}

/// Get default space ID as Uuid
pub fn get_default_space_id() -> Uuid {
    Uuid::parse_str(DEFAULT_SPACE_ID).expect("Invalid default space ID")
}

/// Create CORS layer for localhost development
pub fn create_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin([
            "http://localhost:3000".parse::<HeaderValue>().unwrap(),
            "http://localhost:5173".parse::<HeaderValue>().unwrap(), // Vite default
            "http://127.0.0.1:3000".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:5173".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([axum::http::header::CONTENT_TYPE])
}

/// Check if Neo4j is enabled via environment variable
pub fn is_neo4j_enabled() -> bool {
    std::env::var("ENABLE_NEO4J")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true"
}

/// Get Neo4j URI from environment
pub fn get_neo4j_uri() -> Option<String> {
    std::env::var("NEO4J_URI").ok()
}

/// Get database URL from environment
pub fn get_database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set")
}


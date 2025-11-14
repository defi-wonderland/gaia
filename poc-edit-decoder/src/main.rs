use axum::{
    extract::State,
    http::{StatusCode, Method, HeaderValue},
    response::IntoResponse,
    routing::{get, post},
    Json,
    Router,
};
use tower_http::cors::CorsLayer;
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tracing::{info, error};
use wire::pb::grc20::Edit;
use uuid::Uuid;
use serde::Deserialize;
use serde_json;
use std::env;
use prost::Message;
use base64::{engine::general_purpose, Engine as _};
use indexer::{cache::PreprocessedEdit, KgData, KgIndexer, PostgresStorage, PostgresCache, PropertiesCache};
use stream::{
    pb::sf::substreams::{
        rpc::v2::BlockScopedData,
        v1::Clock,
    },
    utils::BlockMetadata,
    PreprocessedSink,
};
use sqlx::{postgres::PgPoolOptions, Postgres};

use chrono::{DateTime, Utc};

// Message type to send through channel
#[derive(Debug, Clone)]
pub struct EditMessage {
    pub edit: Edit,
    pub space_id: Uuid,
    pub content_uri: String,
}

// Shared application state
#[derive(Clone)]
struct AppState {
    edit_sender: broadcast::Sender<EditMessage>,
}

struct PostgresPool {
    pool: sqlx::Pool<Postgres>,
}

impl PostgresPool {
    pub async fn new(database_url: &String) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url.as_str())
            .await?;
        Ok(PostgresPool { pool })
    }

    pub fn get_pool(&self) -> &sqlx::Pool<Postgres> {
        &self.pool
    }
}
#[derive(Debug, Deserialize)]
pub struct CacheRequest {
    pub data: String, // Base64-encoded protobuf bytes
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Combined server is running")
}

async fn cache_handler(
    State(state): State<AppState>,
    Json(payload): Json<CacheRequest>,
) -> impl IntoResponse {
    info!("Received cache request with {} base64 characters", payload.data.len());
    
    // Decode base64 string to bytes
    let bytes = match general_purpose::STANDARD.decode(&payload.data) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to decode base64 string: {:?}", e);
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "status": "error",
                "message": format!("Failed to decode base64: {}", e)
            })));
        }
    };
    
    info!("Decoded {} bytes from base64", bytes.len());
    
    // Decode the protobuf bytes into an Edit
    let edit = match Edit::decode(&bytes[..]) {
        Ok(edit) => edit,
        Err(e) => {
            error!("Failed to decode Edit from protobuf bytes: {:?}", e);
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "status": "error",
                "message": format!("Failed to decode Edit: {}", e)
            })));
        }
    };
    
    info!("Successfully decoded Edit: {:?}", edit.name);
    
    // Send edit to decoder via channel
    let message = EditMessage {
        edit: edit.clone(),
        space_id: Uuid::parse_str("8ef40bdd-cf69-4ad7-a9a1-f71c15653994").unwrap(), // Replace with actual space_id
        content_uri: format!("ipfs://decoded_{}", uuid::Uuid::new_v4()),
    };
    
    match state.edit_sender.send(message) {
        Ok(receiver_count) => {
            info!("Edit sent to {} decoder(s)", receiver_count);
            (StatusCode::CREATED, Json(serde_json::json!({
                "status": "success",
                "message": format!("Edit sent to decoder ({} receivers)", receiver_count)
            })))
        }
        Err(e) => {
            error!("Failed to send edit to decoder: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "status": "error",
                "message": format!("Failed to send edit to decoder: {}", e)
            })))
        }
    }
}

// Decoder task that processes incoming edits
async fn decoder_task(mut edit_receiver: broadcast::Receiver<EditMessage>) {
    info!("Decoder task started, waiting for edits...");
    
    // Initialize database connection and caches
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let storage = match PostgresStorage::new(&database_url).await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to initialize storage: {:?}", e);
            return;
        }
    };
    
    let cache = match PostgresCache::new().await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to initialize cache: {:?}", e);
            return;
        }
    };

    let name_property_tuple = ("b5a02028-eb65-4e22-9c48-7cb579a67440".to_string(), "String".to_string());
    let rank_type_property_tuple = ("bafdbdc9-2576-40e7-beca-06bdbf99a198".to_string(), "String".to_string());
    let ranks_property_tuple = ("b51f6fc8-556a-40d0-ba87-616a92a626ef".to_string(), "Relation".to_string());
    let score_property_tuple = ("665d731a-ee6f-469d-81d2-11da727ca2cf".to_string(), "Number".to_string());

    let properties = vec![
        name_property_tuple,
        rank_type_property_tuple,
        ranks_property_tuple,
        score_property_tuple,
    ];
    
    let properties_cache = match PropertiesCache::from_tuples(&properties) {
        Ok(pc) => pc,
        Err(e) => {
            error!("Failed to initialize properties cache: {:?}", e);
            return;
        }
    };
    
    let kg_indexer = KgIndexer::new(storage, cache, properties_cache);
    info!("KgIndexer initialized successfully");
    
    // Process incoming edits
    while let Ok(message) = edit_receiver.recv().await {
        info!("Decoder received edit: {}", message.edit.name);
        
        let preprocessed_edit = PreprocessedEdit {
            space_id: message.space_id,
            edit: Some(message.edit),
            is_errored: false,
            cid: message.content_uri.clone(),
        };

        let now: DateTime<Utc> = Utc::now();
        let rfc3339_string = now.to_rfc3339();
        
        let kg_data = KgData {
            block: BlockMetadata {
                cursor: "".to_string(),
                block_number: 0,
                timestamp: rfc3339_string,
            },
            edits: vec![preprocessed_edit],
            spaces: vec![],
            added_editors: vec![],
            added_members: vec![],
            removed_editors: vec![],
            removed_members: vec![],
            added_subspaces: vec![],
            removed_subspaces: vec![],
            executed_proposals: vec![],
            created_proposals: vec![],
        };
        
        let result = kg_indexer.process_block_scoped_data(&BlockScopedData {
            output: None,
            clock: Some(Clock {
                id: "".to_string(),
                number: 0,
                timestamp: None,
            }),
            cursor: "".to_string(),
            final_block_height: 0,
            debug_map_outputs: vec![],
            debug_store_outputs: vec![],
        }, kg_data).await;
        
        match result {
            Ok(_) => info!("Successfully processed edit: {}", message.content_uri),
            Err(e) => error!("Failed to process edit {}: {:?}", message.content_uri, e),
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .compact()
        .init();

    // Create broadcast channel for edit messages
    let (edit_sender, edit_receiver) = broadcast::channel::<EditMessage>(100);
    
    // Spawn decoder task
    tokio::spawn(decoder_task(edit_receiver));
    
    // Create app state
    let state = AppState { edit_sender };
    
    // Configure CORS for localhost
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:3000".parse::<HeaderValue>().unwrap(),
            "http://localhost:5173".parse::<HeaderValue>().unwrap(), // Vite default
            "http://127.0.0.1:3000".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:5173".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([axum::http::header::CONTENT_TYPE]);
    
    // Build router
    let app = Router::new()
        .route("/cache", post(cache_handler))
        .route("/health", get(health_check))
        .layer(cors)
        .with_state(state);
    
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    info!("Combined server listening on {}", addr);
    info!("- Cache endpoint: http://{}/cache", addr);
    info!("- Health endpoint: http://{}/health", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    tokio::signal::ctrl_c().await.unwrap();
    info!("Shutting down...");
}


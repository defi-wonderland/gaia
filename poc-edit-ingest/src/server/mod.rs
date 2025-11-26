// Server module - HTTP server setup and routing
pub mod handlers;
pub mod state;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tracing::info;

use crate::config::create_cors_layer;
use crate::models::EditMessage;
use self::state::AppState;

/// Create the Axum application router with all routes and middleware
pub fn create_app(edit_sender: broadcast::Sender<EditMessage>) -> Router {
    let state = AppState { edit_sender };

    Router::new()
        .route("/cache", post(handlers::cache_handler))
        .route("/health", get(handlers::health_check))
        .layer(create_cors_layer())
        .with_state(state)
}

/// Run the server on the specified address
pub async fn run_server(app: Router, addr: SocketAddr) -> anyhow::Result<()> {
    info!("Server listening on {}", addr);
    info!("- Cache endpoint: http://{}/cache", addr);
    info!("- Health endpoint: http://{}/health", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

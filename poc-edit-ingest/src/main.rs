use poc_edit_ingest::{
    config::{SERVER_HOST, SERVER_PORT},
    decoder,
    models::EditMessage,
    server,
};
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize environment and logging
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .compact()
        .init();

    info!("Starting edit decoder server...");

    // Create broadcast channel for edit messages
    let (edit_sender, edit_receiver) = broadcast::channel::<EditMessage>(100);

    // Spawn decoder background task
    tokio::spawn(decoder::decoder_task(edit_receiver));

    // Create and run server
    let app = server::create_app(edit_sender);
    let addr = SocketAddr::from((SERVER_HOST, SERVER_PORT));

    if let Err(e) = server::run_server(app, addr).await {
        eprintln!("Server error: {:?}", e);
        std::process::exit(1);
    }
}

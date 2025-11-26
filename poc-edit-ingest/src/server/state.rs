// App state for Axum server
use tokio::sync::broadcast;
use crate::models::EditMessage;

#[derive(Clone)]
pub struct AppState {
    pub edit_sender: broadcast::Sender<EditMessage>,
}


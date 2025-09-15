use actions_indexer::{Dependencies, IndexingError};
use dotenv::dotenv;
use actions_indexer_pipeline::orchestrator::Orchestrator;

/// Main entry point for the Actions Indexer application.
///
/// Initializes dotenv, sets up application dependencies, and starts the
/// orchestrator to process actions.
///
/// # Returns
///
/// A `Result` indicating success or an `IndexingError` if an
/// error occurs during initialization or execution.
#[tokio::main]
async fn main() -> Result<(), IndexingError> {
    dotenv().ok();

    let dependencies = Dependencies::new().await?;

    let orchestrator = Orchestrator::new(
        dependencies.consumer,
        dependencies.processor,
        dependencies.loader,
    );
    orchestrator.run().await?;
    Ok(())
}

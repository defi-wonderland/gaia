mod config;
mod errors;

use crate::config::Dependencies;

use dotenv::dotenv;

use crate::errors::IndexingError;
use actions_indexer_pipeline::orchestrator::Orchestrator;

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

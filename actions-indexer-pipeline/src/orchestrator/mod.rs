use crate::{consumer::ConsumeActions, loader::ActionsRepository, processor::ProcessActions};
use crate::errors::OrchestratorError;

pub struct Orchestrator<DB: sqlx::Database> {
    pub actions_consumer: Box<dyn ConsumeActions>,
    pub actions_processor: Box<dyn ProcessActions>,
    pub actions_loader: Box<dyn ActionsRepository<DB>>,
}

impl<DB: sqlx::Database> Orchestrator<DB> {
    pub fn new(
        actions_consumer: Box<dyn ConsumeActions>,
        actions_processor: Box<dyn ProcessActions>,
        actions_loader: Box<dyn ActionsRepository<DB>>,
    ) -> Self {
        Self {
            actions_consumer,
            actions_processor,
            actions_loader,
        }
    }

    pub async fn run(&self) -> Result<(), OrchestratorError> {
        // TODO: Implement
        Ok(())
    }
}
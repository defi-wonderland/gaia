use cache::PreprocessedEdit;
use stream::utils::BlockMetadata;
use uuid::Uuid;

pub mod block_handler;
pub mod cache;
pub mod error;
pub mod models;
pub mod preprocess;
pub mod storage;
pub mod validators;

pub mod test_utils;

// Re-export commonly used types
pub use cache::{postgres::PostgresCache, properties_cache::PropertiesCache, CacheBackend, CacheError};
pub use storage::postgres::PostgresStorage;

use std::sync::Arc;
use stream::{pb::sf::substreams::rpc::v2::BlockScopedData, PreprocessedSink};
use tracing::{info, instrument};

pub struct KgIndexer {
    storage: Arc<PostgresStorage>,
    ipfs_cache: Arc<PostgresCache>,
    properties_cache: Arc<PropertiesCache>,
}

impl KgIndexer {
    pub fn new(
        storage: PostgresStorage,
        ipfs_cache: PostgresCache,
        properties_cache: PropertiesCache,
    ) -> Self {
        KgIndexer {
            storage: Arc::new(storage),
            ipfs_cache: Arc::new(ipfs_cache),
            properties_cache: Arc::new(properties_cache),
        }
    }
}

impl PreprocessedSink<KgData> for KgIndexer {
    type Error = error::IndexingError;

    #[instrument(skip(self), name = "load_cursor")]
    async fn load_persisted_cursor(&self) -> Result<Option<String>, Self::Error> {
        self.storage
            .load_cursor("kg_indexer")
            .await
            .map_err(error::IndexingError::from)
    }

    #[instrument(skip(self), fields(block = block))]
    async fn persist_cursor(&self, cursor: String, block: u64) -> Result<(), Self::Error> {
        info!(cursor = %cursor, block = block, "Persisting cursor");
        self.storage
            .persist_cursor("kg_indexer", &cursor, &block)
            .await
            .map_err(error::IndexingError::from)
    }

    #[instrument(skip_all, fields(block_number = block_data.clock.as_ref().map(|c| c.number).unwrap_or(0)))]
    async fn preprocess_block_scoped_data(
        &self,
        block_data: &BlockScopedData,
    ) -> Result<KgData, Self::Error> {
        let kg_data =
            preprocess::preprocess_block_scoped_data(block_data, &self.ipfs_cache).await?;

        Ok(kg_data)
    }

    #[instrument(skip_all, fields(
        block_number = decoded_data.block.block_number,
        block_timestamp = decoded_data.block.timestamp,
        edit_count = decoded_data.edits.len(),
        space_count = decoded_data.spaces.len()
    ))]
    async fn process_block_scoped_data(
        &self,
        _block_data: &BlockScopedData,
        decoded_data: KgData,
    ) -> Result<(), Self::Error> {
        info!(
            edit_count = decoded_data.edits.len(),
            space_count = decoded_data.spaces.len(),
            member_count = decoded_data.added_members.len(),
            "Processing block data"
        );

        block_handler::root_handler::run(
            &decoded_data,
            &decoded_data.block,
            &self.storage,
            &self.properties_cache,
        )
        .await?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct PersonalSpace {
    pub dao_address: String,
    pub space_address: String,
    pub personal_plugin: String,
}

#[derive(Clone, Debug)]
pub struct PublicSpace {
    pub dao_address: String,
    pub space_address: String,
    pub membership_plugin: String,
    pub governance_plugin: String,
}

#[derive(Clone, Debug)]
pub enum CreatedSpace {
    Personal(PersonalSpace),
    Public(PublicSpace),
}

#[derive(Clone, Debug)]
pub struct AddedMember {
    pub dao_address: String,
    pub editor_address: String,
}

#[derive(Clone, Debug)]
pub struct RemovedMember {
    pub dao_address: String,
    pub editor_address: String,
}

#[derive(Clone, Debug)]
pub struct AddedSubspace {
    pub dao_address: String,
    pub subspace_address: String,
}

#[derive(Clone, Debug)]
pub struct RemovedSubspace {
    pub dao_address: String,
    pub subspace_address: String,
}

#[derive(Clone, Debug)]
pub struct ExecutedProposal {
    pub proposal_id: String,
    pub plugin_address: String,
}

#[derive(Clone, Debug)]
pub enum ProposalCreated {
    PublishEdit {
        proposal_id: Uuid,
        creator: String,
        start_time: String,
        end_time: String,
        content_uri: String,
        dao_address: String,
        plugin_address: String,
        edit_id: Option<Uuid>, // ID from the cached Edit
    },
    AddMember {
        proposal_id: Uuid,
        creator: String,
        start_time: String,
        end_time: String,
        member: String,
        dao_address: String,
        plugin_address: String,
        change_type: String,
    },
    RemoveMember {
        proposal_id: Uuid,
        creator: String,
        start_time: String,
        end_time: String,
        member: String,
        dao_address: String,
        plugin_address: String,
        change_type: String,
    },
    AddEditor {
        proposal_id: Uuid,
        creator: String,
        start_time: String,
        end_time: String,
        editor: String,
        dao_address: String,
        plugin_address: String,
        change_type: String,
    },
    RemoveEditor {
        proposal_id: Uuid,
        creator: String,
        start_time: String,
        end_time: String,
        editor: String,
        dao_address: String,
        plugin_address: String,
        change_type: String,
    },
    AddSubspace {
        proposal_id: Uuid,
        creator: String,
        start_time: String,
        end_time: String,
        subspace: String,
        dao_address: String,
        plugin_address: String,
        change_type: String,
    },
    RemoveSubspace {
        proposal_id: Uuid,
        creator: String,
        start_time: String,
        end_time: String,
        subspace: String,
        dao_address: String,
        plugin_address: String,
        change_type: String,
    },
}

#[derive(Clone, Debug)]
pub struct KgData {
    pub block: BlockMetadata,
    pub edits: Vec<PreprocessedEdit>,
    pub added_editors: Vec<AddedMember>,
    pub removed_editors: Vec<RemovedMember>,
    pub added_members: Vec<AddedMember>,
    pub removed_members: Vec<RemovedMember>,
    pub added_subspaces: Vec<AddedSubspace>,
    pub removed_subspaces: Vec<RemovedSubspace>,
    // Note for now that we only need the dao address. Eventually we'll
    // index the plugin addresses as well.
    pub spaces: Vec<CreatedSpace>,
    pub executed_proposals: Vec<ExecutedProposal>,
    pub created_proposals: Vec<ProposalCreated>,
}

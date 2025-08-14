use serde::{Deserialize, Serialize};
use crate::types::{EntityId, SpaceAddress};

/// Represents the aggregated vote counts for an entity and space.
///
/// This struct is intended to store the total number of upvotes and 
/// downvotes for a particular entity and space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotesCount {
    pub entity_id: EntityId,
    pub space_id: SpaceAddress,
    pub upvotes: i64,
    pub downvotes: i64,
}

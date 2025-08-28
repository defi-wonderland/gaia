use serde::{Deserialize, Serialize};
use crate::types::{EntityId, SpaceId, UserAddress, VoteValue};

/// Represents a user's vote on an entity and space.
///
/// This struct is intended to store information about a user's vote
/// on a specific entity and space.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserVote {
    pub user_id: UserAddress,
    pub entity_id: EntityId,
    pub space_id: SpaceId,
    pub vote_type: VoteValue,
    pub voted_at: u64,
}
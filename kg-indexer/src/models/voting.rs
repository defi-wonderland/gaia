use uuid::Uuid;

/// Vote direction (matches VoteDirection from hermes-schema)
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum VoteValue {
    Up,
    Down,
    Remove,
}

/// Type of object being voted on
#[derive(Clone, Debug, PartialEq, Eq, Copy, Hash)]
pub enum VoteObjectType {
    Entity,
    Relation,
}

/// Processed vote from HermesVoteCast
#[derive(Clone, Debug)]
pub struct VoteItem {
    /// Voter's space ID
    pub voter_id: Uuid,
    /// Entity or relation being voted on
    pub object_id: Uuid,
    /// Type of object (Entity or Relation)
    pub object_type: VoteObjectType,
    /// Space point of view
    pub space_id: Uuid,
    /// Vote direction
    pub vote: VoteValue,
    /// Block number when vote was cast
    pub block_number: u64,
    /// Block timestamp when vote was cast
    pub block_timestamp: u64,
}

/// Current vote state per user/entity/space (for upsert operations)
#[derive(Clone, Debug)]
pub struct UserVoteItem {
    /// Voter's space ID
    pub voter_id: Uuid,
    /// Entity or relation being voted on
    pub object_id: Uuid,
    /// Type of object (Entity or Relation)
    pub object_type: VoteObjectType,
    /// Space point of view
    pub space_id: Uuid,
    /// Current vote type
    pub vote_type: VoteValue,
    /// Timestamp when vote was cast
    pub voted_at: u64,
}

/// Aggregated vote counts per entity/space
#[derive(Clone, Debug)]
pub struct VotesCountItem {
    /// Entity or relation ID
    pub object_id: Uuid,
    /// Type of object (Entity or Relation)
    pub object_type: VoteObjectType,
    /// Space point of view
    pub space_id: Uuid,
    /// Total upvotes
    pub upvotes: i64,
    /// Total downvotes
    pub downvotes: i64,
}

/// Criteria for querying user votes: (voter_id, object_id, space_id, object_type)
pub type UserVoteCriteria = (Uuid, Uuid, Uuid, VoteObjectType);

/// Criteria for querying vote counts: (object_id, space_id, object_type)
pub type VoteCountCriteria = (Uuid, Uuid, VoteObjectType);


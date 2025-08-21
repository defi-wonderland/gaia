use crate::types::ActionRaw;
use serde::{Deserialize, Serialize};

/// Represents the type of vote cast by a user.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum VoteValue {
    /// Indicates an upvote or positive endorsement.
    Up,
    /// Indicates a downvote or negative endorsement.
    Down,
    /// Indicates the removal or retraction of a previous vote.
    Remove,
}

/// Represents a processed vote action.
///
/// This struct combines the raw action data with the specific vote type,
/// providing a structured representation of a user's vote.
#[derive(Clone, Debug, PartialEq)]
pub struct Vote {
    pub raw: ActionRaw,
    pub vote: VoteValue,
}

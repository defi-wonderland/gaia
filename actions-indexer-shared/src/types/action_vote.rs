use crate::types::ActionRaw;

/// Represents the type of vote cast by a user.
#[derive(Clone, Debug, PartialEq)]
pub enum Vote {
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
pub struct VoteAction {
    pub raw: ActionRaw,
    pub vote: Vote,
}

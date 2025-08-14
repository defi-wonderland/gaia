//! This module defines the core data structures and types used across the actions indexer.
//! It re-exports specific types like `ActionEvent`, `Action`, `UserVote`, `VotesCount`, `Changeset`, `ActionRaw`, `Vote`, and `VoteAction`.

use alloy::primitives::Address;
use uuid::Uuid;

mod action_event;
mod action;
mod user_vote;
mod votes_count;
mod changeset;
mod action_raw;
mod action_vote;

pub use action_event::ActionEvent;
pub use action::Action;
pub use user_vote::UserVote;
pub use votes_count::VotesCount;
pub use changeset::Changeset;
pub use action_raw::ActionRaw;
pub use action_vote::{Vote, VoteAction};

pub type EntityId = Uuid;
pub type GroupId = Uuid;
pub type SpaceAddress = Address;
pub type UserAddress = Address;
pub type VoteCriteria = (UserAddress, EntityId, SpaceAddress);
pub type VoteCountCriteria = (EntityId, SpaceAddress);
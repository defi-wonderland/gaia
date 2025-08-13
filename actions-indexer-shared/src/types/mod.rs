//! This module defines the core data structures and types used across the actions indexer.
//! It re-exports specific types like `ActionEvent`, `Action`, `UserVote`, `VotesCount`, and `Changeset`.
mod action_event;
mod action;
mod user_vote;
mod votes_count;
mod changeset;

pub use action_event::ActionEvent;
pub use action::Action;
pub use user_vote::UserVote;
pub use votes_count::VotesCount;
pub use changeset::Changeset;
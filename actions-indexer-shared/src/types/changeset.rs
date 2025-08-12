use crate::types::{Action, UserVote, VotesCount};

pub struct Changeset<'a> {
	pub actions: &'a [Action],
	pub user_votes: &'a [UserVote],
	pub votes_count: &'a [VotesCount]
}
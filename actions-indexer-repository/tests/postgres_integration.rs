//! Integration tests for PostgreSQL actions repository implementation.
//!
//! These tests require a real PostgreSQL database and use SQLx test macros
//! to ensure proper test isolation and cleanup.
//!
//! Run with: `cargo test --test postgres_integration`

use actions_indexer_repository::{ActionsRepository, PostgresActionsRepository};
use actions_indexer_shared::types::{Action, ActionRaw, VoteAction, Vote, UserVote, VotesCount, VoteCriteria};
use alloy::primitives::{Address, TxHash};
use alloy::hex::FromHex;
use uuid::Uuid;
use sqlx::Row;

/// Creates a test action raw data with default values.
fn make_raw_action() -> ActionRaw {
    ActionRaw {
        action_type: 1,
        action_version: 1,
        sender: Address::from_hex("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045").unwrap(),
        entity: Uuid::new_v4(),
        group_id: None,
        space_pov: Address::from_hex("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045").unwrap(),
        metadata: None,
        block_number: 1,
        block_timestamp: 1755182913,
        tx_hash: TxHash::from_hex("0x5427daee8d03277f8a30ea881692c04861e692ce5f305b7a689b76248cae63c4").unwrap(),
    }
}

/// Creates a test user vote with default values.
fn make_user_vote() -> UserVote {
    UserVote {
        user_id: Address::from_hex("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045").unwrap(),
        entity_id: Uuid::new_v4(),
        space_id: Address::from_hex("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap(),
        vote_type: 1,
        voted_at: 1755182913,
    }
}

/// Creates a test votes count with default values.
fn make_votes_count() -> VotesCount {
    VotesCount {
        entity_id: Uuid::new_v4(),
        space_id: Address::from_hex("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap(),
        upvotes: 1,
        downvotes: 0,
    }
}

// ============================================================================
// Raw Actions Tests
// ============================================================================

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_insert_raw_action(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();

    let raw_action = make_raw_action();

    let action = Action::Vote(VoteAction {
        raw: raw_action.clone(),
        vote: Vote::Up,
    });

    repository.insert_actions(&[action]).await.unwrap();

    let actions = sqlx::query("SELECT * FROM raw_actions")
        .fetch_all(&pool).await.unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].get::<String, _>("tx_hash"), raw_action.tx_hash.to_string());
}

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_insert_multiple_raw_actions(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();
    let raw_action = make_raw_action();
    let actions = vec![
        Action::Vote(VoteAction {
            raw: raw_action.clone(),
            vote: Vote::Up,
        }),
        Action::Vote(VoteAction {
            raw: raw_action.clone(),
            vote: Vote::Down,
        }),
        Action::Vote(VoteAction {
            raw: raw_action.clone(),
            vote: Vote::Remove,
        }),
    ];

    repository.insert_actions(&actions).await.unwrap();

    let actions = sqlx::query("SELECT * FROM raw_actions")
        .fetch_all(&pool).await.unwrap();

    assert_eq!(actions.len(), 3);
}

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_insert_empty_actions(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();
    let actions: Vec<Action> = Vec::new();
    repository.insert_actions(&actions).await.unwrap();

    let actions_in_db = sqlx::query("SELECT * FROM raw_actions")
        .fetch_all(&pool).await.unwrap();

    assert!(actions_in_db.is_empty());
}

// ============================================================================
// User Votes Tests
// ============================================================================

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_update_user_vote(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();

    let user_vote = make_user_vote();

    repository.update_user_votes(&[user_vote.clone()]).await.unwrap();

    let votes_in_db = sqlx::query!(
        "SELECT user_id, entity_id, space_id, vote_type, voted_at FROM user_votes WHERE user_id = $1 AND entity_id = $2 AND space_id = $3",
        format!("0x{}", hex::encode(user_vote.user_id.as_slice())),
        user_vote.entity_id,
        format!("0x{}", hex::encode(user_vote.space_id.as_slice())),
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(votes_in_db.user_id, format!("0x{}", hex::encode(user_vote.user_id.as_slice())));
    assert_eq!(votes_in_db.entity_id, user_vote.entity_id);
    assert_eq!(votes_in_db.space_id, format!("0x{}", hex::encode(user_vote.space_id.as_slice())));
    assert_eq!(votes_in_db.vote_type as u8, user_vote.vote_type);
    assert_eq!(votes_in_db.voted_at.unix_timestamp() as u64, user_vote.voted_at);

    // Test update
    let updated_user_vote = UserVote {
        vote_type: 2,
        voted_at: 1755182914,
        ..user_vote.clone()
    };

    repository.update_user_votes(&[updated_user_vote.clone()]).await.unwrap();

    let updated_votes_in_db = sqlx::query!(
        "SELECT user_id, entity_id, space_id, vote_type, voted_at FROM user_votes WHERE user_id = $1 AND entity_id = $2 AND space_id = $3",
        format!("0x{}", hex::encode(updated_user_vote.user_id.as_slice())),
        updated_user_vote.entity_id,
        format!("0x{}", hex::encode(updated_user_vote.space_id.as_slice())),
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(updated_votes_in_db.vote_type as u8, updated_user_vote.vote_type);
    assert_eq!(updated_votes_in_db.voted_at.unix_timestamp() as u64, updated_user_vote.voted_at);
}

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_update_multiple_user_votes(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();

    let user_votes = vec![
        make_user_vote(),
        UserVote {
            entity_id: Uuid::new_v4(),
            ..make_user_vote()
        },
        UserVote {
            entity_id: Uuid::new_v4(),
            ..make_user_vote()
        },
    ];

    repository.update_user_votes(&user_votes).await.unwrap();

    let votes_in_db = sqlx::query!("SELECT * FROM user_votes")
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(votes_in_db.len(), 3);
}

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_update_empty_user_votes(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();
    let user_votes: Vec<UserVote> = Vec::new();
    repository.update_user_votes(&user_votes).await.unwrap();

    let votes_in_db = sqlx::query!("SELECT * FROM user_votes")
        .fetch_all(&pool)
        .await
        .unwrap();

    assert!(votes_in_db.is_empty());
}

// ============================================================================
// Votes Count Tests
// ============================================================================

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_update_votes_count(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();

    let votes_count = make_votes_count();

    repository.update_votes_counts(&[votes_count.clone()]).await.unwrap();

    let counts_in_db = sqlx::query!(
        "SELECT entity_id, space_id, upvotes, downvotes FROM votes_count WHERE entity_id = $1 AND space_id = $2",
        votes_count.entity_id,
        format!("0x{}", hex::encode(votes_count.space_id.as_slice())),
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(counts_in_db.entity_id, votes_count.entity_id);
    assert_eq!(counts_in_db.space_id, format!("0x{}", hex::encode(votes_count.space_id.as_slice())));
    assert_eq!(counts_in_db.upvotes, votes_count.upvotes);
    assert_eq!(counts_in_db.downvotes, votes_count.downvotes);

    // Test update
    let updated_votes_count = VotesCount {
        upvotes: 2,
        downvotes: 1,
        ..votes_count.clone()
    };

    repository.update_votes_counts(&[updated_votes_count.clone()]).await.unwrap();

    let updated_counts_in_db = sqlx::query!(
        "SELECT entity_id, space_id, upvotes, downvotes FROM votes_count WHERE entity_id = $1 AND space_id = $2",
        updated_votes_count.entity_id,
        format!("0x{}", hex::encode(updated_votes_count.space_id.as_slice())),
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(updated_counts_in_db.upvotes, updated_votes_count.upvotes);
    assert_eq!(updated_counts_in_db.downvotes, updated_votes_count.downvotes);
}

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_update_multiple_votes_counts(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();

    let votes_counts = vec![
        make_votes_count(),
        VotesCount {
            entity_id: Uuid::new_v4(),
            ..make_votes_count()
        },
        VotesCount {
            entity_id: Uuid::new_v4(),
            ..make_votes_count()
        },
    ];

    repository.update_votes_counts(&votes_counts).await.unwrap();

    let counts_in_db = sqlx::query!("SELECT * FROM votes_count")
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(counts_in_db.len(), 3);
}

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_update_empty_votes_counts(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();
    let votes_counts: Vec<VotesCount> = Vec::new();
    repository.update_votes_counts(&votes_counts).await.unwrap();

    let counts_in_db = sqlx::query!("SELECT * FROM votes_count")
        .fetch_all(&pool)
        .await
        .unwrap();

    assert!(counts_in_db.is_empty());
}

// ============================================================================
// Query Tests
// ============================================================================

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_get_user_votes(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();

    let user_vote1 = make_user_vote();
    let user_vote2 = UserVote {
        user_id: Address::from_hex("0x1234567890123456789012345678901234567890").unwrap(),
        entity_id: Uuid::new_v4(),
        space_id: Address::from_hex("0x1234567890123456789012345678901234567891").unwrap(),
        vote_type: 1,
        voted_at: 1755182913,
    };
    let user_vote3 = UserVote {
        user_id: Address::from_hex("0x1234567890123456789012345678901234567890").unwrap(),
        entity_id: Uuid::new_v4(),
        space_id: Address::from_hex("0x1234567890123456789012345678901234567892").unwrap(),
        vote_type: 2,
        voted_at: 1755182914,
    };

    repository.update_user_votes(&[user_vote1.clone(), user_vote2.clone(), user_vote3.clone()]).await.unwrap();

    let found_votes = repository.get_user_votes(&[(user_vote1.user_id, user_vote1.entity_id, user_vote1.space_id), (user_vote2.user_id, user_vote2.entity_id, user_vote2.space_id), (user_vote3.user_id, user_vote3.entity_id, user_vote3.space_id)]).await.unwrap();
    assert_eq!(found_votes.len(), 3);
    assert!(found_votes.contains(&user_vote1));
    assert!(found_votes.contains(&user_vote2));
    assert!(found_votes.contains(&user_vote3));
}

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_get_user_votes_empty_input(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();
    
    let empty_vote_criteria: &[VoteCriteria] = &[];
    let result = repository.get_user_votes(empty_vote_criteria).await.unwrap();

    assert!(result.is_empty());
}

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_get_user_votes_partial_matches(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();

    let user_vote1 = make_user_vote();
    let user_vote2 = UserVote {
        user_id: Address::from_hex("0x1234567890123456789012345678901234567890").unwrap(),
        entity_id: Uuid::new_v4(),
        space_id: Address::from_hex("0x1234567890123456789012345678901234567891").unwrap(),
        vote_type: 1,
        voted_at: 1755182913,
    };

    repository.update_user_votes(&[user_vote1.clone()]).await.unwrap();

    let vote_criteria = [
        (user_vote1.user_id, user_vote1.entity_id, user_vote1.space_id),
        (user_vote2.user_id, user_vote2.entity_id, user_vote2.space_id),
    ];
    
    let found_votes = repository.get_user_votes(&vote_criteria).await.unwrap();
    assert_eq!(found_votes.len(), 1);
    assert!(found_votes.contains(&user_vote1));
    assert!(!found_votes.contains(&user_vote2));
}

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_get_user_votes_duplicate_vote_criteria(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();

    let user_vote = make_user_vote();
    repository.update_user_votes(&[user_vote.clone()]).await.unwrap();

    let vote_criteria = [
        (user_vote.user_id, user_vote.entity_id, user_vote.space_id),
        (user_vote.user_id, user_vote.entity_id, user_vote.space_id),
        (user_vote.user_id, user_vote.entity_id, user_vote.space_id),
    ];
    
    let found_votes = repository.get_user_votes(&vote_criteria).await.unwrap();
    assert_eq!(found_votes.len(), 1);
    assert_eq!(found_votes[0], user_vote);
}

#[sqlx::test(migrations = "src/postgres/migrations")]
async fn test_get_user_votes_nonexistent_data(pool: sqlx::PgPool) {
    let repository = PostgresActionsRepository::new(pool.clone()).await.unwrap();

    let vote_criteria = [
        (Address::from_hex("0x1111111111111111111111111111111111111111").unwrap(), Uuid::new_v4(), Address::from_hex("0x2222222222222222222222222222222222222222").unwrap()),
        (Address::from_hex("0x3333333333333333333333333333333333333333").unwrap(), Uuid::new_v4(), Address::from_hex("0x4444444444444444444444444444444444444444").unwrap()),
    ];
    
    let found_votes = repository.get_user_votes(&vote_criteria).await.unwrap();
    assert!(found_votes.is_empty());
}

//! Handler for HermesVoteCast messages from the curation.votes topic.

use hermes_schema::pb::voting::{HermesVoteCast, VoteDirection};
use uuid::Uuid;

use crate::error::HandlerError;
use crate::models::voting::{VoteItem, VoteObjectType, VoteValue};

/// Object type discriminator values (big-endian 4-byte encoding)
const OBJECT_TYPE_ENTITY: [u8; 4] = [0x00, 0x00, 0x00, 0x01];
const OBJECT_TYPE_RELATION: [u8; 4] = [0x00, 0x00, 0x00, 0x02];

/// Parse object type from 4-byte discriminator
fn parse_object_type(bytes: &[u8]) -> Result<VoteObjectType, HandlerError> {
    if bytes.len() < 4 {
        return Err(HandlerError::InvalidObjectType(bytes.to_vec()));
    }

    let type_bytes: [u8; 4] = bytes[0..4].try_into().unwrap();

    match type_bytes {
        OBJECT_TYPE_ENTITY => Ok(VoteObjectType::Entity),
        OBJECT_TYPE_RELATION => Ok(VoteObjectType::Relation),
        _ => Err(HandlerError::InvalidObjectType(bytes.to_vec())),
    }
}

/// Convert HermesVoteCast to VoteItem
pub fn handle_vote_cast(vote: &HermesVoteCast) -> Result<VoteItem, HandlerError> {
    let meta = vote.meta.as_ref().ok_or(HandlerError::MissingPayload)?;

    let voter_id = Uuid::from_slice(&vote.voter_id)?;
    let object_id = Uuid::from_slice(&vote.object_id)?;
    let space_id = Uuid::from_slice(&vote.space_pov)?;
    let object_type = parse_object_type(&vote.object_type)?;

    let vote_value = match VoteDirection::try_from(vote.direction) {
        Ok(VoteDirection::Up) => VoteValue::Up,
        Ok(VoteDirection::Down) => VoteValue::Down,
        Ok(VoteDirection::None) => VoteValue::Remove,
        Err(_) => return Err(HandlerError::InvalidVoteDirection(vote.direction)),
    };

    Ok(VoteItem {
        voter_id,
        object_id,
        object_type,
        space_id,
        vote: vote_value,
        block_number: meta.block_number,
        block_timestamp: meta.created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_schema::pb::blockchain_metadata::BlockchainMetadata;

    fn make_test_meta() -> BlockchainMetadata {
        BlockchainMetadata {
            block_number: 12345,
            created_at: 1700000000,
            sequence: 0,
            is_last: true,
            ..Default::default()
        }
    }

    fn make_test_uuid() -> Vec<u8> {
        vec![
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ]
    }

    #[test]
    fn test_handle_vote_cast_upvote_entity() {
        let vote = HermesVoteCast {
            voter_id: make_test_uuid(),
            object_type: OBJECT_TYPE_ENTITY.to_vec(),
            object_id: make_test_uuid(),
            direction: VoteDirection::Up as i32,
            version: 1,
            group_id: make_test_uuid(),
            space_pov: make_test_uuid(),
            meta: Some(make_test_meta()),
        };

        let result = handle_vote_cast(&vote).unwrap();

        assert_eq!(result.object_type, VoteObjectType::Entity);
        assert_eq!(result.vote, VoteValue::Up);
        assert_eq!(result.block_number, 12345);
        assert_eq!(result.block_timestamp, 1700000000);
    }

    #[test]
    fn test_handle_vote_cast_downvote_relation() {
        let vote = HermesVoteCast {
            voter_id: make_test_uuid(),
            object_type: OBJECT_TYPE_RELATION.to_vec(),
            object_id: make_test_uuid(),
            direction: VoteDirection::Down as i32,
            version: 1,
            group_id: make_test_uuid(),
            space_pov: make_test_uuid(),
            meta: Some(make_test_meta()),
        };

        let result = handle_vote_cast(&vote).unwrap();

        assert_eq!(result.object_type, VoteObjectType::Relation);
        assert_eq!(result.vote, VoteValue::Down);
    }

    #[test]
    fn test_handle_vote_cast_unvote() {
        let vote = HermesVoteCast {
            voter_id: make_test_uuid(),
            object_type: OBJECT_TYPE_ENTITY.to_vec(),
            object_id: make_test_uuid(),
            direction: VoteDirection::None as i32,
            version: 1,
            group_id: make_test_uuid(),
            space_pov: make_test_uuid(),
            meta: Some(make_test_meta()),
        };

        let result = handle_vote_cast(&vote).unwrap();

        assert_eq!(result.vote, VoteValue::Remove);
    }

    #[test]
    fn test_handle_vote_cast_missing_meta() {
        let vote = HermesVoteCast {
            voter_id: make_test_uuid(),
            object_type: OBJECT_TYPE_ENTITY.to_vec(),
            object_id: make_test_uuid(),
            direction: VoteDirection::Up as i32,
            version: 1,
            group_id: make_test_uuid(),
            space_pov: make_test_uuid(),
            meta: None,
        };

        let result = handle_vote_cast(&vote);
        assert!(matches!(result, Err(HandlerError::MissingPayload)));
    }

    #[test]
    fn test_handle_vote_cast_invalid_object_type() {
        let vote = HermesVoteCast {
            voter_id: make_test_uuid(),
            object_type: vec![0xFF, 0xFF, 0xFF, 0xFF],
            object_id: make_test_uuid(),
            direction: VoteDirection::Up as i32,
            version: 1,
            group_id: make_test_uuid(),
            space_pov: make_test_uuid(),
            meta: Some(make_test_meta()),
        };

        let result = handle_vote_cast(&vote);
        assert!(matches!(result, Err(HandlerError::InvalidObjectType(_))));
    }

    #[test]
    fn test_parse_object_type_entity() {
        assert_eq!(
            parse_object_type(&OBJECT_TYPE_ENTITY).unwrap(),
            VoteObjectType::Entity
        );
    }

    #[test]
    fn test_parse_object_type_relation() {
        assert_eq!(
            parse_object_type(&OBJECT_TYPE_RELATION).unwrap(),
            VoteObjectType::Relation
        );
    }

    #[test]
    fn test_parse_object_type_too_short() {
        let result = parse_object_type(&[0x00, 0x00]);
        assert!(matches!(result, Err(HandlerError::InvalidObjectType(_))));
    }
}


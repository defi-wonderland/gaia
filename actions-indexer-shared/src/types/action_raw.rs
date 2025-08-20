use crate::types::{UserAddress, SpaceAddress, EntityId, GroupId};
use alloy::primitives::{BlockNumber, BlockTimestamp, Bytes, TxHash};


/// Represents the raw data of an action as extracted from an event.
///
/// This struct holds the essential fields common to all action types,
/// providing a base for further processing into specific `Action` variants.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionRaw {
    pub action_type: u64,
    pub action_version: u64,
    pub sender: UserAddress,
    pub entity: EntityId,
    pub group_id: Option<GroupId>,
    pub space_pov: SpaceAddress,
    pub metadata: Option<Bytes>,
    pub block_number: BlockNumber,
    pub block_timestamp: BlockTimestamp,
    pub tx_hash: TxHash,
}
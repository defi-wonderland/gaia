use crate::types::ActionEvent;
use alloy::primitives::{Address, BlockNumber, BlockTimestamp, Bytes, TxHash};
use uuid::Uuid;

/// Represents the raw data of an action as extracted from an event.
///
/// This struct holds the essential fields common to all action types,
/// providing a base for further processing into specific `Action` variants.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionRaw {
    pub action_type: u64,
    pub version: u64,
    pub sender: Address,
    pub entity: Uuid,
    pub group_id: Option<Uuid>,
    pub space_pov: Address,
    pub metadata: Option<Bytes>,
    pub block_number: BlockNumber,
    pub block_timestamp: BlockTimestamp,
    pub tx_hash: TxHash,
}

/// Implements conversion from `ActionEvent` to `ActionRaw`.
///
/// This conversion extracts relevant fields from a raw `ActionEvent`
/// and maps them to the corresponding fields in the `ActionRaw` struct.
impl From<ActionEvent> for ActionRaw {
    fn from(event: ActionEvent) -> Self {
        let group_id = (!event.group_id.is_nil()).then(|| event.group_id.clone());
        let metadata = (!event.payload.is_empty()).then(|| event.payload.clone());

        ActionRaw {
            action_type: u64::from(event.kind),
            version: u64::from(event.version),
            sender: event.sender,
            entity: event.entity,
            group_id,
            space_pov: event.space_pov,
            metadata,
            block_number: event.block_number,
            block_timestamp: event.block_timestamp,
            tx_hash: event.tx_hash,
        }
    }
}

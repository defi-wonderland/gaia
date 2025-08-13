use crate::types::ActionEvent;
use alloy::primitives::{Address, BlockNumber, BlockTimestamp, Bytes, TxHash};

/// Represents the raw data of an action as extracted from an event.
///
/// This struct holds the essential fields common to all action types,
/// providing a base for further processing into specific `Action` variants.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionRaw {
    pub action_type: u64,
    pub version: u64,
    pub sender: Address,
    pub entity: String,
    pub group_id: Option<String>,
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
        ActionRaw {
            action_type: u64::from(event.kind),
            version: u64::from(event.version),
            sender: event.space_pov,
            entity: event.entity,
            group_id: Some(event.group_id),
            space_pov: event.space_pov,
            metadata: Some(event.payload),
            block_number: event.block_number,
            block_timestamp: event.block_timestamp,
            tx_hash: event.tx_hash,
        }
    }
}

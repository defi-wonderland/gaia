/// Represents the raw data of an action as extracted from an event.
///
/// This struct holds the essential fields common to all action types,
/// providing a base for further processing into specific `Action` variants.
use alloy::primitives::{Address, BlockNumber, BlockTimestamp, Bytes, TxHash};
use uuid::Uuid;

// Adapted from the Smart Contract action function in
// https://github.com/defi-wonderland/geo-actions/blob/40e0ded517c682308ffaa6894732e3dde22dbc96/src/interfaces/IActions.sol
#[derive(Clone, Debug, PartialEq)]
pub struct ActionRaw {
    pub sender: Address,
    pub kind: u16,
    pub version: u16,
    pub space_pov: Address,
    pub entity: Uuid,
    pub group_id: Option<Uuid>,
    pub payload: Option<Bytes>,
    pub block_number: BlockNumber,
    pub block_timestamp: BlockTimestamp,
    pub tx_hash: TxHash,
}
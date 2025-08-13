/// Represents a raw action event as consumed from the data source.
///
/// This struct is intended to hold the unprocessed data of an event
/// before it undergoes further processing by the indexer pipeline.
use alloy::primitives::{Address, BlockNumber, BlockTimestamp, Bytes, TxHash};

// Adapted from the Smart Contract action function in
// https://github.com/defi-wonderland/geo-actions/blob/40e0ded517c682308ffaa6894732e3dde22dbc96/src/interfaces/IActions.sol
#[derive(Clone, Debug)]
pub struct ActionEvent {
    pub sender: Address,
    pub kind: u16,
    pub version: u16,
    pub space_pov: Address,
    pub entity: String,
    pub group_id: String,
    pub payload: Bytes,
    pub block_number: BlockNumber,
    pub block_timestamp: BlockTimestamp,
    pub tx_hash: TxHash,
}
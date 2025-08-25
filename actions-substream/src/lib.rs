mod pb;
use hex_literal::hex;
use pb::actions::v1::{Action, Actions};
use pb::sf::ethereum::r#type::v2::{Block, Log};

substreams_ethereum::init!();

const FACTORY_TRACKED_CONTRACTS: &[[u8; 20]] = &[hex!("20b8d41da487c80e06667409e81ab8b173c9e076")];

#[substreams::handlers::map]
fn map_actions(blk: Block) -> Result<Actions, substreams::errors::Error> {
    let mut actions = Actions::default();

    for transaction in &blk.transaction_traces {
        if let Some(receipt) = &transaction.receipt {
            let tx_hash = format!("0x{}", hex::encode(&transaction.hash));
            let block_number = blk.number;
            let block_timestamp = blk.header.as_ref().map(|h| h.timestamp.as_ref().map(|t| t.seconds as u64)).flatten().unwrap_or(0);

            for log in &receipt.logs {
                if is_address_in_contracts(&log.address) {
                    if let Some(action) = decode_action_log(&log)? {
                        let action = Action {
                            action_type: action.action_type,
                            action_version: action.action_version,
                            sender: format!("0x{}", hex::encode(&transaction.from)),
                            entity: action.entity,
                            group_id: action.group_id,
                            space_pov: action.space_pov,
                            metadata: action.metadata,
                            block_number,
                            block_timestamp,
                            tx_hash: tx_hash.clone(),
                        };
                        actions.actions.push(action);
                    }
                }
            }
        }
    }

    Ok(actions)
}

fn is_address_in_contracts(address: &Vec<u8>) -> bool {
    if address.len() != 20 {
        return false;
    }

    FACTORY_TRACKED_CONTRACTS.contains(&address.as_slice().try_into().unwrap())
}

/// Decoded event data from the Action event log
struct EventData {
    action_type: u64,
    action_version: u64,
    space_pov: String,
    group_id: Option<String>,
    entity: String,
    metadata: Option<Vec<u8>>,
}

/// Decode an Action event log into an Action message
/// Based on the Action event signature from the smart contract
fn decode_action_log(log: &Log) -> Result<Option<EventData>, substreams::errors::Error> {
    let data = &log.data;
    if data.len() < 128 {
        // Minimum: word0(32) + word1(32) + offset(32) + length(32)
        return Ok(None);
    }

    // Decode word0: action version (255-240) | event type (239-224) | reserved (223-160) | space POV (160-0)
    let word0 = &data[0..32];
    let action_version = u16::from_be_bytes([word0[0], word0[1]]) as u64;
    let action_type = u16::from_be_bytes([word0[2], word0[3]]) as u64;
    // Skip reserved bytes [4..12]
    let space_pov = format!("0x{}", hex::encode(&word0[12..32])); // Last 20 bytes for address

    // Decode word1: group id (255-128) | entity id (128-0)
    let word1 = &data[32..64];
    let group_id_bytes = &word1[0..16]; // First 16 bytes (128 bits)
    let entity_id_bytes = &word1[16..32]; // Last 16 bytes (128 bits)

    // Convert 16-byte arrays to UUID strings
    let entity = format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        entity_id_bytes[0], entity_id_bytes[1], entity_id_bytes[2], entity_id_bytes[3],
        entity_id_bytes[4], entity_id_bytes[5], entity_id_bytes[6], entity_id_bytes[7],
        entity_id_bytes[8], entity_id_bytes[9], entity_id_bytes[10], entity_id_bytes[11],
        entity_id_bytes[12], entity_id_bytes[13], entity_id_bytes[14], entity_id_bytes[15]
    );

    // Check if group_id is zero (optional field)
    let group_id = if group_id_bytes.iter().all(|&x| x == 0) {
        None
    } else {
        Some(format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            group_id_bytes[0], group_id_bytes[1], group_id_bytes[2], group_id_bytes[3],
            group_id_bytes[4], group_id_bytes[5], group_id_bytes[6], group_id_bytes[7],
            group_id_bytes[8], group_id_bytes[9], group_id_bytes[10], group_id_bytes[11],
            group_id_bytes[12], group_id_bytes[13], group_id_bytes[14], group_id_bytes[15]
        ))
    };

    let metadata = if data.len() < 160 {
        None
    } else {
        let payload_length = u64::from_be_bytes([
            data[152], data[153], data[154], data[155], data[156], data[157], data[158], data[159],
        ]) as usize;
        if payload_length > 0 && data.len() >= 160 + payload_length {
            Some(data[160..160 + payload_length].to_vec())
        } else {
            None
        }
    };

    Ok(Some(EventData {
        action_type,
        action_version,
        entity,
        group_id,
        space_pov,
        metadata,
    }))
}

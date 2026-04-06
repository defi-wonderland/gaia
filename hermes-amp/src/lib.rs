//! Hermes Amp — Arrow Flight SQL streaming client for blockchain action logs.
//!
//! Connects to an Amp server via Arrow Flight SQL, streams action logs from the
//! Space Registry contract, and delivers them as `hermes_relay::Action` values
//! grouped by block.

pub mod stream;

/// Space Registry proxy contract address (Geo testnet).
pub const SPACE_REGISTRY_ADDRESS_HEX: &str = "0xb01683b2f0d38d43fcd4d9aab980166988924132";

/// Derived dataset manifest for Amp — maps Hermes actions from raw logs.
pub const HERMES_ACTIONS_MANIFEST_JSON: &str = include_str!("../manifests/hermes-actions.json");

pub use stream::{AmpBlock, AmpStreamConfig, stream_actions};

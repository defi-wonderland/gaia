//! # Actions Indexer Repository
//! This crate provides traits and implementations for interacting with the
//! actions data repository. It includes definitions for errors, interfaces,
//! and concrete implementations for PostgreSQL.
pub mod errors;
pub mod interfaces;
pub mod postgres;

pub use errors::ActionsRepositoryError;
pub use interfaces::ActionsRepository;
pub use postgres::PostgresActionsRepository;

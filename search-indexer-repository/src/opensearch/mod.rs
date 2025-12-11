//! OpenSearch implementation of the search index provider.
//!
//! This module provides a concrete implementation of `SearchIndexProvider`
//! using OpenSearch as the backend.

mod index_config;
mod provider;

pub use index_config::IndexConfig;
pub use provider::OpenSearchProvider;

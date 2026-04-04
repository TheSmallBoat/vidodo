//! Vidodo artifact storage: layout, asset ingestion, and registry queries.

pub mod analysis_bridge;
pub mod artifact_layout;
pub mod asset_ingest;
pub mod query_registry;
pub mod template_registry;

pub use artifact_layout::*;
pub use asset_ingest::*;
pub use query_registry::*;
pub use template_registry::*;

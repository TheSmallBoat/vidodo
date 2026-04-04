//! SuperCollider scsynth bridge for Vidodo audio runtime.
//!
//! Provides OSC message serialization/deserialization, scsynth process management,
//! and server query functionality. The bridge translates Vidodo `AudioEvent`s into
//! OSC commands that scsynth can execute.

pub mod ack_collector;
pub mod backend;
pub mod command_translator;
pub mod health_monitor;
pub mod node_mapping;
pub mod osc;
pub mod process_manager;
pub mod server_query;

#[cfg(test)]
mod tests;

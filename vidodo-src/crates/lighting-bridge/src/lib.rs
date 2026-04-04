//! DMX frame construction and ArtNet packet serialization.
//!
//! Provides 512-channel DMX frame management, ArtNet OpDmx packet building,
//! and UDP sender abstraction for lighting fixtures.

pub mod artnet;
pub mod cue_translator;
pub mod dmx;
pub mod fixture_topology;

#[cfg(test)]
mod tests;

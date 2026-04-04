//! IPC message types for inter-process communication.
//!
//! Defines the message envelope, runtime actions, and acknowledgment types
//! used for communication between Vidodo runtime processes.

pub mod causation;
pub mod channel;
pub mod messages;
pub mod resilience;
pub mod scheduler_ipc;

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod tests;

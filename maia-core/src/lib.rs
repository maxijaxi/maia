//! MAIA Core - The runtime and message router for MAIA modules
//!
//! This crate provides the core functionality for loading, running, and routing
//! messages between MAIA modules across different runtime environments.

#![doc(html_root_url = "https://docs.maia-protocol.org/core")]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

/// Module runtime management
pub mod runtime;
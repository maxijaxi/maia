//! MAIA Core - The minimal runtime for the MAIA protocol.
//!
//! This crate contains the core components needed to run MAIA:
//! - Module system (loading, running, managing modules)
//! - Message router (routing between modules)
//! - Discovery service (finding capabilities)
//! - Network identity (cryptographic identity)
//! - Federation protocol (connecting networks)

#![warn(missing_docs)]

/// Module system - handles loading and running modules
pub mod module_system;

// TODO: Implement these core components
// pub mod router;
// pub mod discovery;
// pub mod identity;
// pub mod federation;

pub use module_system::ModuleSystem;

/// Version of the MAIA core
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!CORE_VERSION.is_empty());
    }
}

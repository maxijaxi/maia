//! MAIA SDK - Software Development Kit for building MAIA modules.
//!
//! This crate provides the core traits, types and utilities needed
//! to develop modules for the MAIA distributed AI infrastructure.
//!
//! # Quick Start
//!
//! ```ignore
//! use maia_sdk::prelude::*;
//! use async_trait::async_trait;
//!
//! struct MyModule;
//!
//! #[async_trait]
//! impl MaiaModule for MyModule {
//!     fn manifest(&self) -> ModuleManifest {
//!         ModuleManifest::minimal("my.module", "My Module")
//!     }
//!
//!     fn capabilities(&self) -> Vec<Capability> {
//!         vec![Capability::new("my.capability")]
//!     }
//!
//!     // ... implement other required methods
//! }
//!
//! // Export for dynamic loading
//! maia_module!(MyModule);
//! ```

#![doc(html_root_url = "https://docs.maia-protocol.org/sdk")]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

/// Error types and handling utilities
pub mod error;

/// Core types for message passing and communication
pub mod types;

/// Module trait and related definitions
pub mod traits;

/// Re-export async_trait for convenience
pub use async_trait::async_trait;

/// Prelude module for convenient imports
///
/// Import everything you need with:
/// ```ignore
/// use maia_sdk::prelude::*;
/// ```
pub mod prelude {
    pub use crate::error::{
        ContextResult, ErrorContext, ErrorExt, FatalError, ModuleError, Result, TemporaryError,
    };

    pub use crate::types::{
        Capability, ModuleId, NetworkId, NodeId, Request, RequestMetadata, Response,
        ResponseMetadata, StreamConfig, StreamDirection, StreamHandle, Version,
    };

    pub use crate::traits::{
        HealthStatus, IsolationLevel, LogLevel, MaiaModule, ModuleCallback, ModuleContext,
        ModuleManifest, Permission, Requirement, ResourceLimits, ResourceRequirements,
    };

    pub use crate::maia_module;
    pub use async_trait::async_trait;
}

/// Version information for the SDK
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn test_prelude_imports() {
        // Just verify that prelude items are accessible
        use crate::prelude::*;

        let _cap = Capability::new("test");
        let _version = Version::new(1, 0, 0);
        let _manifest = ModuleManifest::minimal("test", "Test");
    }
}

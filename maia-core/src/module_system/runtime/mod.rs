//! Module runtime abstraction - defines how modules are executed.
//!
//! Different runtime implementations provide different isolation levels:
//! - **WASM**: Full sandboxing via WebAssembly (default, safest)
//! - **Container**: OS-level isolation via OCI containers (Docker/Podman)
//! - **Native**: Direct execution in same process (zero isolation, trusted only)
//!
//! Note: We deliberately do NOT support "Process" isolation. Use Container instead.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use maia_sdk::prelude::*;

pub mod wasm;
// TODO: Implement these runtimes
// pub mod container;
// pub mod native;

/// Trait that all module runtimes must implement.
///
/// A runtime is responsible for loading, initializing, and executing modules
/// with the appropriate level of isolation.
///
/// # Isolation Levels
///
/// - **WASM**: Sandboxed execution via wasmtime - any language compiled to WASM
/// - **Container**: Containerized via Docker/Podman - any language/runtime
/// - **Native**: Shared library loading - Rust/C/C++ compiled to .so/.dll/.dylib
#[async_trait]
pub trait ModuleRuntime: Send + Sync {
    /// Load a module from the given path
    ///
    /// This should:
    /// - Read the module file
    /// - Parse the manifest (from custom section or separate file)
    /// - Prepare the module for initialization
    ///
    /// Does NOT execute any module code yet.
    async fn load(&mut self, module_path: &Path) -> Result<ModuleManifest>;

    /// Initialize the module with its context
    ///
    /// Calls the module's `initialize()` function with:
    /// - Available capabilities in the system
    /// - Resource limits
    /// - Configuration
    async fn initialize(&mut self, context: ModuleContext) -> Result<()>;

    /// Start the module
    ///
    /// Calls the module's `start()` function.
    /// Module can now begin background tasks and handle requests.
    async fn start(&mut self) -> Result<()>;

    /// Stop the module gracefully
    ///
    /// Calls the module's `stop()` function.
    /// Module should clean up resources and finish in-flight requests.
    async fn stop(&mut self) -> Result<()>;

    /// Invoke a capability on the module
    ///
    /// Routes a request to the module's `handle_request()` function.
    /// This is the main way modules do work.
    async fn invoke(&mut self, request: Request) -> Result<Response>;

    /// Get the capabilities this module provides
    ///
    /// Returns the list of capabilities from the module's `capabilities()` function.
    fn capabilities(&self) -> Vec<Capability>;

    /// Get the isolation level this runtime provides
    ///
    /// This must match the runtime type:
    /// - WasmRuntime → IsolationLevel::Wasm
    /// - ContainerRuntime → IsolationLevel::Container
    /// - NativeRuntime → IsolationLevel::Native
    fn isolation_level(&self) -> IsolationLevel;

    /// Health check the module (optional)
    ///
    /// Can be overridden to check if module is functioning properly.
    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    /// Get module metrics (optional)
    ///
    /// Can be overridden to return runtime-specific metrics.
    async fn get_metrics(&self) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }
}

/// Create a runtime based on the specified isolation level
///
/// # Supported Isolation Levels
///
/// - `IsolationLevel::Wasm` - Returns `WasmRuntime` (implemented)
/// - `IsolationLevel::Container` - Returns ((TODO: `ContainerRuntime`
/// - `IsolationLevel::Native` - Returns ((TODO: `NativeRuntime`
///
/// # Errors
///
/// Returns `NotImplemented` for Container and Native runtimes until they're implemented.
pub fn create_runtime(isolation: IsolationLevel) -> Result<Box<dyn ModuleRuntime>> {
    match isolation {
        IsolationLevel::Wasm => {
            // WASM runtime is fully implemented
            Ok(Box::new(wasm::WasmRuntime::new()?))
        }
        IsolationLevel::Container => {
            // TODO: Implement container isolation using:
            // - bollard (Docker API client)
            // - podman API
            // - or direct OCI runtime (runc, crun)
            Err(ModuleError::Fatal(FatalError::NotImplemented))
        }
        IsolationLevel::Native => {
            // TODO: Implement native runtime using:
            // - libloading for dynamic library loading
            // - dlopen on Unix, LoadLibrary on Windows
            // WARNING: Zero isolation - only for trusted modules!
            Err(ModuleError::Fatal(FatalError::NotImplemented))
        }
    }
}

/// Runtime configuration options
///
/// These are limits and settings enforced by the runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Maximum memory in bytes
    pub max_memory: usize,

    /// Maximum execution time for a single request
    pub max_execution_time: std::time::Duration,

    /// Enable debug mode (more logging, slower execution)
    pub debug: bool,

    /// Custom runtime-specific configuration
    pub custom: HashMap<String, serde_json::Value>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory: 128 * 1024 * 1024,                  // 128MB
            max_execution_time: std::time::Duration::from_secs(30), // 30 seconds
            debug: false,
            custom: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_wasm_runtime() {
        let runtime = create_runtime(IsolationLevel::Wasm);
        assert!(runtime.is_ok());

        if let Ok(runtime) = runtime {
            assert_eq!(runtime.isolation_level(), IsolationLevel::Wasm);
        }
    }

    #[test]
    fn test_create_container_runtime_not_implemented() {
        let runtime = create_runtime(IsolationLevel::Container);
        assert!(runtime.is_err());

        if let Err(ModuleError::Fatal(FatalError::NotImplemented)) = runtime {
            // Expected
        } else {
            panic!("Expected NotImplemented error");
        }
    }

    #[test]
    fn test_create_native_runtime_not_implemented() {
        let runtime = create_runtime(IsolationLevel::Native);
        assert!(runtime.is_err());

        if let Err(ModuleError::Fatal(FatalError::NotImplemented)) = runtime {
            // Expected
        } else {
            panic!("Expected NotImplemented error");
        }
    }

    #[test]
    fn test_runtime_config_defaults() {
        let config = RuntimeConfig::default();

        assert_eq!(config.max_memory, 128 * 1024 * 1024);
        assert_eq!(config.max_execution_time, std::time::Duration::from_secs(30));
        assert!(!config.debug);
        assert!(config.custom.is_empty());
    }
}
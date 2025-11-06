//! Module runtime abstraction - defines how modules are executed.
//!
//! Different runtime implementations provide different isolation levels:
//! - WASM: Full sandboxing via WebAssembly
//! - Process: OS-level process isolation
//! - Native: Direct execution in the same process (no isolation)

use async_trait::async_trait;
use std::path::Path;

use maia_sdk::prelude::*;

pub mod wasm;
// TODO: Implement these runtimes
// pub mod process;
// pub mod native;

/// Trait that all module runtimes must implement.
///
/// A runtime is responsible for loading, initializing, and executing modules
/// with the appropriate level of isolation.
#[async_trait]
pub trait ModuleRuntime: Send + Sync {
    /// Load a module from the given path
    async fn load(&mut self, module_path: &Path) -> Result<ModuleManifest>;

    /// Initialize the module with its context
    async fn initialize(&mut self, context: ModuleContext) -> Result<()>;

    /// Start the module
    async fn start(&mut self) -> Result<()>;

    /// Stop the module
    async fn stop(&mut self) -> Result<()>;

    /// Invoke a capability on the module
    async fn invoke(&mut self, request: Request) -> Result<Response>;

    /// Get the capabilities this module provides
    fn capabilities(&self) -> Vec<Capability>;

    /// Get the isolation level this runtime provides
    fn isolation_level(&self) -> IsolationLevel;

    /// Health check the module
    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    /// Get module metrics
    async fn get_metrics(&self) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }
}

/// Create a runtime based on the specified isolation level
pub fn create_runtime(isolation: IsolationLevel) -> Result<Box<dyn ModuleRuntime>> {
    match isolation {
        IsolationLevel::Wasm => Ok(Box::new(wasm::WasmRuntime::new()?)),
        IsolationLevel::Process => {
            // TODO: Implement process isolation
            Err(ModuleError::Fatal(FatalError::NotImplemented))
        }
        IsolationLevel::Container => {
            // TODO: Implement container isolation
            Err(ModuleError::Fatal(FatalError::NotImplemented))
        }
        IsolationLevel::Native => {
            // TODO: Implement native runtime
            // For now, return not implemented
            Err(ModuleError::Fatal(FatalError::NotImplemented))
        }
    }
}

/// Runtime configuration options
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Maximum memory in bytes
    pub max_memory: usize,

    /// Maximum execution time for a single request
    pub max_execution_time: std::time::Duration,

    /// Enable debug mode
    pub debug: bool,

    /// Custom configuration
    pub custom: HashMap<String, serde_json::Value>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory: 128 * 1024 * 1024, // 128MB
            max_execution_time: std::time::Duration::from_secs(30),
            debug: false,
            custom: HashMap::new(),
        }
    }
}

use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_wasm_runtime() {
        let runtime = create_runtime(IsolationLevel::Wasm);
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_create_unimplemented_runtime() {
        let runtime = create_runtime(IsolationLevel::Process);
        assert!(runtime.is_err());
    }
}

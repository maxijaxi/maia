//! Core trait definitions for MAIA modules.
//!
//! This is the fundamental abstraction that all modules must implement.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{ModuleError, Result};
use crate::types::{Capability, ModuleId, NodeId, Request, Response, StreamHandle, Version};

/// The core trait that all MAIA modules must implement.
///
/// This trait defines the contract between modules and the MAIA runtime.
/// Everything in MAIA is a module except the minimal core routing.
#[async_trait]
pub trait MaiaModule: Send + Sync {
    /// Get the module's manifest containing metadata and requirements.
    fn manifest(&self) -> ModuleManifest;

    /// List capabilities this module provides.
    ///
    /// These are the functions this module can perform.
    /// Example: `["ai.nlp.generate", "ai.nlp.summarize"]`
    fn capabilities(&self) -> Vec<Capability>;

    /// List requirements this module needs from other modules
    ///
    /// These are capabilities this module depends on.
    /// Example: `["storage.kv", "network.http"]`
    fn requirements(&self) -> Vec<Requirement>;

    /// Initialize the module with its runtime context
    ///
    /// Called once when the module is loaded
    async fn initialize(&mut self, context: ModuleContext) -> Result<()>;

    /// Start the module.
    ///
    /// Called after initialization to begin operation.
    async fn start(&mut self) -> Result<()>;

    /// Stop the module gracefully.
    ///
    /// Should clean up resources and save state if needed.
    async fn stop(&mut self) -> Result<()>;

    /// Handle a request for one of this module's capabilities.
    ///
    /// This is the main entry point for module functionality.
    async fn handle_request(&mut self, request: Request) -> Result<Response>;

    /// Handle a streaming operation (optional).
    ///
    /// Not all modules need to support streaming.
    async fn handle_stream(&mut self, stream: StreamHandle) -> Result<()> {
        Err(ModuleError::Fatal(crate::error::FatalError::NotImplemented))
    }

    /// Get current module health status (optional).
    ///
    /// Used for monitoring and load balancing.
    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    /// Get module metrics (optional).
    ///
    /// Used for observability and debugging.
    async fn get_metrics(&self) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }
}

/// Module manifest containing metadata and configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    /// Unique identifier (e.g. "com.example.weather")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Semantic version
    pub version: Version,

    /// Author email or identifier
    pub author: String,

    /// License (e.g. "MIT", "Apache-2.0")
    pub license: String,

    /// Brief description
    pub description: Option<String>,

    /// Homepage or repository URL
    pub homepage: Option<String>,

    /// Cryptographic signature for verification
    pub signature: Option<Vec<u8>>,

    /// Required isolation level
    pub isolation: IsolationLevel,

    /// Resource requirements
    pub resources: ResourceRequirements,

    /// Security permissions needed
    pub permissions: Vec<Permission>,
}

impl ModuleManifest {
    /// Create a minimal manifest for testing
    pub fn minimal(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: Version::new(0, 1, 0),
            author: "unknown".to_string(),
            license: "MIT".to_string(),
            description: None,
            homepage: None,
            signature: None,
            isolation: IsolationLevel::Wasm,
            resources: ResourceRequirements::default(),
            permissions: vec![],
        }
    }
}

/// Module isolation level for security.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// Full sandboxing via WebAssembly
    Wasm,
    /// Separate OS process
    Process,
    /// TODO: (future) Container isolation
    Container,
    /// Same process (only for trusted core modules)
    Native,
}

impl Default for IsolationLevel {
    fn default() -> Self {
        Self::Wasm
    }
}

/// Resource requirements for a module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// Memory limit in MB
    pub memory_mb: u32,

    /// CPU shares (relative weight)
    pub cpu_shares: u32,

    /// Disk space in MB (optional)
    pub disk_mb: Option<u32>,

    /// Network bandwidth in Mbps (optional)
    pub network_mbps: Option<u32>,

    /// GPU requirements (optional)
    pub gpu: Option<GpuRequirement>,
}

impl Default for ResourceRequirements {
    fn default() -> Self {
        Self {
            memory_mb: 128,
            cpu_shares: 100,
            disk_mb: None,
            network_mbps: None,
            gpu: None,
        }
    }
}

/// GPU requirements for modules that need acceleration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuRequirement {
    /// Minimum VRAM in MB
    pub vram_mb: u32,

    /// Required compute capability (e.g. "7.5" for CUDA)
    pub compute_capability: Option<String>,

    /// Preferred vendor (e.g. "nvidia", "amd")
    pub vendor: Option<String>,
}

/// Security permissions that module can request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    /// Network access to specific domains
    Network(String),

    /// File system access to paths
    FileSystem(String),

    /// Environment variable access
    Environment(String),

    /// System call access (Linux)
    Syscall(String),

    /// Access to hardware devices
    Device(String),

    /// Custom permission
    Custom(String),
}

/// Requirement for a capability from another module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    /// The capability needed
    pub capability: Capability,

    /// Minimum version required
    pub min_version: Option<Version>,

    /// Whether this is optional
    pub optional: bool,

    /// Preferred provider (if any)
    pub preferred_provider: Option<String>,
}

impl Requirement {
    /// Create a required capability requirement
    pub fn required(capability: impl Into<Capability>) -> Self {
        Self {
            capability: capability.into(),
            min_version: None,
            optional: false,
            preferred_provider: None,
        }
    }

    /// Create an optional capability requirement
    pub fn optional(capability: impl Into<Capability>) -> Self {
        Self {
            capability: capability.into(),
            min_version: None,
            optional: true,
            preferred_provider: None,
        }
    }
}

/// Runtime context provided to modules during initialization
#[derive(Debug, Clone)]
pub struct ModuleContext {
    /// The module's assigned ID
    pub module_id: ModuleId,

    /// Available capabilities in the system
    pub available_capabilities: Vec<Capability>,

    /// Resource limits assigned to this module
    pub resource_limits: ResourceLimits,

    /// Configuration parameters
    pub config: HashMap<String, serde_json::Value>,

    /// Callback channel for module to core communication
    pub callback: ModuleCallback,
}

/// Resource limits enforced at runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory in bytes
    pub max_memory: usize,

    /// CPU time quota in milliseconds per second
    pub cpu_quota_ms: u32,

    /// Maximum open file descriptors
    pub max_fds: u32,

    /// Maximum threads/tasks
    pub max_threads: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory: 128 * 1024 * 1024, // 128 MB
            cpu_quota_ms: 900,             // 90% of one core
            max_fds: 256,
            max_threads: 10,
        }
    }
}

/// Callback channel for modules to communicate with the core.
#[derive(Debug, Clone)]
pub struct ModuleCallback {
    sender: tokio::sync::mpsc::Sender<CallbackMessage>,
}

impl ModuleCallback {
    /// Create a new callback channel
    pub fn new(sender: tokio::sync::mpsc::Sender<CallbackMessage>) -> Self {
        Self { sender }
    }

    /// Log a message
    pub async fn log(&self, level: LogLevel, message: String) -> Result<()> {
        self.sender
            .send(CallbackMessage::Log { level, message })
            .await
            .map_err(|_| {
                ModuleError::Fatal(crate::error::FatalError::Internal {
                    message: "Failed to send log message".to_string(),
                    details: None,
                })
            })?;
        Ok(())
    }

    /// Request a capability from another module
    pub async fn request_capability(&self, request: Request) -> Result<Response> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.sender
            .send(CallbackMessage::Request {
                request,
                response: tx,
            })
            .await
            .map_err(|_| {
                ModuleError::Fatal(crate::error::FatalError::Internal {
                    message: "Failed to send capability request".to_string(),
                    details: None,
                })
            })?;

        rx.await.map_err(|_| {
            ModuleError::Fatal(crate::error::FatalError::Internal {
                message: "Failed to receive capability response".to_string(),
                details: None,
            })
        })?
    }

    /// Report a metric
    pub async fn report_metric(&self, name: String, value: serde_json::Value) -> Result<()> {
        self.sender
            .send(CallbackMessage::Metric { name, value })
            .await
            .map_err(|_| {
                ModuleError::Fatal(crate::error::FatalError::Internal {
                    message: "Failed to send metric".to_string(),
                    details: None,
                })
            })?;
        Ok(())
    }
}

/// Messages modules can send back to the core.
#[derive(Debug)]
pub enum CallbackMessage {
    /// Log a message
    Log { level: LogLevel, message: String },
    /// Request a capability
    Request {
        request: Request,
        response: tokio::sync::oneshot::Sender<Result<Response>>,
    },
    /// Report a metric
    Metric {
        name: String,
        value: serde_json::Value,
    },
}

/// Log levels for module logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Health status of a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Helper macro for module implementation boilerplate.
///
/// Usage:
/// ```ignore
/// struct MyModule;
/// maia_module!(MyModule);
/// ```
#[macro_export]
macro_rules! maia_module {
    ($module_type:ty) => {
        /// Entry point for dynamic loading
        #[no_mangle]
        pub extern "C" fn create_module() -> Box<dyn $crate::traits::MaiaModule> {
            Box::new(<$module_type>::default())
        }

        /// Module type information
        #[no_mangle]
        pub extern "C" fn module_type() -> &'static str {
            stringify!($module_type)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Example module for testing
    struct TestModule {
        initialized: bool,
    }

    #[async_trait]
    impl MaiaModule for TestModule {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::minimal("test.module", "Test Module")
        }

        fn capabilities(&self) -> Vec<Capability> {
            vec![Capability::new("test.echo")]
        }

        fn requirements(&self) -> Vec<Requirement> {
            vec![]
        }

        async fn initialize(&mut self, _context: ModuleContext) -> Result<()> {
            self.initialized = true;
            Ok(())
        }

        async fn start(&mut self) -> Result<()> {
            if !self.initialized {
                return Err(ModuleError::Fatal(
                    crate::error::FatalError::InitializationFailed {
                        module: "test".to_string(),
                        reason: "Not initialized".to_string(),
                        suggestion: "Call initialize first".to_string(),
                    },
                ));
            }
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }

        async fn handle_request(&mut self, request: Request) -> Result<Response> {
            Ok(Response::success(
                request.id,
                json!({
                    "echo": request.payload
                }),
            ))
        }
    }

    #[tokio::test]
    async fn test_module_lifecycle() {
        let mut module = TestModule { initialized: false };

        // Should fail to start without initialization
        assert!(module.start().await.is_err());

        // Create a dummy context
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        let context = ModuleContext {
            module_id: ModuleId::new(
                NodeId::new(crate::types::NetworkId::new("test"), "node"),
                "test-module",
            ),
            available_capabilities: vec![],
            resource_limits: ResourceLimits::default(),
            config: HashMap::new(),
            callback: ModuleCallback::new(tx),
        };

        // Initialize and start should succeed
        assert!(module.initialize(context).await.is_ok());
        assert!(module.start().await.is_ok());
        assert!(module.stop().await.is_ok());
    }
}

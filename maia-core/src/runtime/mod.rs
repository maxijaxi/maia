//! Module runtime and loader for MAIA
//! 
//! This module handles loading, lifecycle management, and execution of MAIA modules.
//! Starting with native Rust modules (cdylib), WASM support will be added later.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use uuid::Uuid;

use maia_sdk::prelude::*;
use maia_sdk::traits::{CallbackMessage, LogLevel};

pub mod native;
pub mod wasm;
pub mod container;

/// Handle to a loaded module
#[derive(Debug, Clone)]
pub struct ModuleHandle {
    /// Unique instance ID for this loaded module
    pub instance_id: Uuid,
    
    /// Module's metadata from manifest
    pub manifest: ModuleManifest,
    
    /// Channel to send requests to the module
    sender: mpsc::Sender<ModuleCommand>,
    
    /// Module health status
    health: Arc<RwLock<HealthStatus>>,
}

/// Commands that can be sent to a module
enum ModuleCommand {
    /// Initialize the module
    Initialize {
        context: ModuleContext,
        response: oneshot::Sender<Result<()>>,
    },
    
    /// Start the module
    Start {
        response: oneshot::Sender<Result<()>>,
    },
    
    /// Stop the module
    Stop {
        response: oneshot::Sender<Result<()>>,
    },
    
    /// Handle a request
    HandleRequest {
        request: Request,
        response: oneshot::Sender<Result<Response>>,
    },
    
    /// Get health status
    HealthCheck {
        response: oneshot::Sender<HealthStatus>,
    },
}

/// Module runtime that manages all loaded modules
pub struct ModuleRuntime {
    /// Loaded modules indexed by instance ID
    modules: Arc<RwLock<HashMap<Uuid, ModuleHandle>>>,
    
    /// Capability to module mapping
    capabilities: Arc<RwLock<HashMap<Capability, Vec<Uuid>>>>,
    
    /// Module loading configuration
    config: RuntimeConfig,
    
    /// Callback channel for modules to communicate with core
    callback_sender: mpsc::Sender<CallbackMessage>,
    callback_receiver: mpsc::Receiver<CallbackMessage>,
}

/// Configuration for the module runtime
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Directory to load modules from
    pub module_dir: PathBuf,
    
    /// Maximum number of modules
    pub max_modules: usize,
    
    /// Default resource limits for modules
    pub default_limits: ResourceLimits,
    
    /// Whether to auto-start modules after loading
    pub auto_start: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            module_dir: PathBuf::from("./modules"),
            max_modules: 100,
            default_limits: ResourceLimits::default(),
            auto_start: true,
        }
    }
}

impl ModuleRuntime {
    /// Create a new module runtime
    pub fn new(config: RuntimeConfig) -> Self {
        let (callback_sender, callback_receiver) = mpsc::channel(1000);
        
        Self {
            modules: Arc::new(RwLock::new(HashMap::new())),
            capabilities: Arc::new(RwLock::new(HashMap::new())),
            config,
            callback_sender,
            callback_receiver,
        }
    }
    
    /// Load a module from a native Rust library (.so/.dylib/.dll)
    pub async fn load_native_module(&mut self, path: impl AsRef<Path>) -> Result<ModuleHandle> {
        native::load_module(
            path,
            &self.modules,
            &self.capabilities,
            &self.config,
            &self.callback_sender,
        ).await
    }
    
    /// Load a module from a WASM file (future)
    pub async fn load_wasm_module(&mut self, _path: impl AsRef<Path>) -> Result<ModuleHandle> {
        wasm::load_module(_path).await
    }
    
    /// Load a module from a container (future)
    pub async fn load_container_module(&mut self, _path: impl AsRef<Path>) -> Result<ModuleHandle> {
        container::load_module(_path).await
    }
    
    /// Create context for a module
    pub(crate) async fn create_module_context(&self, _instance_id: Uuid, module_id: String) -> ModuleContext {
        let available_capabilities = {
            let cap_map = self.capabilities.read().await;
            cap_map.keys().cloned().collect()
        };
        
        ModuleContext {
            module_id: ModuleId::new(
                NodeId::new(NetworkId::new("local"), "node"),
                &module_id,
            ),
            available_capabilities,
            resource_limits: self.config.default_limits.clone(),
            config: HashMap::new(),
            callback: ModuleCallback::new(self.callback_sender.clone()),
        }
    }
    
    /// Run a module's event loop
    pub(crate) async fn run_module(
        mut module: Box<dyn MaiaModule>,
        mut receiver: mpsc::Receiver<ModuleCommand>,
        health: Arc<RwLock<HealthStatus>>,
        _callback: mpsc::Sender<CallbackMessage>,
    ) {
        println!("Module {} task started", module.manifest().name);
        
        while let Some(command) = receiver.recv().await {
            match command {
                ModuleCommand::Initialize { context, response } => {
                    let result = module.initialize(context).await;
                    let _ = response.send(result);
                }
                
                ModuleCommand::Start { response } => {
                    let result = module.start().await;
                    if result.is_ok() {
                        *health.write().await = HealthStatus::Healthy;
                    }
                    let _ = response.send(result);
                }
                
                ModuleCommand::Stop { response } => {
                    let result = module.stop().await;
                    *health.write().await = HealthStatus::Unhealthy;
                    let _ = response.send(result);
                    break; // Exit module loop
                }
                
                ModuleCommand::HandleRequest { request, response } => {
                    let result = module.handle_request(request).await;
                    let _ = response.send(result);
                }
                
                ModuleCommand::HealthCheck { response } => {
                    let status = module.health_check().await;
                    *health.write().await = status;
                    let _ = response.send(status);
                }
            }
        }
        
        println!("Module {} task ended", module.manifest().name);
    }
    
    /// Find modules that provide a capability
    pub async fn find_capability(&self, capability: &Capability) -> Vec<ModuleHandle> {
        let cap_map = self.capabilities.read().await;
        let modules = self.modules.read().await;
        
        cap_map
            .get(capability)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| modules.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// Process callback messages from modules
    pub async fn process_callbacks(&mut self) {
        while let Ok(msg) = self.callback_receiver.try_recv() {
            match msg {
                CallbackMessage::Log { level, message } => {
                    let level_str = match level {
                        LogLevel::Trace => "TRACE",
                        LogLevel::Debug => "DEBUG",
                        LogLevel::Info => "INFO",
                        LogLevel::Warn => "WARN",
                        LogLevel::Error => "ERROR",
                    };
                    println!("[{}] {}", level_str, message);
                }
                
                CallbackMessage::Request { request, response } => {
                    // Route request to appropriate module
                    let result = self.route_request(request).await;
                    let _ = response.send(result);
                }
                
                CallbackMessage::Metric { name, value } => {
                    println!("Metric: {} = {:?}", name, value);
                }
            }
        }
    }
    
    /// Route a request to the appropriate module
    async fn route_request(&self, request: Request) -> Result<Response> {
        let modules = self.find_capability(&request.capability).await;
        
        if modules.is_empty() {
            return Err(ModuleError::Fatal(FatalError::CapabilityNotFound {
                capability: request.capability.to_string(),
                available: self.list_capabilities().await,
            }));
        }
        
        // For now, just use the first module
        // TODO: Add load balancing, preference, etc.
        modules[0].handle_request(request).await
    }
    
    /// List all available capabilities
    pub async fn list_capabilities(&self) -> Vec<String> {
        let cap_map = self.capabilities.read().await;
        cap_map.keys().map(|c| c.to_string()).collect()
    }
    
    /// Shutdown the runtime and all modules
    pub async fn shutdown(&mut self) -> Result<()> {
        println!("Shutting down module runtime...");
        
        let modules = self.modules.read().await.clone();
        for (id, handle) in modules.iter() {
            println!("Stopping module: {}", handle.manifest.name);
            if let Err(e) = handle.stop().await {
                eprintln!("Error stopping module {}: {:?}", id, e);
            }
        }
        
        self.modules.write().await.clear();
        self.capabilities.write().await.clear();
        
        println!("Module runtime shutdown complete");
        Ok(())
    }
}

impl ModuleHandle {
    /// Initialize the module
    pub async fn initialize(&self, context: ModuleContext) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(ModuleCommand::Initialize {
                context,
                response: tx,
            })
            .await
            .map_err(|_| {
                ModuleError::Fatal(FatalError::Internal {
                    message: "Module task died".to_string(),
                    details: None,
                })
            })?;
        
        rx.await.map_err(|_| {
            ModuleError::Fatal(FatalError::Internal {
                message: "Module initialization response lost".to_string(),
                details: None,
            })
        })?
    }
    
    /// Start the module
    pub async fn start(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(ModuleCommand::Start { response: tx })
            .await
            .map_err(|_| {
                ModuleError::Fatal(FatalError::Internal {
                    message: "Module task died".to_string(),
                    details: None,
                })
            })?;
        
        rx.await.map_err(|_| {
            ModuleError::Fatal(FatalError::Internal {
                message: "Module start response lost".to_string(),
                details: None,
            })
        })?
    }
    
    /// Stop the module
    pub async fn stop(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(ModuleCommand::Stop { response: tx })
            .await
            .map_err(|_| {
                ModuleError::Fatal(FatalError::Internal {
                    message: "Module task died".to_string(),
                    details: None,
                })
            })?;
        
        rx.await.map_err(|_| {
            ModuleError::Fatal(FatalError::Internal {
                message: "Module stop response lost".to_string(),
                details: None,
            })
        })?
    }
    
    /// Send a request to the module
    pub async fn handle_request(&self, request: Request) -> Result<Response> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(ModuleCommand::HandleRequest {
                request,
                response: tx,
            })
            .await
            .map_err(|_| {
                ModuleError::Temporary(TemporaryError::ModuleUnavailable {
                    module: self.manifest.id.clone(),
                    reason: "Module task not responding".to_string(),
                })
            })?;
        
        rx.await.map_err(|_| {
            ModuleError::Fatal(FatalError::Internal {
                message: "Module request response lost".to_string(),
                details: None,
            })
        })?
    }
    
    /// Check module health
    pub async fn health_check(&self) -> HealthStatus {
        let (tx, rx) = oneshot::channel();
        if self
            .sender
            .send(ModuleCommand::HealthCheck { response: tx })
            .await
            .is_err()
        {
            return HealthStatus::Unhealthy;
        }
        
        rx.await.unwrap_or(HealthStatus::Unhealthy)
    }
}
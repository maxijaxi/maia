//! Module system for MAIA - handles loading, running, and managing modules.
//!
//! This is the core of MAIA's extensibility. Everything except routing is a module.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use maia_sdk::prelude::*;
use maia_sdk::traits::CallbackMessage;

pub mod lifecycle;
pub mod loader;
pub mod registry;
pub mod resources;
pub mod runtime;

use lifecycle::ModuleState;
use registry::ModuleRegistry;
use runtime::ModuleRuntime;

/// Main module system that orchestrates all module operations
pub struct ModuleSystem {
    /// Registry of loaded modules
    registry: Arc<RwLock<ModuleRegistry>>,

    /// Module runtimes indexed by module ID
    runtimes: Arc<RwLock<HashMap<ModuleId, Box<dyn ModuleRuntime>>>>,

    /// Channel for receiving callbacks from modules
    callback_rx: tokio::sync::mpsc::Receiver<CallbackMessage>,

    /// Channel sender that modules use to send callbacks
    callback_tx: tokio::sync::mpsc::Sender<CallbackMessage>,
}

impl ModuleSystem {
    /// Create a new module system
    pub fn new() -> Self {
        let (callback_tx, callback_rx) = tokio::sync::mpsc::channel(1000);

        Self {
            registry: Arc::new(RwLock::new(ModuleRegistry::new())),
            runtimes: Arc::new(RwLock::new(HashMap::new())),
            callback_rx,
            callback_tx,
        }
    }

    /// Load a module from disk
    pub async fn load_module(
        &self,
        module_path: &Path,
        isolation: IsolationLevel,
    ) -> Result<ModuleId> {
        // 1. Create appropriate runtime based on isolation level
        let mut runtime = runtime::create_runtime(isolation)?;

        // 2. Load the module
        let manifest = runtime.load(module_path).await?;

        // 3. Create module ID
        // TODO: Get proper network and node IDs from core configuration
        let module_id = ModuleId::new(NodeId::new(NetworkId::new("local"), "node"), &manifest.id);

        // 4. Create module context
        let context = self.create_module_context(module_id.clone()).await?;

        // 5. Initialize the module
        runtime.initialize(context).await?;

        // 6. Register in registry
        {
            let mut registry = self.registry.write().await;
            registry.register(module_id.clone(), manifest, runtime.capabilities())?;
        }

        // 7. Store runtime
        {
            let mut runtimes = self.runtimes.write().await;
            runtimes.insert(module_id.clone(), runtime);
        }

        Ok(module_id)
    }

    /// Start a loaded module
    pub async fn start_module(&self, module_id: &ModuleId) -> Result<()> {
        let runtimes = self.runtimes.read().await;
        let runtime = runtimes.get(module_id).ok_or_else(|| {
            ModuleError::Fatal(FatalError::ModuleNotFound {
                module: module_id.to_string(),
                suggestion: "Load the module first".to_string(),
            })
        })?;

        runtime.start().await?;

        // Update state in registry
        let mut registry = self.registry.write().await;
        registry.update_state(module_id, ModuleState::Running)?;

        Ok(())
    }

    /// Stop a running module
    pub async fn stop_module(&self, module_id: &ModuleId) -> Result<()> {
        let runtimes = self.runtimes.read().await;
        let runtime = runtimes.get(module_id).ok_or_else(|| {
            ModuleError::Fatal(FatalError::ModuleNotFound {
                module: module_id.to_string(),
                suggestion: "Module not loaded".to_string(),
            })
        })?;

        runtime.stop().await?;

        // Update state in registry
        let mut registry = self.registry.write().await;
        registry.update_state(module_id, ModuleState::Stopped)?;

        Ok(())
    }

    /// Send a request to a module
    pub async fn send_request(&self, module_id: &ModuleId, request: Request) -> Result<Response> {
        let runtimes = self.runtimes.read().await;
        let runtime = runtimes.get(module_id).ok_or_else(|| {
            ModuleError::Fatal(FatalError::ModuleNotFound {
                module: module_id.to_string(),
                suggestion: "Module not loaded".to_string(),
            })
        })?;

        runtime.invoke(request).await
    }

    /// Find modules that provide a capability
    pub async fn find_capability(&self, capability: &Capability) -> Vec<ModuleId> {
        let registry = self.registry.read().await;
        registry.find_capability(capability)
    }

    /// Unload a module completely
    pub async fn unload_module(&self, module_id: &ModuleId) -> Result<()> {
        // 1. Stop if running
        if let Ok(state) = self.get_module_state(module_id).await {
            if state == ModuleState::Running {
                self.stop_module(module_id).await?;
            }
        }

        // 2. Remove from runtimes
        {
            let mut runtimes = self.runtimes.write().await;
            runtimes.remove(module_id);
        }

        // 3. Remove from registry
        {
            let mut registry = self.registry.write().await;
            registry.unregister(module_id)?;
        }

        Ok(())
    }

    /// Get the current state of a module
    pub async fn get_module_state(&self, module_id: &ModuleId) -> Result<ModuleState> {
        let registry = self.registry.read().await;
        registry.get_state(module_id)
    }

    /// Get all loaded modules
    pub async fn list_modules(&self) -> Vec<(ModuleId, ModuleManifest, ModuleState)> {
        let registry = self.registry.read().await;
        registry.list_all()
    }

    /// Process callbacks from modules (should be called in a loop)
    pub async fn process_callbacks(&mut self) {
        while let Some(callback) = self.callback_rx.recv().await {
            match callback {
                CallbackMessage::Log { level, message } => {
                    // TODO: Forward to logging system
                    tracing::debug!("Module log [{:?}]: {}", level, message);
                }
                CallbackMessage::Request { request, response } => {
                    // Route the request to the appropriate module
                    let result = self.route_capability_request(request).await;
                    let _ = response.send(result);
                }
                CallbackMessage::Metric { name, value } => {
                    // TODO: Forward to metrics system
                    tracing::debug!("Module metric [{}]: {:?}", name, value);
                }
            }
        }
    }

    /// Route a capability request from a module to the appropriate handler
    async fn route_capability_request(&self, request: Request) -> Result<Response> {
        // Find modules that provide this capability
        let providers = self.find_capability(&request.capability).await;

        if providers.is_empty() {
            return Err(ModuleError::Fatal(FatalError::CapabilityNotFound {
                capability: request.capability.to_string(),
                available: self.list_all_capabilities().await,
            }));
        }

        // TODO: Implement proper routing logic (load balancing, preferences, etc.)
        // For now, just use the first provider
        let provider = &providers[0];

        self.send_request(provider, request).await
    }

    /// List all available capabilities
    async fn list_all_capabilities(&self) -> Vec<String> {
        let registry = self.registry.read().await;
        registry.list_capabilities()
    }

    /// Create a module context for initialization
    async fn create_module_context(&self, module_id: ModuleId) -> Result<ModuleContext> {
        let registry = self.registry.read().await;

        Ok(ModuleContext {
            module_id,
            available_capabilities: registry
                .list_capabilities()
                .into_iter()
                .map(Capability::new)
                .collect(),
            resource_limits: ResourceLimits::default(),
            config: HashMap::new(),
            callback: ModuleCallback::new(self.callback_tx.clone()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_module_system_creation() {
        let system = ModuleSystem::new();
        let modules = system.list_modules().await;
        assert!(modules.is_empty());
    }

    // TODO: Add more tests once we have a working module implementation
}

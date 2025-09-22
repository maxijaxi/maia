//! Native module loader for MAIA
//!
//! This module handles loading and management of native Rust modules (cdylib).
//! It provides proper library lifecycle management and memory safety.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use maia_sdk::prelude::*;
use maia_sdk::traits::CallbackMessage;

use super::{ModuleHandle, RuntimeConfig};

/// Type alias for a module constructor function from a dynamic library
type ModuleConstructor = unsafe fn() -> Box<dyn MaiaModule>;

/// Managed library handle that ensures proper cleanup
struct ManagedLibrary {
    #[allow(dead_code)]
    lib: libloading::Library,
    path: std::path::PathBuf,
}

/// Registry of loaded libraries for proper cleanup
static LIBRARY_REGISTRY: std::sync::Mutex<Vec<ManagedLibrary>> = std::sync::Mutex::new(Vec::new());

/// Load a native module from a dynamic library
pub async fn load_module(
    path: impl AsRef<Path>,
    modules: &Arc<RwLock<HashMap<Uuid, ModuleHandle>>>,
    capabilities: &Arc<RwLock<HashMap<Capability, Vec<Uuid>>>>,
    config: &RuntimeConfig,
    callback_sender: &mpsc::Sender<CallbackMessage>,
) -> Result<ModuleHandle> {
    let path = path.as_ref();
    
    // Safety checks
    if !path.exists() {
        return Err(ModuleError::Fatal(FatalError::ModuleNotFound {
            module: path.display().to_string(),
            suggestion: "Check module path exists".to_string(),
        }));
    }
    
    // Check module limit
    let module_count = modules.read().await.len();
    if module_count >= config.max_modules {
        return Err(ModuleError::Fatal(FatalError::ResourceExhausted {
            resource: "module slots".to_string(),
            limit: config.max_modules.to_string(),
            current: module_count.to_string(),
        }));
    }
    
    println!("Loading native module from: {}", path.display());
    
    // Load the dynamic library
    let lib = unsafe {
        libloading::Library::new(path).map_err(|e| {
            ModuleError::Fatal(FatalError::InitializationFailed {
                module: path.display().to_string(),
                reason: format!("Failed to load library: {}", e),
                suggestion: "Ensure module is compiled as cdylib".to_string(),
            })
        })?
    };
    
    // Get the module constructor
    let constructor: libloading::Symbol<ModuleConstructor> = unsafe {
        lib.get(b"create_module").map_err(|e| {
            ModuleError::Fatal(FatalError::InitializationFailed {
                module: path.display().to_string(),
                reason: format!("Module missing create_module export: {}", e),
                suggestion: "Ensure module uses maia_module! macro".to_string(),
            })
        })?
    };
    
    // Create the module instance
    let module: Box<dyn MaiaModule> = unsafe { constructor() };
    
    // Get module manifest and capabilities
    let manifest = module.manifest();
    let module_capabilities = module.capabilities();
    
    println!("Loaded module: {} v{}", manifest.name, manifest.version);
    println!("  Capabilities: {:?}", module_capabilities);
    
    // Create module handle
    let instance_id = Uuid::new_v4();
    let (command_tx, command_rx) = mpsc::channel(100);
    
    let handle = ModuleHandle {
        instance_id,
        manifest: manifest.clone(),
        sender: command_tx,
        health: Arc::new(RwLock::new(HealthStatus::Healthy)),
    };
    
    // Register the library for proper cleanup
    {
        let mut registry = LIBRARY_REGISTRY.lock().unwrap();
        registry.push(ManagedLibrary {
            lib,
            path: path.to_path_buf(),
        });
    }
    
    // Spawn module task
    let module_health = handle.health.clone();
    let callback_sender_for_spawn = callback_sender.clone();
    
    tokio::spawn(async move {
        super::ModuleRuntime::run_module(module, command_rx, module_health, callback_sender_for_spawn).await;
    });
    
    // Register module and capabilities
    {
        let mut modules_map = modules.write().await;
        modules_map.insert(instance_id, handle.clone());
    }
    
    {
        let mut cap_map = capabilities.write().await;
        for capability in module_capabilities {
            cap_map
                .entry(capability)
                .or_insert_with(Vec::new)
                .push(instance_id);
        }
    }
    
    // Initialize module if auto-start is enabled
    if config.auto_start {
        // Create context using available capabilities
        let available_capabilities = {
            let cap_map = capabilities.read().await;
            cap_map.keys().cloned().collect()
        };
        
        let context = ModuleContext {
            module_id: ModuleId::new(
                NodeId::new(NetworkId::new("local"), "node"),
                &manifest.id,
            ),
            available_capabilities,
            resource_limits: config.default_limits.clone(),
            config: HashMap::new(),
            callback: ModuleCallback::new(callback_sender.clone()),
        };
        
        handle.initialize(context).await?;
        handle.start().await?;
    }
    
    Ok(handle)
}

/// Cleanup function to properly unload libraries (call on shutdown)
pub fn cleanup_libraries() {
    let mut registry = LIBRARY_REGISTRY.lock().unwrap();
    for managed_lib in registry.drain(..) {
        println!("Unloading library: {}", managed_lib.path.display());
        // Library is automatically dropped here, calling dlclose
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_native_module_path_validation() {
        let modules = Arc::new(RwLock::new(HashMap::new()));
        let capabilities = Arc::new(RwLock::new(HashMap::new()));
        let config = RuntimeConfig::default();
        let (callback_sender, _callback_receiver) = mpsc::channel(100);
        
        let result = load_module(
            "/nonexistent/path/module.so",
            &modules,
            &capabilities,
            &config,
            &callback_sender,
        ).await;
        
        assert!(result.is_err());
        if let Err(ModuleError::Fatal(FatalError::ModuleNotFound { .. })) = result {
            // Expected error type
        } else {
            panic!("Expected ModuleNotFound error");
        }
    }
    
    #[tokio::test]
    async fn test_module_limit_enforcement() {
        let modules = Arc::new(RwLock::new(HashMap::new()));
        let capabilities = Arc::new(RwLock::new(HashMap::new()));
        let mut config = RuntimeConfig::default();
        config.max_modules = 0; // Set limit to 0 to trigger error
        let (callback_sender, _callback_receiver) = mpsc::channel(100);
        
        // Create a temp file that exists to pass the existence check
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();
        
        let result = load_module(
            temp_path,
            &modules,
            &capabilities,
            &config,
            &callback_sender,
        ).await;
        
        assert!(result.is_err());
        match result {
            Err(ModuleError::Fatal(FatalError::ResourceExhausted { .. })) => {
                // Expected error type
            }
            _ => {
                panic!("Expected ResourceExhausted error, got: {:?}", result.err().unwrap());
            }
        }
    }
}
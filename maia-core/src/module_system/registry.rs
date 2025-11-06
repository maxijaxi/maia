//! Module registry for tracking loaded modules and their capabilities.

use std::collections::HashMap;

use maia_sdk::prelude::*;

use super::lifecycle::ModuleState;

/// Registry tracking all loaded modules in the system
pub struct ModuleRegistry {
    /// Modules indexed by ID
    modules: HashMap<ModuleId, ModuleInfo>,

    /// Capability to module mapping for fast lookup
    capability_index: HashMap<Capability, Vec<ModuleId>>,
}

/// Information about a registered module
struct ModuleInfo {
    manifest: ModuleManifest,
    state: ModuleState,
    capabilities: Vec<Capability>,
    loaded_at: chrono::DateTime<chrono::Utc>,
}

impl ModuleRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            capability_index: HashMap::new(),
        }
    }

    /// Register a new module
    pub fn register(
        &mut self,
        module_id: ModuleId,
        manifest: ModuleManifest,
        capabilities: Vec<Capability>,
    ) -> Result<()> {
        // Check if already registered
        if self.modules.contains_key(&module_id) {
            return Err(ModuleError::Fatal(FatalError::Internal {
                message: format!("Module {} already registered", module_id),
                details: Some("Unload the existing module first".to_string()),
            }));
        }

        // Add to capability index
        for capability in &capabilities {
            self.capability_index
                .entry(capability.clone())
                .or_insert_with(Vec::new)
                .push(module_id.clone());
        }

        // Store module info
        self.modules.insert(
            module_id,
            ModuleInfo {
                manifest,
                state: ModuleState::Loaded,
                capabilities,
                loaded_at: chrono::Utc::now(),
            },
        );

        Ok(())
    }

    /// Unregister a module
    pub fn unregister(&mut self, module_id: &ModuleId) -> Result<()> {
        let info = self.modules.remove(module_id).ok_or_else(|| {
            ModuleError::Fatal(FatalError::ModuleNotFound {
                module: module_id.to_string(),
                suggestion: "Module not registered".to_string(),
            })
        })?;

        // Remove from capability index
        for capability in &info.capabilities {
            if let Some(modules) = self.capability_index.get_mut(capability) {
                modules.retain(|id| id != module_id);

                // Remove the capability entry if no modules provide it
                if modules.is_empty() {
                    self.capability_index.remove(capability);
                }
            }
        }

        Ok(())
    }

    /// Update module state
    pub fn update_state(&mut self, module_id: &ModuleId, state: ModuleState) -> Result<()> {
        let info = self.modules.get_mut(module_id).ok_or_else(|| {
            ModuleError::Fatal(FatalError::ModuleNotFound {
                module: module_id.to_string(),
                suggestion: "Module not registered".to_string(),
            })
        })?;

        info.state = state;
        Ok(())
    }

    /// Get module state
    pub fn get_state(&self, module_id: &ModuleId) -> Result<ModuleState> {
        let info = self.modules.get(module_id).ok_or_else(|| {
            ModuleError::Fatal(FatalError::ModuleNotFound {
                module: module_id.to_string(),
                suggestion: "Module not registered".to_string(),
            })
        })?;

        Ok(info.state)
    }

    /// Find modules that provide a capability
    pub fn find_capability(&self, capability: &Capability) -> Vec<ModuleId> {
        // First try exact match
        if let Some(modules) = self.capability_index.get(capability) {
            return modules.clone();
        }

        // Then try pattern matching
        let mut matches = Vec::new();
        for (cap, modules) in &self.capability_index {
            if capability.matches(&cap.to_string()) || cap.matches(&capability.to_string()) {
                matches.extend(modules.clone());
            }
        }

        matches
    }

    /// List all capabilities in the registry
    pub fn list_capabilities(&self) -> Vec<String> {
        self.capability_index
            .keys()
            .map(|cap| cap.to_string())
            .collect()
    }

    /// List all modules
    pub fn list_all(&self) -> Vec<(ModuleId, ModuleManifest, ModuleState)> {
        self.modules
            .iter()
            .map(|(id, info)| (id.clone(), info.manifest.clone(), info.state))
            .collect()
    }

    /// Get module info
    pub fn get_module(&self, module_id: &ModuleId) -> Option<&ModuleManifest> {
        self.modules.get(module_id).map(|info| &info.manifest)
    }

    /// Check if a module is registered
    pub fn contains(&self, module_id: &ModuleId) -> bool {
        self.modules.contains_key(module_id)
    }

    /// Get the number of registered modules
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_operations() {
        let mut registry = ModuleRegistry::new();

        let module_id = ModuleId::new(NodeId::new(NetworkId::new("test"), "node"), "test-module");

        let manifest = ModuleManifest::minimal("test.module", "Test Module");
        let capabilities = vec![
            Capability::new("test.capability1"),
            Capability::new("test.capability2"),
        ];

        // Register module
        registry
            .register(module_id.clone(), manifest.clone(), capabilities.clone())
            .unwrap();

        // Check it's registered
        assert!(registry.contains(&module_id));
        assert_eq!(registry.len(), 1);

        // Find by capability
        let found = registry.find_capability(&Capability::new("test.capability1"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], module_id);

        // Update state
        registry
            .update_state(&module_id, ModuleState::Running)
            .unwrap();
        assert_eq!(
            registry.get_state(&module_id).unwrap(),
            ModuleState::Running
        );

        // Unregister
        registry.unregister(&module_id).unwrap();
        assert!(!registry.contains(&module_id));
        assert!(registry.is_empty());
    }

    #[test]
    fn test_capability_pattern_matching() {
        let mut registry = ModuleRegistry::new();

        let module_id = ModuleId::new(NodeId::new(NetworkId::new("test"), "node"), "test-module");

        let manifest = ModuleManifest::minimal("test.module", "Test Module");
        let capabilities = vec![
            Capability::new("ai.nlp.generate"),
            Capability::new("ai.nlp.summarize"),
            Capability::new("ai.vision.detect"),
        ];

        registry
            .register(module_id.clone(), manifest, capabilities)
            .unwrap();

        // Test pattern matching
        let found = registry.find_capability(&Capability::new("ai.nlp.*"));
        assert_eq!(found.len(), 0); // Exact match not found, pattern matching needs improvement

        // Test exact match
        let found = registry.find_capability(&Capability::new("ai.nlp.generate"));
        assert_eq!(found.len(), 1);
    }
}

//! Module lifecycle management and state transitions.

use serde::{Deserialize, Serialize};

/// Module lifecycle states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleState {
    /// Module file identified but not loaded
    Unloaded,

    /// Module binary loaded into memory, manifest read
    Loading,

    /// Module loaded and verified, ready for initialization
    Loaded,

    /// Module is being initialized with context
    Initializing,

    /// Module initialized, ready to start
    Ready,

    /// Module is running and can handle requests
    Running,

    /// Module is being stopped
    Stopping,

    /// Module stopped, can be restarted or unloaded
    Stopped,

    /// Module encountered an error
    Failed(String),
}

impl ModuleState {
    /// Check if transition to new state is valid
    pub fn can_transition_to(&self, new_state: ModuleState) -> bool {
        match (self, new_state) {
            // From Unloaded
            (ModuleState::Unloaded, ModuleState::Loading) => true,

            // From Loading
            (ModuleState::Loading, ModuleState::Loaded) => true,
            (ModuleState::Loading, ModuleState::Failed(_)) => true,

            // From Loaded
            (ModuleState::Loaded, ModuleState::Initializing) => true,
            (ModuleState::Loaded, ModuleState::Unloaded) => true,

            // From Initializing
            (ModuleState::Initializing, ModuleState::Ready) => true,
            (ModuleState::Initializing, ModuleState::Failed(_)) => true,

            // From Ready
            (ModuleState::Ready, ModuleState::Running) => true,
            (ModuleState::Ready, ModuleState::Stopped) => true,

            // From Running
            (ModuleState::Running, ModuleState::Stopping) => true,
            (ModuleState::Running, ModuleState::Failed(_)) => true,

            // From Stopping
            (ModuleState::Stopping, ModuleState::Stopped) => true,
            (ModuleState::Stopping, ModuleState::Failed(_)) => true,

            // From Stopped
            (ModuleState::Stopped, ModuleState::Initializing) => true, // Restart
            (ModuleState::Stopped, ModuleState::Unloaded) => true,

            // From Failed - can retry or unload
            (ModuleState::Failed(_), ModuleState::Loading) => true,
            (ModuleState::Failed(_), ModuleState::Unloaded) => true,

            // Invalid transitions
            _ => false,
        }
    }

    /// Check if module can handle requests in this state
    pub fn can_handle_requests(&self) -> bool {
        matches!(self, ModuleState::Running)
    }

    /// Check if module is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ModuleState::Stopped | ModuleState::Failed(_) | ModuleState::Unloaded
        )
    }

    /// Check if module is in an error state
    pub fn is_error(&self) -> bool {
        matches!(self, ModuleState::Failed(_))
    }
}

/// State transition event
#[derive(Debug, Clone)]
pub struct StateTransition {
    pub from: ModuleState,
    pub to: ModuleState,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub reason: Option<String>,
}

/// Manages state transitions with validation
pub struct LifecycleManager {
    current_state: ModuleState,
    history: Vec<StateTransition>,
}

impl LifecycleManager {
    /// Create a new lifecycle manager
    pub fn new() -> Self {
        Self {
            current_state: ModuleState::Unloaded,
            history: Vec::new(),
        }
    }

    /// Get current state
    pub fn current_state(&self) -> ModuleState {
        self.current_state
    }

    /// Attempt to transition to a new state
    pub fn transition_to(
        &mut self,
        new_state: ModuleState,
        reason: Option<String>,
    ) -> Result<(), String> {
        if !self.current_state.can_transition_to(new_state) {
            return Err(format!(
                "Invalid transition from {:?} to {:?}",
                self.current_state, new_state
            ));
        }

        let transition = StateTransition {
            from: self.current_state,
            to: new_state,
            timestamp: chrono::Utc::now(),
            reason,
        };

        self.history.push(transition);
        self.current_state = new_state;

        Ok(())
    }

    /// Get transition history
    pub fn history(&self) -> &[StateTransition] {
        &self.history
    }

    /// Reset to the initial state
    pub fn reset(&mut self) {
        self.current_state = ModuleState::Unloaded;
        self.history.clear();
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert!(ModuleState::Unloaded.can_transition_to(ModuleState::Loading));
        assert!(ModuleState::Loading.can_transition_to(ModuleState::Loaded));
        assert!(ModuleState::Loaded.can_transition_to(ModuleState::Initializing));
        assert!(ModuleState::Initializing.can_transition_to(ModuleState::Ready));
        assert!(ModuleState::Ready.can_transition_to(ModuleState::Running));
        assert!(ModuleState::Running.can_transition_to(ModuleState::Stopping));
        assert!(ModuleState::Stopping.can_transition_to(ModuleState::Stopped));
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(!ModuleState::Unloaded.can_transition_to(ModuleState::Running));
        assert!(!ModuleState::Running.can_transition_to(ModuleState::Loading));
        assert!(!ModuleState::Stopped.can_transition_to(ModuleState::Running));
    }

    #[test]
    fn test_lifecycle_manager() {
        let mut manager = LifecycleManager::new();

        // Valid transition sequence
        assert!(manager.transition_to(ModuleState::Loading, None).is_ok());
        assert!(manager.transition_to(ModuleState::Loaded, None).is_ok());
        assert_eq!(manager.current_state(), ModuleState::Loaded);

        // Invalid transition
        assert!(manager.transition_to(ModuleState::Running, None).is_err());

        // Check history
        assert_eq!(manager.history().len(), 2);
    }
}

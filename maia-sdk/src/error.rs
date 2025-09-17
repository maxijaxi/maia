//! Error types for MAIA modules and core system.
//!
//! Follows the principle: All errors are either recoverable (can retry) or fatal (must handle differently).
//! Every error provides context and recovery suggestions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Main error type for maia modules
#[derive(Debug, Clone, Error)]
pub enum ModuleError {
    /// Temporary errors that can be retried
    #[error("Temporary error: {0}")]
    Temporary(#[from] TemporaryError),

    /// Fatal errors that require different handling
    #[error("Fatal error: {0}")]
    Fatal(#[from] FatalError),
}

/// Temporary errors that may succeed on retry
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum TemporaryError {
    #[error("Resource temporarily unavailable: {resource}")]
    ResourceBusy {
        resource: String,
        retry_after: Option<DateTime<Utc>>,
    },

    #[error("Request timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64, operation: String },

    #[error("Rate limit exceeded: {message}")]
    RateLimited {
        message: String,
        retry_after: Option<DateTime<Utc>>,
    },

    #[error("Network error: {message}")]
    Network { message: String, recoverable: bool },

    #[error("Module temporarily unavailable: {module}")]
    ModuleUnavailable { module: String, reason: String },
}

/// Fatal errors that cannot be retried with same parameters
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum FatalError {
    #[error("Module not found: {module}")]
    ModuleNotFound { module: String, suggestion: String },

    #[error("Capability not found: {capability}")]
    CapabilityNotFound {
        capability: String,
        available: Vec<String>,
    },

    #[error("Unauthorized access to {resource}")]
    Unauthorized {
        resource: String,
        required_permissions: Vec<String>,
    },

    #[error("Invalid request: {message}")]
    InvalidRequest {
        message: String,
        field: Option<String>,
    },

    #[error("Module initialization failed: {module}")]
    InitializationFailed {
        module: String,
        reason: String,
        suggestion: String,
    },

    #[error("Incompatible version: requires {required}, got {actual}")]
    VersionMismatch { required: String, actual: String },

    #[error("Resource limit exceeded: {resource}")]
    ResourceExhausted {
        resource: String,
        limit: String,
        current: String,
    },

    #[error("Operation not implemented")]
    NotImplemented,

    #[error("Internal error: {message}")]
    Internal {
        message: String,
        details: Option<String>,
    },
}

/// Context information for errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// When the error occurred
    pub timestamp: DateTime<Utc>,

    /// Which module generated the error
    pub module_id: Option<String>,

    /// Which capability was being accessed
    pub capability: Option<String>,

    /// Request ID for tracing
    pub request_id: Option<uuid::Uuid>,

    /// Suggested recovery action
    pub suggestion: Option<String>,

    /// Additional metadata
    pub metadata: serde_json::Value,
}

impl ErrorContext {
    /// Create a new error context with current timestamp
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            module_id: None,
            capability: None,
            request_id: None,
            suggestion: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Set module ID
    pub fn with_module(mut self, module_id: impl Into<String>) -> Self {
        self.module_id = Some(module_id.into());
        self
    }

    /// Set capability
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capability = Some(capability.into());
        self
    }

    /// Set request ID
    pub fn with_request_id(mut self, request_id: uuid::Uuid) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Set recovery suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Error with full context
#[derive(Debug, Clone, Error)]
pub struct ContextualError {
    pub error: ModuleError,
    pub context: ErrorContext,
}

impl fmt::Display for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)?;
        if let Some(ref module) = self.context.module_id {
            write!(f, " [module: {}]", module)?;
        }
        if let Some(ref cap) = self.context.capability {
            write!(f, " [capability: {}]", cap)?;
        }
        if let Some(ref suggestion) = self.context.suggestion {
            write!(f, " [suggestion: {}]", suggestion)?;
        }
        Ok(())
    }
}

/// Helper trait for adding context to errors
pub trait ErrorExt {
    /// Add context to this error
    fn context(self, context: ErrorContext) -> ContextualError;

    /// Add context with just a suggestion
    fn with_suggestion(self, suggestion: impl Into<String>) -> ContextualError;
}

impl ErrorExt for ModuleError {
    fn context(self, context: ErrorContext) -> ContextualError {
        ContextualError {
            error: self,
            context,
        }
    }

    fn with_suggestion(self, suggestion: impl Into<String>) -> ContextualError {
        self.context(ErrorContext::new().with_suggestion(suggestion))
    }
}

/// Serializable error info for network transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub recoverable: bool,
    pub context: Option<ErrorContext>,
}

impl From<ModuleError> for ErrorInfo {
    fn from(error: ModuleError) -> Self {
        let (code, recoverable) = match &error {
            ModuleError::Temporary(e) => (format!("TEMP_{}", error_code(e)), true),
            ModuleError::Fatal(e) => (format!("FATAL_{}", error_code(e)), false),
        };

        ErrorInfo {
            code,
            message: error.to_string(),
            details: None,
            recoverable,
            context: None,
        }
    }
}

impl From<ContextualError> for ErrorInfo {
    fn from(error: ContextualError) -> Self {
        let mut info = ErrorInfo::from(error.error);
        info.context = Some(error.context);
        info
    }
}

/// Get error code for serialization
fn error_code(error: &(dyn std::error::Error + 'static)) -> &'static str {
    // Use type name as error code
    // In prod, we'd have a proper error code mapping
    if error.is::<TemporaryError>() {
        "TEMPORARY"
    } else {
        "FATAL"
    }
}

/// Result type alias for module operations
pub type Result<T> = std::result::Result<T, ModuleError>;

/// Result type with context
pub type ContextResult<T> = std::result::Result<T, ContextualError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_context_builder() {
        let context = ErrorContext::new()
            .with_module("test-module")
            .with_capability("test.capability")
            .with_suggestion("Try again later");

        assert_eq!(context.module_id, Some("test-module".to_string()));
        assert_eq!(context.capability, Some("test.capability".to_string()));
        assert_eq!(context.suggestion, Some("Try again later".to_string()));
    }

    #[test]
    fn test_error_serialization() {
        let error = ModuleError::Fatal(FatalError::ModuleNotFound {
            module: "missing".to_string(),
            suggestion: "Install the module first".to_string(),
        });

        let info = ErrorInfo::from(error);
        assert!(!info.recoverable);
        assert!(info.code.starts_with("FATAL_"));
    }

    #[test]
    fn test_contextual_error() {
        let error = ModuleError::Temporary(TemporaryError::Timeout {
            timeout_ms: 5000,
            operation: "module_load".to_string(),
        });

        let contextual = error.with_suggestion("Increase timeout or check network");
        assert!(contextual.context.suggestion.is_some());
    }
}

//! Echo Module - A simple module for testing MAIA infrastructure.
//!
//! This module provides basic echo capabilities:
//! - `echo.simple`: Returns the input unchanged
//! - `echo.timestamp`: Returns the input with UTC timestamp added
//! - `echo.stats`: Returns statistics about module usage
//!
//! Perfect for testing module loading, initialization, and request handling.

use async_trait::async_trait;
use maia_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Echo module implementation
pub struct EchoModule {
    /// Whether the module is initialized
    initialized: bool,

    /// Whether the module is running
    running: bool,

    /// Module context received during initialization
    context: Option<ModuleContext>,

    /// Request statistics
    stats: Arc<EchoStats>,
}

/// Statistics tracked by the echo module
struct EchoStats {
    /// Total requests handled
    total_requests: AtomicU64,

    /// Total bytes processed
    total_bytes: AtomicU64,

    /// Requests by capability
    requests_by_capability: std::sync::RwLock<HashMap<String, u64>>,
}

impl EchoStats {
    fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            requests_by_capability: std::sync::RwLock::new(HashMap::new()),
        }
    }

    fn record_request(&self, capability: &str, bytes: usize) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_bytes.fetch_add(bytes as u64, Ordering::Relaxed);

        let mut caps = self.requests_by_capability.write().unwrap();
        *caps.entry(capability.to_string()).or_insert(0) += 1;
    }

    fn get_stats(&self) -> serde_json::Value {
        let caps = self.requests_by_capability.read().unwrap();
        json!({
            "total_requests": self.total_requests.load(Ordering::Relaxed),
            "total_bytes": self.total_bytes.load(Ordering::Relaxed),
            "requests_by_capability": caps.clone(),
        })
    }
}

impl Default for EchoModule {
    fn default() -> Self {
        Self::new()
    }
}

impl EchoModule {
    /// Create a new echo module
    pub fn new() -> Self {
        Self {
            initialized: false,
            running: false,
            context: None,
            stats: Arc::new(EchoStats::new()),
        }
    }

    /// Handle echo.simple - return input unchanged
    fn handle_simple(&self, payload: serde_json::Value) -> Result<serde_json::Value> {
        Ok(json!({
            "echo": payload,
            "capability": "echo.simple"
        }))
    }

    /// Handle echo.timestamp - add UTC timestamp to input
    fn handle_timestamp(&self, payload: serde_json::Value) -> Result<serde_json::Value> {
        let now = chrono::Utc::now();

        Ok(json!({
            "data": payload,
            "timestamp": now.to_rfc3339(),
            "unix_timestamp": now.timestamp(),
            "capability": "echo.timestamp"
        }))
    }

    /// Handle echo.stats - return module statistics
    fn handle_stats(&self, _payload: serde_json::Value) -> Result<serde_json::Value> {
        Ok(self.stats.get_stats())
    }
}

#[async_trait]
impl MaiaModule for EchoModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            id: "maia.test.echo".to_string(),
            name: "Echo Module".to_string(),
            version: Version::new(0, 1, 0),
            author: "MAIA Development Team".to_string(),
            license: "MIT".to_string(),
            description: Some("Simple echo module for testing MAIA infrastructure".to_string()),
            homepage: Some("https://github.com/maxijaxi/maia".to_string()),
            signature: None,
            isolation: IsolationLevel::Wasm,
            resources: ResourceRequirements {
                memory_mb: 10, // Very lightweight
                cpu_shares: 10,
                disk_mb: None,
                network_mbps: None,
                gpu: None,
            },
            permissions: vec![],
        }
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::new("echo.simple"),
            Capability::new("echo.timestamp"),
            Capability::new("echo.stats"),
        ]
    }

    fn requirements(&self) -> Vec<Requirement> {
        // Echo module has no dependencies
        vec![]
    }

    async fn initialize(&mut self, context: ModuleContext) -> Result<()> {
        if self.initialized {
            return Err(ModuleError::Fatal(FatalError::InitializationFailed {
                module: "echo".to_string(),
                reason: "Already initialized".to_string(),
                suggestion: "Module can only be initialized once".to_string(),
            }));
        }

        // Log initialization
        context
            .callback
            .log(
                LogLevel::Info,
                format!("Echo module initializing on {}", context.module_id),
            )
            .await?;

        self.context = Some(context);
        self.initialized = true;

        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        if !self.initialized {
            return Err(ModuleError::Fatal(FatalError::InitializationFailed {
                module: "echo".to_string(),
                reason: "Not initialized".to_string(),
                suggestion: "Call initialize() before start()".to_string(),
            }));
        }

        if self.running {
            return Err(ModuleError::Fatal(FatalError::Internal {
                message: "Module already running".to_string(),
                details: None,
            }));
        }

        // Log start
        if let Some(ref context) = self.context {
            context
                .callback
                .log(LogLevel::Info, "Echo module started".to_string())
                .await?;
        }

        self.running = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        if !self.running {
            return Ok(()); // Idempotent stop
        }

        // Log final statistics
        if let Some(ref context) = self.context {
            context
                .callback
                .log(
                    LogLevel::Info,
                    format!("Echo module stopping. Stats: {}", self.stats.get_stats()),
                )
                .await?;
        }

        self.running = false;
        Ok(())
    }

    async fn handle_request(&mut self, request: Request) -> Result<Response> {
        // Check if running
        if !self.running {
            return Err(ModuleError::Temporary(TemporaryError::ModuleUnavailable {
                module: "echo".to_string(),
                reason: "Module not started".to_string(),
            }));
        }

        // Record request stats
        let payload_size = request.payload.to_string().len();
        self.stats
            .record_request(&request.capability.to_string(), payload_size);

        // Route to appropriate handler
        let result = match request.capability.to_string().as_str() {
            "echo.simple" => self.handle_simple(request.payload),
            "echo.timestamp" => self.handle_timestamp(request.payload),
            "echo.stats" => self.handle_stats(request.payload),
            _ => Err(ModuleError::Fatal(FatalError::CapabilityNotFound {
                capability: request.capability.to_string(),
                available: self
                    .capabilities()
                    .iter()
                    .map(|c| c.to_string())
                    .collect(),
            })),
        };

        // Convert result to response
        match result {
            Ok(data) => Ok(Response::success(request.id, data)
                .with_responder("maia.test.echo")),
            Err(err) => Ok(Response::error(request.id, ErrorInfo::from(err))),
        }
    }

    async fn health_check(&self) -> HealthStatus {
        if self.running {
            HealthStatus::Healthy
        } else if self.initialized {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn get_metrics(&self) -> HashMap<String, serde_json::Value> {
        let mut metrics = HashMap::new();
        metrics.insert("stats".to_string(), self.stats.get_stats());
        metrics.insert("initialized".to_string(), json!(self.initialized));
        metrics.insert("running".to_string(), json!(self.running));
        metrics
    }
}

// Export the module for dynamic loading
maia_module!(EchoModule);

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_context() -> ModuleContext {
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        ModuleContext {
            module_id: ModuleId::new(
                NodeId::new(NetworkId::new("test"), "node"),
                "echo-module",
            ),
            available_capabilities: vec![],
            resource_limits: ResourceLimits::default(),
            config: HashMap::new(),
            callback: ModuleCallback::new(tx),
        }
    }

    #[tokio::test]
    async fn test_module_lifecycle() {
        let mut module = EchoModule::new();

        // Should not be able to start without initialization
        assert!(module.start().await.is_err());

        // Initialize
        let context = create_test_context().await;
        assert!(module.initialize(context).await.is_ok());
        assert!(module.initialized);

        // Start
        assert!(module.start().await.is_ok());
        assert!(module.running);

        // Stop
        assert!(module.stop().await.is_ok());
        assert!(!module.running);
    }

    #[tokio::test]
    async fn test_echo_simple() {
        let mut module = EchoModule::new();
        let context = create_test_context().await;

        module.initialize(context).await.unwrap();
        module.start().await.unwrap();

        let request = Request::new("echo.simple", json!({"test": "data"}));
        let response = module.handle_request(request).await.unwrap();

        assert!(response.result.is_ok());
        let data = response.result.unwrap();
        assert_eq!(data["echo"]["test"], "data");
    }

    #[tokio::test]
    async fn test_echo_timestamp() {
        let mut module = EchoModule::new();
        let context = create_test_context().await;

        module.initialize(context).await.unwrap();
        module.start().await.unwrap();

        let request = Request::new(
            "echo.timestamp",
            json!({"message": "test", "value": 42}),
        );
        let response = module.handle_request(request).await.unwrap();

        assert!(response.result.is_ok());
        let data = response.result.unwrap();

        // Check that data is preserved
        assert_eq!(data["data"]["message"], "test");
        assert_eq!(data["data"]["value"], 42);

        // Check that timestamp fields exist
        assert!(data["timestamp"].is_string());
        assert!(data["unix_timestamp"].is_number());

        // Verify timestamp is recent (within last minute)
        let unix_ts = data["unix_timestamp"].as_i64().unwrap();
        let now = chrono::Utc::now().timestamp();
        assert!((now - unix_ts).abs() < 60);
    }

    #[tokio::test]
    async fn test_echo_stats() {
        let mut module = EchoModule::new();
        let context = create_test_context().await;

        module.initialize(context).await.unwrap();
        module.start().await.unwrap();

        // Make a few requests first
        let _ = module
            .handle_request(Request::new("echo.simple", json!({"test": 1})))
            .await;
        let _ = module
            .handle_request(Request::new("echo.timestamp", json!({"message": "test"})))
            .await;

        // Get stats
        let request = Request::new("echo.stats", json!({}));
        let response = module.handle_request(request).await.unwrap();

        assert!(response.result.is_ok());
        let stats = response.result.unwrap();
        assert!(stats["total_requests"].as_u64().unwrap() >= 2);

        // Check that stats includes our capabilities
        let by_cap = &stats["requests_by_capability"];
        assert!(by_cap["echo.simple"].as_u64().unwrap() >= 1);
        assert!(by_cap["echo.timestamp"].as_u64().unwrap() >= 1);
    }

    #[tokio::test]
    async fn test_invalid_capability() {
        let mut module = EchoModule::new();
        let context = create_test_context().await;

        module.initialize(context).await.unwrap();
        module.start().await.unwrap();

        let request = Request::new("echo.nonexistent", json!({}));
        let response = module.handle_request(request).await.unwrap();

        // Should return error response, not fail
        assert!(response.result.is_err());
    }

    #[tokio::test]
    async fn test_health_check() {
        let mut module = EchoModule::new();

        // Uninitialized
        assert_eq!(module.health_check().await, HealthStatus::Unhealthy);

        // Initialized but not running
        let context = create_test_context().await;
        module.initialize(context).await.unwrap();
        assert_eq!(module.health_check().await, HealthStatus::Degraded);

        // Running
        module.start().await.unwrap();
        assert_eq!(module.health_check().await, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_metrics() {
        let mut module = EchoModule::new();
        let context = create_test_context().await;

        module.initialize(context).await.unwrap();
        module.start().await.unwrap();

        let metrics = module.get_metrics().await;
        assert_eq!(metrics["initialized"], json!(true));
        assert_eq!(metrics["running"], json!(true));
        assert!(metrics.contains_key("stats"));
    }

    #[tokio::test]
    async fn test_manifest() {
        let module = EchoModule::new();
        let manifest = module.manifest();

        assert_eq!(manifest.id, "maia.test.echo");
        assert_eq!(manifest.version, Version::new(0, 1, 0));
        assert_eq!(manifest.license, "MIT");
        assert_eq!(manifest.isolation, IsolationLevel::Wasm);
    }

    #[tokio::test]
    async fn test_capabilities() {
        let module = EchoModule::new();
        let capabilities = module.capabilities();

        assert_eq!(capabilities.len(), 3);
        assert!(capabilities
            .iter()
            .any(|c| c.to_string() == "echo.simple"));
        assert!(capabilities
            .iter()
            .any(|c| c.to_string() == "echo.timestamp"));
        assert!(capabilities.iter().any(|c| c.to_string() == "echo.stats"));
    }

    #[tokio::test]
    async fn test_requirements() {
        let module = EchoModule::new();
        let requirements = module.requirements();

        // Echo module should have no requirements
        assert!(requirements.is_empty());
    }
}
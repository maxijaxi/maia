//! Resource management and enforcement for modules.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use maia_sdk::prelude::*;

/// Tracks and enforces resource usage for a module
pub struct ResourceManager {
    /// Module ID this manager is for
    module_id: ModuleId,

    /// Resource limits
    limits: ResourceLimits,

    /// Current usage tracking
    usage: Arc<RwLock<ResourceUsage>>,

    /// Memory usage (atomic for fast updates)
    memory_bytes: Arc<AtomicUsize>,

    /// CPU time tracking
    cpu_tracker: Arc<RwLock<CpuTracker>>,
}

/// Current resource usage
#[derive(Debug, Clone)]
struct ResourceUsage {
    /// Current memory usage in bytes
    memory_bytes: usize,

    /// Total CPU time used (milliseconds)
    cpu_time_ms: u64,

    /// Number of open file descriptors
    open_fds: u32,

    /// Number of threads/tasks
    thread_count: u32,

    /// Last updated
    last_updated: Instant,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            memory_bytes: 0,
            cpu_time_ms: 0,
            open_fds: 0,
            thread_count: 1,
            last_updated: Instant::now(),
        }
    }
}

/// Tracks CPU usage over time
struct CpuTracker {
    /// Start of current measurement period
    period_start: Instant,

    /// CPU time used in current period
    period_cpu_ms: u64,

    /// Total CPU time across all periods
    total_cpu_ms: u64,
}

impl ResourceManager {
    /// Create a new resource manager for a module
    pub fn new(module_id: ModuleId, limits: ResourceLimits) -> Self {
        Self {
            module_id,
            limits,
            usage: Arc::new(RwLock::new(ResourceUsage::default())),
            memory_bytes: Arc::new(AtomicUsize::new(0)),
            cpu_tracker: Arc::new(RwLock::new(CpuTracker {
                period_start: Instant::now(),
                period_cpu_ms: 0,
                total_cpu_ms: 0,
            })),
        }
    }

    /// Check if memory allocation is allowed
    pub fn check_memory_allocation(&self, bytes: usize) -> Result<()> {
        let current = self.memory_bytes.load(Ordering::Relaxed);
        let new_total = current + bytes;

        if new_total > self.limits.max_memory {
            Err(ModuleError::Fatal(FatalError::ResourceExhausted {
                resource: "memory".to_string(),
                limit: format!("{} bytes", self.limits.max_memory),
                current: format!("{} bytes", new_total),
            }))
        } else {
            Ok(())
        }
    }

    /// Allocate memory (if allowed)
    pub fn allocate_memory(&self, bytes: usize) -> Result<()> {
        self.check_memory_allocation(bytes)?;
        self.memory_bytes.fetch_add(bytes, Ordering::Relaxed);
        Ok(())
    }

    /// Deallocate memory
    pub fn deallocate_memory(&self, bytes: usize) {
        self.memory_bytes.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Check if CPU usage is within quota
    pub async fn check_cpu_quota(&self) -> Result<()> {
        let tracker = self.cpu_tracker.read().await;

        let elapsed = tracker.period_start.elapsed();
        let period_ms = elapsed.as_millis() as u64;

        // Check if we're exceeding quota for this period
        // cpu_quota_ms is milliseconds of CPU per second
        let allowed_ms = (period_ms * self.limits.cpu_quota_ms as u64) / 1000;

        if tracker.period_cpu_ms > allowed_ms {
            Err(ModuleError::Temporary(TemporaryError::ResourceBusy {
                resource: "cpu".to_string(),
                retry_after: Some(chrono::Utc::now() + chrono::Duration::seconds(1)),
            }))
        } else {
            Ok(())
        }
    }

    /// Record CPU time used
    pub async fn record_cpu_time(&self, duration: Duration) {
        let mut tracker = self.cpu_tracker.write().await;
        let ms = duration.as_millis() as u64;

        tracker.period_cpu_ms += ms;
        tracker.total_cpu_ms += ms;

        // Reset period if it's been more than a second
        if tracker.period_start.elapsed() > Duration::from_secs(1) {
            tracker.period_start = Instant::now();
            tracker.period_cpu_ms = 0;
        }
    }

    /// Check if we can open a new file descriptor
    pub async fn check_fd_limit(&self) -> Result<()> {
        let usage = self.usage.read().await;

        if usage.open_fds >= self.limits.max_fds {
            Err(ModuleError::Fatal(FatalError::ResourceExhausted {
                resource: "file_descriptors".to_string(),
                limit: format!("{}", self.limits.max_fds),
                current: format!("{}", usage.open_fds),
            }))
        } else {
            Ok(())
        }
    }

    /// Record opening a file descriptor
    pub async fn open_fd(&self) -> Result<()> {
        self.check_fd_limit().await?;
        let mut usage = self.usage.write().await;
        usage.open_fds += 1;
        Ok(())
    }

    /// Record closing a file descriptor
    pub async fn close_fd(&self) {
        let mut usage = self.usage.write().await;
        if usage.open_fds > 0 {
            usage.open_fds -= 1;
        }
    }

    /// Check if we can create a new thread
    pub async fn check_thread_limit(&self) -> Result<()> {
        let usage = self.usage.read().await;

        if usage.thread_count >= self.limits.max_threads {
            Err(ModuleError::Fatal(FatalError::ResourceExhausted {
                resource: "threads".to_string(),
                limit: format!("{}", self.limits.max_threads),
                current: format!("{}", usage.thread_count),
            }))
        } else {
            Ok(())
        }
    }

    /// Get current resource usage snapshot
    pub async fn get_usage(&self) -> ResourceSnapshot {
        let usage = self.usage.read().await;
        let memory = self.memory_bytes.load(Ordering::Relaxed);

        ResourceSnapshot {
            module_id: self.module_id.clone(),
            memory_bytes: memory,
            cpu_time_ms: usage.cpu_time_ms,
            open_fds: usage.open_fds,
            thread_count: usage.thread_count,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Reset all resource tracking (used when module is restarted)
    pub async fn reset(&self) {
        self.memory_bytes.store(0, Ordering::Relaxed);

        let mut usage = self.usage.write().await;
        *usage = ResourceUsage::default();

        let mut tracker = self.cpu_tracker.write().await;
        tracker.period_start = Instant::now();
        tracker.period_cpu_ms = 0;
        tracker.total_cpu_ms = 0;
    }
}

/// Snapshot of resource usage at a point in time
#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    pub module_id: ModuleId,
    pub memory_bytes: usize,
    pub cpu_time_ms: u64,
    pub open_fds: u32,
    pub thread_count: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_limits() {
        let module_id = ModuleId::new(NodeId::new(NetworkId::new("test"), "node"), "test-module");

        let mut limits = ResourceLimits::default();
        limits.max_memory = 1000;

        let manager = ResourceManager::new(module_id, limits);

        // Should allow allocation within limits
        assert!(manager.allocate_memory(500).is_ok());
        assert!(manager.allocate_memory(400).is_ok());

        // Should reject allocation exceeding limits
        assert!(manager.allocate_memory(200).is_err());

        // Deallocation should free space
        manager.deallocate_memory(400);
        assert!(manager.allocate_memory(200).is_ok());
    }

    #[tokio::test]
    async fn test_fd_limits() {
        let module_id = ModuleId::new(NodeId::new(NetworkId::new("test"), "node"), "test-module");

        let mut limits = ResourceLimits::default();
        limits.max_fds = 3;

        let manager = ResourceManager::new(module_id, limits);

        // Should allow opening FDs within limits
        assert!(manager.open_fd().await.is_ok());
        assert!(manager.open_fd().await.is_ok());
        assert!(manager.open_fd().await.is_ok());

        // Should reject opening more FDs
        assert!(manager.open_fd().await.is_err());

        // Closing should free slots
        manager.close_fd().await;
        assert!(manager.open_fd().await.is_ok());
    }

    #[tokio::test]
    async fn test_cpu_tracking() {
        let module_id = ModuleId::new(NodeId::new(NetworkId::new("test"), "node"), "test-module");

        let limits = ResourceLimits::default();
        let manager = ResourceManager::new(module_id, limits);

        // Record some CPU time
        manager.record_cpu_time(Duration::from_millis(100)).await;
        manager.record_cpu_time(Duration::from_millis(50)).await;

        let usage = manager.get_usage().await;
        assert_eq!(usage.cpu_time_ms, 150);
    }

    #[tokio::test]
    async fn test_reset() {
        let module_id = ModuleId::new(NodeId::new(NetworkId::new("test"), "node"), "test-module");

        let limits = ResourceLimits::default();
        let manager = ResourceManager::new(module_id.clone(), limits);

        // Use some resources
        manager.allocate_memory(1000).unwrap();
        manager.open_fd().await.unwrap();
        manager.record_cpu_time(Duration::from_millis(500)).await;

        // Check usage is recorded
        let usage = manager.get_usage().await;
        assert_eq!(usage.memory_bytes, 1000);
        assert_eq!(usage.open_fds, 1);
        assert_eq!(usage.cpu_time_ms, 500);

        // Reset
        manager.reset().await;

        // Check everything is cleared
        let usage = manager.get_usage().await;
        assert_eq!(usage.memory_bytes, 0);
        assert_eq!(usage.open_fds, 0);
        assert_eq!(usage.cpu_time_ms, 0);
    }
}

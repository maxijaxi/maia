//! Module loading and verification utilities.

use maia_sdk::prelude::*;
use std::path::{Path, PathBuf};

/// Module loader configuration
#[derive(Debug, Clone)]
pub struct LoaderConfig {
    /// Module search paths
    pub search_paths: Vec<PathBuf>,

    /// Whether to verify signatures
    pub verify_signatures: bool,

    /// Maximum module size in bytes
    pub max_module_size: usize,

    /// Allowed module types
    pub allowed_types: Vec<ModuleType>,
}

impl Default for LoaderConfig {
    fn default() -> Self {
        Self {
            search_paths: vec![
                PathBuf::from("./modules"),
                PathBuf::from("/usr/local/maia/modules"),
            ],
            verify_signatures: false,           // TODO: Enable in production
            max_module_size: 100 * 1024 * 1024, // 100MB
            allowed_types: vec![ModuleType::Wasm, ModuleType::Native],
        }
    }
}

/// Module file types that can be loaded.
///
/// These correspond to file formats and isolation methods, NOT programming languages.
/// Any language can compile to these formats:
/// - Python/JS/Rust/C++ → WASM
/// - Rust/C++ → Native (.so/.dll)
/// - Any language → Container (Docker image)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleType {
    /// WebAssembly module (.wasm)
    /// - Full sandboxing via wasmtime
    /// - Any language that compiles to WASM
    /// - Preferred for untrusted modules
    Wasm,

    /// Native shared library (.so, .dll, .dylib)
    /// - Zero isolation - runs in same process
    /// - Only for trusted core modules
    /// - Must match host architecture
    Native,

    /// OCI container image (future)
    /// - OS-level isolation via Docker/Podman
    /// - Any language/runtime
    /// - Good for complex dependencies
    Container,
}

impl ModuleType {
    /// Get file extension for this module type
    pub fn extension(&self) -> &str {
        match self {
            ModuleType::Wasm => "wasm",
            ModuleType::Native => {
                #[cfg(target_os = "linux")]
                return "so";
                #[cfg(target_os = "macos")]
                return "dylib";
                #[cfg(target_os = "windows")]
                return "dll";
            }
            ModuleType::Container => "", // Loaded by image name, not file extension
        }
    }

    /// Detect module type from file path
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        match ext {
            "wasm" => Some(ModuleType::Wasm),
            "so" | "dylib" | "dll" => Some(ModuleType::Native),
            _ => None,
        }
    }

    /// Get the corresponding isolation level for this module type
    pub fn isolation_level(&self) -> IsolationLevel {
        match self {
            ModuleType::Wasm => IsolationLevel::Wasm,
            ModuleType::Native => IsolationLevel::Native,
            ModuleType::Container => IsolationLevel::Container,
        }
    }
}

/// Module loader responsible for finding and loading modules
pub struct ModuleLoader {
    config: LoaderConfig,
}

impl ModuleLoader {
    /// Create a new module loader
    pub fn new(config: LoaderConfig) -> Self {
        Self { config }
    }

    /// Find a module by name in search paths
    pub async fn find_module(&self, name: &str) -> Result<PathBuf> {
        for search_path in &self.config.search_paths {
            // Try each supported module type
            for module_type in &self.config.allowed_types {
                let path = search_path
                    .join(name)
                    .with_extension(module_type.extension());

                if path.exists() {
                    return Ok(path);
                }
            }

            // Also try subdirectory with module name
            let subdir_path = search_path.join(name);
            if subdir_path.is_dir() {
                // Look for module.wasm, lib.so, etc. in subdirectory
                for module_type in &self.config.allowed_types {
                    let module_file = subdir_path
                        .join("module")
                        .with_extension(module_type.extension());

                    if module_file.exists() {
                        return Ok(module_file);
                    }

                    // Also try with lib prefix for native modules
                    if *module_type == ModuleType::Native {
                        let lib_file = subdir_path
                            .join(format!("lib{}", name))
                            .with_extension(module_type.extension());

                        if lib_file.exists() {
                            return Ok(lib_file);
                        }
                    }
                }
            }
        }

        Err(ModuleError::Fatal(FatalError::ModuleNotFound {
            module: name.to_string(),
            suggestion: format!("Check if module exists in: {:?}", self.config.search_paths),
        }))
    }

    /// Load module metadata without fully loading the module
    pub async fn load_metadata(&self, path: &Path) -> Result<ModuleMetadata> {
        // Check file size
        let metadata = tokio::fs::metadata(path).await.map_err(|e| {
            ModuleError::Fatal(FatalError::ModuleNotFound {
                module: path.display().to_string(),
                suggestion: format!("Failed to read file metadata: {}", e),
            })
        })?;

        if metadata.len() as usize > self.config.max_module_size {
            return Err(ModuleError::Fatal(FatalError::ResourceExhausted {
                resource: "module_size".to_string(),
                limit: format!("{} bytes", self.config.max_module_size),
                current: format!("{} bytes", metadata.len()),
            }));
        }

        // Detect module type
        let module_type = ModuleType::from_path(path).ok_or_else(|| {
            ModuleError::Fatal(FatalError::InvalidRequest {
                message: format!("Unknown module type for file: {}", path.display()),
                field: Some("extension".to_string()),
            })
        })?;

        // Check if type is allowed
        if !self.config.allowed_types.contains(&module_type) {
            return Err(ModuleError::Fatal(FatalError::InvalidRequest {
                message: format!("Module type {:?} not allowed", module_type),
                field: Some("module_type".to_string()),
            }));
        }

        // TODO: Read actual metadata from module
        // For WASM: Parse custom sections for embedded manifest
        // For Native: Look for accompanying .toml manifest file
        // See: https://github.com/WebAssembly/tool-conventions/blob/main/CustomSections.md

        Ok(ModuleMetadata {
            path: path.to_path_buf(),
            module_type,
            size: metadata.len(),
            hash: None, // TODO: Calculate SHA-256 hash
        })
    }

    /// Verify module signature (if enabled)
    pub async fn verify_module(&self, path: &Path) -> Result<()> {
        if !self.config.verify_signatures {
            return Ok(());
        }

        // TODO: Implement signature verification
        // 1. Look for signature file (e.g., module.wasm.sig)
        // 2. Read public key from trusted keystore
        // 3. Calculate module hash
        // 4. Verify signature matches hash
        //
        // Use ed25519-dalek for signature verification

        Ok(())
    }
}

/// Module metadata extracted from file
#[derive(Debug, Clone)]
pub struct ModuleMetadata {
    /// Path to the module file
    pub path: PathBuf,

    /// Detected module type
    pub module_type: ModuleType,

    /// File size in bytes
    pub size: u64,

    /// Optional cryptographic hash of module contents
    pub hash: Option<Vec<u8>>,
}

impl ModuleMetadata {
    /// Get the isolation level appropriate for this module type
    pub fn isolation_level(&self) -> IsolationLevel {
        self.module_type.isolation_level()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_type_detection() {
        // WASM modules
        assert_eq!(
            ModuleType::from_path(Path::new("test.wasm")),
            Some(ModuleType::Wasm)
        );
        assert_eq!(
            ModuleType::from_path(Path::new("module.wasm")),
            Some(ModuleType::Wasm)
        );

        // Native modules (platform-specific)
        assert_eq!(
            ModuleType::from_path(Path::new("libtest.so")),
            Some(ModuleType::Native)
        );
        assert_eq!(
            ModuleType::from_path(Path::new("test.dylib")),
            Some(ModuleType::Native)
        );
        assert_eq!(
            ModuleType::from_path(Path::new("module.dll")),
            Some(ModuleType::Native)
        );

        // Unsupported types should return None
        assert_eq!(ModuleType::from_path(Path::new("test.txt")), None);
        assert_eq!(ModuleType::from_path(Path::new("test.py")), None);
        assert_eq!(ModuleType::from_path(Path::new("test.js")), None);
        assert_eq!(ModuleType::from_path(Path::new("test.rs")), None);
    }

    #[test]
    fn test_module_type_extension() {
        assert_eq!(ModuleType::Wasm.extension(), "wasm");

        #[cfg(target_os = "linux")]
        assert_eq!(ModuleType::Native.extension(), "so");

        #[cfg(target_os = "macos")]
        assert_eq!(ModuleType::Native.extension(), "dylib");

        #[cfg(target_os = "windows")]
        assert_eq!(ModuleType::Native.extension(), "dll");
    }

    #[test]
    fn test_isolation_level_mapping() {
        assert_eq!(
            ModuleType::Wasm.isolation_level(),
            IsolationLevel::Wasm
        );
        assert_eq!(
            ModuleType::Native.isolation_level(),
            IsolationLevel::Native
        );
        assert_eq!(
            ModuleType::Container.isolation_level(),
            IsolationLevel::Container
        );
    }

    #[tokio::test]
    async fn test_module_loader_creation() {
        let config = LoaderConfig::default();
        let loader = ModuleLoader::new(config);

        // Verify default configuration
        assert_eq!(loader.config.allowed_types.len(), 2);
        assert!(loader.config.allowed_types.contains(&ModuleType::Wasm));
        assert!(loader.config.allowed_types.contains(&ModuleType::Native));
    }

    #[tokio::test]
    async fn test_module_loader_find_nonexistent() {
        let config = LoaderConfig::default();
        let loader = ModuleLoader::new(config);

        // This should fail since we don't have actual modules yet
        let result = loader.find_module("nonexistent_module").await;
        assert!(result.is_err());

        if let Err(ModuleError::Fatal(FatalError::ModuleNotFound { module, .. })) = result {
            assert_eq!(module, "nonexistent_module");
        } else {
            panic!("Expected ModuleNotFound error");
        }
    }
}
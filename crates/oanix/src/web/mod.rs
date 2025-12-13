//! Browser/WASM support for OANIX
//!
//! Provides a JavaScript-accessible API for interacting with OANIX namespaces
//! in the browser via WebAssembly.

use wasm_bindgen::prelude::*;

use crate::Namespace;
use crate::service::OpenFlags;
use crate::services::MemFs;

/// OANIX runtime for the browser
///
/// Provides a JavaScript-accessible interface to an OANIX namespace.
#[wasm_bindgen]
pub struct OanixWeb {
    namespace: Namespace,
}

#[wasm_bindgen]
impl OanixWeb {
    /// Create a new OANIX runtime with a default namespace
    ///
    /// The default namespace includes:
    /// - `/workspace` - Editable in-memory filesystem
    /// - `/tmp` - Temporary in-memory filesystem
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        // Set up panic hook for better error messages in browser
        #[cfg(feature = "browser")]
        console_error_panic_hook::set_once();

        let namespace = Namespace::builder()
            .mount("/workspace", MemFs::new())
            .mount("/tmp", MemFs::new())
            .build();

        OanixWeb { namespace }
    }

    /// Create a new OANIX runtime with an empty namespace
    #[wasm_bindgen]
    pub fn empty() -> Self {
        OanixWeb {
            namespace: Namespace::builder().build(),
        }
    }

    /// Read a file as text
    #[wasm_bindgen]
    pub fn read_text(&self, path: &str) -> Result<String, JsValue> {
        let (service, remaining) = self
            .namespace
            .resolve(path)
            .ok_or_else(|| JsValue::from_str(&format!("No mount found for path: {}", path)))?;

        let mut handle = service
            .open(remaining, OpenFlags::read_only())
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;

        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            let n = handle
                .read(&mut tmp)
                .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
        }

        String::from_utf8(buf).map_err(|e| JsValue::from_str(&format!("Invalid UTF-8: {}", e)))
    }

    /// Read a file as bytes
    #[wasm_bindgen]
    pub fn read_bytes(&self, path: &str) -> Result<Vec<u8>, JsValue> {
        let (service, remaining) = self
            .namespace
            .resolve(path)
            .ok_or_else(|| JsValue::from_str(&format!("No mount found for path: {}", path)))?;

        let mut handle = service
            .open(remaining, OpenFlags::read_only())
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;

        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            let n = handle
                .read(&mut tmp)
                .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
        }

        Ok(buf)
    }

    /// Write text to a file (creates if doesn't exist)
    #[wasm_bindgen]
    pub fn write_text(&self, path: &str, content: &str) -> Result<(), JsValue> {
        self.write_bytes(path, content.as_bytes())
    }

    /// Write bytes to a file (creates if doesn't exist)
    #[wasm_bindgen]
    pub fn write_bytes(&self, path: &str, content: &[u8]) -> Result<(), JsValue> {
        let (service, remaining) = self
            .namespace
            .resolve(path)
            .ok_or_else(|| JsValue::from_str(&format!("No mount found for path: {}", path)))?;

        let mut handle = service
            .open(
                remaining,
                OpenFlags {
                    read: false,
                    write: true,
                    create: true,
                    truncate: true,
                    append: false,
                },
            )
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;

        handle
            .write(content)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
        handle
            .flush()
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;

        Ok(())
    }

    /// List directory contents
    ///
    /// Returns a JSON array of objects with { name, is_dir, size }
    #[wasm_bindgen]
    pub fn list_dir(&self, path: &str) -> Result<JsValue, JsValue> {
        let (service, remaining) = self
            .namespace
            .resolve(path)
            .ok_or_else(|| JsValue::from_str(&format!("No mount found for path: {}", path)))?;

        let entries = service
            .readdir(remaining)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;

        serde_wasm_bindgen::to_value(&entries)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Get file/directory metadata
    ///
    /// Returns a JSON object with { is_dir, size, modified, readonly }
    #[wasm_bindgen]
    pub fn stat(&self, path: &str) -> Result<JsValue, JsValue> {
        let (service, remaining) = self
            .namespace
            .resolve(path)
            .ok_or_else(|| JsValue::from_str(&format!("No mount found for path: {}", path)))?;

        let meta = service
            .stat(remaining)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;

        serde_wasm_bindgen::to_value(&meta)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Create a directory
    #[wasm_bindgen]
    pub fn mkdir(&self, path: &str) -> Result<(), JsValue> {
        let (service, remaining) = self
            .namespace
            .resolve(path)
            .ok_or_else(|| JsValue::from_str(&format!("No mount found for path: {}", path)))?;

        service
            .mkdir(remaining)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))
    }

    /// Remove a file or directory
    #[wasm_bindgen]
    pub fn remove(&self, path: &str) -> Result<(), JsValue> {
        let (service, remaining) = self
            .namespace
            .resolve(path)
            .ok_or_else(|| JsValue::from_str(&format!("No mount found for path: {}", path)))?;

        service
            .remove(remaining)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))
    }

    /// Check if a path exists
    #[wasm_bindgen]
    pub fn exists(&self, path: &str) -> bool {
        if let Some((service, remaining)) = self.namespace.resolve(path) {
            service.stat(remaining).is_ok()
        } else {
            false
        }
    }

    /// Get all mount points
    ///
    /// Returns a JSON array of mount point paths
    #[wasm_bindgen]
    pub fn mounts(&self) -> JsValue {
        let mount_paths: Vec<&str> = self
            .namespace
            .mounts()
            .iter()
            .map(|m| m.path.as_str())
            .collect();
        serde_wasm_bindgen::to_value(&mount_paths).unwrap_or(JsValue::NULL)
    }
}

impl Default for OanixWeb {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize OANIX for the browser
///
/// This is called automatically when the WASM module loads.
#[wasm_bindgen(start)]
pub fn init() {
    // Set up panic hook for better error messages
    #[cfg(feature = "browser")]
    console_error_panic_hook::set_once();
}

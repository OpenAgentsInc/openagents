//! Environment types for TestGen
//!
//! These types describe the execution environment for test generation,
//! including platform info, available tools, and file context.

use serde::{Deserialize, Serialize};

/// Complete environment information for test generation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    /// Platform information
    pub platform: PlatformInfo,
    /// Available and prohibited tools
    pub tools: ToolsInfo,
    /// File system context
    pub files: FilesInfo,
    /// Resource limits
    #[serde(default)]
    pub resources: ResourceInfo,
}

/// Platform information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlatformInfo {
    /// Platform type (e.g., "docker", "native")
    #[serde(rename = "type")]
    pub platform_type: String,
    /// Whether running in a container
    #[serde(default)]
    pub is_container: bool,
    /// Operating system
    #[serde(default)]
    pub os: String,
    /// Architecture
    #[serde(default)]
    pub arch: String,
}

/// Tools information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolsInfo {
    /// Available tools
    #[serde(default)]
    pub available: Vec<AvailableTool>,
    /// Prohibited tools (for anti-cheat)
    #[serde(default)]
    pub prohibited: Vec<ProhibitedTool>,
}

/// An available tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableTool {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

/// A prohibited tool (for anti-cheat testing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProhibitedTool {
    pub name: String,
    #[serde(default)]
    pub reason: Option<String>,
}

/// File system information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilesInfo {
    /// Directory listing
    #[serde(default)]
    pub listing: Vec<String>,
    /// Task-specific files with previews
    #[serde(default)]
    pub task_files: Vec<FileInfo>,
}

/// File information with preview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    #[serde(default)]
    pub preview: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub is_directory: bool,
}

/// Resource limits
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceInfo {
    /// Memory limit in bytes
    #[serde(default)]
    pub memory_limit: Option<u64>,
    /// CPU limit
    #[serde(default)]
    pub cpu_limit: Option<f64>,
    /// Time limit in milliseconds
    #[serde(default)]
    pub time_limit_ms: Option<u64>,
}

impl EnvironmentInfo {
    /// Create a minimal environment for testing
    pub fn minimal() -> Self {
        Self {
            platform: PlatformInfo {
                platform_type: "native".to_string(),
                is_container: false,
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
            },
            tools: ToolsInfo::default(),
            files: FilesInfo::default(),
            resources: ResourceInfo::default(),
        }
    }

    /// Create a Docker environment
    pub fn docker() -> Self {
        Self {
            platform: PlatformInfo {
                platform_type: "docker".to_string(),
                is_container: true,
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
            },
            tools: ToolsInfo::default(),
            files: FilesInfo::default(),
            resources: ResourceInfo::default(),
        }
    }

    /// Add a prohibited tool
    pub fn with_prohibited_tool(mut self, name: &str, reason: Option<&str>) -> Self {
        self.tools.prohibited.push(ProhibitedTool {
            name: name.to_string(),
            reason: reason.map(String::from),
        });
        self
    }

    /// Add a task file with preview
    pub fn with_task_file(mut self, path: &str, preview: &str) -> Self {
        self.files.task_files.push(FileInfo {
            path: path.to_string(),
            preview: preview.to_string(),
            size: None,
            is_directory: false,
        });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_environment() {
        let env = EnvironmentInfo::minimal();
        assert_eq!(env.platform.platform_type, "native");
        assert!(!env.platform.is_container);
    }

    #[test]
    fn test_docker_environment() {
        let env = EnvironmentInfo::docker();
        assert_eq!(env.platform.platform_type, "docker");
        assert!(env.platform.is_container);
    }

    #[test]
    fn test_with_prohibited_tool() {
        let env = EnvironmentInfo::minimal()
            .with_prohibited_tool("python", Some("Use bash only"));
        assert_eq!(env.tools.prohibited.len(), 1);
        assert_eq!(env.tools.prohibited[0].name, "python");
    }
}

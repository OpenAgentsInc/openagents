//! ACP agent discovery.
//!
//! Reads the ACP registry on disk and checks which known agents are available
//! on the current system. Used by coder-lite to announce discovered ACP agents
//! at startup.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A discovered ACP agent.
#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub name: String,
}

/// Discover all ACP agents in the registry that are also available locally.
///
/// Looks for `ACP_REGISTRY` first, then falls back to the default checkout path
/// under the user's home directory.
pub async fn find_agents() -> Result<Vec<Agent>, Box<dyn std::error::Error>> {
    let registry_dir = std::env::var("ACP_REGISTRY")
        .ok()
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join("work/projects/agentclientprotocol/repos/registry")));

    let Some(dir) = registry_dir else {
        return Ok(Vec::new());
    };
    if !tokio::fs::metadata(&dir).await.map(|m| m.is_dir()).unwrap_or(false) {
        return Ok(Vec::new());
    }

    let mut entries = tokio::fs::read_dir(&dir).await?;
    let mut found = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let agent_json = path.join("agent.json");
        let content = match tokio::fs::read_to_string(&agent_json).await {
            Ok(c) => c,
            Err(_) => continue,
        };

        let agent: RegistryAgent = match serde_json::from_str(&content) {
            Ok(a) => a,
            Err(_) => continue,
        };

        if is_available(&agent).await {
            found.push(Agent {
                id: agent.id,
                name: agent.name,
            });
        }
    }

    found.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(found)
}

#[derive(Debug, Deserialize)]
struct RegistryAgent {
    id: String,
    name: String,
    #[allow(dead_code)]
    version: String,
    #[serde(default)]
    distribution: Option<Distribution>,
}

#[derive(Debug, Deserialize)]
struct Distribution {
    #[serde(default)]
    binary: Option<HashMap<String, BinaryTarget>>,
    #[serde(default)]
    npx: Option<NpxUvx>,
    #[serde(default)]
    uvx: Option<NpxUvx>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BinaryTarget {
    cmd: String,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct NpxUvx {
    package: String,
    #[serde(default)]
    args: Option<Vec<String>>,
}

async fn is_available(agent: &RegistryAgent) -> bool {
    let Some(dist) = &agent.distribution else {
        return false;
    };

    if let Some(binary) = &dist.binary {
        let platform = current_platform();
        if let Some(target) = binary.get(&platform) {
            let name = Path::new(&target.cmd)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&target.cmd);
            if has_command(name).await {
                return true;
            }
        }
    }

    if dist.npx.is_some() && has_command("npx").await {
        return true;
    }

    if dist.uvx.is_some() && has_command("uvx").await {
        return true;
    }

    false
}

fn current_platform() -> String {
    use std::env::consts::{ARCH, OS};
    let os = match OS {
        "macos" => "darwin",
        "windows" => "windows",
        _ => "linux",
    };
    format!("{}-{}", os, ARCH)
}

async fn has_command(name: &str) -> bool {
    if cfg!(target_os = "windows") {
        tokio::process::Command::new("where")
            .arg(name)
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        tokio::process::Command::new("which")
            .arg(name)
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn find_agents_does_not_panic() {
        let agents = find_agents().await.unwrap();
        // We cannot assert exact IDs because PATH and installed agents vary by host.
        // The call must simply return Ok and not panic on missing registry or which.
        for agent in &agents {
            assert!(!agent.id.is_empty());
        }
    }
}

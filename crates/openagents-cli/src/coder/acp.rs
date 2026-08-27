//! ACP agent discovery.
//!
//! Reads the ACP registry on disk and checks which known agents are available
//! on the current system. Used by Coder to announce discovered ACP agents
//! at startup.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A discovered ACP agent.
#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub name: String,
    /// Command to run the agent, as found on this system.
    pub command: String,
    /// Args to pass after `command`.
    pub args: Vec<String>,
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
    if !tokio::fs::metadata(&dir)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false)
    {
        return Ok(Vec::new());
    }

    let mut entries = tokio::fs::read_dir(&dir).await?;
    let mut found = Vec::new();

    // Compute these once; many agents use npx or uvx, so avoid repeated calls.
    let npm_root = npm_root().await;
    let uv_tools = uv_tools().await;

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

        if is_available(&agent, &npm_root, &uv_tools).await {
            let (command, args) = launch_for(&agent);
            found.push(Agent {
                id: agent.id,
                name: agent.name,
                command,
                args,
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

async fn is_available(
    agent: &RegistryAgent,
    npm_root: &Option<String>,
    uv_tools: &Option<String>,
) -> bool {
    let Some(dist) = &agent.distribution else {
        return false;
    };

    // Binary distribution for this platform.
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

    // An executable named exactly like the agent id (e.g., `grok-build`).
    if has_command(&agent.id).await {
        return true;
    }

    // npx package installed globally.
    if let Some(npx) = &dist.npx {
        if npx_installed(&npx.package, npm_root).await {
            return true;
        }
    }

    // uvx package installed as a uv tool.
    if let Some(uvx) = &dist.uvx {
        if uvx_installed(&uvx.package, uv_tools).await {
            return true;
        }
    }

    false
}

fn launch_for(agent: &RegistryAgent) -> (String, Vec<String>) {
    let Some(dist) = &agent.distribution else {
        return (agent.id.clone(), Vec::new());
    };

    if let Some(binary) = &dist.binary {
        let platform = current_platform();
        if let Some(target) = binary.get(&platform) {
            let name = Path::new(&target.cmd)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&target.cmd)
                .to_string();
            let mut args = target.args.clone().unwrap_or_default();
            // ACP mode is the agent's streaming protocol.
            if !args.iter().any(|a| a == "acp") {
                args.push("acp".to_string());
            }
            return (name, args);
        }
    }

    if let Some(npx) = &dist.npx {
        let mut args = vec!["-y".to_string(), npx.package.clone()];
        if let Some(extra) = &npx.args {
            args.extend(extra.iter().cloned());
        }
        if !args.iter().any(|a| a == "acp") {
            args.push("acp".to_string());
        }
        return ("npx".to_string(), args);
    }

    if let Some(uvx) = &dist.uvx {
        let mut args = vec![uvx.package.clone()];
        if let Some(extra) = &uvx.args {
            args.extend(extra.iter().cloned());
        }
        if !args.iter().any(|a| a == "acp") {
            args.push("acp".to_string());
        }
        return ("uvx".to_string(), args);
    }

    (agent.id.clone(), Vec::new())
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

async fn npm_root() -> Option<String> {
    let output = tokio::process::Command::new("npm")
        .args(["root", "-g"])
        .output()
        .await;
    output
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

async fn uv_tools() -> Option<String> {
    let output = tokio::process::Command::new("uv")
        .args(["tool", "list"])
        .output()
        .await;
    output
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
}

async fn npx_installed(package: &str, npm_root: &Option<String>) -> bool {
    let Some(root) = npm_root else {
        return false;
    };
    let package = package_dir(package);
    let marker = Path::new(root)
        .join("node_modules")
        .join(package)
        .join("package.json");
    tokio::fs::metadata(&marker)
        .await
        .map(|m| m.is_file())
        .unwrap_or(false)
}

async fn uvx_installed(package: &str, uv_tools: &Option<String>) -> bool {
    let Some(list) = uv_tools else {
        return false;
    };
    let package = package_dir(package);
    list.lines()
        .any(|line| line.split_whitespace().next() == Some(package))
}

fn package_dir(spec: &str) -> &str {
    // Strip an `@<version>` suffix while preserving scoped package names.
    // "@scope/pkg@1.0.0" -> "@scope/pkg", "pkg@1.0.0" -> "pkg".
    spec.rsplit_once('@')
        .and_then(|(left, right)| {
            if right.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                Some(left)
            } else {
                None
            }
        })
        .unwrap_or(spec)
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

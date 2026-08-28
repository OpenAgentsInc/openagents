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
            if is_grok_registry_id(&agent.id) {
                push_grok_aliases(&mut found, agent.name.clone(), command, args);
            } else {
                found.push(Agent {
                    id: agent.id,
                    name: agent.name,
                    command,
                    args,
                });
            }
        }
    }

    found.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(found)
}

fn push_grok_aliases(found: &mut Vec<Agent>, name: String, command: String, args: Vec<String>) {
    for id in ["grok", "grok-build"] {
        if found.iter().any(|agent| agent.id == id) {
            continue;
        }
        found.push(Agent {
            id: id.to_string(),
            name: name.clone(),
            command: command.clone(),
            args: args.clone(),
        });
    }
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

    // Grok is the `grok` binary, not the registry id and not an npx fallback.
    if is_grok_registry_id(&agent.id) {
        return has_command("grok").await;
    }

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

    // An executable named exactly like the agent id (e.g. `devin`).
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

fn is_grok_registry_id(id: &str) -> bool {
    id == "grok-build" || id == "grok"
}

/// Grok's ACP server is `grok agent stdio`. A trailing `acp` is a different
/// product's mode token and takes Grok out of the protocol.
pub fn grok_stdio_args() -> Vec<String> {
    vec!["agent".to_string(), "stdio".to_string()]
}

fn already_in_acp_mode(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "acp" || arg == "stdio" || arg == "--experimental-acp")
}

fn ensure_acp_mode(mut args: Vec<String>) -> Vec<String> {
    if !already_in_acp_mode(&args) {
        args.push("acp".to_string());
    }
    args
}

fn launch_for(agent: &RegistryAgent) -> (String, Vec<String>) {
    if is_grok_registry_id(&agent.id) {
        return ("grok".to_string(), grok_stdio_args());
    }

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
            let args = ensure_acp_mode(target.args.clone().unwrap_or_default());
            return (name, args);
        }
    }

    if let Some(npx) = &dist.npx {
        let mut args = vec!["-y".to_string(), npx.package.clone()];
        if let Some(extra) = &npx.args {
            args.extend(extra.iter().cloned());
        }
        return ("npx".to_string(), ensure_acp_mode(args));
    }

    if let Some(uvx) = &dist.uvx {
        let mut args = vec![uvx.package.clone()];
        if let Some(extra) = &uvx.args {
            args.extend(extra.iter().cloned());
        }
        return ("uvx".to_string(), ensure_acp_mode(args));
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

    fn registry(id: &str, npx_package: &str, npx_args: &[&str]) -> RegistryAgent {
        RegistryAgent {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            distribution: Some(Distribution {
                binary: None,
                npx: Some(NpxUvx {
                    package: npx_package.to_string(),
                    args: Some(npx_args.iter().map(|s| s.to_string()).collect()),
                }),
                uvx: None,
            }),
        }
    }

    #[test]
    fn grok_build_registry_launch_is_grok_agent_stdio() {
        let agent = registry(
            "grok-build",
            "@xai-official/grok@1.0.10",
            &["agent", "stdio"],
        );
        let (command, args) = launch_for(&agent);
        assert_eq!(command, "grok");
        assert_eq!(args, grok_stdio_args());
        assert!(!args.iter().any(|arg| arg == "acp"));
    }

    #[test]
    fn grok_registry_id_also_pins_stdio() {
        let agent = registry("grok", "@xai-official/grok@1.0.10", &["agent", "stdio"]);
        let (command, args) = launch_for(&agent);
        assert_eq!(command, "grok");
        assert_eq!(args, vec!["agent".to_string(), "stdio".to_string()]);
    }

    #[test]
    fn launch_for_does_not_append_acp_when_stdio_is_already_present() {
        let agent = registry("other", "@example/other", &["agent", "stdio"]);
        let (_command, args) = launch_for(&agent);
        assert_eq!(
            args,
            vec![
                "-y".to_string(),
                "@example/other".to_string(),
                "agent".to_string(),
                "stdio".to_string()
            ]
        );
    }

    #[test]
    fn launch_for_still_appends_acp_for_devin_shaped_npx() {
        let agent = registry("devin", "@cognition/devin", &[]);
        let (_command, args) = launch_for(&agent);
        assert_eq!(
            args,
            vec![
                "-y".to_string(),
                "@cognition/devin".to_string(),
                "acp".to_string()
            ]
        );
    }

    #[test]
    fn launch_for_keeps_experimental_acp() {
        let agent = registry("gemini", "@google/gemini", &["--experimental-acp"]);
        let (_command, args) = launch_for(&agent);
        assert_eq!(
            args,
            vec![
                "-y".to_string(),
                "@google/gemini".to_string(),
                "--experimental-acp".to_string()
            ]
        );
        assert!(!args.iter().any(|arg| arg == "acp"));
    }

    #[test]
    fn grok_aliases_are_both_offered() {
        let mut found = Vec::new();
        push_grok_aliases(
            &mut found,
            "Grok Build".to_string(),
            "grok".to_string(),
            grok_stdio_args(),
        );
        let ids: Vec<&str> = found.iter().map(|agent| agent.id.as_str()).collect();
        assert_eq!(ids, vec!["grok", "grok-build"]);
        for agent in &found {
            assert_eq!(agent.command, "grok");
            assert_eq!(agent.args, grok_stdio_args());
        }
    }
}

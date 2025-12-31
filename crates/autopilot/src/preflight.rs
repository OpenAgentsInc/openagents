//! Preflight configuration discovery
//!
//! Scans the environment to build a configuration that can be fed to AI agents.
//! Detects auth, git repos, project structure, inference providers, and more.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info};

use crate::auth::{self, AuthEntry, AuthStatus};

/// Main preflight configuration - saved as JSON for AI consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightConfig {
    /// Timestamp when preflight was run
    pub created_at: String,

    /// The working directory this config was created for
    pub working_directory: PathBuf,

    /// Hash of the working directory path (for folder identification)
    pub path_hash: String,

    /// Git repository information
    pub git: Option<GitInfo>,

    /// Authentication status for various providers
    pub auth: AuthInfo,

    /// Project-level configuration from .openagents folder
    pub project: Option<ProjectInfo>,

    /// Inference provider availability
    pub inference: InferenceInfo,

    /// Available CLI tools
    pub tools: ToolsInfo,
}

/// Git repository information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    /// Whether we're in a git repository
    pub is_repo: bool,

    /// Current branch name
    pub branch: Option<String>,

    /// Remote origin URL
    pub remote_url: Option<String>,

    /// Whether there are uncommitted changes
    pub has_changes: bool,

    /// Number of unpushed commits
    pub unpushed_commits: usize,

    /// Root of the git repository
    pub repo_root: PathBuf,
}

/// Authentication information for providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthInfo {
    /// Providers with valid authentication
    pub authenticated_providers: Vec<String>,

    /// Whether OpenAgents auth.json exists
    pub has_openagents_auth: bool,

    /// Whether OpenCode auth.json exists (can be imported)
    pub has_opencode_auth: bool,

    /// Per-provider status
    pub providers: HashMap<String, ProviderAuth>,
}

/// Per-provider authentication details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAuth {
    /// Whether this provider is authenticated
    pub authenticated: bool,

    /// Type of authentication (oauth, api, wellknown)
    pub auth_type: Option<String>,

    /// Whether auth appears to be expired (for oauth)
    pub expired: bool,
}

/// Project-level information from .openagents folder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// Path to .openagents folder
    pub path: PathBuf,

    /// Whether directives exist
    pub has_directives: bool,

    /// Number of directives found
    pub directive_count: usize,

    /// Whether issues.json exists
    pub has_issues_json: bool,

    /// Whether DIRECTIVES.md exists
    pub has_directives_md: bool,

    /// Whether TODO.md exists
    pub has_todo_md: bool,

    /// Whether autopilot.db exists
    pub has_autopilot_db: bool,

    /// List of directive IDs found
    pub directive_ids: Vec<String>,
}

/// Inference provider availability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceInfo {
    /// Cloud API providers available
    pub cloud_providers: Vec<String>,

    /// Local model backends available
    pub local_backends: Vec<LocalBackend>,

    /// Whether any swarm providers are configured
    pub has_swarm_providers: bool,

    /// Environment variables relevant to inference
    pub env_vars: HashMap<String, bool>,

    /// Local Pylon daemon status
    #[serde(default)]
    pub pylon: Option<PylonInfo>,

    /// Remote swarm providers discovered via NIP-89
    #[serde(default)]
    pub swarm_providers: Vec<SwarmProvider>,

    /// Neobank treasury status
    #[serde(default)]
    pub neobank: Option<NeobankInfo>,
}

/// Local inference backend information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalBackend {
    /// Backend name (ollama, llama-cpp, fm-bridge, gpt-oss)
    pub name: String,

    /// Whether the backend is available
    pub available: bool,

    /// Endpoint URL if applicable
    pub endpoint: Option<String>,

    /// Models available (if detectable)
    pub models: Vec<String>,
}

/// Local Pylon daemon status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PylonInfo {
    /// Whether pylon daemon is running
    pub running: bool,

    /// Process ID if running
    pub pid: Option<u32>,

    /// Uptime in seconds if running
    pub uptime_secs: Option<u64>,

    /// Jobs completed this session
    pub jobs_completed: u64,

    /// Available models from pylon backends
    pub models: Vec<String>,
}

impl Default for PylonInfo {
    fn default() -> Self {
        Self {
            running: false,
            pid: None,
            uptime_secs: None,
            jobs_completed: 0,
            models: Vec::new(),
        }
    }
}

/// Neobank treasury status
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NeobankInfo {
    /// Whether neobank is available
    pub available: bool,

    /// BTC balance in satoshis
    pub btc_balance_sats: u64,

    /// USD balance in cents
    pub usd_balance_cents: u64,

    /// Whether treasury agent is active
    pub treasury_active: bool,

    /// Current BTC/USD rate if available
    pub btc_usd_rate: Option<f64>,
}

/// Remote compute provider discovered via NIP-89
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmProvider {
    /// Provider's npub (short form)
    pub pubkey: String,

    /// Human-readable name
    pub name: String,

    /// Price in millisats per request
    pub price_msats: Option<u64>,

    /// Relay where discovered
    pub relay: String,
}

/// Complete compute availability summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeMix {
    /// Local pylon status
    pub pylon: Option<PylonInfo>,

    /// Local inference backends
    pub local_backends: Vec<LocalBackend>,

    /// Cloud API providers
    pub cloud_providers: Vec<String>,

    /// Remote swarm providers via NIP-89
    pub swarm_providers: Vec<SwarmProvider>,
}

impl ComputeMix {
    /// Generate a summary line for display
    pub fn summary(&self) -> String {
        let local_count = self.local_backends.iter().filter(|b| b.available).count();
        let cloud_count = self.cloud_providers.len();
        let swarm_count = self.swarm_providers.len();

        format!(
            "Local: {} backends, Cloud: {} providers, Swarm: {} providers",
            local_count, cloud_count, swarm_count
        )
    }
}

/// Available CLI tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsInfo {
    /// Claude CLI path and version
    pub claude: Option<ToolInfo>,

    /// Codex CLI path and version
    pub codex: Option<ToolInfo>,

    /// OpenCode CLI path and version
    pub opencode: Option<ToolInfo>,

    /// Git path and version
    pub git: Option<ToolInfo>,

    /// Cargo path and version
    pub cargo: Option<ToolInfo>,
}

/// Information about an installed tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    /// Path to the tool
    pub path: PathBuf,

    /// Version string if available
    pub version: Option<String>,
}

impl PreflightConfig {
    /// Run preflight checks and build configuration
    pub fn run(working_dir: &Path) -> Result<Self> {
        info!("Running preflight checks for {:?}", working_dir);

        let working_directory = working_dir
            .canonicalize()
            .unwrap_or_else(|_| working_dir.to_path_buf());

        let path_hash = hash_path(&working_directory);

        let created_at = chrono_now();

        // Run all detection in parallel where possible
        let git = detect_git(&working_directory);
        let auth = detect_auth();
        let project = detect_project(&working_directory);
        let inference = detect_inference();
        let tools = detect_tools();

        let config = PreflightConfig {
            created_at,
            working_directory,
            path_hash,
            git,
            auth,
            project,
            inference,
            tools,
        };

        info!("Preflight complete");
        Ok(config)
    }

    /// Save the config to ~/.openagents/folders/<path_hash>/config.json
    pub fn save(&self) -> Result<PathBuf> {
        let config_dir = self.config_dir();

        // Create directory
        std::fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

        let config_path = config_dir.join("config.json");

        let json = serde_json::to_string_pretty(self).context("Failed to serialize config")?;

        std::fs::write(&config_path, &json).context("Failed to write config file")?;

        info!("Saved preflight config to {:?}", config_path);

        Ok(config_path)
    }

    /// Get the config directory for this working directory
    pub fn config_dir(&self) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".openagents")
            .join("folders")
            .join(&self.path_hash)
    }

    /// Load an existing config for a path, if it exists
    pub fn load(working_dir: &Path) -> Result<Option<Self>> {
        let working_directory = working_dir
            .canonicalize()
            .unwrap_or_else(|_| working_dir.to_path_buf());
        let path_hash = hash_path(&working_directory);

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let config_path = PathBuf::from(home)
            .join(".openagents")
            .join("folders")
            .join(&path_hash)
            .join("config.json");

        if !config_path.exists() {
            return Ok(None);
        }

        let content =
            std::fs::read_to_string(&config_path).context("Failed to read config file")?;

        let config: PreflightConfig =
            serde_json::from_str(&content).context("Failed to parse config file")?;

        Ok(Some(config))
    }

    /// Generate a system prompt summary for AI agents
    pub fn to_system_prompt(&self) -> String {
        let mut lines = Vec::new();

        lines.push("# Autopilot Environment Configuration".to_string());
        lines.push(format!(
            "Working directory: {}",
            self.working_directory.display()
        ));
        lines.push(String::new());

        // Git info
        if let Some(ref git) = self.git {
            lines.push("## Git Repository".to_string());
            if let Some(ref branch) = git.branch {
                lines.push(format!("- Branch: {}", branch));
            }
            if let Some(ref url) = git.remote_url {
                lines.push(format!("- Remote: {}", url));
            }
            if git.has_changes {
                lines.push("- Status: Has uncommitted changes".to_string());
            }
            if git.unpushed_commits > 0 {
                lines.push(format!("- Unpushed commits: {}", git.unpushed_commits));
            }
            lines.push(String::new());
        }

        // Project info
        if let Some(ref project) = self.project {
            lines.push("## Project Structure".to_string());
            if project.has_directives {
                lines.push(format!("- Directives: {} found", project.directive_count));
                if !project.directive_ids.is_empty() {
                    lines.push(format!("  - IDs: {}", project.directive_ids.join(", ")));
                }
            }
            if project.has_issues_json {
                lines.push("- Issues: issues.json present".to_string());
            }
            if project.has_autopilot_db {
                lines.push("- Database: autopilot.db present".to_string());
            }
            lines.push(String::new());
        }

        // Auth info
        lines.push("## Authentication".to_string());
        if !self.auth.authenticated_providers.is_empty() {
            lines.push(format!(
                "- Authenticated: {}",
                self.auth.authenticated_providers.join(", ")
            ));
        } else {
            lines.push("- No providers authenticated".to_string());
        }
        lines.push(String::new());

        // Inference info
        lines.push("## Inference Providers".to_string());
        if !self.inference.cloud_providers.is_empty() {
            lines.push(format!(
                "- Cloud: {}",
                self.inference.cloud_providers.join(", ")
            ));
        }
        for backend in &self.inference.local_backends {
            if backend.available {
                let models = if backend.models.is_empty() {
                    String::new()
                } else {
                    format!(" (models: {})", backend.models.join(", "))
                };
                lines.push(format!("- Local: {}{}", backend.name, models));
            }
        }
        lines.push(String::new());

        // Tools
        lines.push("## Available Tools".to_string());
        if let Some(ref claude) = self.tools.claude {
            lines.push(format!("- claude: {}", claude.path.display()));
        }
        if let Some(ref codex) = self.tools.codex {
            lines.push(format!("- codex: {}", codex.path.display()));
        }
        if let Some(ref opencode) = self.tools.opencode {
            lines.push(format!("- opencode: {}", opencode.path.display()));
        }

        lines.join("\n")
    }
}

fn hash_path(path: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();
    let nanos = duration.subsec_nanos();

    // Simple ISO-ish format without chrono dependency
    format!("{}.{}", secs, nanos)
}

fn detect_git(working_dir: &Path) -> Option<GitInfo> {
    debug!("Detecting git info for {:?}", working_dir);

    // Check if we're in a git repo
    let is_repo = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(working_dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !is_repo {
        return None;
    }

    // Get repo root
    let repo_root = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(working_dir)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|| working_dir.to_path_buf());

    // Get current branch
    let branch = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(working_dir)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Get remote URL
    let remote_url = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(working_dir)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Check for changes
    let has_changes = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(working_dir)
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    // Count unpushed commits
    let unpushed_commits = Command::new("git")
        .args(["rev-list", "--count", "@{upstream}..HEAD"])
        .current_dir(working_dir)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(0);

    Some(GitInfo {
        is_repo,
        branch,
        remote_url,
        has_changes,
        unpushed_commits,
        repo_root,
    })
}

fn detect_auth() -> AuthInfo {
    debug!("Detecting auth status");

    let has_openagents_auth = matches!(auth::check_openagents_auth(), AuthStatus::Found { .. });
    let has_opencode_auth = matches!(auth::check_opencode_auth(), AuthStatus::Found { .. });

    let mut authenticated_providers = Vec::new();
    let mut providers = HashMap::new();

    // Check known providers
    for provider in &["anthropic", "openai", "openrouter", "google"] {
        let entry = auth::get_provider_auth(provider).ok().flatten();
        let authenticated = entry.is_some();

        if authenticated {
            authenticated_providers.push(provider.to_string());
        }

        let (auth_type, expired) = match &entry {
            Some(AuthEntry::Oauth { expires, .. }) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                (Some("oauth".to_string()), *expires < now)
            }
            Some(AuthEntry::Api { .. }) => (Some("api".to_string()), false),
            Some(AuthEntry::Wellknown { .. }) => (Some("wellknown".to_string()), false),
            None => (None, false),
        };

        providers.insert(
            provider.to_string(),
            ProviderAuth {
                authenticated,
                auth_type,
                expired,
            },
        );
    }

    // Also check environment variables for API keys
    if std::env::var("ANTHROPIC_API_KEY").is_ok()
        && !authenticated_providers.contains(&"anthropic".to_string())
    {
        authenticated_providers.push("anthropic".to_string());
        providers.insert(
            "anthropic".to_string(),
            ProviderAuth {
                authenticated: true,
                auth_type: Some("env".to_string()),
                expired: false,
            },
        );
    }

    if std::env::var("OPENAI_API_KEY").is_ok()
        && !authenticated_providers.contains(&"openai".to_string())
    {
        authenticated_providers.push("openai".to_string());
        providers.insert(
            "openai".to_string(),
            ProviderAuth {
                authenticated: true,
                auth_type: Some("env".to_string()),
                expired: false,
            },
        );
    }

    AuthInfo {
        authenticated_providers,
        has_openagents_auth,
        has_opencode_auth,
        providers,
    }
}

fn detect_project(working_dir: &Path) -> Option<ProjectInfo> {
    debug!("Detecting project info for {:?}", working_dir);

    let openagents_dir = working_dir.join(".openagents");

    if !openagents_dir.exists() {
        return None;
    }

    let directives_dir = openagents_dir.join("directives");
    let has_directives = directives_dir.exists();

    let mut directive_ids = Vec::new();
    let mut directive_count = 0;

    if has_directives {
        if let Ok(entries) = std::fs::read_dir(&directives_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    directive_count += 1;
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        directive_ids.push(stem.to_string());
                    }
                }
            }
        }
    }

    directive_ids.sort();

    Some(ProjectInfo {
        path: openagents_dir.clone(),
        has_directives,
        directive_count,
        has_issues_json: openagents_dir.join("issues.json").exists(),
        has_directives_md: openagents_dir.join("DIRECTIVES.md").exists(),
        has_todo_md: openagents_dir.join("TODO.md").exists(),
        has_autopilot_db: openagents_dir.join("autopilot.db").exists(),
        directive_ids,
    })
}

fn detect_inference() -> InferenceInfo {
    debug!("Detecting inference providers");

    let mut cloud_providers = Vec::new();
    let mut local_backends = Vec::new();
    let mut env_vars = HashMap::new();

    // Check cloud API keys
    let anthropic_key = std::env::var("ANTHROPIC_API_KEY").is_ok();
    let openai_key = std::env::var("OPENAI_API_KEY").is_ok();
    let openrouter_key = std::env::var("OPENROUTER_API_KEY").is_ok();
    let google_key = std::env::var("GOOGLE_API_KEY").is_ok();

    env_vars.insert("ANTHROPIC_API_KEY".to_string(), anthropic_key);
    env_vars.insert("OPENAI_API_KEY".to_string(), openai_key);
    env_vars.insert("OPENROUTER_API_KEY".to_string(), openrouter_key);
    env_vars.insert("GOOGLE_API_KEY".to_string(), google_key);

    if anthropic_key {
        cloud_providers.push("anthropic".to_string());
    }
    if openai_key {
        cloud_providers.push("openai".to_string());
    }
    if openrouter_key {
        cloud_providers.push("openrouter".to_string());
    }
    if google_key {
        cloud_providers.push("google".to_string());
    }

    // Check GPT-OSS (llama.cpp server on port 8000)
    let gpt_oss_available = check_http_endpoint("http://localhost:8000/health");
    let gpt_oss_models = if gpt_oss_available {
        get_gpt_oss_models().unwrap_or_default()
    } else {
        Vec::new()
    };

    local_backends.push(LocalBackend {
        name: "gpt-oss".to_string(),
        available: gpt_oss_available,
        endpoint: Some("http://localhost:8000".to_string()),
        models: gpt_oss_models,
    });

    // Check fm-bridge (macOS Foundation Models)
    let fm_bridge_available = check_http_endpoint("http://localhost:8081/v1/models");
    local_backends.push(LocalBackend {
        name: "fm-bridge".to_string(),
        available: fm_bridge_available,
        endpoint: Some("http://localhost:8081".to_string()),
        models: Vec::new(),
    });

    // Check for swarm providers (placeholder - would need actual implementation)
    let has_swarm_providers = std::env::var("OPENAGENTS_SWARM_URL").is_ok();

    InferenceInfo {
        cloud_providers,
        local_backends,
        has_swarm_providers,
        env_vars,
        pylon: None,                 // Filled in later by pylon integration
        swarm_providers: Vec::new(), // Filled in later by pylon integration
        neobank: None,               // Filled in later by pylon integration
    }
}

fn check_http_endpoint(url: &str) -> bool {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    let url = url.strip_prefix("http://").unwrap_or(url);
    let (host_port, path) = url.split_once('/').unwrap_or((url, ""));

    let addrs: Vec<_> = match host_port.to_socket_addrs() {
        Ok(addrs) => addrs.collect(),
        Err(_) => return false,
    };

    for addr in addrs {
        if let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
            let path = if path.is_empty() { "/" } else { path };
            let request = format!("GET /{} HTTP/1.0\r\nHost: {}\r\n\r\n", path, host_port);
            if stream.write_all(request.as_bytes()).is_ok() {
                let mut response = [0u8; 32];
                if stream.read(&mut response).is_ok() {
                    if response.starts_with(b"HTTP/") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn get_gpt_oss_models() -> Option<Vec<String>> {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    let addrs: Vec<_> = "localhost:8000".to_socket_addrs().ok()?.collect();
    let mut stream = None;
    for addr in addrs {
        if let Ok(s) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
            stream = Some(s);
            break;
        }
    }
    let mut stream = stream?;

    let request = "GET /v1/models HTTP/1.0\r\nHost: localhost:8000\r\n\r\n";
    stream.write_all(request.as_bytes()).ok()?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response).ok()?;

    let response_str = String::from_utf8_lossy(&response);
    let body_start = response_str.find("\r\n\r\n")? + 4;
    let body = &response_str[body_start..];

    #[derive(serde::Deserialize)]
    struct ModelInfo {
        id: String,
    }
    #[derive(serde::Deserialize)]
    struct ModelsResponse {
        data: Vec<ModelInfo>,
    }

    if let Ok(resp) = serde_json::from_str::<ModelsResponse>(body) {
        return Some(resp.data.into_iter().map(|m| m.id).collect());
    }

    if let Ok(models) = serde_json::from_str::<Vec<ModelInfo>>(body) {
        return Some(models.into_iter().map(|m| m.id).collect());
    }

    None
}

fn detect_tools() -> ToolsInfo {
    debug!("Detecting CLI tools");

    ToolsInfo {
        claude: detect_tool("claude"),
        codex: detect_tool("codex"),
        opencode: detect_tool("opencode"),
        git: detect_tool("git"),
        cargo: detect_tool("cargo"),
    }
}

fn detect_tool(name: &str) -> Option<ToolInfo> {
    let path = Command::new("which")
        .arg(name)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| p.exists())?;

    let version = Command::new(name)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty());

    Some(ToolInfo { path, version })
}

/// Run preflight and save config for a directory
pub fn run_preflight(working_dir: &Path) -> Result<PreflightConfig> {
    let config = PreflightConfig::run(working_dir)?;
    config.save()?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_hash_path() {
        let path = PathBuf::from("/home/user/project");
        let hash = hash_path(&path);
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 16); // DefaultHasher produces 64-bit = 16 hex chars
    }

    #[test]
    fn test_preflight_run() {
        let cwd = env::current_dir().unwrap();
        let result = PreflightConfig::run(&cwd);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert!(!config.path_hash.is_empty());
        assert_eq!(config.working_directory, cwd.canonicalize().unwrap_or(cwd));
    }

    #[test]
    fn test_detect_git() {
        let cwd = env::current_dir().unwrap();
        let git_info = detect_git(&cwd);

        // This test runs in a git repo, so should return Some
        if let Some(info) = git_info {
            assert!(info.is_repo);
            assert!(info.repo_root.exists());
        }
    }

    #[test]
    fn test_detect_tools() {
        let tools = detect_tools();

        // Git should always be available
        if let Some(git) = tools.git {
            assert!(git.path.exists());
        }
    }

    #[test]
    fn test_to_system_prompt() {
        let cwd = env::current_dir().unwrap();
        let config = PreflightConfig::run(&cwd).unwrap();

        let prompt = config.to_system_prompt();
        assert!(prompt.contains("Autopilot Environment Configuration"));
        assert!(prompt.contains("Working directory:"));
    }
}

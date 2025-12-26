//! Docker Compose wrapper for parallel agents

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// Cached compose command preference (modern `docker compose` or legacy `docker-compose`)
static COMPOSE_CMD: OnceLock<ComposeCommand> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
enum ComposeCommand {
    /// Modern `docker compose` subcommand
    Modern,
    /// Legacy standalone `docker-compose` binary
    Legacy,
}

impl ComposeCommand {
    /// Detect which compose command is available, preferring modern `docker compose`
    fn detect() -> Self {
        // Try modern `docker compose` first
        if Command::new("docker")
            .args(["compose", "version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return ComposeCommand::Modern;
        }

        // Fall back to legacy `docker-compose`
        ComposeCommand::Legacy
    }

    /// Run a compose command with the given args
    fn run(&self, args: &[String], current_dir: &std::path::Path) -> Result<std::process::Output> {
        match self {
            ComposeCommand::Modern => {
                let mut full_args = vec!["compose".to_string()];
                full_args.extend(args.iter().cloned());
                Command::new("docker")
                    .args(&full_args)
                    .current_dir(current_dir)
                    .output()
                    .context("Failed to run docker compose")
            }
            ComposeCommand::Legacy => Command::new("docker-compose")
                .args(args)
                .current_dir(current_dir)
                .output()
                .context("Failed to run docker-compose"),
        }
    }
}

/// Get the cached compose command, detecting it on first call
fn get_compose_command() -> &'static ComposeCommand {
    COMPOSE_CMD.get_or_init(ComposeCommand::detect)
}

/// Agent container status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    /// Agent is actively running and processing issues
    Running,
    /// Agent container is stopped (not processing issues)
    Stopped,
    /// Agent is in the process of starting up
    Starting,
    /// Agent encountered an error and may have crashed
    Error,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Stopped => write!(f, "stopped"),
            AgentStatus::Starting => write!(f, "starting"),
            AgentStatus::Error => write!(f, "error"),
        }
    }
}

/// Information about a running agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Agent ID (e.g., "001")
    pub id: String,
    /// Container name
    pub container_name: String,
    /// Current status
    pub status: AgentStatus,
    /// Issue currently being worked on (if any)
    pub current_issue: Option<i32>,
    /// Uptime in seconds
    pub uptime_seconds: Option<u64>,
}

fn agent_service_names(count: usize) -> Vec<String> {
    (1..=count).map(|i| format!("agent-{:03}", i)).collect()
}

fn normalize_agent_id(agent_id: &str) -> Result<String> {
    let trimmed = agent_id.trim();
    let numeric = trimmed
        .strip_prefix("agent-")
        .or_else(|| trimmed.strip_prefix("autopilot-"))
        .unwrap_or(trimmed);

    if numeric.is_empty() {
        anyhow::bail!("Agent ID cannot be empty");
    }

    let id: u16 = numeric
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid agent id '{}'", agent_id))?;

    if id == 0 {
        anyhow::bail!("Agent ID must be >= 1");
    }

    Ok(format!("{:03}", id))
}

fn agent_service_name(agent_id: &str) -> Result<String> {
    Ok(format!("agent-{}", normalize_agent_id(agent_id)?))
}

fn stop_agent_args(agent_id: &str, compose_file: &std::path::Path) -> Result<Vec<String>> {
    let service = agent_service_name(agent_id)?;
    Ok(vec![
        "-f".to_string(),
        compose_file.to_string_lossy().to_string(),
        "stop".to_string(),
        service,
    ])
}

/// Start N agents using docker-compose
///
/// This will:
/// 1. Create git worktrees for each agent
/// 2. Start the docker containers
pub async fn start_agents(count: usize) -> Result<Vec<AgentInfo>> {
    // Run blocking operations in a separate thread to not block the async runtime
    tokio::task::spawn_blocking(move || -> Result<()> {
        let project_root = find_project_root()?;
        let compose_file = project_root.join("docker/autopilot/docker-compose.yml");

        if !compose_file.exists() {
            anyhow::bail!("docker-compose.yml not found at {:?}", compose_file);
        }

        // Create worktrees first
        super::worktree::create_worktrees(&project_root, count)?;

        // Build services list
        let services = agent_service_names(count);

        // Determine profiles
        let mut args = vec!["-f".to_string(), compose_file.to_string_lossy().to_string()];

        if count > 3 {
            args.extend(["--profile".to_string(), "extended".to_string()]);
        }
        if count > 5 {
            args.extend(["--profile".to_string(), "linux-full".to_string()]);
        }

        args.extend(["up".to_string(), "-d".to_string()]);
        args.extend(services);

        // Run docker compose (modern) or docker-compose (legacy)
        let output = get_compose_command().run(&args, &project_root)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("docker compose failed: {}", stderr);
        }

        Ok(())
    })
    .await
    .context("spawn_blocking failed")??;

    // Return agent info
    list_agents().await
}

/// Stop all agents
pub async fn stop_agents() -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        let project_root = find_project_root()?;
        let compose_file = project_root.join("docker/autopilot/docker-compose.yml");

        let args = vec![
            "-f".to_string(),
            compose_file.to_string_lossy().to_string(),
            "--profile".to_string(),
            "extended".to_string(),
            "--profile".to_string(),
            "linux-full".to_string(),
            "down".to_string(),
        ];

        let output = get_compose_command().run(&args, &project_root)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("docker compose down failed: {}", stderr);
        }

        Ok(())
    })
    .await
    .context("spawn_blocking failed")??;

    Ok(())
}

/// Stop a single agent by ID
pub async fn stop_agent(agent_id: &str) -> Result<()> {
    let agent_id = agent_id.to_string();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let project_root = find_project_root()?;
        let compose_file = project_root.join("docker/autopilot/docker-compose.yml");

        let args = stop_agent_args(&agent_id, &compose_file)?;

        let output = get_compose_command().run(&args, &project_root)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("docker compose stop failed: {}", stderr);
        }

        Ok(())
    })
    .await
    .context("spawn_blocking failed")??;

    Ok(())
}

/// Query current issue for a given agent/container from database
fn query_current_issue(container_name: &str) -> Option<i32> {
    let db_path = find_project_root().ok()?.join(".openagents/autopilot.db");
    if !db_path.exists() {
        return None;
    }

    let conn = Connection::open(&db_path).ok()?;

    // Query for issues claimed by this container that are in progress
    conn.query_row(
        "SELECT number FROM issues WHERE claimed_by = ? AND status = 'in_progress' LIMIT 1",
        [container_name],
        |row| row.get(0),
    )
    .ok()
}

/// Parse uptime in seconds from docker status string
fn parse_uptime_seconds(status_str: &str) -> Option<u64> {
    // Docker status format: "Up X seconds/minutes/hours/days"
    if !status_str.starts_with("Up ") {
        return None;
    }

    let parts: Vec<&str> = status_str[3..].split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let value: u64 = parts[0].parse().ok()?;
    let unit = parts[1];

    let seconds = match unit {
        "second" | "seconds" => value,
        "minute" | "minutes" => value * 60,
        "hour" | "hours" => value * 3600,
        "day" | "days" => value * 86400,
        "week" | "weeks" => value * 604800,
        _ => return None,
    };

    Some(seconds)
}

fn parse_agent_ps_output_with_lookup<F>(output: &str, lookup: F) -> Vec<AgentInfo>
where
    F: Fn(&str) -> Option<i32>,
{
    let mut agents = Vec::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }

        let name = parts[0];
        let status_str = parts[1];

        if !name.starts_with("autopilot-") {
            continue;
        }

        let id = name.strip_prefix("autopilot-").unwrap_or(name);
        let status = if status_str.contains("Up") {
            AgentStatus::Running
        } else if status_str.contains("Exited") {
            AgentStatus::Stopped
        } else if status_str.contains("Starting") {
            AgentStatus::Starting
        } else {
            AgentStatus::Error
        };

        agents.push(AgentInfo {
            id: id.to_string(),
            container_name: name.to_string(),
            status,
            current_issue: lookup(name),
            uptime_seconds: parse_uptime_seconds(status_str),
        });
    }

    agents.sort_by(|a, b| a.id.cmp(&b.id));
    agents
}

fn parse_agent_ps_output(output: &str) -> Vec<AgentInfo> {
    parse_agent_ps_output_with_lookup(output, query_current_issue)
}

/// List all agents and their status
pub async fn list_agents() -> Result<Vec<AgentInfo>> {
    tokio::task::spawn_blocking(move || -> Result<Vec<AgentInfo>> {
        let output = Command::new("docker")
            .args(["ps", "--format", "{{.Names}}\t{{.Status}}", "-a"])
            .output()
            .context("Failed to run docker ps")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parse_agent_ps_output(&stdout))
    })
    .await
    .context("spawn_blocking failed")?
}

/// Get logs for a specific agent
pub async fn get_logs(agent_id: &str, lines: Option<usize>) -> Result<String> {
    let container_name = if agent_id.starts_with("autopilot-") {
        agent_id.to_string()
    } else {
        format!("autopilot-{}", agent_id)
    };

    let lines = lines;
    tokio::task::spawn_blocking(move || -> Result<String> {
        let mut args = vec!["logs".to_string()];
        if let Some(n) = lines {
            args.push("--tail".to_string());
            args.push(n.to_string());
        }
        args.push(container_name);

        let output = Command::new("docker")
            .args(&args)
            .output()
            .context("Failed to get docker logs")?;

        let logs = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Docker logs go to stderr for container stderr
        Ok(format!("{}{}", logs, stderr))
    })
    .await
    .context("spawn_blocking failed")?
}

/// Find project root by looking for Cargo.toml with [workspace]
fn find_project_root() -> Result<PathBuf> {
    let mut current = std::env::current_dir()?;

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = std::fs::read_to_string(&cargo_toml)?;
            if content.contains("[workspace]") {
                return Ok(current);
            }
        }

        if !current.pop() {
            break;
        }
    }

    // Fall back to current directory
    std::env::current_dir().context("Failed to get current directory")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status_serialize() {
        let status = AgentStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
    }

    #[test]
    fn test_agent_info_serialize() {
        let info = AgentInfo {
            id: "001".to_string(),
            container_name: "autopilot-001".to_string(),
            status: AgentStatus::Running,
            current_issue: Some(42),
            uptime_seconds: Some(300),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"id\":\"001\""));
        assert!(json.contains("\"current_issue\":42"));
    }

    #[test]
    fn test_parse_uptime_seconds() {
        assert_eq!(parse_uptime_seconds("Up 5 seconds"), Some(5));
        assert_eq!(parse_uptime_seconds("Up 30 seconds"), Some(30));
        assert_eq!(parse_uptime_seconds("Up 2 minutes"), Some(120));
        assert_eq!(parse_uptime_seconds("Up 1 hour"), Some(3600));
        assert_eq!(parse_uptime_seconds("Up 3 hours"), Some(10800));
        assert_eq!(parse_uptime_seconds("Up 2 days"), Some(172800));
        assert_eq!(parse_uptime_seconds("Up 1 week"), Some(604800));
    }

    #[test]
    fn test_parse_uptime_invalid() {
        assert_eq!(parse_uptime_seconds("Exited (0) 5 minutes ago"), None);
        assert_eq!(parse_uptime_seconds("Created"), None);
        assert_eq!(parse_uptime_seconds("Up"), None);
        assert_eq!(parse_uptime_seconds(""), None);
    }

    #[test]
    fn test_agent_service_names() {
        let names = agent_service_names(3);
        assert_eq!(
            names,
            vec![
                "agent-001".to_string(),
                "agent-002".to_string(),
                "agent-003".to_string(),
            ]
        );
    }

    #[test]
    fn test_agent_service_name_parsing() {
        assert_eq!(agent_service_name("1").unwrap(), "agent-001");
        assert_eq!(agent_service_name("001").unwrap(), "agent-001");
        assert_eq!(agent_service_name("agent-010").unwrap(), "agent-010");
        assert_eq!(agent_service_name("autopilot-007").unwrap(), "agent-007");
        assert!(agent_service_name("0").is_err());
        assert!(agent_service_name("agent-abc").is_err());
    }

    #[test]
    fn test_stop_agent_args() {
        let args = stop_agent_args("2", std::path::Path::new("/tmp/compose.yml")).unwrap();
        assert_eq!(
            args,
            vec![
                "-f".to_string(),
                "/tmp/compose.yml".to_string(),
                "stop".to_string(),
                "agent-002".to_string(),
            ]
        );
    }

    #[test]
    fn test_parse_agent_ps_output() {
        let output = "\
autopilot-002\tExited (0) 5 minutes ago\n\
autopilot-001\tUp 2 minutes\n\
random\tUp 1 hour\n\
autopilot-003\tStarting 10 seconds\n";

        let agents = parse_agent_ps_output_with_lookup(output, |_| None);

        assert_eq!(agents.len(), 3);
        assert_eq!(agents[0].id, "001");
        assert_eq!(agents[0].status, AgentStatus::Running);
        assert_eq!(agents[1].id, "002");
        assert_eq!(agents[1].status, AgentStatus::Stopped);
        assert_eq!(agents[2].id, "003");
        assert_eq!(agents[2].status, AgentStatus::Starting);
    }
}

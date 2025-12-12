# Parallel Agent Execution with Container Isolation

This document describes the container-based parallel agent execution system that enables multiple AI agents to work on tasks simultaneously in isolated environments.

## Overview

The parallel execution system replaces the previous git worktree approach (which caused agent collisions) with strict container isolation. Each agent gets:

- Fresh `git clone` of the repository
- Isolated Docker container environment
- Dedicated branch (`agent/<id>`)
- Push-to-branch workflow for PR-based merging

## Architecture

```
ParallelOrchestrator
    ├── ContainerManager (creates isolated containers with fresh git clones)
    │   └── Backend: Docker | macOS Container (via crates/sandbox)
    ├── AgentPool (manages N agent instances)
    │   ├── Agent[0] → Container[0] → agent/agent-0 branch
    │   ├── Agent[1] → Container[1] → agent/agent-1 branch
    │   └── Agent[N] → Container[N] → agent/agent-N branch
    └── ResultAggregator (tracks completions, reports progress)
```

## Components

### Execution Types (`crates/taskmaster/src/types/execution.rs`)

```rust
/// Where a task executes
pub enum ExecutionMode {
    None,       // Not scheduled for execution
    Local,      // Local machine (legacy)
    Container,  // Isolated Docker container (default)
}

/// Task execution lifecycle
pub enum ExecutionState {
    Unscheduled,   // Not yet scheduled
    Queued,        // Waiting for available agent
    Provisioning,  // Container being created
    Running,       // Agent actively working
    Succeeded,     // Completed successfully
    Failed,        // Completed with errors
    Lost,          // Container/agent lost
    Cancelled,     // User cancelled
}
```

### Issue Execution Fields (`crates/taskmaster/src/types/issue.rs`)

Each task (Issue) tracks its execution context:

| Field | Type | Description |
|-------|------|-------------|
| `execution_mode` | `ExecutionMode` | Where task executes (Container/Local/None) |
| `execution_state` | `ExecutionState` | Current lifecycle state |
| `container_id` | `Option<String>` | Docker container ID |
| `agent_id` | `Option<String>` | Agent identifier (e.g., "agent-0") |
| `execution_branch` | `Option<String>` | Git branch for agent's work |
| `execution_started_at` | `Option<DateTime>` | When execution began |
| `execution_finished_at` | `Option<DateTime>` | When execution ended |
| `execution_exit_code` | `Option<i32>` | Container exit code |

### Container Manager (`crates/parallel/src/container_manager.rs`)

Manages the full lifecycle of containerized agents:

```rust
pub struct ContainerManager {
    backend: Arc<dyn ContainerBackend>,  // Docker or macOS container
    containers: HashMap<String, ContainerAgentState>,
    workspace_base: PathBuf,
    credential_mount: Option<CredentialMount>,
}

impl ContainerManager {
    /// Create manager with auto-detected backend
    pub async fn new(workspace_base: PathBuf, default_image: String) -> Result<Self>;

    /// Provision container with fresh git clone
    pub async fn provision(&self, config: ContainerAgentConfig) -> Result<String>;

    /// Execute task via Claude CLI in container
    pub async fn execute_task(&self, agent_id: &str, task_id: &str, desc: &str) -> Result<ContainerRunResult>;

    /// Commit and push changes to agent branch
    pub async fn push_changes(&self, agent_id: &str, commit_message: &str) -> Result<String>;

    /// Clean up container and workspace
    pub async fn cleanup(&self, agent_id: &str) -> Result<()>;

    /// Detect lost/failed containers
    pub async fn health_check(&self) -> Vec<String>;
}
```

### Container Agent Configuration

```rust
pub struct ContainerAgentConfig {
    pub agent_id: String,           // Unique identifier
    pub image: String,              // Docker image (default: "openagents/agent:latest")
    pub remote_url: String,         // Git remote to clone
    pub branch: String,             // Branch name (default: "agent/{agent_id}")
    pub memory_limit: Option<String>, // Memory limit (default: "8G")
    pub cpu_limit: Option<f32>,     // CPU limit (default: 2.0)
    pub timeout_secs: Option<u64>,  // Timeout (default: 3600s / 1 hour)
}
```

### Orchestrator (`crates/parallel/src/orchestrator.rs`)

The orchestrator supports both legacy worktree and new container backends:

```rust
pub enum ExecutionBackend {
    Worktree(WorktreeManager),           // Legacy: git worktrees
    Container(Arc<ContainerManager>),    // New: isolated containers
}

pub struct ParallelConfig {
    // ... existing fields ...
    pub use_containers: bool,        // Default: true
    pub container_image: String,     // Default: "openagents/agent:latest"
    pub git_remote_url: String,      // Required for container mode
    pub container_memory: Option<String>,  // Default: "8G"
    pub container_cpus: Option<f32>,       // Default: 2.0
}
```

## Docker Agent Image

### Building the Image

```bash
# Build with default tag (:latest)
./docker/agent/build.sh

# Build with specific version
./docker/agent/build.sh v1.0.0
```

### Image Contents (`docker/agent/Dockerfile`)

Base: Ubuntu 22.04 LTS

Installed tools:
- **Bun** - JavaScript/TypeScript runtime
- **Rust** - Stable toolchain
- **Node.js 20** - For compatibility
- **Git** - Pre-configured for agent commits
- **ripgrep, fd-find, jq** - Search and parsing tools

Git configuration:
```
user.email = agent@openagents.com
user.name = OpenAgents Agent
init.defaultBranch = main
push.autoSetupRemote = true
```

Environment variables:
- `AGENT_ID` - Agent identifier
- `TASK_ID` - Current task ID
- `GIT_BRANCH` - Agent's working branch

### Running Manually

```bash
docker run -it -v $(pwd):/workspace openagents/agent:latest
```

## Database Schema

The taskmaster SQLite database includes execution tracking columns (SCHEMA_V2):

```sql
ALTER TABLE issues ADD COLUMN execution_mode TEXT DEFAULT 'none';
ALTER TABLE issues ADD COLUMN execution_state TEXT DEFAULT 'unscheduled';
ALTER TABLE issues ADD COLUMN container_id TEXT;
ALTER TABLE issues ADD COLUMN agent_id TEXT;
ALTER TABLE issues ADD COLUMN execution_branch TEXT;
ALTER TABLE issues ADD COLUMN execution_started_at TEXT;
ALTER TABLE issues ADD COLUMN execution_finished_at TEXT;
ALTER TABLE issues ADD COLUMN execution_exit_code INTEGER;

CREATE INDEX idx_issues_execution_state ON issues(execution_state);
CREATE INDEX idx_issues_agent_id ON issues(agent_id);
CREATE INDEX idx_issues_container_id ON issues(container_id);
```

## Workflow

### 1. Task Assignment

```rust
// Update task with execution context
issue.set_execution_mode(ExecutionMode::Container)
     .set_execution_state(ExecutionState::Queued)
     .set_agent_id("agent-0")
     .set_execution_branch("agent/agent-0");
```

### 2. Container Provisioning

```rust
let config = ContainerAgentConfig::new("agent-0", "openagents/agent:latest", &remote_url)
    .memory_limit("8G")
    .cpu_limit(2.0)
    .timeout_secs(3600);

container_manager.provision(config).await?;
// State: Provisioning → Running
```

### 3. Task Execution

```rust
let result = container_manager.execute_task(
    "agent-0",
    "task-123",
    "Implement feature X with tests"
).await?;

if result.exit_code == 0 {
    // Push changes to agent branch
    let sha = container_manager.push_changes("agent-0", "Implement feature X").await?;
    // State: Running → Succeeded
} else {
    // State: Running → Failed
}
```

### 4. Merge via PR

After agent pushes to `agent/agent-0` branch:
1. Create PR from `agent/agent-0` to `main`
2. Review changes
3. Merge PR
4. Clean up container

```rust
container_manager.cleanup("agent-0").await?;
```

## Error Handling

The system handles these failure modes:

| Error | Recovery |
|-------|----------|
| `ContainerNotAvailable` | No Docker runtime - fall back to local or fail |
| `CloneFailed` | Git clone failed - check remote URL, network |
| `ContainerExecutionFailed` | Task failed in container - check logs, retry |
| `PushFailed` | Git push failed - check auth, branch protection |
| `ContainerLost` | Container disappeared - reprovision and retry |

## Configuration

### ParallelConfig Defaults

```rust
ParallelConfig {
    max_agents: 2,
    auto_merge: true,
    use_containers: true,  // Container mode by default
    container_image: "openagents/agent:latest".to_string(),
    git_remote_url: String::new(),  // Must be set
    container_memory: Some("8G".to_string()),
    container_cpus: Some(2.0),
}
```

### Creating Container-Based Orchestrator

```rust
let config = ParallelConfig {
    git_remote_url: "https://github.com/org/repo.git".to_string(),
    ..Default::default()
};

let orchestrator = ParallelOrchestrator::new_with_containers(
    repo_path,
    config,
    workspace_base,
).await?;
```

## Credentials

The system automatically extracts and mounts Claude API credentials into containers using the credential mount system from `crates/sandbox`. This allows agents to make Claude API calls from within containers.

## Related Files

| File | Purpose |
|------|---------|
| `crates/taskmaster/src/types/execution.rs` | Execution mode/state types |
| `crates/taskmaster/src/types/issue.rs` | Issue struct with execution fields |
| `crates/taskmaster/src/storage/schema.rs` | Database schema with V2 migration |
| `crates/taskmaster/src/storage/sqlite.rs` | Persistence layer |
| `crates/parallel/src/container_manager.rs` | Container lifecycle management |
| `crates/parallel/src/orchestrator.rs` | Parallel execution orchestrator |
| `crates/parallel/src/error.rs` | Error types |
| `crates/sandbox/` | Container backend abstraction |
| `docker/agent/Dockerfile` | Agent container image |
| `docker/agent/build.sh` | Image build script |

## User Stories Implemented

- **PAR-001**: Run multiple agents in parallel
- **PAR-002**: Load balance tasks across agents
- **PAR-003**: Handle agent failures gracefully
- **PAR-004**: Aggregate results from parallel runs
- **PAR-005**: Report progress across all agents
- **PAR-010**: Create isolated containers for each agent
- **PAR-011**: Manage container lifecycle (provision/cleanup)
- **PAR-012**: Push completed work to agent branches
- **PAR-013**: Handle container failures

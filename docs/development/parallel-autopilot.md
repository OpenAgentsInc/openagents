# Parallel Autopilot - Multi-Agent Container System

Run multiple autopilot agents in parallel using Docker containers with shared issue database and git worktrees for maximum throughput.

## Quick Start

```bash
# Start 3 agents (default)
./scripts/parallel-autopilot.sh start

# Start 5 agents
./scripts/parallel-autopilot.sh start 5

# Check status
./scripts/parallel-autopilot.sh status

# View logs
./scripts/parallel-autopilot.sh logs     # all agents
./scripts/parallel-autopilot.sh logs 2   # agent-002 only

# Stop all
./scripts/parallel-autopilot.sh stop

# Cleanup worktrees and branches
./scripts/parallel-autopilot.sh cleanup
```

## Architecture

### Overview

```
Host Machine
├── .git/                    (shared object database)
├── autopilot.db             (SHARED - atomic issue claiming)
├── .worktrees/
│   ├── agent-001/           (worktree → branch agent/001)
│   ├── agent-002/           (worktree → branch agent/002)
│   └── agent-00N/
└── docs/logs/               (shared log output)

Docker Containers
├── autopilot-001 → /workspace mounted from .worktrees/agent-001
├── autopilot-002 → /workspace mounted from .worktrees/agent-002
└── autopilot-00N → each mounts shared autopilot.db
```

### Key Design Principles

1. **Atomic Issue Claiming**: All containers share `autopilot.db` via bind mount. SQLite's atomic operations prevent race conditions. The existing 15-minute claim expiry in `crates/issues/src/issue.rs` ensures crashed agents release issues automatically.

2. **Git Isolation**: Each agent works in a separate git worktree with its own branch (`agent/001`, `agent/002`, etc.). This prevents merge conflicts between simultaneous pushes.

3. **Shared Logs**: All agents write to the shared `docs/logs/` directory for centralized monitoring.

4. **Resource Limits**: Per-platform defaults ensure optimal performance:
   - Linux: 12GB RAM, 4 CPUs per agent (max 10 agents)
   - macOS: 3GB RAM, 2 CPUs per agent (max 5 agents)

## Platform Detection

The system automatically detects your platform and applies appropriate limits:

```bash
# Linux (Intel i7-14700K, 128GB RAM, RTX 4080)
AGENT_MEMORY=12G AGENT_CPUS=4 MAX_AGENTS=10

# macOS (Apple M2 Pro, 16GB RAM)
AGENT_MEMORY=3G AGENT_CPUS=2 MAX_AGENTS=5
```

Override with environment variables:

```bash
AGENT_MEMORY=8G AGENT_CPUS=2 ./scripts/parallel-autopilot.sh start 5
```

## Container Profiles

The docker-compose.yml uses profiles to control which agents start:

- **Default** (no profile): agents 001-003
- **extended**: agents 001-005
- **linux-full**: agents 001-010

Examples:

```bash
# 3 agents (default)
docker-compose -f docker/autopilot/docker-compose.yml up -d

# 5 agents (extended)
docker-compose -f docker/autopilot/docker-compose.yml --profile extended up -d

# 10 agents (Linux only)
docker-compose -f docker/autopilot/docker-compose.yml --profile linux-full up -d
```

The `parallel-autopilot.sh` script handles this automatically based on the count you provide.

## Git Worktrees

### How Worktrees Work

A git worktree is a linked working directory that shares the same `.git` database. Each worktree can be on a different branch without affecting others.

```bash
# Create worktree for agent-001
git worktree add .worktrees/agent-001 -b agent/001 main

# List all worktrees
git worktree list
```

### Automatic Management

The `parallel-autopilot.sh` script manages worktrees automatically:

- **On start**: Creates missing worktrees before launching containers
- **On cleanup**: Removes all worktrees and agent branches

### Manual Worktree Management

If you need to manually manage worktrees:

```bash
# Create worktree
git worktree add .worktrees/agent-005 -b agent/005 main

# Remove worktree
git worktree remove .worktrees/agent-005

# Prune stale references
git worktree prune

# Delete agent branch
git branch -D agent/005
```

## Container Lifecycle

### Starting Agents

When you run `./scripts/parallel-autopilot.sh start N`:

1. Platform detection sets resource limits
2. Worktrees are created (if missing)
3. Docker builds the autopilot image (if not cached)
4. Containers start with mounted volumes
5. Each agent runs the full autopilot loop

### What Runs in Each Container

Each container executes:

```bash
/usr/local/bin/autopilot --workdir /workspace --project openagents --full-auto
```

With environment variables:

- `ISSUES_DB=/shared/autopilot.db` - shared database path
- `AUTOPILOT_MODEL=sonnet` - model to use
- `AUTOPILOT_FULL_AUTO=true` - enable autonomous loop
- `AGENT_ID=001` - unique agent identifier

### Monitoring

View real-time status:

```bash
./scripts/parallel-autopilot.sh status
```

Output:

```
=== Running Containers ===
NAME              STATE    CREATED
autopilot-001     Up       2 minutes ago
autopilot-002     Up       2 minutes ago
autopilot-003     Up       2 minutes ago

=== Issue Status ===
Open issues:
#42   [high]   Implement feature X
#45   [medium] Fix bug in Y

In progress:
#42   [agent-001] Implement feature X
#45   [agent-002] Fix bug in Y
```

### Logs

Each agent writes to:

- Container stdout/stderr: `docker logs autopilot-001`
- Trajectory logs: `docs/logs/YYYYMMDD/HHMMSS-*.rlog`

Tail logs:

```bash
# All agents
./scripts/parallel-autopilot.sh logs

# Specific agent
./scripts/parallel-autopilot.sh logs 2
```

## Issue Coordination

### Atomic Claiming

The `crates/issues/src/issue.rs` module provides atomic issue claiming:

```rust
pub fn claim_issue(number: i32, agent: &str) -> Result<bool>
```

This uses SQLite's atomic write-ahead-log (WAL) mode to ensure only one agent can claim an issue:

```sql
UPDATE issues
SET status = 'in_progress',
    claimed_by = ?,
    claimed_at = CURRENT_TIMESTAMP
WHERE number = ?
  AND status = 'open'
  AND (claimed_by IS NULL OR claimed_at < datetime('now', '-15 minutes'))
```

### Claim Expiry

If an agent crashes, its claimed issues are automatically released after 15 minutes. This prevents orphaned issues from blocking the queue.

### No Coordination Protocol Needed

Because the issue database is shared and SQLite provides atomic operations, there's no need for explicit coordination between agents. They simply:

1. Query for open issues
2. Attempt to claim one atomically
3. Work on it if claim succeeded
4. Push and complete when done
5. Repeat

## Dockerfile

The container image is defined in `docker/autopilot/Dockerfile`:

```dockerfile
FROM rust:1.85-bookworm

# Install system dependencies
RUN apt-get update && apt-get install -y \
    sqlite3 git curl build-essential

# Install Claude Code CLI
RUN curl -fsSL https://install.claude.com | sh

# Build autopilot binary
COPY . /build
WORKDIR /build
RUN cargo build --release --bin autopilot

# Install binaries
RUN cp target/release/autopilot /usr/local/bin/ && \
    cp target/release/issues-mcp /usr/local/bin/

# Create agent user
ARG HOST_UID=1000
RUN useradd -m -u ${HOST_UID} agent

USER agent
WORKDIR /workspace

CMD ["/usr/local/bin/autopilot", "--full-auto"]
```

Key points:

- Uses Rust 1.85 base image for consistent toolchain
- Installs Claude Code CLI for agent execution
- Pre-builds autopilot binary for faster startup
- Creates non-root user matching host UID for file permissions
- Mounts `/workspace` from worktree

## docker-compose.yml

The compose file uses YAML anchors to define common configuration:

```yaml
x-agent-common: &agent-common
  build:
    context: ../..
    dockerfile: docker/autopilot/Dockerfile
    args:
      HOST_UID: ${HOST_UID:-1000}
  environment:
    - ISSUES_DB=/shared/autopilot.db
    - AUTOPILOT_MODEL=${AUTOPILOT_MODEL:-sonnet}
    - AUTOPILOT_FULL_AUTO=true
  restart: unless-stopped
  deploy:
    resources:
      limits:
        memory: ${AGENT_MEMORY:-12G}
        cpus: "${AGENT_CPUS:-4}"
```

Each agent service extends this common configuration:

```yaml
services:
  agent-001:
    <<: *agent-common
    container_name: autopilot-001
    hostname: agent-001
    volumes:
      - ../../.worktrees/agent-001:/workspace:rw
      - ../../autopilot.db:/shared/autopilot.db:rw
      - ../../docs/logs:/workspace/docs/logs:rw
      - ~/.claude:/home/agent/.claude:rw
      - ~/.gitconfig:/home/agent/.gitconfig:ro
      - ~/.ssh:/home/agent/.ssh:ro
    environment:
      - AGENT_ID=001
```

## Programmatic API

Use the Rust API from `crates/autopilot-core/src/parallel/`:

### Basic Usage

```rust
use autopilot::parallel::{start_agents, stop_agents, list_agents};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Start 3 agents with default settings
    let agents = start_agents(3).await?;
    println!("Started {} agents", agents.len());

    for agent in &agents {
        println!("Agent {}: {:?}", agent.id, agent.status);
    }

    // Stop all agents when done
    stop_agents().await?;

    Ok(())
}
```

### Custom Configuration

```rust
use autopilot::parallel::{ParallelConfig, Platform};

let platform = Platform::detect();
let config = ParallelConfig {
    agent_count: 5,
    memory_limit: "8G".to_string(),
    cpu_limit: "2".to_string(),
    model: "opus".to_string(),
    project_root: std::env::current_dir()?,
};

println!("Platform: {:?}", platform);
println!("Max agents: {}", platform.max_agents());
```

### Monitoring Agent Status

```rust
use autopilot::parallel::{list_agents, AgentStatus};
use tokio::time::{sleep, Duration};

async fn monitor_agents() -> anyhow::Result<()> {
    loop {
        let agents = list_agents().await?;

        for agent in agents {
            match agent.status {
                AgentStatus::Running => {
                    println!("✓ Agent {} is working", agent.id);
                }
                AgentStatus::Stopped => {
                    println!("○ Agent {} is stopped", agent.id);
                }
                AgentStatus::Failed => {
                    println!("✗ Agent {} failed", agent.id);
                }
            }
        }

        sleep(Duration::from_secs(5)).await;
    }
}
```

### Viewing Agent Logs

```rust
use autopilot::parallel::get_logs;

async fn tail_agent_logs(agent_id: &str) -> anyhow::Result<()> {
    let logs = get_logs(agent_id, Some(50)).await?;
    println!("Last 50 lines from agent {}:\n{}", agent_id, logs);
    Ok(())
}
```

### Complete Example: Dynamic Scaling

```rust
use autopilot::parallel::{start_agents, stop_agents, list_agents, Platform};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let platform = Platform::detect();

    // Start with 3 agents
    let mut agent_count = 3;
    start_agents(agent_count).await?;
    println!("Started {} agents", agent_count);

    // Monitor and scale up if needed
    loop {
        let agents = list_agents().await?;
        let running = agents.iter().filter(|a| a.status == AgentStatus::Running).count();

        println!("Running: {}/{}", running, agent_count);

        // If all agents are busy and we can add more
        if running == agent_count && agent_count < platform.max_agents() {
            agent_count += 2;
            println!("Scaling up to {} agents", agent_count);
            stop_agents().await?;
            start_agents(agent_count).await?;
        }

        sleep(Duration::from_secs(30)).await;
    }
}
```

### Integration with Issue Database

```rust
use autopilot::parallel::start_agents;
use autopilot::db::IssueDatabase;

async fn run_parallel_autopilot() -> anyhow::Result<()> {
    // Connect to issue database
    let db = IssueDatabase::open("autopilot.db").await?;

    // Count open issues
    let open_count = db.count_open_issues().await?;
    println!("Found {} open issues", open_count);

    // Determine optimal agent count
    let agent_count = match open_count {
        0..=5 => 1,
        6..=15 => 3,
        16..=30 => 5,
        _ => 10,
    };

    println!("Starting {} agents", agent_count);
    start_agents(agent_count).await?;

    Ok(())
}
```

## Troubleshooting

### Container fails to start

Check logs:

```bash
docker logs autopilot-001
```

Common issues:

- **Missing credentials**: Ensure `~/.claude/` exists
- **Out of memory**: Reduce agent count or increase Docker memory limit
- **Missing database**: Run `cargo run -p autopilot -- issue list` to initialize

### Issues not being claimed

Check database status:

```bash
sqlite3 autopilot.db "SELECT number, title, status, claimed_by FROM issues WHERE status IN ('open', 'in_progress')"
```

If issues are stuck as claimed with old timestamps:

```bash
# Release stale claims
cargo run -p autopilot -- issue release-stale
```

### Merge conflicts on push

Each agent uses its own branch, so this shouldn't happen. If it does:

1. Check the agent's worktree:

```bash
cd .worktrees/agent-001
git status
git log --oneline -5
```

2. The agent should be on branch `agent/001`, not `main`
3. If needed, reset the worktree:

```bash
./scripts/parallel-autopilot.sh cleanup
./scripts/parallel-autopilot.sh start N
```

### Out of memory

Reduce agent count or memory per agent:

```bash
# Fewer agents
./scripts/parallel-autopilot.sh stop
./scripts/parallel-autopilot.sh start 3

# Less memory per agent
AGENT_MEMORY=8G ./scripts/parallel-autopilot.sh start 3
```

### Cleanup stuck worktrees

If worktrees become corrupted:

```bash
# Force cleanup
./scripts/parallel-autopilot.sh cleanup

# Manual cleanup
git worktree list | grep '.worktrees' | awk '{print $1}' | xargs -I{} git worktree remove --force {}
git worktree prune
git branch | grep 'agent/' | xargs git branch -D
```

## Performance Tuning

### Optimal Agent Count

Based on hardware:

| Platform | CPU | RAM | Recommended Agents | Max Agents |
|----------|-----|-----|-------------------|------------|
| Linux Desktop | Intel i7-14700K | 128GB | 6-8 | 10 |
| Linux Server | AMD EPYC | 256GB | 10-15 | 20 |
| MacBook Pro | Apple M2 Pro | 16GB | 3 | 5 |
| MacBook Pro | Apple M3 Max | 64GB | 5-7 | 10 |

### Resource Monitoring

Monitor Docker resource usage:

```bash
docker stats
```

Monitor SQLite database:

```bash
# Check database size
ls -lh autopilot.db

# Check locks
sqlite3 autopilot.db "PRAGMA wal_checkpoint"
```

### Scaling Guidelines

Start with fewer agents and scale up:

1. Start with 3 agents
2. Monitor CPU and memory usage
3. If CPU < 80% and memory < 80%, add 2 more agents
4. Repeat until performance degrades
5. Back off to previous count

## Security Considerations

### Credential Mounting

The containers mount several sensitive directories:

- `~/.claude/` - Claude API credentials (read-write for debug logs)
- `~/.ssh/` - SSH keys for git push (read-only)
- `~/.gitconfig` - Git configuration (read-only)

These are necessary for agents to authenticate with Claude API and push commits.

### Network Isolation

The `autopilot-net` bridge network isolates containers from the host network. Agents only have outbound internet access for:

- Claude API
- GitHub (for git push)
- Package registries (if needed)

### Database Access

All containers share write access to `autopilot.db`. This is safe because:

1. SQLite uses WAL mode for concurrent writes
2. Issue claiming is atomic
3. No sensitive data is stored (only issue metadata)

## Future Improvements

- [ ] GUI integration (Phase 4 of d-018)
- [ ] Real-time WebSocket updates for agent status
- [ ] Per-agent resource monitoring dashboard
- [ ] Automatic scaling based on issue queue depth
- [ ] macOS native containers (macOS 26+)
- [ ] Kubernetes deployment for cloud servers

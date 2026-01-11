# Parallel Autopilot Container Isolation

Run multiple autopilot instances (3-10) simultaneously in isolated Docker containers to parallelize issue resolution.

## Quick Start

```bash
# Start 5 parallel agents
scripts/parallel-autopilot.sh start 5

# Check status
scripts/parallel-autopilot.sh status

# View logs
scripts/parallel-autopilot.sh logs

# Stop all agents
scripts/parallel-autopilot.sh stop

# Cleanup worktrees and branches
scripts/parallel-autopilot.sh cleanup
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PARALLEL AUTOPILOT ORCHESTRATOR                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Host Machine (Linux/macOS)                                         │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │ openagents/                                                     │ │
│  │ ├── .git/                    (shared object database)          │ │
│  │ ├── autopilot.db             (SHARED - all containers mount)   │ │
│  │ ├── .worktrees/                                                │ │
│  │ │   ├── agent-001/           (worktree → branch agent/001)     │ │
│  │ │   ├── agent-002/           (worktree → branch agent/002)     │ │
│  │ │   └── agent-00N/           (worktree → branch agent/00N)     │ │
│  │ └── docs/logs/               (shared log output)               │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  Docker Containers (one per agent)                                  │
│  ┌──────────────┐ ┌──────────────┐     ┌──────────────┐            │
│  │  agent-001   │ │  agent-002   │ ... │  agent-00N   │            │
│  │              │ │              │     │              │            │
│  │ /workspace   │ │ /workspace   │     │ /workspace   │            │
│  │ (worktree)   │ │ (worktree)   │     │ (worktree)   │            │
│  │              │ │              │     │              │            │
│  │ /shared/db   │ │ /shared/db   │     │ /shared/db   │            │
│  │ (autopilot.db)  (autopilot.db)      (autopilot.db) │            │
│  └──────────────┘ └──────────────┘     └──────────────┘            │
│                                                                      │
│  Resource Limits per Container:                                     │
│  • Linux (128GB): --memory=12g --cpus=4 (up to 10 agents)          │
│  • macOS (16GB):  --memory=3g  --cpus=2 (up to 5 agents)           │
└─────────────────────────────────────────────────────────────────────┘
```

## Key Concepts

### Git Worktrees (Not Full Clones)

Each agent gets an isolated working directory via git worktrees:

```bash
# Creates .worktrees/agent-001 with branch agent/001
git worktree add .worktrees/agent-001 -b agent/001 main
```

Benefits:
- 46% disk savings vs full clones
- Shared object database (`.git/objects`)
- Isolated index/staging per worktree
- Independent commits per agent
- Easy cleanup: `git worktree remove .worktrees/agent-001`

### Issue Coordination

All containers share a single `autopilot.db` via bind mount. The existing `claim_issue()` function in `crates/issues/src/issue.rs` provides atomic coordination:

```rust
// Atomic claim with race-condition prevention
UPDATE issues SET
  status = 'in_progress',
  claimed_by = ?,
  claimed_at = ?
WHERE id = ?
  AND status = 'open'
  AND is_blocked = 0
  AND (claimed_by IS NULL OR claimed_at < datetime('now', '-15 minutes'))
```

Features:
- Atomic UPDATE prevents duplicate claims
- 15-minute claim expiry handles crashed agents
- Priority ordering (urgent → high → medium → low)
- SQLite WAL mode for concurrent access

### Credential Injection

Credentials are mounted read-only from the host:

| Host Path | Container Path | Mode |
|-----------|----------------|------|
| `~/.codex/` | `/home/agent/.codex/` | read-only |
| `~/.gitconfig` | `/home/agent/.gitconfig` | read-only |
| `~/.ssh/` | `/home/agent/.ssh/` | read-only |

No credentials are stored in container images.

## Prerequisites

### Linux (Arch)

```bash
# Install Docker
sudo pacman -S docker docker-compose

# Start Docker daemon
sudo systemctl enable --now docker

# Add user to docker group (logout required)
sudo usermod -aG docker $USER
```

### macOS

```bash
# Install Docker Desktop
brew install --cask docker

# Start Docker Desktop from Applications
```

## Configuration

### Resource Limits

| Platform | Memory/Agent | CPU/Agent | Max Agents |
|----------|--------------|-----------|------------|
| Linux (128GB RAM) | 12 GB | 4 cores | 10 |
| macOS (16GB RAM) | 3 GB | 2 cores | 5 |

Override via environment variables:

```bash
export AGENT_MEMORY=8G
export AGENT_CPUS=2
scripts/parallel-autopilot.sh start 5
```

### Dynamic Scaling

Start agents based on open issue count:

```bash
OPEN_ISSUES=$(sqlite3 autopilot.db "SELECT COUNT(*) FROM issues WHERE status='open' AND is_blocked=0")
MAX_AGENTS=10
N=$(( OPEN_ISSUES < MAX_AGENTS ? OPEN_ISSUES : MAX_AGENTS ))
scripts/parallel-autopilot.sh start $N
```

## GUI Control (v0.1)

Autopilot v0.1 does not ship a GUI or HTTP API for parallel orchestration.
Use `scripts/parallel-autopilot.sh` for management.

## Troubleshooting

### Container Can't Access Database

Check that `autopilot.db` exists and SQLite WAL mode is enabled:

```bash
sqlite3 autopilot.db "PRAGMA journal_mode=WAL;"
```

### Permission Denied on Bind Mounts

Ensure container user UID matches host UID:

```bash
# Build with matching UID
docker-compose build --build-arg HOST_UID=$(id -u)
```

### Worktree Already Exists

Clean up stale worktrees:

```bash
git worktree prune
scripts/parallel-autopilot.sh cleanup
```

### Agent Claiming Same Issue

This shouldn't happen due to atomic claims. If it does:

1. Check SQLite isn't in rollback journal mode
2. Verify all containers mount the same `autopilot.db` file
3. Check for clock skew between containers

## Files

| File | Purpose |
|------|---------|
| `docker/autopilot/Dockerfile` | Container image definition |
| `docker/autopilot/docker-compose.yml` | Container orchestration |
| `scripts/parallel-autopilot.sh` | CLI wrapper script |

## Related

- [Directive d-018: Parallel Autopilot Container Isolation](../../.openagents/directives/d-018.md)
- [Development Computer Specs](./dev-computer-specs.md)
- [Autopilot Daemon Documentation](../autopilot/DAEMON.md)

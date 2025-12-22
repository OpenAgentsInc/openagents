# Agent Instructions

You are helping on the OpenAgents desktop foundation repo.

## Tech Stack

- **Rust** with edition 2024
- **wry/tao** for the native webview shell
- **Actix + Maud + HTMX** for the local UI server
- **Nostr core** for NIP-90 types and events
- **claude-agent-sdk** for Claude Code integration

## UI Architecture (Desktop Shell)

The desktop app is a local Actix server rendered inside a native webview:

```
openagents/            → workspace root
crates/desktop         → desktop shell (wry/tao + Actix)
crates/ui              → Maud/HTMX components
crates/compute         → NIP-90 provider core
crates/nostr/core      → protocol types
```

Conventions:
- Inline-first CSS with custom properties
- No border radius (sharp corners)
- Keep UI server-rendered (no SPA)

---

## Git Conventions

**Safety:**
- NEVER `push --force` to main
- NEVER commit unless explicitly asked
- NEVER use `-i` flag (interactive not supported)
- NEVER use destructive git commands (`git reset --hard`, `git checkout -- .`, `git restore .`) without asking first

**CRITICAL: Do NOT discard other agents' work!**

Multiple agents may be working on this repo simultaneously. If you see uncommitted changes in files you didn't modify:
- **DO NOT** run `git restore` on those files
- **DO NOT** run `git checkout -- <file>` on those files
- **DO NOT** run `git stash` without checking what you're stashing
- **DO** use `git diff <file>` to understand what changed
- **DO** commit your own changes in separate files, or wait

If a file has changes that conflict with your work, ASK the user before discarding anything. Another agent may have spent significant time on that implementation.

Example of what NOT to do:
```bash
# WRONG - discards another agent's work without checking
git restore crates/frostr/src/ecdh.rs

# RIGHT - check what's there first
git diff crates/frostr/src/ecdh.rs
# Then ask user: "This file has uncommitted changes. Should I discard them?"
```

**Autopilot Commits:**
When running in autopilot mode (autonomous issue processing), include an additional co-author line to identify work done through the autopilot system:

```
Co-Authored-By: Autopilot <autopilot@openagents.com>
```

This should appear after the Claude co-author line in commit messages:

```
Your commit message here

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
Co-Authored-By: Autopilot <autopilot@openagents.com>
```

This makes it easy to identify commits that came through the autonomous autopilot workflow vs regular Claude Code usage.

---

## Agent Coordination (CRITICAL)

**Multiple agents working on the same repo WILL cause conflicts.**

When both a human-operated Claude Code session AND autopilot are running:
1. They share the same working directory
2. File changes can be overwritten
3. Git operations can conflict
4. Uncommitted work can be lost

**Best practices:**

1. **Commit frequently** - Don't let changes sit uncommitted
2. **Push after committing** - Get changes into remote ASAP
3. **Check git status before starting work** - See if other agents have uncommitted changes
4. **Use worktrees for parallel work** (recommended):
   ```bash
   # Create a worktree for autopilot
   git worktree add ../openagents-autopilot main

   # Now autopilot can work in ../openagents-autopilot/
   # while human works in ./openagents/
   ```

5. **Stop autopilot before making major UI changes** - If you're editing files that autopilot might touch

**If you see "rejected because remote contains work":**
```bash
git stash push -m "my-work"
git pull --rebase origin main
git stash pop
git push origin main
```

---

## Database Operations

**NEVER use raw sqlite3 commands to insert or modify data.** Always use the provided APIs:

- Use `cargo autopilot issue create` or `issue_create` MCP tool to create issues
- Use `cargo autopilot issue claim/complete/block` for issue state changes
- Direct sqlite3 commands bypass counters and triggers, causing data inconsistency

If you need to query data for debugging, read-only sqlite3 commands are fine:
```bash
sqlite3 autopilot.db "SELECT * FROM issues"  # OK - read only
sqlite3 autopilot.db "INSERT INTO ..."        # NEVER - use the API
```

---

## Rust Crates

All crates in `crates/` must use `edition = "2024"` in their `Cargo.toml`.

**Testing:** Tests go in their respective crates (e.g., `crates/foo/src/tests/`). Do NOT create separate test crates.

---

## Nostr Protocol Development

When implementing NIPs (Nostr Implementation Possibilities):

**IMPORTANT: NIP specifications are in `~/code/nips/` directory.**
- Read specs from local files: `~/code/nips/09.md` for NIP-09, etc.
- Do NOT web search for NIP specifications
- Reference implementations in `~/code/nostr` and `~/code/nostr-rs-relay` (for study only)

---

## Unified OpenAgents Binary

All OpenAgents functionality is available through a single `openagents` binary.

**Running the binary:**
```bash
# During development (from workspace root)
cargo run --bin openagents -- <subcommand>

# Or build first, then run
cargo build --bin openagents --release
./target/release/openagents <subcommand>

# After installing globally
cargo install --path .
openagents <subcommand>
```

**Note:** `cargo openagents` is NOT valid syntax. Cargo subcommands require a `cargo-` prefix package.

**Available commands:**
```bash
# Launch GUI (default)
openagents

# Wallet commands
openagents wallet init          # Initialize wallet
openagents wallet whoami        # Show identity
openagents wallet balance       # Show balance
openagents wallet send <addr> <amt>

# Marketplace commands
openagents marketplace compute providers
openagents marketplace skills browse
openagents marketplace data search
openagents marketplace trajectories contribute

# Autopilot commands
openagents autopilot run "task"
openagents autopilot dashboard
openagents autopilot replay <file>

# AgentGit commands
openagents agentgit gui

# Daemon commands
openagents daemon start --workdir /path --project myproject
openagents daemon status
openagents daemon stop
```

**Note:** The legacy separate binaries (`wallet`, `marketplace`, `autopilot`, `autopilotd`, `agentgit`) have been deprecated. Use the unified `openagents` binary instead.

---

## Autopilot Daemon

The `openagents daemon` (formerly `autopilotd`) supervises autopilot worker processes, handling crashes and memory pressure.

**Starting the daemon:**
```bash
# Development
openagents daemon start --workdir /path/to/project --project myproject

# Production (systemd)
systemctl --user start openagents-daemon
```

**Commands:**
```bash
openagents daemon status         # Check daemon and worker status
openagents daemon restart-worker # Restart worker without restarting daemon
openagents daemon stop           # Stop daemon
```

**Viewing logs:**
```bash
# Worker logs are in the standard rlog location
tail -f docs/logs/$(date +%Y%m%d)/*.rlog

# Daemon logs (when running with systemd)
journalctl --user -u autopilotd -f
```

**Memory management:**
- Daemon monitors system memory every 5 seconds
- Kills node processes >500MB when memory is low (<2GB)
- Force-restarts worker when memory is critical (<1GB)
- Automatic exponential backoff on crashes (1s → 5min max)

---

## Directive Completion Standards (CRITICAL)

**A directive is NOT complete just because issues are marked "done".**

Before marking any issue as "done", you MUST verify:

1. **d-012 compliance** - No stubs, no mocks, no TODOs, no NotImplemented errors
2. **Code actually works** - Run it, test it, verify it does what it claims
3. **Real integrations** - If the issue references an SDK/library, it must be INTEGRATED not stubbed

### Spark SDK Integration (d-001)

The Spark SDK at `~/code/spark-sdk` is the reference for Breez SDK integration:
- Public API: `crates/breez-sdk/core/src/sdk.rs` (BreezSdk struct)
- Builder: `crates/breez-sdk/core/src/sdk_builder.rs`
- Models: `crates/breez-sdk/core/src/models/mod.rs`
- Examples: `docs/breez-sdk/snippets/rust/src/`

**You MUST integrate this SDK directly.** Do NOT:
- Return "requires Breez SDK integration" errors
- Comment out code with "BLOCKED" notes
- Mark Phase 1 complete when Phase 2+ are all stubbed
- Say "will integrate later" - integrate NOW or don't claim it's done

### Verification Before Marking Done

For any payment/wallet related issue:
- Does `cargo test -p spark` pass?
- Can you actually call the function and get real data back?
- Is there a dependency on `breez-sdk-spark` in Cargo.toml?
- Are the actual SDK functions being called (not mocked)?

**If ANY of these fail, the issue is NOT done.**

---

## Performance Optimization

### Async Patterns

**Choose the right async primitive for the task:**

```rust
// tokio::spawn - for independent concurrent tasks
let handle1 = tokio::spawn(async { fetch_relays() });
let handle2 = tokio::spawn(async { process_events() });
let (r1, r2) = tokio::try_join!(handle1, handle2)?;

// tokio::select - for racing tasks (first to complete wins)
tokio::select! {
    result = fetch_from_relay_a() => handle_result(result),
    result = fetch_from_relay_b() => handle_result(result),
    _ = tokio::time::sleep(Duration::from_secs(5)) => handle_timeout(),
}

// tokio::join - for parallel execution, wait for all
let (events, metadata, contacts) = tokio::join!(
    fetch_events(),
    fetch_metadata(),
    fetch_contacts(),
);
```

**Avoid blocking in async contexts:**
- Use `tokio::task::spawn_blocking` for CPU-intensive work
- Never call `.await` inside a `std::sync::Mutex` lock
- Prefer `tokio::sync::RwLock` for async-friendly locking

### Database Optimization

**Connection pooling:**
```rust
// Use sqlx pool, not individual connections
let pool = SqlitePool::connect("sqlite:autopilot.db").await?;

// Reuse connections across requests
async fn query_data(pool: &SqlitePool) -> Result<Vec<Row>> {
    sqlx::query("SELECT * FROM issues")
        .fetch_all(pool)
        .await
}
```

**Prepared statements and batch operations:**
```rust
// Bad - N queries
for issue in issues {
    sqlx::query("INSERT INTO issues (title) VALUES (?)")
        .bind(&issue.title)
        .execute(&pool)
        .await?;
}

// Good - single transaction with prepared statement
let mut tx = pool.begin().await?;
let mut query = sqlx::query("INSERT INTO issues (title) VALUES (?)");
for issue in issues {
    query.bind(&issue.title).execute(&mut *tx).await?;
}
tx.commit().await?;
```

**Indexes for common queries:**
```sql
CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
CREATE INDEX IF NOT EXISTS idx_issues_directive ON issues(directive_id);
CREATE INDEX IF NOT EXISTS idx_events_kind_created ON events(kind, created_at);
```

### Memory Management

**Avoid unnecessary clones:**
```rust
// Bad - unnecessary clone
fn process(data: Vec<u8>) {
    let copy = data.clone();
    worker(copy);
}

// Good - move or borrow
fn process(data: Vec<u8>) {
    worker(data); // move
}

fn process(data: &[u8]) {
    worker(data); // borrow
}
```

**Use Cow for conditional cloning:**
```rust
use std::borrow::Cow;

fn maybe_modify(input: &str, should_modify: bool) -> Cow<str> {
    if should_modify {
        Cow::Owned(input.to_uppercase())
    } else {
        Cow::Borrowed(input)
    }
}
```

**Arc vs Rc - choose based on thread safety needs:**
```rust
use std::sync::Arc;  // For multi-threaded sharing
use std::rc::Rc;     // For single-threaded sharing (cheaper)

// Multi-threaded
let config = Arc::new(Config::load());
tokio::spawn({
    let config = config.clone();
    async move { use_config(config).await }
});

// Single-threaded
let data = Rc::new(expensive_data());
let ref1 = data.clone();  // cheap pointer copy
let ref2 = data.clone();
```

### Parallel Execution

**Use rayon for CPU-bound parallel work:**
```rust
use rayon::prelude::*;

// Process large collections in parallel
let results: Vec<_> = events
    .par_iter()
    .filter(|e| e.kind == 1)
    .map(|e| verify_signature(e))
    .collect();

// Parallel fold for aggregation
let total = values
    .par_iter()
    .map(|v| expensive_computation(v))
    .sum();
```

**Don't parallelize small workloads:**
```rust
// Bad - overhead exceeds benefit
(0..10).into_par_iter().for_each(|i| process(i));

// Good - sequential is faster for small N
for i in 0..10 {
    process(i);
}
```

### Caching Strategies

**Cache relay connections:**
```rust
use std::collections::HashMap;
use tokio::sync::RwLock;

struct RelayPool {
    connections: RwLock<HashMap<String, RelayConnection>>,
}

impl RelayPool {
    async fn get_or_connect(&self, url: &str) -> Result<RelayConnection> {
        // Try read lock first (common case)
        if let Some(conn) = self.connections.read().await.get(url) {
            return Ok(conn.clone());
        }

        // Fall back to write lock for insertion
        let mut conns = self.connections.write().await;
        let conn = RelayConnection::new(url).await?;
        conns.insert(url.to_string(), conn.clone());
        Ok(conn)
    }
}
```

**Cache frequently accessed data with TTL:**
```rust
use moka::future::Cache;
use std::time::Duration;

let cache: Cache<String, Event> = Cache::builder()
    .max_capacity(10_000)
    .time_to_live(Duration::from_secs(300))
    .build();

async fn get_event(&self, id: &str) -> Result<Event> {
    cache.try_get_with(id.to_string(), async {
        fetch_from_relay(id).await
    }).await
}
```

### Profile-Guided Optimization

**Use cargo-flamegraph for CPU profiling:**
```bash
# Install
cargo install flamegraph

# Profile a binary
cargo flamegraph --bin openagents -- autopilot run "task"

# Profile tests
cargo flamegraph --test integration_tests

# Output: flamegraph.svg - open in browser
```

**Use perf for system-level profiling:**
```bash
# Record
perf record -F 99 -g ./target/release/openagents daemon start

# Report
perf report

# Generate flamegraph from perf data
perf script | stackcollapse-perf.pl | flamegraph.pl > perf.svg
```

**Benchmark critical paths:**
```rust
#[cfg(test)]
mod benches {
    use criterion::{black_box, criterion_group, criterion_main, Criterion};

    fn bench_event_verification(c: &mut Criterion) {
        let event = create_test_event();
        c.bench_function("verify_signature", |b| {
            b.iter(|| verify_signature(black_box(&event)))
        });
    }

    criterion_group!(benches, bench_event_verification);
    criterion_main!(benches);
}
```

**Measure allocation pressure:**
```bash
# Use dhat for heap profiling
cargo add --dev dhat

# Add to main.rs
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

# Run with heap profiling
cargo run --features dhat-heap

# Analyze dhat-heap.json output
```

### Build Optimization

**Release builds with LTO:**
```toml
[profile.release]
lto = "fat"           # Full link-time optimization
codegen-units = 1     # Single codegen unit for max optimization
opt-level = 3         # Maximum optimization
strip = true          # Strip symbols for smaller binaries
```

**Incremental compilation for dev:**
```bash
# Faster rebuilds during development
export CARGO_INCREMENTAL=1

# Use sccache for caching across builds
cargo install sccache
export RUSTC_WRAPPER=sccache
```

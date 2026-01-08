# OANIX → Autopilot: Replace Claude Code CLI

## Goal
Replace Claude Code CLI with OANIX/Autopilot CLI for our own workflow. OANIX boots, reads `.openagents/` folder (directives, issues), and starts cranking on autopilot tasks using the compute mix (local/swarm) and RLMs.

---

## Current State

| Component | Status | Notes |
|-----------|--------|-------|
| **OANIX Discovery** | ✅ Working | Hardware, compute, network, identity - boots and displays manifest |
| **OANIX Auto-start** | ✅ Working | Auto-starts Apple FM bridge if not running |
| **`.openagents/` folder** | ✅ Exists | 27 directives, issues.json, pending-issues/, TODO.md |
| **Issue System** | ✅ Built | JSON-based with claim/complete workflow |
| **RLM** | ✅ Built | `pylon rlm <query>` works with fanout, budget, local fallback |
| **FRLM** | ✅ Built | Federated RLM with NIP-90 swarm, quorum policies |
| **Autopilot Agent** | ✅ Functional | 3-phase (plan→execute→review), but not wired to OANIX |
| **Swarm Discovery** | ❌ Stub | `discover_swarm_providers()` returns empty |

---

## The Vision: `autopilot` replaces `claude`

```bash
# Instead of:
claude "implement feature X"

# We run:
autopilot              # Boots OANIX in background, starts Adjutant working
autopilot run          # Same as above
autopilot status       # Show current tasks, compute, progress
autopilot issue list   # List available issues
autopilot issue claim  # Claim next issue and start working
```

**Key naming:**
- `autopilot` = User-facing CLI (the product surface)
- `OANIX` = Background infrastructure (discovery, manifests)
- `Adjutant` = The actual agent that DOES THE WORK (not just routing)

---

## Phase 1: OANIX Reads `.openagents/` (Day 1)

### 1.1 Add workspace discovery to OANIX boot
**File:** `crates/oanix/src/discovery/workspace.rs` (NEW)

```rust
pub struct WorkspaceManifest {
    pub root: PathBuf,
    pub has_openagents: bool,
    pub directives: Vec<DirectiveSummary>,
    pub open_issues: u32,
    pub pending_issues: u32,
    pub active_directive: Option<String>,
}

pub async fn discover_workspace() -> anyhow::Result<WorkspaceManifest> {
    // Find .openagents/ in current dir or parents
    // Parse directives/*.md for active ones
    // Count issues in issues.json
    // Count pending-issues/*.json
}
```

### 1.2 Add to OanixManifest
**File:** `crates/oanix/src/manifest.rs`

```rust
pub struct OanixManifest {
    pub hardware: HardwareManifest,
    pub compute: ComputeManifest,
    pub network: NetworkManifest,
    pub identity: IdentityManifest,
    pub workspace: Option<WorkspaceManifest>,  // NEW
    pub discovered_at: Instant,
}
```

### 1.3 Display workspace in boot output
**File:** `crates/oanix/src/display.rs`

```
Workspace
  [OK] Project: openagents
  Active directive: d-027 (Autopilot Demo + Funnel)
  Issues: 12 open, 3 pending

Recommended: Claim issue #108 (High priority)
```

---

## Phase 2: OANIX Issue Commands (Day 1-2)

### 2.1 Add CLI subcommands
**File:** `crates/oanix/src/bin/main.rs`

```rust
#[derive(Subcommand)]
enum Commands {
    /// Boot and show environment (default)
    Run,
    /// Show current status
    Status,
    /// Issue management
    Issue {
        #[command(subcommand)]
        command: IssueCommand,
    },
}

#[derive(Subcommand)]
enum IssueCommand {
    /// List open issues
    List { #[arg(short, long)] all: bool },
    /// Claim next available issue
    Claim { number: Option<u32> },
    /// Mark current issue complete
    Complete,
    /// Show issue details
    Show { number: u32 },
}
```

### 2.2 Implement issue operations
**File:** `crates/oanix/src/issues.rs` (NEW)

```rust
/// Read issues from .openagents/issues.json
pub fn load_issues(workspace: &Path) -> Vec<Issue>

/// Claim an issue (update JSON)
pub fn claim_issue(workspace: &Path, number: u32, run_id: &str) -> Result<Issue>

/// Complete an issue
pub fn complete_issue(workspace: &Path, number: u32) -> Result<()>

/// Get next available issue (highest priority, not blocked)
pub fn next_issue(workspace: &Path) -> Option<Issue>
```

---

## Phase 3: Adjutant - The Agent That Does The Work (Day 2-3)

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      AUTOPILOT CLI                           │
│  (user-facing: `autopilot run`, `autopilot issue claim`)     │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   OANIX (background)                         │
│  (discovers environment, reads .openagents/, provides manifest)│
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                       ADJUTANT                               │
│  The actual agent that DOES THE WORK                         │
│  - Uses tools directly (Read, Edit, Bash, Glob, Grep)        │
│  - Sometimes delegates to Claude Code for complex stuff      │
│  - Uses RLM for large context analysis                       │
└─────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
    ┌─────────┐         ┌──────────┐         ┌─────────┐
    │  Tools  │         │  Claude  │         │   RLM   │
    │  (self) │         │   Code   │         │ (swarm) │
    └─────────┘         │ (delegate)│         └─────────┘
                        └──────────┘
```

**Adjutant is NOT just a router.** It:
1. **Does work itself** - Uses tools directly (Read, Edit, Bash)
2. **Delegates when needed** - Complex multi-file refactors → Claude Code
3. **Scales out** - Large context analysis → RLM fanout

### 3.1 Adjutant agent definition
**File:** `crates/adjutant/src/lib.rs` (NEW CRATE)

```rust
/// Adjutant: The agent that DOES THE WORK.
/// Named after StarCraft's command & control AI.
///
/// Adjutant is not just a router - it directly uses tools to accomplish tasks.
/// For complex work, it can delegate to Claude Code.
pub struct Adjutant {
    /// Tools available to Adjutant
    tools: ToolRegistry,
    /// OANIX manifest (compute, network, identity)
    manifest: OanixManifest,
    /// Workspace info (.openagents/)
    workspace: WorkspaceManifest,
}

impl Adjutant {
    /// Execute a task - Adjutant does the work itself
    pub async fn execute(&mut self, task: &Task) -> Result<TaskResult> {
        // 1. Understand the task
        let plan = self.plan_task(task).await?;

        // 2. Decide: do it myself or delegate?
        if plan.complexity > Complexity::High || plan.files.len() > 20 {
            // Complex multi-file work → delegate to Claude Code
            return self.delegate_to_claude_code(task).await;
        }

        if plan.context_tokens > 100_000 {
            // Huge context → use RLM
            return self.execute_with_rlm(task, plan).await;
        }

        // 3. Do the work myself using tools
        self.execute_with_tools(task, plan).await
    }

    /// Adjutant does the work itself
    async fn execute_with_tools(&mut self, task: &Task, plan: TaskPlan) -> Result<TaskResult> {
        // Read relevant files
        for file in &plan.files {
            let content = self.tools.read(file).await?;
            self.context.add_file(file, content);
        }

        // Make edits
        for edit in &plan.edits {
            self.tools.edit(&edit.file, &edit.old, &edit.new).await?;
        }

        // Run tests
        let test_result = self.tools.bash("cargo test").await?;

        // Commit if successful
        if test_result.success {
            self.tools.bash(&format!(
                "git add -A && git commit -m '{}'",
                task.title
            )).await?;
        }

        Ok(TaskResult { success: test_result.success, .. })
    }

    /// Delegate complex work to Claude Code
    async fn delegate_to_claude_code(&self, task: &Task) -> Result<TaskResult> {
        // Spawn Claude Code subprocess with task prompt
        let output = self.tools.bash(&format!(
            "claude --print '{}' --allowedTools Read,Edit,Bash,Glob,Grep",
            task.to_prompt()
        )).await?;

        TaskResult::from_claude_output(output)
    }
}
```

### 3.2 Task analysis and strategy selection
**File:** `crates/oanix/src/adjutant/strategy.rs` (NEW)

```rust
pub enum Strategy {
    /// Use Claude Agent SDK with specified tools
    ClaudeAgent {
        tools: Vec<Tool>,  // Read, Edit, Bash, Glob, Grep...
    },
    /// Use a custom Rust agent
    CustomAgent {
        name: String,  // "coder", "reviewer", "planner"
    },
    /// Use RLM for large context tasks
    RLM {
        fanout: u32,
        budget_sats: u64,
    },
}

pub struct TaskRequirements {
    /// Estimated context size (tokens)
    pub context_size: usize,
    /// Tools needed
    pub tools_needed: Vec<ToolType>,
    /// Task category
    pub category: TaskCategory,
}

pub enum TaskCategory {
    /// Simple code edit
    CodeEdit,
    /// Multi-file refactor
    Refactor,
    /// Codebase exploration/analysis
    Exploration,
    /// Test generation
    Testing,
    /// Bug investigation
    Debugging,
}

impl Adjutant {
    fn select_strategy(&self, req: &TaskRequirements) -> Strategy {
        // Large context → RLM
        if req.context_size > 100_000 {
            return Strategy::RLM {
                fanout: 10,
                budget_sats: 1000,
            };
        }

        // Multi-file refactor → Claude Agent with full toolset
        if req.category == TaskCategory::Refactor {
            return Strategy::ClaudeAgent {
                tools: vec![Tool::Read, Tool::Edit, Tool::Glob, Tool::Grep, Tool::Bash],
            };
        }

        // Simple edit → Claude Agent with minimal tools
        Strategy::ClaudeAgent {
            tools: vec![Tool::Read, Tool::Edit],
        }
    }
}
```

### 3.3 Claude Agent SDK execution (fallback)
**File:** `crates/oanix/src/adjutant/claude.rs` (NEW)

```rust
/// Execute task using Claude Agent SDK (Claude Code-style)
pub async fn execute_with_claude(
    &self,
    task: &Task,
    tools: Vec<Tool>,
) -> Result<TaskResult> {
    // Build Claude Agent SDK client
    let client = ClaudeAgentClient::new()
        .with_tools(tools)
        .with_system_prompt(ADJUTANT_SYSTEM_PROMPT);

    // Convert task to conversation
    let messages = vec![
        Message::user(format!(
            "Issue #{}: {}\n\n{}\n\nAcceptance criteria:\n{}",
            task.issue_number,
            task.title,
            task.description,
            task.acceptance_criteria.join("\n- ")
        )),
    ];

    // Execute agent loop
    let result = client.run(messages).await?;

    // Parse result for file changes, commits, etc.
    TaskResult::from_claude_response(result)
}

const ADJUTANT_SYSTEM_PROMPT: &str = r#"
You are Adjutant, an AI coding assistant working on behalf of Autopilot.

Your task is to implement the requested changes following these principles:
1. Read files before editing
2. Make minimal, focused changes
3. Run tests after changes
4. Commit with descriptive messages

Available tools: Read, Edit, Bash, Glob, Grep

Begin by exploring the codebase to understand the context.
"#;
```

### 3.4 Autopilot loop delegating to Adjutant
**File:** `crates/oanix/src/runner.rs` (NEW)

```rust
pub async fn run_autopilot_loop(manifest: &OanixManifest) -> anyhow::Result<()> {
    // Initialize Adjutant with discovered compute
    let adjutant = Adjutant::new(
        manifest.compute.clone(),
        manifest.network.clone(),
    );

    loop {
        // 1. Check for claimed issue or claim next
        let issue = get_or_claim_next_issue(&manifest.workspace)?;
        println!("Autopilot claimed issue #{}: {}", issue.number, issue.title);

        // 2. Convert issue to task
        let task = Task::from_issue(&issue);

        // 3. Delegate to Adjutant
        println!("Adjutant → analyzing task requirements...");
        let result = adjutant.execute(&task).await;

        // 4. Handle result
        match result {
            Ok(r) if r.success => {
                println!("Adjutant → task completed successfully");
                complete_issue(&issue)?;
            }
            Ok(r) => {
                println!("Adjutant → task failed: {}", r.error.unwrap_or_default());
                // Don't complete, leave for retry
            }
            Err(e) => {
                println!("Adjutant → error: {}", e);
            }
        }

        // 5. Continue to next issue
        if no_more_issues() {
            println!("No issues available. Adjutant standing by...");
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }
}
```

---

## Phase 4: Wire OANIX ↔ Autopilot (Day 3)

### 4.1 Use OANIX manifest in Autopilot preflight
**File:** `crates/autopilot/src/preflight.rs`

Replace manual checks with:
```rust
use oanix::boot;

pub async fn preflight_check() -> Result<PreflightResult> {
    let manifest = boot().await?;

    PreflightResult {
        compute_available: !manifest.compute.backends.is_empty(),
        network_connected: manifest.network.has_internet,
        identity_initialized: manifest.identity.initialized,
        workspace_found: manifest.workspace.is_some(),
    }
}
```

### 4.2 Use OANIX swarm discovery
**File:** `crates/autopilot/src/pylon_integration.rs`

```rust
pub fn discover_swarm_providers() -> Vec<SwarmProvider> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let manifest = rt.block_on(oanix::boot()).ok()?;

    manifest.network.pylon_pubkeys
        .iter()
        .map(|pk| SwarmProvider { pubkey: pk.clone(), online: true })
        .collect()
}
```

---

## Phase 5: RLM Integration (Day 4)

### 5.1 Add RLM task type for large contexts
**File:** `crates/oanix/src/tasks.rs`

```rust
pub enum TaskExecution {
    /// Simple prompt → single inference call
    Simple { prompt: String },
    /// Large context → RLM with chunking
    RLM {
        prompt: String,
        files: Vec<PathBuf>,
        fanout: u32,
        budget_sats: u64,
    },
}
```

### 5.2 Auto-detect when to use RLM
```rust
fn should_use_rlm(task: &AutopilotTask) -> bool {
    // If task involves many files or large codebase analysis
    task.files_to_read.len() > 10 || task.estimated_tokens > 100_000
}
```

---

## Key Files to Create/Modify

### New Crate: `crates/adjutant/`
| File | Purpose |
|------|---------|
| `crates/adjutant/Cargo.toml` | Adjutant crate config |
| `crates/adjutant/src/lib.rs` | **Adjutant** - the agent that does the work |
| `crates/adjutant/src/tools.rs` | Tool registry (Read, Edit, Bash, Glob, Grep) |
| `crates/adjutant/src/planner.rs` | Task planning - understand what needs to be done |
| `crates/adjutant/src/executor.rs` | Execute plan using tools |
| `crates/adjutant/src/delegate.rs` | Delegate to Claude Code when needed |
| `crates/adjutant/src/rlm.rs` | RLM fanout for large contexts |

### OANIX Updates (background infrastructure)
| File | Purpose |
|------|---------|
| `crates/oanix/src/discovery/workspace.rs` | Discover .openagents/ folder |
| `crates/oanix/src/manifest.rs` | Add WorkspaceManifest |

### Autopilot Updates (user-facing CLI)
| File | Purpose |
|------|---------|
| `crates/autopilot/src/cli.rs` | Add `autopilot run`, `autopilot issue` commands |
| `crates/autopilot/src/runner.rs` | Main loop: claim issue → Adjutant → complete |
| `crates/autopilot/src/issues.rs` | Issue CRUD using .openagents/issues.json |
| `crates/autopilot/Cargo.toml` | Add adjutant + oanix dependencies |

---

## Timeline

| Day | Focus | Deliverable |
|-----|-------|-------------|
| **Day 1** | Workspace discovery | `autopilot` shows directives/issues on boot |
| **Day 2** | Adjutant crate + tools | Adjutant can read/edit/bash |
| **Day 3** | Adjutant execution loop | `autopilot run` claims and works on issues |
| **Day 4** | Claude Code delegation + RLM | Complex tasks delegate, large tasks use RLM |
| **Day 5** | Polish + dogfood | Use `autopilot run` for our own work |

---

## Success Criteria

1. **Autopilot boot shows workspace:**
   ```
   $ autopilot
   OANIX v0.1.0 - OpenAgents NIX
   ═══════════════════════════════════════

   Hardware: Apple M2 Pro (12 cores), 16 GB RAM
   Compute: Ollama (5 models), Apple FM
   Network: 3 relays, 188 providers (7 Pylons, 1 online)
   Identity: npub1qcm2vnpm...

   Workspace
     [OK] Project: openagents
     Active directive: d-027 (Autopilot Demo + Funnel)
     Issues: 12 open, 3 pending

   Adjutant standing by. Run `autopilot run` to start working.
   ```

2. **Can claim and work issues:**
   ```bash
   autopilot issue claim 108
   # → Adjutant claims issue, plans work, executes, commits
   ```

3. **Autonomous loop:**
   ```bash
   autopilot run
   # → Adjutant claims issue → does the work → completes → claims next...
   ```

4. **Adjutant does the work itself:**
   - Simple edits → Adjutant uses tools directly
   - Complex refactors → Delegates to Claude Code
   - Large context → RLM with swarm fanout

---

## References

### Existing Issue System
- `.openagents/issues.json` - Issue database
- `.openagents/pending-issues/` - Pending issue files
- `.openagents/directives/` - 27 directive files (d-001.md to d-027.md)

### RLM System
- `crates/rlm/` - RLM engine with chunking, orchestration
- `crates/frlm/` - Federated RLM with NIP-90 swarm
- `pylon rlm <query>` - CLI interface

### Autopilot
- `crates/autopilot/src/agent.rs` - 3-phase state machine
- `crates/autopilot/src/pylon_integration.rs` - Compute detection (to replace)

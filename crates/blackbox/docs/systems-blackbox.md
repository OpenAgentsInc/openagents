# BlackBox System

Flight recorder for AI agent sessions. Records everything an agent does for replay, debugging, auditing, and training.

---

## Why "BlackBox"?

Aircraft have flight data recorders (colloquially "black boxes") that capture everything during a flight. If something goes wrong, investigators reconstruct what happened by replaying the recording.

AI agents need the same thing:

| Aviation | AI Agents |
|----------|-----------|
| Flight data recorder | BlackBox log |
| Cockpit voice recorder | User/agent messages |
| Sensor readings | Tool calls and results |
| Control inputs | Commands, modes, plans |
| Post-crash analysis | Session debugging |

When an agent fails, hallucinates, or produces unexpected results, you need to reconstruct exactly what happened. BlackBox provides that.

---

## Core Problems Solved

### 1. Debugging Agent Failures

Without BlackBox:
```
Agent: "I fixed the bug"
You: "It's still broken"
Agent: *no idea what it actually did*
```

With BlackBox:
```
t:read src/auth.rs step=12 → [186 lines]
t:edit src/auth.rs old="check_token" → [patched wrong function]
# ^ found it: edited check_token instead of validate_token
```

### 2. Reproducibility

Agent sessions are non-deterministic. Same prompt, different results. BlackBox captures the exact sequence so you can:
- Replay sessions in a simulator
- Generate training data from good sessions
- Create regression tests from failures

### 3. Cost Visibility

```
# metrics step=47 prompt_tokens=12400 completion_tokens=890 cached_tokens=8000 cost_usd=0.0234
# budget: $50.00 remaining=$38.17 tokens=284000
```

Know exactly where tokens went. Find expensive operations. Optimize.

### 4. Auditing and Trust

When agents run autonomously for hours:
- What files did it modify?
- What external APIs did it call?
- Did it access anything sensitive?
- What decisions did it make and why?

BlackBox provides a complete audit trail.

### 5. Handoff Between Sessions

When resuming work:
```
r: "billing feature" → [2 matches]
  sess_20250617: PR #203 created, pending review
  sess_20250615: planned implementation, not started
```

Agents recall what happened. Humans can read the logs.

---

## Design Principles

### 1. Line-Based

Every event is one line. No multi-line JSON blobs. This means:
- Stream in real-time (append-only)
- Grep through logs easily
- Git diffs are readable
- Parse with simple regex

### 2. Human-Readable

```
t:read src/main.rs → [200 lines]
a: Found the entry point. Main function starts server on port 8000.
t:grep "TODO" type=rs → [7 matches]
```

You can read this without tooling. No schema lookup needed.

### 3. Machine-Parseable

Structured enough for programmatic analysis:
```
^t:(\w+)(?: id=(\w+))?(?: step=(\d+))? (.+?)→(.+)$
```

The CLI validates, extracts stats, and exports to JSON.

### 4. Streamable

Write as you go. No buffering required. Session can crash and you still have everything up to that point.

### 5. Git-Friendly

Text format, one event per line. Diffs show exactly what changed:
```diff
+ t:edit src/auth.rs old="validate" → [ok]
+ t:test → [ok] 12 tests passed
```

### 6. Extensible

New line types, new metadata fields, new primitives. Format grows without breaking existing parsers (unknown prefixes become comments).

---

## Format Overview

### File Structure

```
---
[YAML header with session metadata]
---

[Line-based body with events]
```

### Header

```yaml
---
format: bbox/1
id: sess_20250618_001
mode: chat
model: sonnet-4
repo: OpenAgentsInc/platform
repo_sha: 215db51
branch: main
runner: daytona
sandbox_id: dtn_abc123
skills: [code-review]
mcp: [github, linear]
budget: $50
duration: 12h
---
```

### Line Types

| Prefix | Type | Example |
|--------|------|---------|
| `u:` | User message | `u: Fix the login bug` |
| `a:` | Agent message | `a: I'll investigate the auth module.` |
| `t:` | Tool call | `t:read src/auth.rs → [186 lines]` |
| `o:` | Observation | `o: id=call_1 → [ok] Build complete` |
| `c:` | MCP call | `c:github.issues state=open → [8 issues]` |
| `x:` | Subagent | `x:explore "find tests" → [3 files]` |
| `r:` | Memory recall | `r: "auth bug" → [2 matches]` |
| `s:` | Skill | `s:deploy activate → [loaded]` |
| `p:` | Plan | `p:create "Fix auth" → [4 steps]` |
| `m:` | Mode change | `m: auto` |
| `@` | Lifecycle | `@checkpoint hour=4 cost=$12.30` |
| `#` | Comment/meta | `# t=00:15:30` |

### Structured Metadata

Lines carry optional structured data:
```
t:read id=call_7 src/main.rs step=23 ts=2025-12-18T03:21:11Z → [200 lines]
```

- `id=` — Call ID for correlation
- `step=` — Sequential step number
- `ts=` — ISO 8601 timestamp
- `→` — Separates call from result

### Results

| Shorthand | Meaning |
|-----------|---------|
| `[ok]` | Success |
| `[N files]` | N files found |
| `[N lines]` | N lines of content |
| `[err: msg]` | Error |
| `@blob sha256=...` | Large output stored externally |

---

## The `blackbox` CLI

Rust CLI for validating, analyzing, and fixing BlackBox files.

### Installation

```bash
# From platform repo
cargo install --path crates/blackbox

# Or run directly
cargo run -p blackbox -- <command>
```

### Commands

#### `validate` — Check file validity

```bash
blackbox validate session.bbox
```

Output:
```
✓ session.bbox

  Statistics:
    Lines:        692
    User msgs:    3
    Agent msgs:   14
    Tool calls:   54
    Observations: 11
    Subagents:    2
    MCP calls:    21
    Call IDs:     77
    Max step:     121
    Timestamps:   yes
    Blobs:        4
    Redacted:     1

  ✓ Valid BlackBox file
```

Checks:
- Required header fields (`format`, `id`, `repo_sha`)
- Format version (`bbox/1`)
- Call ID correlation (every `o:` has matching `t:`)
- Step ordering (sequential, no gaps)
- Blob sha256 validity (hex format)
- Timestamp format (ISO 8601)

Options:
```bash
--verbose    # Show statistics
--json       # Output as JSON (for CI)
```

#### `stats` — Detailed analysis

```bash
blackbox stats session.bbox
```

Output:
```
File: session.bbox

Header:
  format: bbox/1
  id: sess_12h
  mode: auto
  model: sonnet-4
  repo: OpenAgentsInc/platform

Statistics:
  Total lines:      692
  Content lines:    576
  Blank lines:      102
  Comment lines:    14

  User messages:    3
  Agent messages:   14
  Tool calls:       54
  Observations:     11
  MCP calls:        21
  Subagents:        2
  Skills:           1
  Plans:            4
  Mode changes:     2
  Lifecycle:        8
  Memory recalls:   4

Metadata:
  Call IDs:         77 unique
  Step range:       1-121
  Timestamps:       yes (121 with ts=)
  Blob references:  4
  Redacted values:  1

Tools used:
  read:    18 calls
  grep:    12 calls
  edit:     8 calls
  git:      6 calls
  test:     4 calls
  shell:    3 calls
  glob:     3 calls
```

#### `parse` — Dump structure

```bash
blackbox parse session.bbox
```

Shows every line with its parsed type:
```
Line 1: Header start
Line 2: HeaderField(format = bbox/1)
Line 3: HeaderField(id = sess_12h)
...
Line 41: Lifecycle(@start id=sess_12h budget=$50 duration=12h)
Line 43: Agent(Starting 12-hour autonomous session...)
Line 44: Comment(metrics step=2 prompt_tokens=1200...)
```

Options:
```bash
--json       # Output as JSON array
--header     # Header only
--body       # Body only
```

#### `fix` — Auto-repair issues

```bash
# Preview changes (dry-run)
blackbox fix session.bbox --renumber-steps

# Apply changes
blackbox fix session.bbox --renumber-steps --write

# Write to new file
blackbox fix session.bbox --renumber-steps --output fixed.bbox
```

Current fixers:
- `--renumber-steps` — Renumber all `step=N` references sequentially

Example output:
```
Fixing session.bbox

  ✓ Renumbered 119 step references

  Total changes: 119

  Line 44:
    - # metrics step=2 prompt_tokens=1200...
    + # metrics step=3 prompt_tokens=1200...

  Line 51:
    - r: "platform" "priorities" step=3...
    + r: "platform" "priorities" step=4...

  ... and more changes

  ! Dry run - use --write to apply changes
```

---

## Session Lifecycle

For long-running autonomous sessions, use lifecycle primitives:

```
# t=00:00:00
@start id=sess_001 budget=$50 duration=12h step=1

# ... work happens ...

# t=04:00:00
@checkpoint hour=4 tokens=45000 cost=$12.30

# ... more work ...

# t=08:00:00
@pause reason="waiting for CI"

# t=08:45:00
@resume

# ... final work ...

# t=12:00:00
@end summary="8 issues closed, 4 PRs merged" prs=[201,202,203,204]
```

### Primitives

| Primitive | Purpose |
|-----------|---------|
| `@start` | Begin session with budget/duration |
| `@checkpoint` | Progress marker (recommended every 4h) |
| `@pause` | Halt for external event |
| `@resume` | Continue after pause |
| `@end` | Complete session with summary |
| `@assess` | Priority assessment |
| `@notify` | Alert human (non-blocking) |
| `@wait` | Block for approval |
| `@escalate` | Request human help |
| `@batch` | Group related issues |
| `@phase` | Plan mode phase transition |
| `@compact` | Context compaction |

---

## Plan Mode (Claude Code Style)

Plan mode is a structured 5-phase workflow for complex tasks, based on how Claude Code handles planning.

### Modes

```
m:chat      # Conversational mode
m:auto      # Autonomous execution
m:plan      # Plan mode (read-only except plan file)
```

### Phases

```
m:plan
@phase explore    # Phase 1: Launch Explore agents, understand codebase
@phase design     # Phase 2: Launch Plan agents, design approach
@phase review     # Phase 3: Read critical files, ensure alignment
@phase final      # Phase 4: Write final plan to plan file
@phase exit       # Phase 5: Exit plan mode, optionally launch swarm
m:auto
```

### Questions and Clarifications

```
# Ask question with options
q: id=q_1 "Which database?" options=["Postgres", "SQLite"] → [pending]
q: id=q_1 → [selected: Postgres]

# In AFK mode, agent decides autonomously
m:auto afk=true
q: id=q_2 "Which approach?" → [auto: OAuth, reason="existing infra"]
```

### Context Management

Multiple strategies for managing context as sessions grow:

**Progressive Condensation** (preferred)
```
# condense range=50-100 detail=summary
# condense range=1-49 detail=outline
```

**Anchored Forgetting**
```
# anchor step=75 reason="key decision"
# anchor step=92 reason="user feedback"
```

**Memory + Retrieval**
```
r: "auth decisions" → [3 matches from steps 45, 67, 89]
```

**Hard Compaction** (Claude Code style, disruptive)
```
@compact reason="context limit" tokens_before=180000
```

BlackBox captures what happened—context management strategy is an implementation choice.

---

## Concurrency

When agents spawn subagents or run parallel tool calls:

```
# Main agent (tid=1 implicit)
a: Spawning analysis subagent.

# Subagent starts
x:explore tid=2 span=x17 "find auth failures" → [started]

# Subagent's tool calls
t:grep tid=2 span=t91 "AuthError" → [12 matches]
t:read tid=2 span=t92 src/auth.rs → [186 lines]

# Subagent completes
x:explore tid=2 span=x17 → summary="Found 3 patterns"

# Main continues
a: Subagent found 3 auth error patterns.
```

Fields:
- `tid=` — Thread/agent ID
- `span=` — Operation ID (for tracing)
- `parent=` — Parent span (for nesting)

---

## Streaming Operations

Long-running tools use start/progress/complete markers:

```
t!:test cargo test → [running]
t~:test "[12/128 passed]"
t~:test "[64/128 passed]"
t:test latency_ms=34200 → [ok] 128 tests passed
```

| Marker | Meaning |
|--------|---------|
| `t!:` | Tool started |
| `t~:` | Progress update |
| `t:` | Tool completed |

---

## Blob Storage

Large outputs stored externally, referenced by hash:

```
t:read src/huge_file.rs → @blob sha256=ab12cd34 bytes=128000 mime=text/x-rust
```

Blobs live in `.bbox-blobs/` directory alongside the `.bbox` file, or in object storage for production.

Threshold: Inline ≤1KB, blob >1KB (configurable).

---

## Secret Redaction

Tools must never emit raw secrets:

```
# In tool output
t:read .env → API_KEY=[redacted:env_var]

# In shell commands
t:shell export DAYTONA_API_KEY=[redacted:api_key] → [ok]

# In agent messages
a: Configured auth with [redacted:github_token]
```

The `blackbox validate` command counts redacted values.

---

## Cost Tracking

Per-step metrics:
```
a: Here's what I found.
# metrics step=3 prompt_tokens=520 completion_tokens=80 cached_tokens=200 cost_usd=0.00045
```

Session budget:
```
# budget: $50.00 remaining=$38.17 tokens=284000
```

End-of-session summary:
```
# tokens=487000 cost=$42.17 duration=12h
```

---

## ATIF Interoperability

BlackBox converts losslessly to [ATIF (Agent Trajectory Interchange Format)](../rfcs/0001-trajectory-format.md) for:
- Training data (SFT/RL)
- Benchmarking
- Cross-platform interchange

### Mapping

| BlackBox | ATIF |
|----------|------|
| `u:` | `StepObject(source="user")` |
| `a:` | `StepObject(source="agent")` |
| `t: id=call_1` | `tool_calls[{tool_call_id}]` |
| `o: id=call_1` | `observation.results[{source_call_id}]` |
| `# metrics step=N` | `StepObject.metrics` |
| `@blob` | `extra.blobs[...]` |

### When to Use Which

| Use Case | Format |
|----------|--------|
| Debugging | BlackBox |
| Human review | BlackBox |
| Training data | ATIF |
| Benchmarks | ATIF |
| Version control | BlackBox |
| Long sessions | BlackBox |

---

## Integration Points

### Writing BlackBox Logs

```rust
use std::io::Write;

fn log_tool_call(
    writer: &mut impl Write,
    tool: &str,
    call_id: &str,
    args: &str,
    step: u32,
    result: &str,
) -> std::io::Result<()> {
    let ts = chrono::Utc::now().to_rfc3339();
    writeln!(
        writer,
        "t:{} id={} {} step={} ts={} → {}",
        tool, call_id, args, step, ts, result
    )
}
```

### Parsing BlackBox Logs

```rust
use blackbox::{Parser, LineType};

let parser = Parser::new();
let content = std::fs::read_to_string("session.bbox")?;
let doc = parser.parse(&content)?;

for line in &doc.lines {
    match &line.line_type {
        LineType::Tool { name, call_id, result, .. } => {
            println!("Tool: {} ({})", name, call_id.as_deref().unwrap_or("-"));
        }
        LineType::Agent(text) => {
            println!("Agent: {}", text);
        }
        _ => {}
    }
}
```

### Validating in CI

```yaml
# .github/workflows/validate-bbox.yml
- name: Validate BlackBox examples
  run: |
    cargo run -p blackbox -- validate docs/examples/*.bbox --json
```

---

## File Locations

| Path | Purpose |
|------|---------|
| `docs/decisions/blackbox.md` | Format specification |
| `docs/examples/*.bbox` | Example sessions |
| `crates/blackbox/` | CLI tool |
| `.bbox-blobs/` | Blob storage (gitignored) |

---

## Example Session

See `docs/examples/12h-autopilot-session.bbox` for a complete 12-hour autonomous session demonstrating all primitives.

Quick excerpt:
```
---
format: bbox/1
id: sess_12h
mode: auto
model: sonnet-4
repo: OpenAgentsInc/platform
repo_sha: 215db51
budget: $50
duration: 12h
---

# t=00:00:00
@start id=sess_12h budget=$50 duration=12h step=1

a: Starting 12-hour autonomous session. step=2 ts=2025-12-18T00:00:02Z
# metrics step=3 prompt_tokens=1200 completion_tokens=45 cached_tokens=800

r: "platform" "priorities" step=4 → [2 matches]
  sess_20250617: Phase 1 roadmap
  sess_20250615: issue system design

t:read id=call_1 docs/decisions/userstory.md step=8 → [19 lines]
t:read id=call_2 docs/decisions/agent-algorithms.md step=9 → @blob sha256=a1b2c3d4 bytes=12847

c:github.issues id=call_4 state=open step=13 → [8 issues]

x:explore id=sub_1 "architectural components" step=24 → session_id=sess_sub_1 summary="6 crates"

# t=04:00:00
@checkpoint hour=4 tokens=145000 cost=$12.30

# ... continues for 12 hours ...

@end summary="8 issues closed, 4 PRs merged" prs=[201,202,203,204] step=121
# tokens=487000 cost=$42.17 duration=12h
```

---

## Future Work

1. **Blob storage backends** — S3, R2, local filesystem abstraction
2. **ATIF export** — `blackbox export --format atif session.bbox`
3. **Replay simulator** — Step through sessions for debugging
4. **Training data extraction** — Export good sessions for SFT
5. **Real-time streaming** — WebSocket feed for live dashboards
6. **Compression** — Zstd for long sessions
7. **Signing** — Cryptographic verification for audit trails

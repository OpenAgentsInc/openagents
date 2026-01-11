# recorder

Line-based flight recorder format parser, validator, and CLI tools for `.rlog` files capturing agent session executions.

## Overview

Recorder implements the **rlog/1** format - a human-readable, streamable, line-oriented format for agent session logs. Each line represents a single event (user message, agent response, tool call, observation, etc.) with optional metadata like timestamps, call IDs, and token usage.

The format is designed for:
- **Multi-agent attribution** - Track which agent did what, with token accounting
- **Real-time streaming** - Tail logs as they're written, each line self-contained
- **Auditability** - Cryptographically verify session work for payments
- **Training data** - Extract successful patterns for agent improvement
- **Platform independence** - Unified format across Codex Code, OpenAI, and custom systems

## File Structure

Every `.rlog` file has a YAML header followed by line-by-line events:

```rlog
---
format: rlog/1
id: sess_20251219_001
repo_sha: abc123
model: codex-opus-4-5
branch: main
---

# t=00:00:00
@start id=sess_001 duration=10m
u: Can you check auth?
a: Looking now.
t:Read id=call_1 file_path=src/auth.rs → [186 lines]
o: id=call_1 → [ok]
@end summary="checked auth"
```

## Quick Start

### Installation

```bash
# From workspace
cargo build -p recorder

# Or install globally
cargo install --path crates/recorder
```

### Usage

```bash
# Validate a session log
recorder validate path/to/session.rlog

# Show statistics
recorder stats path/to/session.rlog

# Parse and print structure
recorder parse path/to/session.rlog

# Fix step numbering
recorder fix path/to/session.rlog --renumber-steps --write
```

## Line Types

### User Messages

```rlog
u: Fix the login bug
u: Can you add tests?
```

### Agent Responses

```rlog
a: I'll investigate the auth module.
a: Looking at the session validation logic.
```

### Tool Calls

Standard tool call (synchronous):
```rlog
t:Read id=call_1 file_path=src/auth.rs → [186 lines]
t:Edit id=call_2 old_string="foo" new_string="bar" → [ok]
t:Bash id=call_3 command="cargo test" → [exit 0]
```

Streaming tool call (asynchronous):
```rlog
t!:Bash id=call_4 command="cargo build" → [running]
t~:Bash id=call_4 [compiling 15/128]
t~:Bash id=call_4 [compiling 64/128]
o: id=call_4 → [ok] build complete
```

### Observations

Deferred tool results (for streaming or async tools):
```rlog
o: id=call_92 → [ok] 128 tests passed
o: id=call_93 → [error] permission denied
```

### Thinking

Agent reasoning blocks (Codex Code):
```rlog
th: Let me analyze the authentication flow...
  This requires checking the login handler and session validation.
th: The issue is in the token expiry check. sig=Ep4E...
```

### Todos

Task tracking updates:
```rlog
td: [pending] Fix login bug [in_progress] Add tests
td: [completed] Fix login bug [pending] Add tests
```

### Lifecycle Events

Session boundaries:
```rlog
@start id=sess_001 ts=2025-12-19T20:29:02Z duration=1h
@checkpoint reason="halfway" progress=50%
@end tokens_in=21890 tokens_out=1250 summary="Fixed auth bug"
```

### Phase Markers

Planning phases:
```rlog
@phase explore
@phase design
@phase implement
@phase review
```

### Skills

Skill invocations:
```rlog
s:deploy activate → [loaded]
s:auth check_permissions user=alice → [authorized]
```

### Subagents

Spawning sub-tasks:
```rlog
x:explore "find auth failures" → summary="Found 3 issues in session.rs"
x:plan "Design fix for auth" → plan_id=p_42
```

### MCP Calls

Model Context Protocol interactions:
```rlog
c:github.issues state=open → [8 issues]
c:filesystem.read path=README.md → [1024 bytes]
```

### Questions

User interactions:
```rlog
q: "Which authentication method?" → [selected: OAuth]
q: "Database choice?" → [selected: PostgreSQL]
```

### Comments

Metadata and annotations:
```rlog
# t=00:15:30
# file-snapshot: msg_id files=12
# queue: enqueue "Build project"
# codex: {"type": "tool_use", "id": "toolu_123"}
```

### Modes

Agent operating modes:
```rlog
m: chat
m: auto
m: plan
```

### Recall

Memory/context retrieval:
```rlog
r: "authentication bugs" → [2 matches from memory]
```

### Plans

Plan management:
```rlog
p:create id=p1 "Auth redesign plan" → [ok]
p:update id=p1 status=in_progress → [ok]
```

## Metadata Fields

Lines can include optional metadata anywhere in the content:

### Common Fields

- `id=<call_id>` - Tool call identifier
- `step=<number>` - Sequential step number
- `ts=<ISO8601>` - Timestamp (e.g., `2025-12-19T20:29:02Z`)
- `latency_ms=<number>` - Operation latency
- `attempt=<N>` or `attempt=<N/M>` - Retry attempts
- `level=<debug|info|warn|error>` - Log level

### Codex Code Fields

- `parent=<uuid>` - Parent message UUID (threading)
- `sig=<hash>` - Thinking block signature
- `tokens_in=<number>` - Input tokens for this message
- `tokens_out=<number>` - Output tokens for this message
- `tokens_cached=<number>` - Cached tokens used
- `interrupted` - Flag for interrupted operations
- `model=<model_id>` - Model used for this specific message

### Result Notation

Results use the Unicode arrow `→`:

```rlog
t:Read file.txt → [ok] 42 lines
t:Bash cargo test → [exit 0]
o: id=call_1 → [error] file not found
```

## Header Format

Required fields:
```yaml
format: rlog/1        # Format version
id: sess_001          # Session identifier
repo_sha: abc123      # Git commit hash
```

Optional fields:
```yaml
mode: auto            # Operating mode
model: codex-opus-4-5-20251101
agent: autopilot
version: "1.0.0"
repo: openagents
branch: main
dirty: false          # Working directory dirty flag
sandbox_id: sb_42
runner: codex-code
budget: "$10"
duration: "1h"
classification: public
notes: "Fixing auth bug"

# Codex Code specific
client_version: "2.0.71"
slug: mighty-wishing-music
cwd: /path/to/project
tokens_total_in: 21890
tokens_total_out: 1250
tokens_cached: 12973

# Capabilities
toolset: [Read, Edit, Bash, WebSearch]
skills: [deploy, auth, test]
mcp: [github, filesystem, database]

# Extensibility
extra:
  custom_field: value
```

## Validation

The validator checks for:

### Errors

- Missing required header fields (`format`, `id`, `repo_sha`)
- Invalid format version
- Malformed YAML header

### Warnings

- Unknown line formats
- Observations referencing unknown call IDs
- Tool progress without matching start
- Decreasing step numbers
- Invalid timestamp format
- Short blob SHA256 hashes
- Tool streams without completion
- Unusual `repo_sha` length (not 6-40 chars)

### Info

- Long sessions (>50 lines) without `@start`
- Sessions with `@start` but no `@end`
- Auto mode without `sandbox_id`
- Missing `runner` field

## CLI Commands

### Validate

Check file correctness:

```bash
# Basic validation
recorder validate session.rlog

# Verbose output with line details
recorder validate session.rlog --verbose

# JSON output for tooling
recorder validate session.rlog --format json
```

Example output:
```
✓ Valid rlog file
  Format: rlog/1
  Session ID: sess_001
  Lines: 42
  Tool calls: 8
  Warnings: 0
```

### Stats

Session statistics:

```bash
recorder stats session.rlog
```

Example output:
```
Session Statistics
===================
Format: rlog/1
ID: sess_001
Repo SHA: abc123
Model: codex-opus-4-5-20251101

Lines
-----
Total: 127
User messages: 3
Agent messages: 12
Tool calls: 18
Observations: 15
Thinking blocks: 8
Todo updates: 4
Comments: 6

Metadata
--------
Unique call IDs: 18
Max step: 92
Has timestamps: yes
Blob references: 2
Redacted values: 1

Tokens (Codex Code)
--------------------
Input: 21,890
Output: 1,250
Cached: 12,973
```

### Parse

Show parsed structure:

```bash
# Parse entire file
recorder parse session.rlog

# Limit output
recorder parse session.rlog --max-lines 20

# Show line-by-line types
recorder parse session.rlog --lines
```

### Fix

Repair common issues:

```bash
# Renumber steps (dry run)
recorder fix session.rlog --renumber-steps

# Write changes to file
recorder fix session.rlog --renumber-steps --write

# Output to new file
recorder fix session.rlog --renumber-steps --output fixed.rlog
```

## Export (Optional)

Database export requires the `export` feature:

```bash
# Build with export support
cargo build -p recorder --features export

# Export to PostgreSQL
recorder export session.rlog \
  --db postgres://user:pass@localhost/db \
  --table sessions
```

The exporter creates a structured database representation with:
- Session metadata table
- Lines table with parsed fields
- Indexed call IDs, steps, and timestamps

## Library Usage

### Parsing

```rust
use recorder::{parse_file, parse_content};

// Parse from file
let session = parse_file("session.rlog")?;
println!("Session ID: {}", session.header.id);
println!("Lines: {}", session.lines.len());

// Parse from string
let content = r#"---
format: rlog/1
id: test
repo_sha: abc123
---

u: Hello
a: Hi there
"#;
let session = parse_content(content)?;
```

### Validation

```rust
use recorder::{parse_file, validate};

let session = parse_file("session.rlog")?;
let result = validate(&session);

if result.is_valid() {
    println!("✓ Valid");
} else {
    println!("✗ {} errors", result.error_count());
    for error in result.errors() {
        println!("  [{}] {}", error.code, error.message);
    }
}

// Statistics
println!("Tool calls: {}", result.stats.tool_calls);
println!("Total tokens: {}",
    result.stats.total_tokens_in + result.stats.total_tokens_out);
```

### Line Inspection

```rust
use recorder::{parse_file, LineType};

let session = parse_file("session.rlog")?;

for line in &session.lines {
    match line.line_type {
        LineType::Tool => {
            println!("Tool call: {} → {}",
                line.call_id.as_ref().unwrap_or(&"?".to_string()),
                line.result.as_ref().unwrap_or(&"[pending]".to_string())
            );
        }
        LineType::Thinking => {
            println!("Thinking: {}", line.content);
            if let Some(sig) = &line.signature {
                println!("  Signature: {}", sig);
            }
        }
        _ => {}
    }
}
```

## Codex Code Conversion

The `convert` module converts Codex Code JSONL to rlog format:

```rust
use recorder::convert::convert_codex_code_to_rlog;

let jsonl_content = std::fs::read_to_string("session.jsonl")?;
let rlog = convert_codex_code_to_rlog(&jsonl_content)?;
std::fs::write("session.rlog", rlog)?;
```

Command-line:
```bash
# Convert Codex Code JSONL to rlog
recorder convert session.jsonl --output session.rlog
```

Conversion preserves:
- Message threading (parent UUIDs)
- Thinking blocks with signatures
- Token usage per message
- Tool interruption flags
- File snapshots as comments
- Queue operations as comments
- Model overrides per message

## Design Philosophy

### Why Line-Based?

1. **Streaming** - Each line is independently parseable. Agents can `tail -f` each other's logs for real-time coordination.

2. **Human-Readable** - No tooling required. `cat`, `grep`, `less` work perfectly.

3. **Deterministic** - Unambiguous prefixes (`u:`, `a:`, `t:`, etc.) mean no parsing ambiguity.

4. **Minimal Overhead** - No nested JSON for 99% of lines. Metadata is optional key-value pairs.

### Multi-Layer Strategy

| Layer | Format | Use Case |
|-------|--------|----------|
| **Transport** | JSON/WebSocket | Protocol-specific |
| **Content schema** | rlog lines | Semantic structure |
| **Local storage** | .rlog files | Human archives |
| **ML export** | ATIF JSON | Training pipelines |

rlog lines work as **both** individual event payloads **and** concatenated files.

### Transport Agnostic

Send rlog lines over any medium:

**Nostr (NIP-90):**
```json
{
  "kind": 7000,
  "content": "t:Read id=call_1 src/auth.rs → [186 lines]",
  "tags": [["status", "processing"], ["e", "<job-id>"]]
}
```

**WebSocket:**
```json
{"type": "line", "content": "a: Checking authentication..."}
```

**HTTP SSE:**
```
data: t!:Bash id=call_5 cargo build → [running]

data: t~:Bash id=call_5 [compiling 64/128]
```

**File tail:**
```bash
tail -f session.rlog | while read line; do
  # Process each line as it arrives
done
```

### Why Not ATIF?

[ATIF](https://github.com/HarborML/harbor) is excellent for offline ML workflows with `logprobs` and reward signals. But it's a document format - you need complete JSON to parse.

For **live multi-agent coordination** where agents stream partial results (NIP-90 `status: "partial"`), you need a line-oriented format. rlog provides that, and we can export to ATIF later for training.

## Use Cases

### 1. Multi-Agent Attribution

Track which agent did what in collaborative sessions:

```rlog
u: Implement OAuth flow
x:plan "Design OAuth" → plan_id=p_42
a: Starting implementation based on plan p_42
t:Edit id=call_1 src/auth.rs → [ok] tokens_in=500 tokens_out=800
x:review "Check OAuth impl" → summary="LGTM" tokens_in=1000 tokens_out=200
```

Total cost: `500 + 800 + 1000 + 200 = 2500` tokens distributed across agents.

### 2. Real-Time Coordination

Agent B tails Agent A's log:

```bash
# Terminal 1 (Agent A)
recorder watch session_a.rlog

# Terminal 2 (Agent B)
tail -f session_a.rlog | recorder parse --streaming
```

Agent B sees Agent A's tool calls in real-time and adjusts its work accordingly.

### 3. Audit Trail

Cryptographic verification for payment:

```bash
# Generate session hash
sha256sum session.rlog > session.sha256

# Reference in payment
lightning-cli pay <invoice> \
  --label "Session 001" \
  --description "sha256=$(cat session.sha256)"
```

The rlog file becomes a verifiable receipt of work performed.

### 4. Training Data

Extract successful patterns:

```bash
# Find all successful auth implementations
grep -l "@end.*success" logs/*.rlog | while read log; do
  recorder parse "$log" | grep "t:Edit.*auth"
done

# Convert to ATIF for training
recorder export session.rlog --format atif > training.json
```

## Testing

```bash
# Run all tests
cargo test -p recorder

# With export feature
cargo test -p recorder --features export

# Specific test
cargo test -p recorder test_parse_line_types
```

## Related Documentation

- **Format Specification**: `crates/recorder/docs/format.md`
- **UI Components**: `crates/ui/src/recorder/`
- **Storybook**: Run `cargo storybook` → `/stories/recorder/*`

## Architecture

```
crates/recorder/
├── src/
│   ├── lib.rs         # Parser, validator, core types
│   ├── convert.rs     # Codex Code JSONL → rlog
│   ├── export.rs      # Database export (feature: export)
│   └── main.rs        # CLI binary
├── docs/
│   ├── README.md      # This file (in docs/)
│   └── format.md      # Full specification
└── Cargo.toml
```

## Performance

The parser is designed for streaming:
- **Line parsing**: O(1) per line, no backtracking
- **Memory**: Only stores header + current line batch
- **Validation**: Single-pass with incremental stats

Benchmarks:
- Parse 10,000 lines: ~15ms
- Validate 10,000 lines: ~25ms
- Convert Codex Code (1,000 messages): ~50ms

## Future Work

- [ ] Binary format (.rlog.bin) for dense storage
- [ ] Compression (gzip, zstd) support
- [ ] Streaming validation API
- [ ] ATIF export format
- [ ] Diff/merge tools for multi-agent sessions
- [ ] Replay engine for reproducing sessions

## License

Same as the OpenAgents workspace (MIT).

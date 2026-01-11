# Recorder Format Specification (rlog/1)

Line-based format for logging agent sessions. Each line is a single event. A required YAML header defines session metadata.

**Design priorities**: streamability (each line parses independently), human readability (no tooling required), deterministic parsing (unambiguous prefixes), minimal overhead (no nested JSON for common cases).

## File Layout

```
---
format: rlog/1
id: sess_20251219_001
repo_sha: abc123
---

u: User message
u: Another line
```

## Dual-Format System (rlog + JSONL)

Autopilot uses a **dual-format logging system** to balance human readability with complete data capture:

| File | Purpose | Content |
|------|---------|---------|
| `.rlog` | Human-readable summary | Truncated content (200 chars messages, 150 thinking, 100 tool output) |
| `.jsonl` | Full data capture | Untruncated, Codex Code SDK compatible |

**File naming convention:**
```
docs/logs/20251222/
  123456-task-name.rlog           # Human review
  123456-task-name.jsonl          # Full data (APM, replay, NIP-SA)
  123456-task-name.sub-abc123.jsonl  # Subagent session
```

### Why Dual Format?

- **rlog truncates intentionally** - Large tool outputs (10KB+) make logs unreadable
- **JSONL preserves everything** - Required for APM calculations, trajectory replay, NIP-SA publishing
- **Same session ID** - Both files share the session identifier for correlation

### JSONL Companion Format

The JSONL companion file uses Codex Code SDK format (same as `~/.codex/projects/*.jsonl`):

```jsonl
{"type":"user","message":{"content":"Fix the login bug"},"timestamp":"2025-12-22T10:00:00Z"}
{"type":"assistant","message":{"content":"I'll investigate..."},"timestamp":"2025-12-22T10:00:05Z"}
{"type":"tool_use","tool":"Read","input":{"file_path":"src/auth.rs"},"timestamp":"2025-12-22T10:00:06Z"}
{"type":"tool_result","tool_use_id":"toolu_1","content":"[file contents...]","timestamp":"2025-12-22T10:00:07Z"}
```

**Key differences from rlog:**
- No truncation - full content preserved
- Machine-parseable JSON
- Includes token metrics per message
- Compatible with Codex Code tooling

### Subagent Sessions

When autopilot spawns subagents (via Task tool), each subagent gets its own JSONL file:

**Parent session header (rlog):**
```yaml
---
format: rlog/1
id: 123456-task-name
repo_sha: abc123
---
```

**Subagent session header (JSONL):**
```json
{"type":"header","session_id":"sub-abc123","parent_session":"123456-task-name","agent_type":"explore","started_at":"2025-12-22T10:05:00Z"}
```

**Subagent tracking in parent rlog:**
```
x:explore id=abc123 → [started]
... (other work) ...
x:explore id=abc123 → [done] summary="found 3 files matching pattern"
```

### APM Data Source Priority

For APM (Actions Per Minute) calculations, always use JSONL files:

1. **Autopilot sessions**: `docs/logs/**/*.jsonl` (includes subagent files)
2. **Interactive sessions**: `~/.codex/projects/*.jsonl`

**Never use rlog for APM** - truncation loses action counts.

### Required Header Fields

- `format` (must start with `rlog/`)
- `id`
- `repo_sha`

### Optional Header Fields (parsed)

`mode`, `model`, `agent`, `version`, `repo`, `branch`, `dirty`, `sandbox_id`, `runner`, `toolset`, `skills`, `mcp`, `budget`, `duration`, `classification`, `notes`, and any `extra.*` fields.

### Codex Code Header Fields (optional)

When converting from Codex Code JSONL, these additional fields are captured:

- `client_version` - Codex Code client version (e.g., "2.0.71")
- `slug` - Human-readable session name (e.g., "mighty-wishing-music")
- `cwd` - Working directory path
- `tokens_total_in` - Session total input tokens
- `tokens_total_out` - Session total output tokens
- `tokens_cached` - Session total cached tokens
- `tokens_cache_create` - Session total cache creation tokens

## Line Prefixes

| Prefix | Type | Example |
| --- | --- | --- |
| `u:` | User | `u: Fix the login bug` |
| `a:` | Agent | `a: Investigating auth module.` |
| `t:` | Tool | `t:read src/auth.rs -> [186 lines]` |
| `t!:` | Tool start | `t!:test cargo test -> [running]` |
| `t~:` | Tool progress | `t~:test [64/128 passed]` |
| `o:` | Observation | `o: id=call_92 -> [ok] 128 tests passed` |
| `s:` | Skill | `s:deploy activate -> [loaded]` |
| `p:` | Plan | `p:create "Auth plan" -> [ok]` |
| `m:` | Mode | `m: auto` |
| `r:` | Recall | `r: "auth" -> [2 matches]` |
| `x:` | Subagent | `x:explore "find auth failures" -> summary=...` |
| `c:` | MCP | `c:github.issues state=open -> [8 issues]` |
| `q:` | Question | `q: "Which auth?" -> [selected: OAuth]` |
| `#` | Comment/meta | `# t=00:15:30` |
| `@` | Lifecycle | `@start id=sess_001 duration=12h` |
| `@phase` | Phase | `@phase explore` |
| `th:` | Thinking | `th: Analyzing the request... sig=Ep4E...` |
| `td:` | Todos | `td: [pending] Fix bug [completed] Add test` |

Notes:
- The parser recognizes the Unicode arrow `→` for results. The examples use `->` for readability, but logs should use `→` for result parsing.
- Indented lines (two spaces or a tab) are treated as continuations.

## Common Metadata Fields

These fields are extracted when present anywhere in a line:

- `id=` (call id)
- `step=`
- `ts=` (ISO timestamp)
- `tid=`
- `span=`
- `latency_ms=`
- `attempt=` (supports `N` or `N/M`)
- `level=`

The parser also extracts the result after `→` as `result`.

### Codex Code Metadata Fields

When converting from Codex Code, these additional fields may appear:

- `parent=` - Parent message UUID (for threading)
- `sig=` - Thinking block signature (optional, for verification)
- `tokens_in=` - Input tokens for this message
- `tokens_out=` - Output tokens for this message
- `tokens_cached=` - Cached tokens for this message
- `interrupted` - Flag indicating tool was interrupted
- `model=` - Model ID for this specific message

### Codex Code Comment Types

File snapshots, queue operations, and raw events are converted to comments:

```
# file-snapshot: <message_id> files=<count>
# queue: enqueue "<message>"
# queue: remove
# codex: {<raw jsonl>}
```

## Validation Rules (Current)

The validator reports warnings and info based on the implemented checks:

- Missing or invalid header fields (`format`, `id`, `repo_sha`).
- `format` not matching `rlog/1` or `rlog/1.0` (warning).
- `repo_sha` length not in 6..40 (warning).
- Unknown line formats (warning).
- Observation lines referencing unknown call ids (warning).
- Tool progress without a matching start or call id (warning).
- Step numbers decreasing (warning).
- Invalid timestamp format (warning).
- `@start` missing in long sessions (>50 lines, info).
- `@end` missing when `@start` exists (info).
- Blob and redaction markers are counted for stats.

## Example (Minimal)

```
---
format: rlog/1
id: sess_demo
repo_sha: abc123
---

# t=00:00:00
@start id=sess_demo duration=10m
u: Can you check auth?
a: Looking now.
t:read id=call_1 src/auth.rs → [186 lines]
o: id=call_1 → [ok]
@end summary="checked auth"
```

## Example (Codex Code Conversion)

```
---
format: rlog/1
id: 28da5a65-98ed-43b1-8b53-4f7216160d9c
repo_sha: 50446e6d5
client_version: "2.0.71"
slug: mighty-wishing-music
branch: main
model: codex-opus-4-5-20251101
tokens_total_in: 21890
tokens_total_out: 1250
tokens_cached: 12973
---

# file-snapshot: b2d8cc51 files=0
@start id=28da5a65 ts=2025-12-19T20:29:02Z
u: Fix the login bug ts=2025-12-19T20:29:02Z id=abc123
th: Let me analyze the authentication flow... sig=Ep4E...
  This requires checking the login handler.
a: I'll investigate the auth module. ts=2025-12-19T20:29:07Z parent=abc123 tokens_in=100 tokens_out=50
t!:Read id=toolu_1 file_path=src/auth.rs → [running] ts=2025-12-19T20:29:08Z
o: id=toolu_1 → [ok] 186 lines
td: [in_progress] Fix login bug [pending] Add tests
a: Found the issue - the session token validation is incorrect.
@end tokens_in=21890 tokens_out=1250
```

# BlackBox Format (bbox/1)

BlackBox is a line-based format for logging agent sessions. Each line is a single event. A required YAML header defines session metadata.

## File Layout

```
---
format: bbox/1
id: sess_20251219_001
repo_sha: abc123
---

u: User message
u: Another line
```

### Required Header Fields

- `format` (must start with `bbox/`)
- `id`
- `repo_sha`

### Optional Header Fields (parsed)

`mode`, `model`, `agent`, `version`, `repo`, `branch`, `dirty`, `sandbox_id`, `runner`, `toolset`, `skills`, `mcp`, `budget`, `duration`, `classification`, `notes`, and any `extra.*` fields.

### Claude Code Header Fields (optional)

When converting from Claude Code JSONL, these additional fields are captured:

- `client_version` - Claude Code client version (e.g., "2.0.71")
- `slug` - Human-readable session name (e.g., "mighty-wishing-music")
- `cwd` - Working directory path
- `tokens_total_in` - Session total input tokens
- `tokens_total_out` - Session total output tokens
- `tokens_cached` - Session total cached tokens

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

### Claude Code Metadata Fields

When converting from Claude Code, these additional fields may appear:

- `parent=` - Parent message UUID (for threading)
- `sig=` - Thinking block signature (optional, for verification)
- `tokens_in=` - Input tokens for this message
- `tokens_out=` - Output tokens for this message
- `tokens_cached=` - Cached tokens for this message
- `interrupted` - Flag indicating tool was interrupted
- `model=` - Model ID for this specific message

### Claude Code Comment Types

File snapshots and queue operations are converted to comments:

```
# file-snapshot: <message_id> files=<count>
# queue: enqueue "<message>"
# queue: remove
```

## Validation Rules (Current)

The validator reports warnings and info based on the implemented checks:

- Missing or invalid header fields (`format`, `id`, `repo_sha`).
- `format` not matching `bbox/1` or `bbox/1.0` (warning).
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
format: bbox/1
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

## Example (Claude Code Conversion)

```
---
format: bbox/1
id: 28da5a65-98ed-43b1-8b53-4f7216160d9c
repo_sha: 50446e6d5
client_version: "2.0.71"
slug: mighty-wishing-music
branch: main
model: claude-opus-4-5-20251101
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

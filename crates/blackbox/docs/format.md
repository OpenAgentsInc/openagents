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

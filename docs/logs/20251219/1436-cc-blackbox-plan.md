# Plan: Claude Code to BlackBox Converter

## Summary

Claude Code stores conversations in JSONL format at `~/.claude/projects/<project>/<session-id>.jsonl`. The BlackBox format (`bbox/1`) is a human-readable line-based format. This plan covers updating bbox to capture missing data, writing a converter, and validating the output.

## Data Gap Analysis

### Claude Code captures that bbox/1 does NOT currently support:

| Field | Description | Priority |
|-------|-------------|----------|
| `parentUuid` | Message threading/parent chain | High |
| `isSidechain` | Conversation branching | Medium |
| `thinkingMetadata` | Thinking level/triggers/disabled | High |
| `signature` | Cryptographic signature on thinking | Low |
| `todos` | Task list state per message | High |
| `cache_creation` | Token breakdown (5m/1h ephemeral) | Medium |
| `service_tier` | API service tier used | Low |
| `requestId` | API request ID for correlation | Medium |
| `slug` | Human-readable session names | Medium |
| `version` | Claude Code client version | High |
| `userType` | External vs internal user | Low |
| `interrupted` | Tool was interrupted | High |
| `isImage` | Tool result is image | Medium |
| `file-history-snapshot` | File backup tracking | Low |
| `queue-operation` | Message queue state | Low |
| `stop_reason` | API response termination | Medium |

---

## Implementation Steps

### Step 1: Update bbox/1 Format Spec

**File:** `crates/blackbox/docs/format.md`

Add new header fields:
```yaml
# New optional header fields
client_version: "2.0.71"      # Claude Code client version
slug: "mighty-wishing-music"   # Human-readable session name
tokens_total_in: 12345        # Session total input tokens
tokens_total_out: 5678        # Session total output tokens
tokens_cached: 1000           # Session cached tokens
```

Add new line types:
```
| `th:` | Thinking | `th: Analyzing the request... sig=Ep4E...` |
| `td:` | Todos | `td: [pending] Fix bug [completed] Add test` |
```

File snapshots and queue operations become comments:
```
# file-snapshot: file.rs sha256=abc123
# queue: enqueue "Fix the bug"
```

Add new metadata fields extractable from lines:
- `sig=` - thinking block signature (optional, for verification)
- `parent=` - parent message UUID
- `interrupted` - tool was interrupted flag
- `tokens_in=`, `tokens_out=`, `tokens_cached=` - per-line token counts

### Step 2: Update `lib.rs` Parser

**File:** `crates/blackbox/src/lib.rs`

Changes:
1. Add new `Header` fields: `client_version`, `slug`
2. Add new `LineType` variants: `Thinking`, `Todos`, `FileSnapshot`
3. Add new `ParsedLine` metadata fields: `parent_uuid`, `signature`, `interrupted`, token counts
4. Add regex patterns for new line types and metadata
5. Update validator to handle new types

### Step 3: Create Converter Module

**File:** `crates/blackbox/src/convert.rs` (new)

```rust
pub fn convert_claude_session(jsonl_path: &Path, repo_sha: &str) -> Result<String, ConvertError>
```

Conversion logic:
1. Read JSONL file line by line
2. Extract header info from first user message (sessionId, version, gitBranch, slug, cwd)
3. For each event:
   - `user` → `u: <content>`
   - `assistant` with `thinking` → `th: <content> sig=<signature>`
   - `assistant` with `text` → `a: <content>`
   - `assistant` with `tool_use` → `t!:<name> id=<id> <input_json> → [running]`
   - `tool_result` → `o: id=<id> → [ok|error] <content>`
   - `file-history-snapshot` → `f:backup <details>`
   - Append metadata: `ts=<timestamp> parent=<parentUuid>`
4. Track token usage and add to header or comments
5. Handle continuation lines for long content

### Step 4: CLI Command

**File:** `crates/blackbox/src/main.rs`

Add `convert` subcommand:
```
blackbox convert <claude-jsonl> --repo-sha <sha> [-o output.bbox]
```

Options:
- `--repo-sha` - Git SHA (required, or auto-detect from cwd)
- `--output` / `-o` - Output file path (default: stdout)
- `--include-thinking` - Include thinking blocks (default: true)
- `--include-snapshots` - Include file-history-snapshot events (default: false)

### Step 5: Validator Check (DONE)

Verified: `~/code/platform/crates/blackbox/` is **identical** to openagents:
- `lib.rs`: 819 lines (identical)
- `main.rs`: 838 lines (identical)
- `export.rs`: 364 lines (identical)

**No copy needed** - the validator is already in sync.

### Step 6: Add Tests

**File:** `crates/blackbox/src/convert.rs`

Tests:
1. `test_convert_minimal_session` - Basic user/assistant exchange
2. `test_convert_with_tool_calls` - Tool use and results
3. `test_convert_with_thinking` - Thinking blocks with signatures
4. `test_convert_with_todos` - TodoWrite tool calls
5. `test_convert_with_subagents` - Task tool calls
6. `test_roundtrip_validation` - Convert then validate passes
7. `test_no_data_loss` - Compare parsed structures for completeness

### Step 7: Data Loss Check Script

**File:** `crates/blackbox/src/bin/validate_conversion.rs` (or add to main.rs)

A command that:
1. Converts Claude JSONL to bbox
2. Parses the bbox
3. Compares key metrics:
   - Same number of user messages
   - Same number of tool calls
   - Same number of agent responses
   - Token counts preserved in comments
   - All UUIDs preserved as call IDs

---

## Critical Files

| File | Action |
|------|--------|
| `crates/blackbox/docs/format.md` | Update spec |
| `crates/blackbox/src/lib.rs` | Add types, metadata |
| `crates/blackbox/src/convert.rs` | New converter module |
| `crates/blackbox/src/main.rs` | Add convert CLI |
| `crates/blackbox/Cargo.toml` | Add serde_json dep |

---

## Execution Order

1. Update format.md with new fields/types
2. Update lib.rs with new enums and metadata
3. Create convert.rs with conversion logic
4. Update main.rs with CLI command
5. Write tests
6. Run on a real Claude session and validate
7. Compare original JSONL vs converted bbox for data loss

---

## Sample Output

Input (Claude JSONL):
```json
{"type":"file-history-snapshot","messageId":"b2d8cc51",...}
{"type":"user","message":{"content":"Fix the bug"},"uuid":"abc123","timestamp":"2025-12-19T20:29:02Z",...}
{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"Let me analyze...","signature":"Ep4E..."}],"usage":{"input_tokens":100,"output_tokens":50}},...}
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"src/lib.rs"}}]},...}
```

Output (bbox/1):
```yaml
---
format: bbox/1
id: 28da5a65-98ed-43b1-8b53-4f7216160d9c
repo_sha: 50446e6d5
client_version: "2.0.71"
slug: mighty-wishing-music
branch: main
model: claude-opus-4-5-20251101
tokens_total_in: 100
tokens_total_out: 50
tokens_cached: 0
---

# file-snapshot: b2d8cc51 files=0
@start id=28da5a65 ts=2025-12-19T20:29:02Z
u: Fix the bug ts=2025-12-19T20:29:02Z id=abc123
th: Let me analyze... sig=Ep4E... ts=2025-12-19T20:29:07Z parent=abc123 tokens_in=100 tokens_out=50
t!:Read id=toolu_1 file_path=src/lib.rs → [running] ts=2025-12-19T20:29:08Z parent=abc123
@end tokens_in=100 tokens_out=50
```

## Decisions Made

1. **Thinking blocks**: Include with signature (signature is optional in spec)
2. **Token usage**: Both per-line metadata AND session summary in header/footer
3. **Meta events**: `file-history-snapshot` and `queue-operation` → `# comment` lines

# APM Methodology

## Overview

APM (Actions Per Minute) measures agent execution velocity - how many meaningful actions an agent performs per unit time. This document describes the data sources, collection methods, and calculation formulas used in APM reporting.

## Data Architecture

Autopilot uses a **dual-format logging system**:

| Format | Purpose | Content |
|--------|---------|---------|
| `.rlog` | Human-readable summary | Truncated content (200 chars max) |
| `.jsonl` | Full data capture | Untruncated, Claude Code compatible |

Both files are created for each autopilot session:
```
docs/logs/20251222/
  123456-task-name.rlog      # Human review
  123456-task-name.jsonl     # APM/metrics data
  123456-task-name.sub-*.jsonl  # Subagent sessions
```

**IMPORTANT:** APM calculations should use `.jsonl` files, NOT `.rlog` files. The rlog truncation loses data.

## Formula

```
APM = total_actions / duration_minutes
```

Where:
- **total_actions**: Count of user messages + assistant messages + tool calls
- **duration_minutes**: Actual session duration from timestamps (first to last entry)

### Counting Actions in JSONL Files

```
actions = count("type":"user") + count("type":"assistant") + count("type":"tool_use")
```

- `"type":"user"` - User messages (top-level record type)
- `"type":"assistant"` - Assistant responses (top-level record type)
- `"type":"tool_use"` - Tool calls (nested in assistant content arrays)

**Note:** Older reports may use `"type":"message"` which is different and deprecated.

## APM Tiers

| Range | Tier | Description |
|-------|------|-------------|
| 0-5 | Baseline | Human-in-the-loop, lots of thinking time |
| 5-15 | Active | Steady work pace |
| 15-30 | Productive | Efficient autonomous execution |
| 30-50 | High Performance | Fast parallel tool usage |
| 50+ | Elite | Maximum efficiency |

## Data Sources

### Claude Code Sessions (Interactive)

**Location:** `~/.claude/projects/-<path-encoded>/*.jsonl`

**File Types:**
- **UUID files** (e.g., `d7dad8fc-386b-46db-ba1a-a4d5ca4d23fe.jsonl`): Interactive sessions
- **Agent files** (e.g., `agent-a0f54f0.jsonl`): Subagent/subprocess sessions spawned by autopilot

**Parsing:**
```bash
# Count messages
grep -c '"type":"message"' session.jsonl

# Count tool calls
grep -c '"type":"tool_use"' session.jsonl
```

**Duration Calculation:**
- Extract first and last `"timestamp"` field from the file
- Duration = last_timestamp - first_timestamp
- **Do NOT use arbitrary averages** (e.g., "15 min/session") - this produces garbage data

### Autopilot Sessions (Autonomous)

**Location:** `docs/logs/YYYYMMDD/`

**File Types:**
- **`.rlog`** - Human-readable summary (truncated, for quick review)
- **`.jsonl`** - Full data capture (untruncated, for APM calculations)
- **`.sub-*.jsonl`** - Subagent session data (linked to parent)

**For APM, always use `.jsonl` files:**
```bash
# Count actions in autopilot JSONL
cat session.jsonl | grep -c '"type":"user"'
cat session.jsonl | grep -c '"type":"assistant"'
cat session.jsonl | grep -o '"type":"tool_use"' | wc -l
```

**Subagent Tracking:**

Autopilot tracks subagent spawns with `x:` lines in rlog and separate `.sub-*.jsonl` files:
```
x:explore id=abc123 → [started]
x:explore id=abc123 → [done] summary="found 3 files"
```

Subagent JSONL files are linked via `parent_session` header field.

**Duration:** Extract from first/last timestamp in JSONL file

## Collection Scripts

### Interactive Claude Code Stats
```bash
# Path varies by OS (path-encoded project directory)
CLAUDE_DIR="$HOME/.claude/projects/-home-christopherdavid-code-openagents"  # Linux
# CLAUDE_DIR="$HOME/.claude/projects/-Users-christopherdavid-code-openagents"  # macOS

# Count actions
cat "$CLAUDE_DIR"/[0-9a-f]*-*-*-*-*.jsonl | grep -c '"type":"user"'
cat "$CLAUDE_DIR"/[0-9a-f]*-*-*-*-*.jsonl | grep -c '"type":"assistant"'
cat "$CLAUDE_DIR"/[0-9a-f]*-*-*-*-*.jsonl | grep -o '"type":"tool_use"' | wc -l
```

### Autopilot Stats
```bash
# Use JSONL files (full data), not rlog files (truncated)
cat docs/logs/2025*/*.jsonl | grep -c '"type":"user"'
cat docs/logs/2025*/*.jsonl | grep -c '"type":"assistant"'
cat docs/logs/2025*/*.jsonl | grep -o '"type":"tool_use"' | wc -l

# Include subagent sessions
cat docs/logs/2025*/*.sub-*.jsonl | grep -c '"type":"user"'
# ... etc
```

## Multi-Machine Aggregation

When combining data from multiple machines:

1. **Sum actions**: Add message counts and tool_use counts across machines
2. **Sum durations**: Add estimated duration from each machine
3. **Calculate combined APM**: total_actions / total_duration

**Note:** Avoid double-counting agent subprocess files with rlog data - they represent different views of the same autopilot sessions.

## Caveats

1. **Duration is estimated** - Actual session time may vary significantly from averages
2. **Idle time included** - Sessions left open but inactive inflate duration
3. **Error sessions** - Failed or restarted sessions still count toward totals
4. **Agent overlap** - Agent JSONL files may overlap with rlog counts (counted separately)

## File References

- **Report template**: [`docs/apm/report-YYYYMMDD.md`](report-20251222.md)
- **APM calculation code**: [`crates/autopilot-core/src/apm.rs`](../../crates/autopilot-core/src/apm.rs)
- **APM parser**: [`crates/autopilot-core/src/apm_parser.rs`](../../crates/autopilot-core/src/apm_parser.rs)

## Historical Reports

- [December 22, 2025](report-20251222.md) - First comprehensive multi-machine report

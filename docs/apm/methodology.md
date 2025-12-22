# APM Methodology

## Overview

APM (Actions Per Minute) measures agent execution velocity - how many meaningful actions an agent performs per unit time. This document describes the data sources, collection methods, and calculation formulas used in APM reporting.

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

**CRITICAL LIMITATION:** Autopilot data is split across incompatible sources:

1. **rlog files** (`docs/logs/YYYYMMDD/*.rlog`)
   - Main autopilot loop activity
   - Only captures tool calls (`t!:` lines), NOT messages
   - Duration: Use @start timestamp and file mtime

2. **Agent JSONL files** (`~/.claude/projects/.../agent-*.jsonl`)
   - Subagent processes spawned by autopilot
   - Short-lived (avg ~23 sec) exploration/search tasks
   - Full action logging (user + assistant + tool_use)

**These cannot be meaningfully combined** because they capture different aspects of autopilot activity with different logging granularity.

**rlog Parsing:**
```bash
# Extract start timestamp
grep -m1 "^@start" session.rlog | sed 's/.*ts=\([^ ]*\).*/\1/'

# Count tool calls (INCOMPLETE - missing messages)
grep -c "^t!:" session.rlog
```

**Duration:** Use file modification time minus @start timestamp

## Collection Scripts

### Linux Collection
```bash
# Claude Code stats
CLAUDE_DIR="$HOME/.claude/projects/-home-christopherdavid-code-openagents"
find "$CLAUDE_DIR" -name '*.jsonl' -exec cat {} + | grep -c '"type":"message"'
find "$CLAUDE_DIR" -name '*.jsonl' -exec cat {} + | grep -c '"type":"tool_use"'

# Autopilot stats
grep -r "^t!:" docs/logs/2025*/*.rlog | wc -l
```

### macOS Collection
```bash
# Claude Code stats (different path encoding)
CLAUDE_DIR="$HOME/.claude/projects/-Users-christopherdavid-code-openagents"
# Same grep commands as Linux
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
- **APM calculation code**: [`crates/autopilot/src/apm.rs`](../../crates/autopilot/src/apm.rs)
- **APM parser**: [`crates/autopilot/src/apm_parser.rs`](../../crates/autopilot/src/apm_parser.rs)

## Historical Reports

- [December 22, 2025](report-20251222.md) - First comprehensive multi-machine report

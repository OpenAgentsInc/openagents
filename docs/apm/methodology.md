# APM Historical Report Plan

## Goal
Generate a multi-computer APM report comparing historical data to the last 48 hours, with instructions for combining data from both systems.

## Data Sources Discovered

### This Computer (macOS - christopherdavid)

| Source | Location | Count | Date Range |
|--------|----------|-------|------------|
| Claude Code Sessions | `~/.claude/projects/-Users-christopherdavid-code-openagents/*.jsonl` | 2,062 files (~983MB) | Nov 28 - Dec 22 |
| Autopilot Logs | `docs/logs/YYYYMMDD/*.rlog` | 103 files | Dec 19-22 |
| Metrics DB | `autopilot-metrics.db` | 1 session | Dec 20 |

**Autopilot logs by day:**
- 20251219: 17 rlog files
- 20251220: 31 rlog files
- 20251221: 28 rlog files
- 20251222: 27 rlog files

### APM Formula
```
APM = (messages + tool_calls) / duration_minutes
```

## Implementation Plan

### Step 1: Create APM Report Script
Write a shell script that:
1. Parses all rlog files to extract tool_calls count and duration
2. Parses Claude Code JSONL files using jq to count messages and tool_use entries
3. Calculates APM per session and aggregated

### Step 2: Extract Data from rlog Files
Parse the rlog format:
```
t!:<ToolName> id=<id> ... → [running|ok|error]
@start ts=<timestamp>
```
- Count lines matching `^t!:` for tool_calls
- Extract timestamps from `@start ts=` and last `o:` line

### Step 3: Extract Data from Claude Code JSONL
Parse JSONL lines with types:
- `"type":"message"` → increment messages
- `"type":"tool_use"` or `"type":"tool_call"` → increment tool_calls

### Step 4: Generate Report Structure
```
docs/logs/apm-report-YYYYMMDD.md

## APM Historical Report
### Part 1: <hostname> (INCOMPLETE - awaiting second computer)

#### All-Time Statistics
- Sessions: X
- Total Actions: Y
- Total Duration: Z min
- Lifetime APM: W

#### Last 48 Hours
- Sessions: X
- Actions: Y
- Duration: Z min
- 48h APM: W

#### By Source
| Source | Sessions | Actions | Duration | APM |
|--------|----------|---------|----------|-----|
| Autopilot | ... | ... | ... | ... |
| Claude Code | ... | ... | ... | ... |

### Instructions for Second Computer
[script to run]
```

### Step 5: Script to Run on Other Computer
Create a self-contained script that:
1. Finds Claude Code sessions for openagents
2. Parses rlog files if they exist
3. Outputs JSON with stats
4. Can be appended to the report

## Files to Create/Modify

| File | Purpose |
|------|---------|
| `docs/logs/apm-report-20251222.md` | **NEW** - The APM report with Part 1 data |

## Two-Computer Strategy

**This computer (macOS):** Generate Part 1 with:
- All available data statistics
- Last 48 hours statistics
- Breakdown by source

**Other computer (Linux):** Include a script in the report that:
- Uses Linux paths (`/home/christopherdavid/`)
- Outputs data in same format
- Instructions for appending to report

## Report Contents

1. **Header** - Explain this is Part 1, incomplete until second computer runs
2. **Methodology** - How APM is calculated
3. **This Computer's Data**:
   - Lifetime APM (all available data)
   - Last 48 hours APM
   - Breakdown by source (autopilot vs claude code)
   - Session count and distribution
4. **Instructions for Second Computer**:
   - Copy-paste script
   - Where to append results
   - How to compute combined totals

## Execution Steps

1. Count and parse all rlog files for autopilot APM
2. Sample/count Claude Code JSONL for claude code APM
3. Calculate aggregate stats
4. Write report to `docs/logs/apm-report-20251222.md`
5. Include script for second computer

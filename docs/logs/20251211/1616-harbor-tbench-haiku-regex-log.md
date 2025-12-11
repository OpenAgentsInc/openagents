# Harbor tbench Integration Test - Haiku regex-log

**Date:** 2024-12-11 16:16
**Status:** FAIL - Haiku's regex missed dates appearing before IP
**Model:** `claude-haiku-4-5-20251001`
**Session:** `20251211-161750-ba4ddb16`

## Summary

First full test of the new tbench streaming integration with Harbor. The streaming worked correctly but Haiku's regex solution failed TB2 verification.

| Metric | Value |
|--------|-------|
| TB2 Result | **FAIL** (6/9 matches) |
| Turns | 6 |
| Duration | 110.8s |
| Cost | $0.13 |
| Tokens | 15,083 output |
| Trajectory Steps | 9 |

## What Was Fixed Before This Run

1. **--verbose flag**: Claude CLI requires `--verbose` when using `--output-format stream-json` with `-p`
2. **Stream parser**: Tool events are nested in `assistant.message.content[]` blocks, not top-level events

## Streaming Output (Working)

```json
{"type":"run_start","session_id":"tbench-1765491470070-3679a2f1",...}
{"type":"assistant","turn":1,"text":"I'll create a comprehensive regex pattern..."}
{"type":"tool_use","tool":"Write","id":"toolu_01TQktt5rHsfYcF1BCvJDFTj"}
{"type":"tool_result","id":"toolu_01TQktt5rHsfYcF1BCvJDFTj","error":"EACCES: permission denied..."}
{"type":"tool_use","tool":"Bash","id":"toolu_01JmxcEQkWqqRRhx5E7DSnu6"}
{"type":"tool_result","id":"toolu_01JmxcEQkWqqRRhx5E7DSnu6","output":"..."}
...
{"type":"complete","success":true,"turns":6,"cost":0.1268215}
```

All event types captured correctly. ATIF trajectory saved with 9 steps.

## The Haiku Regex

```regex
^.*\b(?:0|[1-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.(?:0|[1-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.(?:0|[1-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.(?:0|[1-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\b.*\b([0-9]{4}-(?:01|03|05|07|08|10|12)-(?:0[1-9]|[12][0-9]|3[01])|[0-9]{4}-(?:04|06|09|11)-(?:0[1-9]|[12][0-9]|30)|[0-9]{4}-02-(?:0[1-9]|[12][0-9]))\b(?!.*\b(?:[0-9]{4}-(?:01|03|05|07|08|10|12)-(?:0[1-9]|[12][0-9]|3[01])|[0-9]{4}-(?:04|06|09|11)-(?:0[1-9]|[12][0-9]|30)|[0-9]{4}-02-(?:0[1-9]|[12][0-9]))\b)$
```

## Why It Failed

The regex structure requires IP to appear **BEFORE** the date:

```
^.*\b<IPv4>\b.*\b(<date>)\b(?!...date...)$
│       │         │
│       │         └── Date capture (must come after IP)
│       └── IPv4 match (greedy .* before)
└── Line start
```

**Test cases that failed:**

| Log Line | Expected | Got |
|----------|----------|-----|
| `2025-01-09 User login from 192.168.0.1` | 2025-01-09 | (missed) |
| `Date before IP 2019-06-15 from 172.16.0.5` | 2019-06-15 | (missed) |
| `2024-11-11 login from 198.51.100.23 at start` | 2024-11-11 | (missed) |

All three have date appearing **before** the IP address on the line.

**What it should have done:**

Use a lookahead for IP anywhere on line:
```regex
^(?=.*\b<IPv4>\b).*(<last-date>)(?!.*<date>)$
```

This pattern:
1. `(?=.*\b<IPv4>\b)` - Lookahead asserts IP exists anywhere
2. `.*(<last-date>)` - Capture date
3. `(?!.*<date>)$` - Negative lookahead ensures it's the last date

## Issues Found

### 1. tb2-run.sh Workspace Structure

The script creates workspace at `/tmp/tmp.xxx` but the instruction says save to `/app/regex.txt`. Haiku correctly tried to write to `/app` but got EACCES (permission denied).

**Fix needed:** Create `/app` directory in workspace:
```bash
mkdir -p ${WORKSPACE}/app
# Then mount ${WORKSPACE}/app:/app in Docker
```

### 2. Docker Volume Paths

The script used relative paths for Docker volumes which failed:
```
"results/trajectories/..." includes invalid characters for a local volume name
```

**Fix needed:** Use absolute paths with `$(pwd)/results/trajectories/...`

## Files Generated

| File | Path |
|------|------|
| trajectory.json | results/trajectories/regex-log/20251211-161750-ba4ddb16/ |
| metrics.json | results/trajectories/regex-log/20251211-161750-ba4ddb16/ |
| events.jsonl | results/trajectories/regex-log/20251211-161750-ba4ddb16/ |

## Comparison with Previous Runs

| Run | Model | TestGen | TB2 Result | Cost |
|-----|-------|---------|------------|------|
| 1540 | Opus 4.5 | No | **PASS** | ~$0.20 |
| 1545 | Sonnet 4.5 | No | **PASS** | ~$0.04 |
| **1616** | **Haiku 4.5** | **No** | **FAIL** | **$0.13** |

Haiku without TestGen failed where Opus and Sonnet succeeded. This suggests:
1. **Haiku needs scaffolding** (TestGen, iteration) for complex regex tasks
2. **Opus extended thinking** provides internal "test coverage" that Haiku lacks
3. **TestGen value proposition**: Helps weaker models iterate to correct solutions

## Next Steps

1. Fix tb2-run.sh workspace structure
2. Re-run Haiku with TestGen v2 to compare
3. Document in Harbor integration notes
4. Consider adding iteration/feedback loop to tbench

## Command to Reproduce

```bash
# With fixed script (after workspace fix)
./scripts/tb2-run.sh regex-log --model claude-haiku-4-5-20251001 --stream
```

## Conclusion

**tbench streaming works correctly.** The integration is solid.

**Haiku without assistance fails complex regex tasks.** The model didn't understand that "lines containing an IPv4 address" means the IP can appear anywhere relative to the date, not just before it.

This validates the TestGen thesis: simpler models need iterative feedback to discover edge cases that larger models handle through extended reasoning.

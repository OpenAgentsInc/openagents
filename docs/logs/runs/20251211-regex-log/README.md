# regex-log TB2 Test Runs - 2025-12-11

Raw Claude Code session logs from TB2 regex-log benchmark tests.

## Main Sessions

| File | Model | Protocol | TB2 Result | Size |
|------|-------|----------|------------|------|
| `haiku-testgen-v2.jsonl` | claude-haiku-4-5-20251001 | TestGen v2 | PASS | 557K |
| `opus-raw.jsonl` | claude-opus-4-5-20251101 | Raw (no scaffolding) | PASS | 80K |
| `sonnet-raw.jsonl` | claude-sonnet-4-5-20250929 | Raw (no scaffolding) | PASS | 43K |

## Subagents

The `subagents/` directory contains Task tool spawns from the Haiku TestGen v2 run (review loop iterations).

## Format

These are Claude Code native JSONL session logs, not ATIF format. Each line is a JSON object representing a message in the conversation.

## Analysis Logs

See companion analysis docs:
- `docs/logs/20251211/1535-testgen-v2-tb2-pass.md` - Haiku + TestGen analysis
- `docs/logs/20251211/1540-opus-raw-tb2-pass.md` - Opus raw analysis
- `docs/logs/20251211/1545-sonnet-raw-tb2-pass.md` - Sonnet raw analysis

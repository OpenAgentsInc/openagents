# Terminal-Bench Integration Guide

Use this as a fast path for running Terminal-Bench with MechaCoder—locally for internal evaluation or via Harbor for official leaderboard submissions. For the full deep dive, read `docs/tbench/README.md`.

## Prerequisites

- Dataset: `tasks/terminal-bench-2.json` (already checked in from the upstream repo)
- Local/internal runs: Bun + Node, Python 3 with `pytest`, Claude Code CLI signed in
- Harbor/leaderboard: Docker, Python 3.12 with `uv`, **real Anthropic API key** (OAuth tokens from Claude Max do not work in containers)

## Local workflow (internal evaluation)

Run a subset of tasks:
```bash
export TB_OUT=/tmp/tbench-run

bun src/cli/tbench-local.ts \
  --suite tasks/terminal-bench-2.json \
  --output "$TB_OUT" \
  --tasks "regex-log,chess-best-move" \
  --timeout 3600 \
  --max-turns 300 \
  --parallel 2

cat $TB_OUT/report.md                 # human-readable summary
cat $TB_OUT/results.json              # machine-readable results
ls -la $TB_OUT/regex-log/workspace/   # files the agent created
cat $TB_OUT/regex-log/verification.txt
```

Generate an HTML dashboard from multiple runs:
```bash
# Collect several results.json files under ./results/
bun src/cli/tbench-dashboard.ts --results-dir ./results --output ./dashboard.html
open ./dashboard.html
```
Pass `--suite` if your suite file lives elsewhere (defaults to tasks/terminal-bench-2.json for category labels).

Run the full suite: drop `--tasks` (consider `--parallel 4` if your machine can handle it).

Compare against a baseline from a previous run:
```bash
bun src/cli/tbench-local.ts \
  --suite tasks/terminal-bench-2.json \
  --output "$TB_OUT" \
  --baseline /path/to/previous/results.json
```

Generate a per-category markdown summary from `results.json`:
```bash
TB_OUT=/tmp/tbench-run
bunx tsx <<'EOF'
import { readFileSync } from "node:fs";
import { buildTerminalBenchReport, formatTerminalBenchMarkdown } from "./src/bench/reporter.ts";

const suite = JSON.parse(readFileSync("tasks/terminal-bench-2.json", "utf8"));
const results = JSON.parse(readFileSync(`${process.env.TB_OUT}/results.json`, "utf8"));
console.log(formatTerminalBenchMarkdown(buildTerminalBenchReport(suite, results)));
EOF
```

## Leaderboard workflow (Harbor)

```bash
cd src/harbor
uv venv && source .venv/bin/activate
uv pip install -e ".[dev]"

export ANTHROPIC_API_KEY="sk-ant-api03-..."
harbor run \
  --agent-import-path openagents_harbor:MechaCoderAgent \
  --dataset terminal-bench@2.0 \
  -k 1 \
  -o results/
```

Artifacts land in `results/` (per-task workspaces plus `results.json`/`report.md`). Harbor ignores `--model` flags; MechaCoder chooses models internally.

## Outputs to expect
- `results.json` and `report.md` for the overall run
- Per-task folders with `workspace/`, `output.txt` (agent log), `verification.txt` (pytest output)
- Token/turn stats and category rollups available via `src/bench/reporter.ts`

## Helpful references
- `docs/tbench/README.md` – full adapter/CLI details
- `src/cli/tbench-local.ts` – local runner implementation and options
- `src/harbor/openagents_harbor/mechacoder_agent.py` – Harbor entrypoint
- `src/bench/reporter.ts` – reporters for per-category summaries and markdown

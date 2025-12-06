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

## Using Apple Foundation Models (FM)

Run Terminal-Bench with Apple's on-device Foundation Models for free, local inference.

### Prerequisites

- macOS 26 (Tahoe) or later with Apple Intelligence enabled
- Swift bridge built and running

```bash
# Build and start the FM bridge (keep running in one terminal)
cd swift/foundation-bridge
./build.sh
./run.sh

# Verify it's working (in another terminal)
curl http://localhost:11435/health
```

### Basic FM Run

Run FM mini-suite (7 quick regression tasks):
```bash
# Using package.json script
bun run tbench:fm-mini

# Or manually with tbench-local
bun src/cli/tbench-local.ts \
  --suite tasks/fm-mini-suite.json \
  --output .openagents/tb-runs/fm-test \
  --model fm
```

Run full Terminal-Bench with FM:
```bash
bun src/cli/tbench-iterate.ts \
  --suite tasks/terminal-bench-2.json \
  --model fm \
  --iterations 1 \
  --output ./results/fm-run
```

### FM with Learning Features

FM supports three learning layers that improve performance over time:

1. **Skills** (Voyager-style): Inject successful patterns from past runs
2. **Memory** (Generative Agents): Retrieve relevant episodic memories
3. **Reflexion**: Generate deep reflections on failures for retry

```bash
# Full learning stack
bun src/cli/tbench-iterate.ts \
  --suite tasks/terminal-bench-2.json \
  --model fm \
  --skills \
  --memory \
  --reflect \
  --max-retries 2 \
  --iterations 10

# Overnight learning sweep (extract skills after each iteration)
bun src/cli/tbench-iterate.ts \
  --suite tasks/terminal-bench-2.json \
  --model fm \
  --skills --memory --reflect --learn \
  --iterations 10
```

### Learning Flags Reference

| Flag | Description | Default |
|------|-------------|---------|
| `--skills` | Enable skill injection | true for FM |
| `--no-skills` | Disable skill injection | - |
| `--memory` | Enable memory retrieval and injection | false |
| `--reflect` | Enable FM-generated reflexion on failures | false |
| `--max-retries` | Max reflection-based retries per task | 2 |
| `--learn` | Extract skills and reflections after each iteration | false |

### FM in Subagent Router

FM is also available as a subagent in the MechaCoder orchestrator. When enabled, the routing order is:

1. Claude Code (if available and appropriate)
2. FM (if macOS and bridge healthy)
3. Minimal subagent (OpenRouter fallback)

### FM Mini-Suite

The FM mini-suite (`tasks/fm-mini-suite.json`) contains 7 tasks designed for quick FM regression testing:

| Task | Description | Tools Used |
|------|-------------|------------|
| fm-hello-world | Create hello.txt | write_file |
| fm-read-and-echo | Read and copy file | read_file, write_file |
| fm-append-to-file | Append to existing file | read_file, write_file |
| fm-list-directory | List files via command | run_command, write_file |
| fm-create-and-run | Create and execute script | write_file, run_command |
| fm-simple-edit | Edit existing file | edit_file |
| fm-word-count | Count words via command | run_command, write_file |

Run with:
```bash
bun run tbench:fm-mini           # Local run with FM
bun run tbench:fm-mini:iterate   # Iterate with learning features
```

### FM Context Limits

Apple FM has a very limited context window (~1100 chars). The adapter automatically:
- Truncates message history to fit context
- Keeps system prompt and last message
- Retries with minimal context if initial call fails

Configure via `FM_MODEL_CONFIGS` in `src/bench/model-adapter.ts`.

## Helpful references
- `docs/tbench/README.md` – full adapter/CLI details
- `src/cli/tbench-local.ts` – local runner implementation and options
- `src/cli/tbench-iterate.ts` – overnight iteration runner with learning
- `src/harbor/openagents_harbor/mechacoder_agent.py` – Harbor entrypoint
- `src/bench/reporter.ts` – reporters for per-category summaries and markdown
- `src/fm/` – Foundation Models service implementation
- `swift/foundation-bridge/` – Swift bridge for Apple FM

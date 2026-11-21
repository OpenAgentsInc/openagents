# Claude History Evals Specification

**Status:** Proposed  
**Target:** `evals/claude-history/` (extraction pipeline + hermetic task suite)  
**Related docs:** [`desktop-evals-spec.md`](desktop-evals-spec.md), [`evaluation-qualification.md`](evaluation-qualification.md), [`../../evals/coder-desktop/README.md`](../../evals/coder-desktop/README.md)

---

## 1. Goals and Core Thesis

Local Claude Code history in `~/.claude/` contains thousands of real engineering sessions driven by frontier models (`claude-opus-5-5`, `claude-fable-5-2`, and `claude-haiku-5-5` subagents). Those trajectories represent **observed production baselines** on real repositories (`openagents`, `aethyr-cpp`, `gamebuilder`, etc.): real user prompts, real multi-file edits, real failure-recovery loops, and real verification commands.

This specification defines how to **extract, sanitize, structure, and run** those sessions as reproducible evaluations so that smaller, faster, or cheaper agents (e.g., `mcworker` on Gemini 4 Hydrogen/Carbon/Neon, OpenRouter models, or local Inkling/llama.cpp models) can be measured against Opus/Fable baselines and systematically improved to **meet or exceed** them.

### 1.1 Non-Goals

- Not a raw dump of `~/.claude/` into git (`~/.claude` is ~17 GB and contains private paths, tokens, and debug blobs).
- Not a subjective LLM-as-judge-only benchmark; every promoted eval must have **objective, deterministic graders** (exit codes, unit/integration tests, diff/seal invariants).
- Not a replacement for `evals/coder-desktop/` tiers 0–4; instead, it **feeds** real-world tasks into the same `host_driver` + `mcworker` harness.

---

## 2. Source Archive Anatomy (`~/.claude`)

Per `~/.claude/CHAT_AUDIT.md` and direct inspection of `~/.claude/projects/`:

- **Scale:** ~17 GB across 82 projects, 2,083 JSONL files (324 top-level sessions + 1,759 subagent transcripts), and 13,957 user prompts in `~/.claude/history.jsonl`.
- **Concentration:** Top 4 projects (`work/openagents`, `work/aethyr-cpp`, `code/openagents`, `work/gamebuilder`) account for **98.0%** of transcript bytes.
- **Models present in `assistant.message.model`:** `claude-opus-5-5`, `claude-fable-5-2`, `claude-haiku-5-5`.

### 2.1 JSONL Record Filtering

Each session file (`~/.claude/projects/<encoded-project>/<sessionId>.jsonl`) and subagent file (`<sessionId>/subagents/*.jsonl`) is line-delimited JSON. Ingestion separates signal from bloat:

| Record `type` | Action | Extracted Fields |
|---|---|---|
| `user` | **Keep** | `uuid`, `parentUuid`, `timestamp`, `cwd`, `gitBranch`, `sessionId`, `message.content` (text prompts and `tool_result` blocks) |
| `assistant` | **Keep** | `uuid`, `parentUuid`, `timestamp`, `message.model`, `message.content` (`thinking`, `text`, `tool_use`), `message.usage` (input/output/cache tokens), `effort` |
| `system`, `tools-used`, `cost-state`, `custom-title`, `ai-title` | **Keep** | Session metadata, tool inventory, cost totals, titles |
| `api-request-blob`, `api-request`, `api-request-shape` | **Drop** | Raw HTTP/wire debug blobs (~60%+ of archive bytes) |
| `file-history-snapshot`, `file-history-delta`, `queue-operation`, `atis-latch` | **Drop** | Redundant with git or internal UI state |
| Inline base64 images inside `user`/`assistant` | **Strip** | Replace with `[image:sha256]` placeholder unless task explicitly tests vision |

---

## 3. Repository & Local Directory Structure

Large raw indexes and extracted episode caches live outside git under `~/.openagents/evals/claude-history/`. Only the lightweight extraction tooling, schemas, sanitized task definitions, and small fixtures are committed to git:

```text
evals/claude-history/
  README.md                 # Usage, commands, and baseline summary
  run.sh                    # Entrypoint: extract, mine, run, compare
  schema/                   # Task & baseline metric schemas
  tools/                    # Coax tools (or Firm-reasoned scripts per AGENTS.md)
    ingest.coax             # Stream-parse ~/.claude JSONLs -> local index
    mine.coax               # Score & select candidate episodes
    sanitize.coax           # Secret/PII scrubbing & hermeticity check
  tasks/                    # Committed sanitized eval task JSONs (tier2/tier4 compatible)
    <task_id>.json
  fixtures/                 # Minimal fixtures for non-git-pinned tasks (if any)
  results/                  # Gitignored run outputs & comparison reports
```

> **Tooling rule (`AGENTS.md`):** Any non-Coax script (`.py`, `.sh`) in `evals/claude-history/` must include a valid `Firm reason: N (...)` header and pass `coax/web/tool.sh tools-check`.

---

## 4. Extraction & Curation Pipeline

The pipeline has five stages:

### Stage 1 — Ingest & Index (Read-Only over `~/.claude`)
Stream-parse `~/.claude/projects/**/*.jsonl` (excluding bloat record types) into a local index at `~/.openagents/evals/claude-history/index.sqlite` (or JSONL index) keyed by `(project, session_id)`. Record:
- Primary model (`opus-5-5`, `fable-5-2`, etc.) and subagent models
- Total turns, tool calls (broken down by `Read`, `Edit`, `Write`, `Bash`, `Grep`, `Glob`, `Agent`), failed tool calls, and token/cost totals
- `cwd`, `gitBranch`, start/end timestamps, and any git SHAs or test commands observed in `Bash` tool calls

### Stage 2 — Episode Mining & Stratification
A session often contains multiple distinct user tasks. Split sessions into **episodes** (from a substantive `user` prompt to task completion or next unrelated prompt) and score them for eval suitability.

**Inclusion criteria (must satisfy all):**
1. **Anchorable starting state:** Either `cwd` maps to a git repo with an identifiable `base_sha` (resolved via timestamp/branch/first-commit-after-prompt or recorded `git rev-parse`), or the task can be packaged as a small self-contained fixture.
2. **Verifiable outcome:** The episode contains an objective check — e.g., a test command (`coax test`, `pytest`, `unittest`, `cargo test`, `tools-check`), a compiler/build check, or a clean target diff/commit.
3. **Substantive work:** At least 2 tool calls and >= 1 file edit or diagnostic command (excludes `/status`, `/model`, `/fast`, and pure Q&A).

**Exclusion criteria:**
- Tasks requiring live external secrets, production DBs, or interactive GUI clicks that cannot run headlessly.
- Unreproducible local-only scratch directories whose files no longer exist.

**Stratification buckets (so weaker models are tested across the real failure surface):**
- **Bucket A — Targeted Bugfix & Edit Precision:** 1–3 file fixes where smaller models often fail `old`/`new` exact-match edits.
- **Bucket B — Multi-File Feature / Refactor:** 4+ file changes requiring search-before-read and plan discipline.
- **Bucket C — Verification & "Done Means Real":** Tasks where the baseline ran real end-to-end checks (not stubs/mocks) and caught subtle regressions.
- **Bucket D — Error Recovery & Debugging:** Episodes where the baseline hit a failing test/build, diagnosed it from output, and pivoted.
- **Bucket E — Long-Context / Compaction & Subagent Delegation:** Episodes with >20 turns or subagent fan-out (`Agent` tool calls).

### Stage 3 — Sanitization & Hermeticity Gate
Before any mined episode is written to `evals/claude-history/tasks/`:
1. **Secret & PII scrub:** Scan prompts, fixtures, and expected outputs for API keys, bearer tokens, private keys, Nostr `nsec`s, emails, and user-specific absolute paths (`/Users/<name>/...` -> relative repo paths).
2. **Path normalization:** Rewrite `cwd` references to the isolated eval worktree root.
3. **Baseline verification:** Replay or verify the baseline's final state passes the extracted grader commands on `base_sha` + baseline patch (confirming the eval is actually solvable and the grader is not broken).

### Stage 4 — Materialize in `coder-desktop` Task Format
Each curated task is stored as JSON compatible with `evals/coder-desktop/tier2` and `tier4/golden-template.json`, extended with a `baseline` block:

```json
{
  "id": "ch_001_example_fix",
  "category": "claude-history",
  "bucket": "A_edit_precision",
  "source": {
    "session_id": "...",
    "project": "work/openagents",
    "base_sha": "<git-commit>",
    "target_sha": "<optional-reference-commit>"
  },
  "baseline": {
    "model": "claude-opus-5-5",
    "turns": 6,
    "tool_calls": 14,
    "failed_tool_calls": 1,
    "cost_usd_micros": 185000,
    "wall_s": 42,
    "passed": true
  },
  "budget_usd_micros": 150000,
  "timeout_s": 300,
  "steps": [
    { "send": "Sanitized user prompt..." }
  ],
  "graders": [
    { "type": "not_timed_out" },
    { "type": "ended" },
    { "type": "command", "run": "coax/bin/coax test ..." },
    { "type": "outside_unchanged" },
    { "type": "no_guard_trips" }
  ]
}
```

### Stage 5 — Run & Compare Against Baseline
Execute tasks through the existing isolated `host_driver` + `mcworker` harness (temp `repo/` checked out at `base_sha`, temp `HOME/`, `outside/` canary files, per-task budget cap). Record per-task and aggregate results in `evals/claude-history/results/<run_id>/summary.json`.

---

## 5. "Meet or Exceed" Metrics & Failure Taxonomy

A candidate model/harness configuration **meets or exceeds** the Opus/Fable baseline on a slice when:

1. **Pass Rate Parity:** `candidate_pass_rate >= baseline_pass_rate` on the stratified slice (or >=95% on tasks where baseline passed, with zero regressions on Bucket A/C).
2. **Cost & Latency Advantage:** `candidate_cost_usd <= baseline_cost_usd` (target: 5x–20x lower cost for Hydrogen/Carbon/local models) and `p95_wall_s <= baseline_p95_wall_s`.
3. **Tool Discipline:**
   - Lower or equal `failed_edit_rate` (exact-match `old` failures).
   - Zero fabricated completions (passing `command` graders without modifying or weakening tests).
   - Clean budget/turn adherence (no runaway loops).

### 5.1 Feedback Loop into Agent Design
Every failing candidate trajectory is tagged with a primary failure mode so improvements target the real bottleneck:

| Failure Tag | Symptom | Engineering Lever |
|---|---|---|
| `edit_mismatch` | Repeated failed `edits` due to stale line/whitespace | Improve read-before-edit guidance, fuzzy/line-range helpers |
| `premature_done` | Agent claims completion without running checks or uses stubs | Strengthen `/goal` & verification prompts, require command output |
| `search_bloat` | Reads whole files instead of `git grep` / ranged views | Prompt & tool-output truncation tuning |
| `compaction_drop` | Loses task constraints after context compaction | Tune `notes` / working memory & compaction summaries |
| `bad_delegation` | Redundant or underspecified subagent tasks | Refine `delegate` level routing & task-prompt templates |

---

## 6. Phased Rollout

1. **Phase 1 — Indexer & Bloat Filter:** Build `ingest` to scan `~/.claude/projects/` read-only, strip debug blobs, and emit a local session/episode index + summary report.
2. **Phase 2 — Miner, Git Anchor & Sanitizer:** Extract an initial **25–30 task stratified slice** across Buckets A–E from `work/openagents`, verify every task's grader passes on the baseline commit, and commit sanitized task JSONs to `evals/claude-history/tasks/`.
3. **Phase 3 — Harness Integration & Scorecard:** Wire `evals/claude-history/run.sh` to the `evals/coder-desktop` `host_driver` runner and produce the first comparison scorecard (Gemini 4 Hydrogen / Carbon / Neon vs. Opus/Fable baselines).
4. **Phase 4 — Continuous Improvement Loop:** Use failure tags from Phase 3 scorecards to ship targeted `mcworker` prompt/tool/compaction fixes and re-run to verify measurable gains.

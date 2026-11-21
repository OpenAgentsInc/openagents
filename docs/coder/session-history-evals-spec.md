# Session History Evals Specification (ATIF-v1.8)

**Status:** Proposed  
**Target:** `evals/session-history/` (ATIF-v1.8 extraction pipeline + hermetic task suite)  
**Related docs:** [`desktop-evals-spec.md`](desktop-evals-spec.md), [`evaluation-qualification.md`](evaluation-qualification.md), [`../../evals/openagents-desktop/README.md`](../../evals/openagents-desktop/README.md), [`../../nostr/nips/openagents/NIP-ATIF.md`](../../nostr/nips/openagents/NIP-ATIF.md)

---

## 1. Goals and Core Thesis

Local agent conversation histories across **Claude Code**, **OpenAgents Desktop**, **Antigravity**, **Codex**, and other local harnesses contain thousands of real engineering sessions driven by frontier models (`claude-opus-5-5`, `claude-fable-5-2`, `claude-haiku-5-5`, Gemini 4 Neon/Pro, GPT/Codex, etc.). Those trajectories represent **observed production baselines** on real repositories (`openagents`, `aethyr-cpp`, `gamebuilder`, etc.): real user prompts, real multi-file edits, real failure-recovery loops, and real verification commands.

This specification defines how to **normalize those heterogeneous histories into `ATIF-v1.8` (`NIP-ATIF`)**, then **mine, sanitize, materialize, and run** them as reproducible evaluations in `evals/openagents-desktop/` so that smaller, faster, or cheaper agents (e.g., `mcworker` on Gemini 4 Hydrogen/Carbon/Neon, OpenRouter models, or local Inkling/llama.cpp models) can be measured against observed frontier baselines and systematically improved to **meet or exceed** them.

### 1.1 Non-Goals

- Not a raw dump of local conversation histories (`~/.claude`, `~/.openagents`, `~/.gemini/antigravity`, `~/.codex`) into git — raw logs are tens of gigabytes and contain private paths, tokens, and debug blobs.
- Not a bespoke intermediate session schema — all ingested sessions standardize on **Harbor `ATIF-v1.8`** as carried by [`NIP-ATIF`](../../nostr/nips/openagents/NIP-ATIF.md).
- Not a subjective LLM-as-judge-only benchmark — every promoted eval must have **objective, deterministic graders** (exit codes, unit/integration tests, diff/seal invariants).
- Not a replacement for `evals/openagents-desktop/` tiers 0–4 — instead, it **feeds** real-world tasks into the same `host_driver` + `mcworker` harness.

---

## 2. Canonical Session Representation: `ATIF-v1.8` (`NIP-ATIF`)

Every supported source harness is converted read-only into **ATIF-v1.8** (`.atif.jsonl` append-only log form + `openagents.atif-manifest.v1` manifest, per [`NIP-ATIF`](../../nostr/nips/openagents/NIP-ATIF.md)) before episode mining, sanitization, or grading touches it.

Standardizing on `ATIF-v1.8` gives the eval pipeline:

1. **One downstream pipeline for every harness:** Miner, git-anchor resolver, sanitizer, and scorecard tools read `.atif.jsonl` only — never harness-specific formats.
2. **Native step, tool-call, observation, and metric modeling:** Each ATIF step carries `step_id`, `source` (`system` | `user` | `agent`), `message`, `reasoning_content`, `tool_calls`, `observation` (results), `model_name`, and `metrics` (prompt/completion/cached tokens, `cost_usd`).
3. **Subagent lineage out of the box:** Delegated transcripts map directly to ATIF `subagent_trajectories` / `subagent_trajectory_ref` and `NIP-ATIF` manifest `parent` / `children` links.
4. **Content-addressed provenance without leaking private bytes:** Each extracted eval task pins its source via `trajectory_id`, `step_range` (`[start_step_id, end_step_id]`), and `steps_digest` (ATIF sorted-key SHA-256).
5. **Honest disclosure & derivation tracking:** Converted local histories record `derivation: {"kind": "converted", "source": null}` and `coverage: "partial"` (per `NIP-ATIF`); sanitized excerpts record `derivation: {"kind": "redacted", "source": null}`. Trajectories stay private on disk under `~/.openagents/evals/session-history/` by default (`NIP-POL` / `NIP-SOV`).

---

## 3. Source Harnesses & Existing Readers to Reuse

Per `AGENTS.md` ("Never hand-roll what already exists"), Stage 1 converters reuse the repo's existing Coax observation and journal modules rather than writing parallel parsers:

| Source Harness | Local Root(s) | Native Format | Existing Coax Module(s) to Reuse | Bloat / Filtering Rules |
| --- | --- | --- | --- | --- |
| **Claude Code** | `~/.claude/projects/<project>/<sessionId>.jsonl` and `<sessionId>/subagents/*.jsonl` | JSONL | `coax/lib/agent/observe/claude-history.coax`, `claude-files.coax`, `transcripts.coax` | Keep `user`, `assistant`, `system`, `tools-used`, `cost-state`, `custom-title`, `ai-title`. Drop `api-request-blob`, `api-request`, `api-request-shape`, `file-history-snapshot`, `file-history-delta`, `queue-operation`, `atis-latch`. Replace inline base64 images with `[image:sha256]`. |
| **OpenAgents Desktop** | `~/.openagents/coder/sessions/*.ajr` (and `*.jsonl`) | `AJR1` binary-framed journal / JSONL | `coax/lib/agent/journal.coax`, `coax/lib/agent/observe/transcripts.coax` | Decode `AJR1` frames into ATIF `user`/`agent` steps, tool calls, results, worktree `cwd`, model, and cost metadata. |
| **Antigravity** | `~/.gemini/antigravity/brain/<id>/.system_generated/logs/transcript.jsonl` and `~/.gemini/antigravity-cli/brain/<id>/.system_generated/logs/transcript.jsonl` | JSONL (`transcript.jsonl` + `transcript_full.jsonl`) | `coax/lib/agent/observe/antigravity.coax`, `transcripts.coax` (extend walk to include both `antigravity/brain` and `antigravity-cli/brain`) | Read `transcript.jsonl`; fetch corresponding line from `transcript_full.jsonl` only when `truncated_fields` is present. Map `USER_INPUT` → `user`, `PLANNER_RESPONSE` → `agent` (with `tool_calls` and `thinking`), `GENERIC` tool outputs → `observation`. |
| **Codex** | `~/.codex/sessions/YYYY/MM/DD/*.jsonl` | JSONL | `coax/lib/agent/observe/transcripts.coax` | Map user prompts, assistant turns, function/shell calls, and tool outputs into ATIF-v1.8 steps. |
| **OpenCode / Other** *(optional)* | `~/.local/share/opencode/`, etc. | Local store | Pluggable adapter | Same contract: emit valid `ATIF-v1.8` `.atif.jsonl` + manifest. |

Existing ATIF carriage utilities live in `coax/lib/nostr/atif/chunks.coax`; shared `ATIF-v1.8` log writer/reader and sorted-key `steps_digest` helpers should live in `coax/lib/nostr/atif/` so both `evals/session-history/` and OpenAgents Desktop share one implementation.

---

## 4. Repository & Local Directory Structure

Large raw ATIF caches and local indexes live outside git under `~/.openagents/evals/session-history/`. Only lightweight extraction tooling, schemas, sanitized task definitions, and minimal fixtures are committed to git:

```text
evals/session-history/
  README.md                 # Usage, commands, and baseline summary
  run.sh                    # Entrypoint: ingest, mine, sanitize, run, compare
  schema/                   # Task & baseline metric schemas (ATIF provenance + openagents-desktop task)
  tools/                    # Coax tools (or Firm-reasoned scripts per AGENTS.md)
    ingest.coax             # Read-only source adapters -> local ATIF-v1.8 logs + index
    mine.coax               # Score & select candidate episodes from ATIF-v1.8 logs
    sanitize.coax           # Secret/PII scrubbing & hermeticity gate
  tasks/                    # Committed sanitized eval task JSONs (openagents-desktop tier2/tier4 compatible)
    <task_id>.json
  fixtures/                 # Minimal fixtures for non-git-pinned tasks (if any)
  results/                  # Gitignored run outputs & comparison reports
```

Local private store (never committed):

```text
~/.openagents/evals/session-history/
  atif/<source>/<trajectory_id>.atif.jsonl
  atif/<source>/<trajectory_id>.manifest.json
  index.jsonl               # (or index.sqlite) keyed by (source, project, trajectory_id)
  inventory.json            # Per-source counts, models, projects, and anchorable episodes
```

> **Tooling rule (`AGENTS.md`):** Prefer Coax for new tools; any non-Coax script (`.py`, `.sh`) in `evals/session-history/` must include a valid `Firm reason: N (...)` header and pass `coax/web/tool.sh tools-check`.

---

## 5. Extraction & Curation Pipeline

The pipeline has five stages:

### Stage 1 — Ingest, Convert to `ATIF-v1.8` & Index (Read-Only)
Walk configured source roots read-only via `transcripts.coax` and each source's adapter, emitting local `ATIF-v1.8` `.atif.jsonl` logs, manifests (`derivation.kind = "converted"`, `coverage = "partial"`), an index keyed by `(source, project, trajectory_id)`, and an `inventory.json` summary. Index fields:
- Source harness (`claude`, `openagents`, `antigravity`, `codex`), primary model, and subagent models
- Step count, total turns, tool call counts (normalized by role: read, edit/write, search, shell, delegate), failed tool calls, and token/cost totals
- `cwd`, `git_branch`, start/end timestamps, and observed git SHAs or verification commands in shell tool calls

### Stage 2 — Episode Mining & Stratification (Over `ATIF-v1.8`)
A session often contains multiple distinct user tasks. Split ATIF trajectories into **episodes** (contiguous `[start_step_id, end_step_id]` ranges from a substantive `user` step to completion or the next unrelated `user` step) and score them for eval suitability.

**Inclusion criteria (must satisfy all):**
1. **Anchorable starting state:** Either `cwd` maps to a git repo with an identifiable `base_sha` (resolved via timestamp/branch/first-commit-after-prompt or recorded `git rev-parse`), or the task can be packaged as a small self-contained fixture.
2. **Verifiable outcome:** The episode contains an objective check — e.g., a test command (`coax test`, `pytest`, `unittest`, `cargo test`, `tools-check`), a compiler/build check, or a clean target diff/commit.
3. **Substantive work:** At least 2 tool calls and `>= 1` file edit or diagnostic command (excludes `/status`, `/model`, `/fast`, and pure Q&A).

**Exclusion criteria:**
- Tasks requiring live external secrets, production DBs, or interactive GUI clicks that cannot run headlessly.
- Unreproducible local-only scratch directories whose files no longer exist.

**Stratification buckets (so weaker models are tested across the real failure surface):**
- **Bucket A — Targeted Bugfix & Edit Precision:** 1–3 file fixes where smaller models often fail `old`/`new` exact-match edits.
- **Bucket B — Multi-File Feature / Refactor:** 4+ file changes requiring search-before-read and plan discipline.
- **Bucket C — Verification & "Done Means Real":** Tasks where the baseline ran real end-to-end checks (not stubs/mocks) and caught subtle regressions.
- **Bucket D — Error Recovery & Debugging:** Episodes where the baseline hit a failing test/build, diagnosed it from output, and pivoted.
- **Bucket E — Long-Context / Compaction & Subagent Delegation:** Episodes with >20 turns or subagent fan-out (`subagent_trajectory_ref` / `children`).

### Stage 3 — Sanitization & Hermeticity Gate
Before any mined episode is written to `evals/session-history/tasks/`:
1. **Secret & PII scrub:** Scan prompts, fixtures, and expected outputs for API keys, bearer tokens, private keys, Nostr `nsec`s, emails, and user-specific absolute paths (`/Users/<name>/...` -> relative repo paths). Mark any exported trajectory excerpt `derivation: {"kind": "redacted", "source": null}`.
2. **Path normalization:** Rewrite `cwd` references to the isolated eval worktree root.
3. **Baseline verification:** Verify the baseline's final state passes the extracted grader commands on `base_sha` + baseline patch (confirming the eval is actually solvable and the grader is not broken).

### Stage 4 — Materialize in `openagents-desktop` Task Format
Each curated task is stored as JSON compatible with `evals/openagents-desktop/tier2` and `tier4/golden-template.json`, extended with ATIF `source` provenance and a `baseline` block:

```json
{
  "id": "sh_001_example_fix",
  "category": "session-history",
  "bucket": "A_edit_precision",
  "source": {
    "harness": "claude",
    "trajectory_id": "...",
    "step_range": [1, 14],
    "steps_digest": "<64-hex-atif-sorted-sha256>",
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
Execute tasks through the existing isolated `evals/openagents-desktop/` `host_driver` + `mcworker` harness (temp `repo/` checked out at `base_sha`, temp `HOME/`, `outside/` canary files, per-task budget cap). Record per-task and aggregate results in `evals/session-history/results/<run_id>/summary.json`, grouped by both `bucket` and `source.harness` / `baseline.model`.

---

## 6. "Meet or Exceed" Metrics & Failure Taxonomy

A candidate model/harness configuration **meets or exceeds** the observed baseline on a slice when:

1. **Pass Rate Parity:** `candidate_pass_rate >= baseline_pass_rate` on the stratified slice (or `>= 95%` on tasks where baseline passed, with zero regressions on Bucket A/C).
2. **Cost & Latency Advantage:** `candidate_cost_usd <= baseline_cost_usd` (target: 5x–20x lower cost for Hydrogen/Carbon/local models) and `p95_wall_s <= baseline_p95_wall_s`.
3. **Tool Discipline:**
   - Lower or equal `failed_edit_rate` (exact-match `old` failures).
   - Zero fabricated completions (passing `command` graders without modifying or weakening tests).
   - Clean budget/turn adherence (no runaway loops).

### 6.1 Feedback Loop into Agent Design
Every failing candidate trajectory is tagged with a primary failure mode so improvements target the real bottleneck:

| Failure Tag | Symptom | Engineering Lever |
| --- | --- | --- |
| `edit_mismatch` | Repeated failed `edits` due to stale line/whitespace | Improve read-before-edit guidance, fuzzy/line-range helpers |
| `premature_done` | Agent claims completion without running checks or uses stubs | Strengthen `/goal` & verification prompts, require command output |
| `search_bloat` | Reads whole files instead of `git grep` / ranged views | Prompt & tool-output truncation tuning |
| `compaction_drop` | Loses task constraints after context compaction | Tune `notes` / working memory & compaction summaries |
| `bad_delegation` | Redundant or underspecified subagent tasks | Refine `delegate` level routing & task-prompt templates |

---

## 7. Phased Rollout

1. **Phase 1 — `ATIF-v1.8` Converters & Local Index:** Build `ingest.coax` (reusing `transcripts.coax`, `claude-history.coax`, `journal.coax`, and `antigravity.coax`) to convert Claude Code, OpenAgents Desktop, Antigravity, and Codex sessions read-only into local `ATIF-v1.8` `.atif.jsonl` logs + `inventory.json`.
2. **Phase 2 — Vertical Slice Proof (1 Task per Source):** Prove the complete end-to-end loop on 3 real tasks (1 Claude, 1 OpenAgents Desktop, 1 Antigravity): sanitize → verify baseline patch passes grader at `base_sha` → run via `evals/openagents-desktop` `host_driver` + `mcworker` → emit comparison scorecard.
3. **Phase 3 — Stratified 25–30 Task Suite & Scorecard:** Scale Stage 2 mining across Buckets A–E from `work/openagents`, verify every task's grader passes on the baseline commit, commit sanitized task JSONs to `evals/session-history/tasks/`, and produce the first multi-model comparison scorecard.
4. **Phase 4 — Continuous Improvement Loop:** Use failure tags from Phase 3 scorecards to ship targeted `mcworker` prompt/tool/compaction fixes and re-run to verify measurable gains.

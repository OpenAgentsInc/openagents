<!-- GitHub issue #13706 -->
# Context Compaction Theory & Architecture for OpenAgents Coder

This document audits how OpenAgents Microcoder and Coder Desktop currently manage context growth and multi-turn recall, compares our approach with **OpenAI Codex** (`openai/codex`) and **T3 Code** (`pingdotgg/t3code`), formulates a compaction theory for OpenAgents Coder, and specifies the deterministic and long-horizon evaluations in `evals/coder-desktop/compaction/`.

---

## 1. Audit: What OpenAgents Coder Does Today

OpenAgents splits context management across three layers: the **in-run step log** (`coax/lib/microcoder/state.coax`), **structured working memory** (`coax/lib/agent/workmem/`), and **cross-turn recall** (`MWW_RECALL` across `apps/coder-desktop/page.coax`, `coder/tui/mcworker.coax`, and `coder/tui/microcoder_keys.coax`).

### 1.1 Dual-Log Deterministic Compaction (`coax/lib/microcoder/state.coax`)

Microcoder does **not** use an LLM call to summarize past steps. Instead, every action is written simultaneously to two fixed-size buffers inside `McStateBuffers`:

| Constant | Value | Role |
| --- | --- | --- |
| `MC_LOG_BYTES` | `147,456` (144 KB) | Full recent steps (`log`) and parallel pre-compacted mirror (`clog`) |
| `MC_ACTION_ROOM` | `49,152` (48 KB) | Minimum free space in `log` before `mc_action_begin` triggers compaction |
| `MC_KEEP_RECENT` | `24,576` (24 KB) | Target bytes of full recent steps kept after `mc_compact` |
| `MC_EARLIER_BYTES` | `40,960` (40 KB) | Ring-like buffer of compacted older steps (`earlier`) |
| `MC_MAX_ACTIONS` | `512` | Hard cap on step count in `log` before forced compaction |
| `MC_STEP_BUDGET` | `32,768` (32 KB) | Per-step result budget so one noisy step cannot flush prior steps |
| `MC_OUTPUT_HEAD` / `TAIL` | `6,000` / `6,000` B | Full-step command output head/tail window |
| `MC_BRIEF_RATIONALE` | `1,200` B | Rationale bytes preserved per step in `clog` / `earlier` |
| `MC_BRIEF_OUTPUT` | `160` B | Command output head/tail bytes preserved per step in `clog` / `earlier` |
| `MC_FACTS_BYTES` | `131,072` (128 KB) | Environment JSON + cross-turn recall (`mk_facts`) |
| `MC_PROMPT_BYTES` | `573,440` (560 KB) | Maximum serialized `mc_state_json` output buffer |

**Compaction trigger (`mc_action_begin`, lines 196–200):**
When starting a new step, if `nacts >= MC_MAX_ACTIONS` or `log.count - nlog < MC_ACTION_ROOM`, Microcoder calls `mc_compact(st, MC_KEEP_RECENT)`. If `nacts` is still at `MC_MAX_ACTIONS`, it calls `mc_compact(st, 0)`.

**What `mc_compact(st, keep)` does (lines 169–186):**
1. Finds the smallest prefix `k` of closed steps such that the remaining bytes in `log` are `<= keep` (never compacting the currently open step).
2. Copies the pre-formatted compacted bytes `clog[0..<ccut]` onto the tail of `earlier`.
3. If `earlier` would overflow `MC_EARLIER_BYTES`, calls `mc_earlier_drop(st, earlier.count / 2 - ccut)` (falling back to `0`), dropping the oldest compacted steps in one block at a `,{"step":` boundary and incrementing `left_out`.
4. Shifts `log`, `clog`, `act_at`, and `cact_at` down by `cut` / `ccut` and increments `compactions`.

**What survives compaction in `clog` / `earlier`:**
- **Rationales**: kept up to `MC_BRIEF_RATIONALE` (1,200 B). The system prompt explicitly instructs the model to write file paths, line numbers, and facts it will need later into `rationale`.
- **Edits**: applied edits keep path + line range (new text body is dropped); failed edits keep path + failure marker (nearest-region context is dropped).
- **Commands**: keep command string, exit code (or refusal/timeout), and `MC_BRIEF_OUTPUT` (160 B) of output head and tail.
- **Views**: normal file views drop their body and keep only `{view: path, bytes: N}`; missing/errored views keep their error status.

**Provider overflow fallback (`coax/lib/microcoder/loop.coax:514`):**
If the LLM provider rejects a request for exceeding context length, the loop calls `mc_compact(l[0].st[..], 0)` to flush all closed steps into `earlier` and retries.

### 1.2 Prompt-Cache Prefix Layout (`mc_state_json`, lines 477–501)

Microcoder serializes the entire state as a single JSON object in a deliberate order designed for KV-cache prefix reuse:

```text
{"environment":..., "task":..., "route":...,
 ["left_out":N,] ["earlier":[...],] "actions":[...],
 ["memory":{"findings":...,"changed":...,"open":...},] "notes":...}
```

- **Immutable head**: `environment`, `task`, and `route` never change within a run.
- **Append-only `earlier`**: between rare `mc_earlier_drop` evictions, new compacted blocks are appended strictly at the end of `earlier`. Previously compacted steps stay byte-for-byte identical.
- **Append-only `actions`**: between compactions, new steps are appended strictly at the end of `actions`.
- **Mutable tail (`memory` + `notes`)**: `memory` (populated from `coax/lib/agent/workmem/` via `mc_memory_set`, capped at `4000` B findings / `2000` B changed / `1500` B open) is intentionally placed **after** `actions` so that updating findings, edits, or open requirements on every step never invalidates the cached prefix of `earlier` + `actions`.

### 1.3 Working Memory (`coax/lib/agent/workmem/`)

`workmem` maintains an append-only, hash-addressed evidence store on disk (`file`, `read`, `finding`, `change`, `run`, `check`, `requirement`) with automatic staleness detection via SHA-256 content hashes and a tree digest (`wm_tree`). Its projected views (`findings`, `changed`, `open`) are injected into `state.memory` and are **never compacted**—even after the originating steps are evicted from `earlier` (`left_out > 0`).

### 1.4 Cross-Turn Recall (`MWW_RECALL`)

Across user turns in a desktop or TUI session:
- `coax/lib/agent/engine/mcworker-wire.coax:43` defines `MWW_RECALL :: 17`, carrying `{"recall":[{"task":T,"reply":R}]}`.
- `apps/coder-desktop/page.coax:1374` (`coder_page_recall_line`) serializes prior turns into a `MWW_RECALL` frame. However, the caller at `apps/coder-desktop/page.coax:2803` allocates a fixed **`out: [4096] u8`** buffer (4 KB), which silently truncates or drops older/longer turns long before hitting Microcoder's `MC_FACTS_BYTES` (128 KB) limit.
- `coder/tui/mcworker.coax:204` parses `MWW_RECALL` into `MkTurn` slots (up to `MK_MAX_TURNS = 16` in `coder/tui/microcoder_keys.coax`), and `mk_facts` embeds them into `environment.earlier` before the run starts.

### 1.5 Concrete Gaps Identified in the Audit

1. **First-compaction and `left_out` key insertion**: `left_out` and `earlier` are omitted when zero. When `nearly` transitions `0 -> >0`, `,"earlier":[` is inserted before `,"actions":[`; when `left_out` transitions `0 -> >0` or increments, `,"left_out":N` mutates the prefix *before* `earlier`, invalidating all of `earlier` at once (though `earlier` already shifts when `mc_earlier_drop` runs).
2. **No semantic checkpoint before `left_out` eviction**: when `earlier` exceeds 40 KB, its oldest half is discarded permanently. Unless the model wrote durable notes into `workmem` (`findings`/`changed`/`open`), rationale details from those steps are gone.
3. **4 KB desktop recall bottleneck**: `apps/coder-desktop/page.coax:2803` caps `coder_page_recall_line` at 4,096 bytes despite `MC_FACTS_BYTES` supporting 128 KB, starving multi-turn desktop conversations of prior turn context.
4. **Byte-only trigger, no model-window awareness**: `MC_LOG_BYTES` (144 KB) and `MC_EARLIER_BYTES` (40 KB) are static compile-time constants regardless of whether the active model has a 64K, 200K, or 1M token window.
5. **No omission manifest**: `left_out` reports only an integer count of evicted steps, not which files/edits/decisions were in those steps.

---

## 2. Comparative Study: OpenAI Codex & T3 Code

### 2.1 OpenAI Codex (`openai/codex`)

Codex treats compaction as a **token-budgeted, semantic checkpointing operation** with two backends (local LLM summarization and server-side Responses compaction):

- **Token-budget auto-compaction (`codex-rs/core/src/state/auto_compact_window.rs`, `codex-rs/core/src/compact_token_budget.rs`)**:
  Tracks context occupancy using server-observed `input_tokens` (`AutoCompactWindowPrefill::ServerObserved(usage.input_tokens.max(0))`) tied to a `ContextWindowId`. Runs pre- and post-compaction lifecycle hooks (`PreCompactHookOutcome`, `PostCompactHookOutcome`) that can inspect or abort compaction.
- **Local LLM handoff compaction (`codex-rs/core/src/compact.rs`, `codex-rs/prompts/templates/compact/prompt.md`, `summary_prefix.md`)**:
  When triggered (manually or automatically), Codex runs a dedicated summarization turn using `SUMMARIZATION_PROMPT` (*"CONTEXT CHECKPOINT COMPACTION. Create a handoff summary for another LLM that will resume the task..."*). `build_compacted_history` then reconstructs the conversation history by:
  1. Retaining the most recent user messages up to a token cap (`annotated_user_messages` walked in reverse up to `max_tokens`),
  2. Inserting the LLM summary wrapped in `SUMMARY_PREFIX`, and
  3. Installing a fresh turn context and `world_state_baseline`.
- **Remote Responses API compaction (`codex-rs/core/src/compact_remote_v2.rs`, `compact_remote_history.rs`)**:
  Delegates compaction to OpenAI's Responses endpoint (`CompactionImplementation::ResponsesCompactionV2`), which returns opaque encrypted compaction items while retaining selected client items (`should_retain_client_item` / `compact_retained_tool_outputs.rs`) and tracking unclamped token totals.
- **Tradeoffs**: High semantic fidelity across very long horizons and exact model-window awareness, at the cost of an extra LLM round-trip (latency + token cost) and a full KV-cache prefix reset whenever history is replaced by a summary.

### 2.2 T3 Code (`pingdotgg/t3code`)

T3 Code is a multi-provider orchestration layer (driving Claude Code, Codex, etc.), so it separates **native provider compaction scheduling** from **cross-provider / thread-switch context handoff**:

- **Idle & resume compaction orchestration (`apps/server/src/orchestration-v2/IdleCompaction.ts`, `packages/shared/src/claudeCompaction.ts`, `apps/web/src/components/chat/compaction.logic.ts`)**:
  Delegates in-session compaction to the underlying provider, but actively manages *when* to suggest or trigger it. Sets `COMPACTION_SUGGESTION_MIN_TOKENS = 200_000` (matching Claude Code's idle compaction threshold), tracks cache-expiry lead windows (`IdleCompaction.ts`), and prompts users on stale-session resume (`"This session is X old and uses Y tokens. Compact it before continuing?"`).
- **Budgeted deterministic context handoff (`apps/server/src/orchestration-v2/ContextHandoffService.ts`, `packages/provider-core/src/server/handoffBudget.ts`)**:
  When handing a thread to a different provider or fresh native thread, T3 Code computes a pessimistic byte/token budget:
  - Caps imported history at `DEFAULT_HANDOFF_TOKEN_CAP = 16_000` (hard `HANDOFF_BYTE_CAP = 64_000`).
  - Computes `handoffBudget = min(tokenCap, HANDOFF_BYTE_CAP, window - native - current - workReserve)`, where `window = min(modelContextWindow, maxTokens, autoCompactThreshold)` and `workReserve = clamp(window / 4, 16_000, window / 2)`.
  - Accounts for attachment overhead (`8,192` tokens per image, `4,096` per other attachment) and fixed system/MCP overhead (`apps/server/src/provider/agentContextBudget.json`: ~6.2 KB Claude append, ~7.6 KB Codex context, ~57.3 KB MCP tool schemas).
  - `selectHistory` selects intact historical items (`user_message`, `assistant_message`, `command_execution`, `file_change`, `proposed_plan`, `error`) without mid-item truncation, while reserving up to 1/8 of the budget for an explicit **Omitted item IDs** manifest (`MAX_LISTED_OMITTED_ITEMS = 20`) so the receiving model knows *what* was omitted and that historical items are context rather than new instructions.

### 2.3 Side-by-Side Comparison

| Dimension | OpenAgents Microcoder | OpenAI Codex | T3 Code |
| --- | --- | --- | --- |
| **Primary mechanism** | Dual-log structural compaction (`log` -> `clog` -> `earlier`) + `workmem` | Local LLM checkpoint summary or Responses API remote compaction | Native provider compaction + budgeted deterministic handoff |
| **Compaction cost / latency** | **Zero** (deterministic in-memory shift, < 1 ms) | 1 full LLM call (or server-side compaction API) | Zero for handoff; provider-native for in-session |
| **Trigger** | Static byte/step thresholds (`MC_ACTION_ROOM`, `MC_MAX_ACTIONS`) + overflow retry | Server-observed `input_tokens` vs. model window | `COMPACTION_SUGGESTION_MIN_TOKENS` (200K), cache expiry, or provider switch |
| **KV-cache friendliness** | **Very high** — append-only `earlier` and `actions`; mutable `memory` placed after `actions` | Resets prefix on compaction (amortized over long windows) | Aware of cache TTL (`IdleCompaction.ts`) to compact right before cache expiry |
| **Evidence / failure fidelity** | High for rationales (1.2 KB), edits, exit codes; views drop to `{view, bytes}` | Depends on LLM summary quality + retained user messages/tool outputs | Selects intact items; explicit `Omitted item IDs` manifest |
| **Multi-turn / cross-session** | `MWW_RECALL` (currently bottlenecked at 4 KB in Desktop) | Persistent thread rollout + compaction items | `handoffBudget` (16K tokens / 64 KB cap) with explicit omission accounting |

---

## 3. Compaction Theory & Recommended Architecture for OpenAgents Coder

### 3.1 Five Core Invariants

A coding agent's context manager should satisfy five invariants:

1. **Prefix Monotonicity (Cache-First Layout)**: Every byte before the active append point must remain byte-for-byte identical between turns and across compactions whenever possible. Mutable summaries or working-memory projections belong **after** append-only logs, never before them.
2. **Lossless Evidence, Lossy Transcript**: Raw file views and verbose stdout are ephemeral and safe to collapse to `{path, bytes}` or `head/tail` snippets; *evidence* (which files were edited, what failed, what requirements remain open, verified findings) must live in a hash-verified, non-compacted store (`workmem`).
3. **Bounded Per-Step Blast Radius**: No single tool output or file read may evict the rest of the working set (`MC_STEP_BUDGET = 32 KB`).
4. **Explicit Omission Accounting**: Whenever context is truncated, held back, or evicted (`left_out`), the model must see *what* was omitted and *how* to recover it (line range, saved output file path, or omitted step/file manifest)—never silent data loss.
5. **Zero-Cost Default, Semantic Escalation Only at Eviction**: Structural compaction (`log` -> `earlier`) should remain free, deterministic, and instantaneous. LLM-generated semantic checkpointing should only be considered when `earlier` itself would otherwise evict un-captured history (`mc_earlier_drop`), or when a user explicitly invokes `/compact`.

### 3.2 Tiered Memory Hierarchy

OpenAgents Coder should explicitly treat context as four tiers:

1. **Tier 1 — Hot Window (`actions`, <= 144 KB, kept to 24 KB on compact)**: Full step results, capped at 32 KB/step.
2. **Tier 2 — Warm Window (`earlier`, <= 40 KB)**: Deterministic compacted steps (`clog`): 1.2 KB rationale, edit line ranges, command exit codes + 160 B head/tail, view byte counts. Appended in rare blocks to preserve KV-cache hits.
3. **Tier 3 — Durable Evidence (`memory` from `workmem`, <= 7.5 KB)**: Hash-verified `findings`, `changed`, and `open` views rendered *after* `actions`. Never compacted; survives `left_out > 0`.
4. **Tier 4 — Cross-Turn Recall (`MWW_RECALL`, target 16–32 KB budget)**: Prior conversation turns embedded in `environment.earlier` via `mk_facts`, budgeted using T3 Code's pessimistic byte-budget + intact-item selection + omission count.

### 3.3 Concrete Recommended Improvements

| Priority | Component | Recommendation |
| --- | --- | --- |
| **P0** | `apps/coder-desktop/page.coax:2803` | Raise `coder_page_recall_line`'s output buffer from `4096` B to `32768` B (or stream via dynamic buffer) and apply intact-turn selection with an explicit omitted-turn count, matching `handoffBudget.ts` discipline. |
| **P1** | `coax/lib/microcoder/state.coax` | Keep `workmem` (`mc_memory_on`) enabled by default in Desktop/TUI sessions so `findings`/`changed`/`open` always survive `earlier` eviction (`left_out > 0`). |
| **P1** | `coax/lib/microcoder/state.coax` | Add an explicit omission summary when `mc_earlier_drop` runs (e.g., step range `steps 1..k omitted`) instead of only an opaque `left_out` integer. |
| **P2** | `coax/lib/microcoder/loop.coax` | Track provider-reported `input_tokens` against the active model's context window (like Codex's `AutoCompactWindowPrefill::ServerObserved`) to scale `MC_ACTION_ROOM` / `MC_KEEP_RECENT` dynamically and avoid reactive overflow retries. |
| **P2** | `apps/coder-desktop/` | Adopt T3 Code's cache-TTL awareness (`IdleCompaction.ts`): surface a non-intrusive "Compact & continue" chip when resuming a stale session whose KV cache has expired and whose context exceeds a configurable threshold. |
| **P3** | `coax/lib/microcoder/loop.coax` | Optional semantic checkpoint hook right before `mc_earlier_drop` (or on user `/compact`) that distills evicted steps into a single `workmem` checkpoint note, gated behind a config flag so default runs stay zero-cost. |

---

## 4. What We Should Evaluate

To prevent regressions and ground future compaction changes in measurements, `evals/coder-desktop/compaction/` implements deterministic checks covering six dimensions:

1. **Step-to-step prefix stability**: Between compactions, step `N`'s prompt starts with step `N-1`'s exact bytes up to the closing `actions` bracket (KV-cache hit invariant).
2. **Cross-compaction prefix stability**: Across multiple compactions (before `earlier` eviction), `environment` + `task` + `route` + previously compacted `earlier` entries remain a byte-for-byte prefix.
3. **Mutable `memory` cache isolation**: Updating `findings`, `changed`, and `open` via `mc_memory_set` on every step never alters any byte before `,"memory":`.
4. **Compaction fidelity**: Rationales (up to 1,200 B), applied edit line ranges, failed edits, non-zero command exits + head/tail snippets, refused commands, and missing-file errors survive compaction; large normal file views collapse to `{view, bytes}`.
5. **Long-horizon multi-compaction + eviction resilience**: Over 40+ steps triggering `>= 3` compactions and `earlier` eviction (`left_out > 0`), every state renders valid JSON, step numbers stay strictly monotonic, `mc_compact(st, 0)` overflow recovery works cleanly, and `workmem`-backed `memory` preserves critical edits/open items even after their originating steps leave `earlier`.
6. **Multi-turn `MWW_RECALL` wire & budget behavior**: `mww_recall` / `mww_parse` round-trips multi-turn payloads with JSON-escaped content cleanly and enforces buffer bounds without partial/corrupt JSON frames.

Run the suite directly via:

```sh
evals/coder-desktop/compaction/run.sh
```

or as part of the full Coder Desktop evaluation harness:

```sh
evals/coder-desktop/run.sh --tier 1
```

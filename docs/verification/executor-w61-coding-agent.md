# Executor W6.1 Verification Receipt — Microcoder, OpenAgents Desktop & Universal MCP

- **Issue:** GitHub #13924 (Executor W6.1: First-Class Executor & Compiled Weight-Module Integration in Microcoder, OpenAgents Desktop & MCP/ACP)
- **Date:** 2026-11-21
- **Hardware:** Apple Silicon Mac (M5 Max, 64 GB unified memory, macOS arm64, Metal 4)
- **Base Local Model:** `~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf` (17,627,940,768 bytes, 16.4 GB IQ4_NL GGUF served by native Psionic Metal 4 `psionic-inkling-serve`)
- **Compiled Weight Store:** `~/.openagents/modules/` (W2.2 Class-E dense F64 `exec-vm-v1` safetensors artifacts):
  - `oa-exec-diff-verify` — `606049b83f17f20a0fc7f3f6e6278f740f8dc9f0221ead98a79b195b155b9c6a` (1,221 F64 weights)
  - `oa-exec-json-schema` — `5293cf7460beeb58025291821a20f99c4f811b11cdcaf62dc117dd6172a29895` (1,908 F64 weights)
  - `oa-exec-dep-sched` — `7a821819a3ad14b583e456c8d909954e27b90d5a3dae7406027e6fbaf06b58f8` (972 F64 weights)
  - `oa-exec-arith-budget` — `a93a4b53a4a5a3319cd4d5a55a6168029b95ffa782fc50119c8eef55366ae2d5` (1,188 F64 weights)
  - `oa-exec-schema-diff-linked` — `f595062a4577366332225ac5c192207d561aeca7e7fe4d45a2b537a2eecd6454` (4,221 F64 weights, composite linked)

---

## 1. Pillar 1 — Microcoder Integration (`coax/lib/microcoder/` + `coder/tui/`)

### What Landed
- `#authority` `coax/lib/microcoder/executor-guard.coax`: executes compiled W2.2 Class-E dense F64 weight modules (`oa-exec-diff-verify`, `oa-exec-json-schema`, `oa-exec-dep-sched`, `oa-exec-arith-budget`) behind W3.1 `hx_admit_call` bounds, computes Ch 234 LE32 SHA-256 replayable trace digests, provides independent replay verification (`mc_exec_verify_replay`), and formats compact replay-verification pills (`[E] replay verified · sha256:<12hex>` / `[E] replay refused · sha256:<12hex>`).
- `coax/lib/microcoder/tools.coax`: pre-verifies structured edit arguments (`oa-exec-json-schema`) and hunk line bounds / context uniqueness (`oa-exec-diff-verify`) in `mc_edit_file` before any disk write, preserving exact `mc_edit_file` signature and refusal text while attaching typed `McExecCode` and Ch 234 digests.
- `coax/lib/microcoder/run.coax`: appends `\ [E] replay verified · sha256:<12hex>` after unified diffs (and `[E] replay refused · sha256:<12hex>` on failed edits) so OpenAgents Desktop renders replay-verification pills directly on edit rows without disturbing diff stats.
- `coder/tui/microcoder_keys.coax` & `coder/tui/mcworker.coax`: exposes `hybrid-exec-v1` / `hybrid-executor` as a first-class local Psionic model route (`mk_is_hybrid_model`, `mk_plan_local_for`) with hybrid-executor instructions.

### Paired Multi-File Edit Benchmark — Part A (Deterministic 24-Case Suite on 6 Real Repository Files)
Command: `coax/web/tool.sh coax/lib/microcoder/executor-bench.coax`

Targets: real copies of `psionic/lib/executor-desktop.coax`, `psionic/lib/hybrid-exec-policy.coax`, `psionic/lib/module-v2-policy.coax`, `coax/lib/microcoder/executor-guard.coax`, `coax/lib/microcoder/tools.coax`, and `apps/openagents-cli/executor-mcp.coax`.

| Metric | Result |
| --- | --- |
| Valid multi-file edits applied cleanly | **12 / 12** (`6,201 us` total, `517 us/edit` incl. disk I/O + weight eval + SHA-256) |
| Malformed LLM edit traps (stale context, whitespace drift, duplicate anchors) intercepted pre-disk | **8 / 8** (`5,568 us` total) |
| Target files bit-identical after refused edits (verified by SHA-256) | **8 / 8 (100%)** |
| Extra Class-E traps (over-cap schema, OOB past EOF, DAG cycle, over-budget) refused with typed codes | **4 / 4 (100%)** |
| Independent Ch 234 LE32 SHA-256 replay verification rate | **24 / 24 (100.0%)** |

#### Native CPU Reference vs. Compiled Dense F64 Weight-Core Timing (Master Spec Rule #1)
Measured on Apple M5 Max arm64 (`-O2`, observable accumulator sink verified identical `sink=1220000`):

| Path | Iterations | Total Wall Time | Per-Call Latency |
| --- | --- | --- | --- |
| Native Coax CPU reference (`mv_ref_diff_verify`) | 10,000 | `< 1 us` | `< 0.1 ns/call` |
| Compiled dense F64 weight-core eval (`mv_eval_model`, 1,221 F64 weights) | 10,000 | `5,415 us` | `541 ns/call` (`0.54 us/call`) |
| Full Microcoder guard (`mc_exec_check_diff`: compile + dense F64 eval + Ch 234 SHA-256) | 1,000 | `6,515 us` | `6.52 us/call` |

The compiled dense F64 weight core is ~5,400x slower than native hand-written C/Coax branches on scalar CPU (0.54 us vs. < 0.1 ns), as expected and documented under Master Spec Rule #1, while adding only `6.52 us` per edit — ~80x faster than file I/O (`517 us/edit`) and ~10,000,000x faster than a local LLM turn — in exchange for digest-pinned weight execution and 100% bit-for-bit Ch 234 replay receipts.

### Paired Multi-File Coding Benchmark — Part B (Real Live Metal 4 Inference on `Inkling-2-Mini-IQ4_NL.gguf`)
Server: `OpenAgents.app/Contents/Resources/psionic-inkling-serve --model ~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf --metal .../inkling-kernels.metal --ctx 8192`
Worker: compiled `OpenAgents.app/Contents/Resources/mcworker` editing copies of `executor-desktop.coax` and `hybrid-exec-policy.coax` in isolated git checkouts.

| Run | Model ID | Gateway | Wall Time | Attempts | Input Tokens | Output Tokens | Total Tokens | Structured Edits | Ending | `git diff` Correct | Ch 234 Replay Receipts |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Baseline (local) | `inkling-2-mini` | `psionic` | `69.15 s` | 5 | 33,875 | 588 | 34,463 | 0 (used 4 shell cmds) | `bad_replies` | Yes (2/2 files) | 0 |
| Hybrid (unconstrained prompt) | `hybrid-exec-v1` | `psionic` | `91.24 s` | 5 | 33,755 | 684 | 34,439 | 0 (used 2 shell cmds) | `bad_replies` | Yes (2/2 files) | 0 |
| Hybrid (structured-edits mode) | `hybrid-exec-v1` | `psionic` | **`61.96 s`** | **4** | **19,376** | **1,030** | **20,406** (-40.8%) | **2 ok / 2 refused** | **`finished`** | **Yes (2/2 files)** | **4 / 4 (100%)** |

Exact live `mcworker` structured-edit replay receipts emitted during the `hybrid-exec-v1` run on Apple M5 Max Metal 4:
1. Turn 1 mismatch intercepted pre-disk: `edit executor-desktop.coax` -> `[E] replay refused · sha256:7a93ffccff82`
2. Turn 1 mismatch intercepted pre-disk: `edit hybrid-exec-policy.coax` -> `[E] replay refused · sha256:e1c5d8b6fc3e`
3. Turn 2 self-corrected edit applied cleanly: `edit executor-desktop.coax` -> `\ [E] replay verified · sha256:0760bed0e862`
4. Turn 2 self-corrected edit applied cleanly: `edit hybrid-exec-policy.coax` -> `\ [E] replay verified · sha256:c3e731afbc4d`

**Honest limitation noted:** The base `Inkling-2-Mini-IQ4_NL.gguf` checkpoint does not yet include a fine-tuned W3.1 LoRA routing adapter; when shell commands are permitted without nudging structured edits, it often falls back to `sed` and can emit trailing prose after finishing (`bad_replies`). When structured edits are used, the compiled Class-E guard intercepts malformed edits before disk I/O and enables clean self-correction with 40.8% fewer total tokens and 100% Ch 234 replay receipts.

---

## 2. Pillar 2 — OpenAgents Desktop Integration (`psionic/lib/executor-desktop.coax` + `apps/openagents-desktop/`)

### What Landed
- `#authority` `psionic/lib/executor-desktop.coax`: truthful admission gate for compiled weight modules and `hybrid-exec-v1` composer picker gating (`Exact Replay [E]`, `Deterministic [D]`, `Statistical [S]`, `Unverified [N]`; atomic refusal on any 1-byte tamper, missing base checkpoint, missing required module, or `< 32 GB` RAM).
- `coax/lib/agent/desktop/executor-store.coax` + `apps/openagents-desktop/desktop-core.coax` (`oa_dc_exec_scan`): scans `~/.openagents/modules/<64-hex>/` and `~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf`, validates GGUF header magic + 16 GB minimum size, computes real SHA-256 digests over every `weights.safetensors` via `ev_deserialize`, and calls `#authority` `exec_desktop_admit_module` / `exec_desktop_hybrid_status`.
- `apps/openagents-desktop/` (`settings.coax`, `page/models.coax`, `page/providers.coax`, `page/state.coax`, `page/host-api.coax`, `host.h`, `host.c`, `Window*.swift`):
  - **Settings > Models > Executor Models & Weight Modules** section showing `Hybrid Executor (hybrid-exec-v1)` verification class/status and `Compiled Weight Modules` SHA-256 lineage, opening an interactive verification & install dialog (`Verify Now`, `Install Built-ins` -> `openagents module install-builtins`).
  - **Composer model picker**: exposes `Hybrid Executor [E]` (`hybrid-exec-v1`) ONLY when `oa_dc_exec_scan` passes, and re-verifies SHA-256 immediately before launching a `hybrid-exec-v1` worker run.
- `components/app/activity.coax` + `apps/openagents-desktop/page/layout/conversation.coax`: extracts and renders compact replay-verification pills (`[E] replay verified · sha256:<12hex>` / `[E] replay refused · sha256:<12hex>`) directly on deterministic edit and executor tool rows in the chat transcript.

### Real On-Disk SHA-256 & 1-Byte Tamper-Refusal E2E Verification
Command: `coax/web/tool.sh coax/lib/agent/desktop/executor-store-e2e.coax`

- **Live `$HOME`:** `selectable=1 state=4 verified=4/4 tampered=0`, `status: Ready · Exact Replay [E] (4/4)`, `lineage: diff:606049b8 · schema:5293cf74 · dep:7a821819 · bud:a93a4b53`, `badge: Exact Replay [E]`, `+1 linked composite module(s) verified`.
- **Temp `HOME` (empty modules):** `selectable=0 state=2 status=Modules missing (0/4 verified)`.
- **Temp `HOME` (clean 4/4 modules + real GGUF symlink):** `selectable=1 state=4 status=Ready · Exact Replay [E] (4/4)`.
- **Temp `HOME` (16 GB RAM):** `selectable=0 state=0 status=Requires 32 GB RAM`.
- **Temp `HOME` (1-byte bit flip in `oa-exec-diff-verify/weights.safetensors`):** `selectable=0 state=3 verified=3/4 tampered=1`, `status: Refused: digest tamper detected`, `lineage: Refused (tampered: oa-exec-diff-verify)`, `badge: Unverified [N]` — **NEVER displays Ready**.

---

## 3. Pillar 3 — Universal MCP Re-Verification (`openagents mcp --domains executor`)

Binary: `apps/openagents-cli/build/openagents mcp --domains executor` (built via `apps/openagents-cli/build.sh`).

- `initialize`: `protocolVersion: "2025-06-18"`, `serverInfo.name: "openagents-executor"`.
- `tools/list`: exposes all 5 deterministic tools (`executor_verify_diff`, `executor_validate_schema`, `executor_schedule_dag`, `executor_run_module`, `executor_verify_trace`).
- `executor_verify_diff`: `{"module":"oa-exec-diff-verify","class":"E","applicable":1,"new_file_lines":51,"delta":1}` (`isError: false`).
- `executor_validate_schema`: `{"module":"oa-exec-json-schema","class":"E","valid":1,"payload_bytes":90,"within_cap":1}` (`isError: false`).
- `executor_schedule_dag`: `{"module":"oa-exec-dep-sched","class":"E","ready":1,"next_running":2,"score":53}` (`isError: false`).
- `executor_run_module` (`oa-exec-arith-budget`): `{"verified":true,"outputs":[1,600,400]}` (`isError: false`).
- `executor_verify_trace` (`oa-exec-diff-verify`): `{"verified":true,"weights":1221,"layers":3}` (`isError: false`).
- **1-byte tamper in `oa-exec-diff-verify/weights.safetensors` (temp `HOME`):** both `executor_verify_trace` and `executor_run_module` return `isError: true` (`"admission gate refused module (tampered digest or failed conformance)"`).

---

## 4. Verification Matrix Summary

| Gate / Suite | Command | Result |
| --- | --- | --- |
| `#authority` Microcoder guard tests | `coax/bin/coax test coax/lib/microcoder/executor-guard.coax --library` | 76/76 passed |
| `#authority` Microcoder guard mutation | `coax/bin/coax mutate coax/lib/microcoder/executor-guard.coax --root-only` | 314 mutants: 309 killed, **0 survived**, 5 equivalent |
| `#authority` Desktop admission tests | `coax/bin/coax test psionic/lib/executor-desktop.coax --library` | 23/23 passed |
| `#authority` Desktop admission mutation | `coax/bin/coax mutate psionic/lib/executor-desktop.coax --root-only` | 85 mutants: 85 killed, **0 survived** |
| Microcoder unit suites | `coax/bin/coax test coax/lib/microcoder/{tools,run}.coax --library` & `coder/tui/microcoder_keys.coax --library` | All passed (459/459 in `microcoder_keys`) |
| Desktop store unit + tamper E2E | `coax/bin/coax test coax/lib/agent/desktop/executor-store.coax --library` & `coax/web/tool.sh coax/lib/agent/desktop/executor-store-e2e.coax` | 178/178 passed + `EXECUTOR_STORE_E2E_PASS` |
| Activity pill component tests | `coax/bin/coax test components/app/activity.coax --library` | 160/160 passed |
| OpenAgents Desktop page tests | `coax/bin/coax test apps/openagents-desktop/page.coax --library` | 819/819 passed |
| Full macOS Desktop build & checks | `apps/openagents-desktop/check.sh` | `OpenAgents.app` built + `account-check`, `sign-check`, `windows args` ok |
| Repo invariant checks | `coax/web/tool.sh tools-check`, `wire-check`, `layer-check` | 0 failed, 0 allowlist additions |

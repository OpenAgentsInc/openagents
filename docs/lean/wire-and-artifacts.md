# Lean Gym Wire Protocols, Schemas & On-Disk Artifacts

**Date:** 2026-11-21
**Companion documents:** [README.md](README.md) · [evaluation.md](evaluation.md) · [spec.md](spec.md) · [verification-and-security.md](verification-and-security.md) · [roadmap.md](roadmap.md)

Every wire shape and durable record follows [proto/README.md](../../proto/README.md) and `AGENTS.md`:

- Published contracts keep their identity; breaking changes require a new version (never mutated in place).
- Every shape gets one row in `proto/registry.json` (`owner: "coder"`), one Coax codec (`coax/lib/lean/wire.coax`), and pinned golden byte fixtures checked by `coax/web/tool.sh wire-check`.
- All app-owned persistent files live under `~/.openagents/lean/` (`AGENTS.md`, *"Shared OpenAgents files"*).

---

## 1. On-Disk Directory Layout (`~/.openagents/lean/`)

```text
~/.openagents/lean/
├── leanhost.sock                 # UNIX domain socket (mode 0600, dir 0700)
├── leanhost.lock                 # flock single-instance startup lock
├── leanhost.log                  # rolling daemon log (capped at 4 MiB)
├── gym-request.json              # atomic file-queue request from `lean-run gym`
├── gym-state.json                # atomic reply written by OpenAgents Desktop
├── scratch/
│   └── <toolchain-sha256-16>/    # shared Init-only scratch workspace for standalone .lean files
└── runs/
    └── <run-id>/                 # e.g. sample-1732200000, bench-minif2f-1732201234, verify-...
        ├── lean-run.json         # run manifest (lean-run.v1)
        ├── Spec.lean             # immutable frozen theorem specification (when graded)
        ├── Solution.lean         # current / final proof file
        ├── trace.jsonl           # step-by-step tactic & proof-tree log (lean-trace.v1)
        ├── verify.json           # authoritative kernel replay receipt (lean-verify.v1)
        ├── card.jpg              # rendered inline chat summary card (for OA-SHOW)
        ├── run.log               # combined compiler / verifier / harness output
        ├── run.pid               # present only for detached `lean-run bench start` runs
        ├── bench-run.json        # detached benchmark metadata (when started via `bench start`)
        ├── snapshots/            # optional per-step full `.lean` files (`step_000.lean` ...)
        └── trace.atif.jsonl      # optional canonical ATIF-v1.8 export (`lean-run export-atif`)
```

Separate benchmark leaderboard history (sibling to `~/.openagents/gym/last-benchmark.json`):

```text
~/.openagents/gym/
├── last-benchmark.json           # existing coding benchmarks (untouched)
├── last-lean-benchmark.json      # Lean head-to-head benchmark results
└── last-lean-run.log             # live log mirror for Lean benchmarks
```

---

## 2. Protocol Registry Plan (`proto/registry.json`)

When implementation begins (Phases 0–2), register these rows in `proto/registry.json` with `owner: "coder"` and **zero** new lines in `proto/wire-allowlist.txt`:

| Schema ID | Kind | Schema / Profile File | Sole Codec | Check Command |
| --- | --- | --- | --- | --- |
| `leanhost-lines-v1` | `profile` | `proto/coder/lean-host-lines-v1.json` | `coax/lib/lean/wire.coax` | `coax/web/tool.sh coax/lib/lean/wire-checks.coax proto/coder/lean-host-lines-v1.json` |
| `lean-run.v1` | `profile` | `proto/coder/lean-run-v1.json` | `coax/lib/lean/wire.coax` | `coax/web/tool.sh coax/lib/lean/wire-checks.coax proto/coder/lean-run-v1.json` |
| `lean-trace.v1` | `profile` | `proto/coder/lean-trace-v1.json` | `coax/lib/lean/wire.coax` | `coax/web/tool.sh coax/lib/lean/wire-checks.coax proto/coder/lean-trace-v1.json` |
| `lean-verify.v1` | `profile` | `proto/coder/lean-verify-v1.json` | `coax/lib/lean/wire.coax` | `coax/web/tool.sh coax/lib/lean/wire-checks.coax proto/coder/lean-verify-v1.json` |
| `lean-gym-request.v1` | `profile` | `proto/coder/lean-gym-v1.json` | `coax/lib/lean/gym.coax` | `coax/bin/coax test coax/lib/lean/gym.coax --library` |
| `lean-gym-state.v1` | `profile` | `proto/coder/lean-gym-v1.json` | `coax/lib/lean/gym.coax` | `coax/bin/coax test coax/lib/lean/gym.coax --library` |
| `lean-bench-task.v1` | `profile` | `proto/coder/lean-bench-task-v1.json` | `coax/lib/lean/wire.coax` | `coax/web/tool.sh coax/lib/lean/wire-checks.coax proto/coder/lean-bench-task-v1.json` |

Existing `ple-gym-request.v1` and `ple-gym-state.v1` (`coax/lib/agent/desktop/gym.coax`) remain unchanged.

---

## 3. Helper Socket Wire (`leanhost-lines-v1`)

Newline-delimited JSON objects over `~/.openagents/lean/leanhost.sock` (maximum line size `262144` bytes, mirroring `SW_LINE` in `coax/lib/agent/device/sim-wire.coax`).

### 3.1 Handshake & Control

Server greets every newly connected client with one line:

```json
{"event":"hello","protocol":"leanhost-lines-v1","version":1,"pid":48210,"toolchain":"leanprover/lean4:v4.19.0","elan":true,"lake":true,"lean4checker":true,"warm_workspaces":1,"max_workspaces":2}
```

### 3.2 Request Envelope

```json
{"id":1,"op":"check","project":"/Users/u/work/my-lean","file":"/Users/u/work/my-lean/Solution.lean","timeout_ms":15000}
```

Supported `op` values:

| `op` | Required Fields | Optional Fields | Purpose |
| --- | --- | --- | --- |
| `doctor` | — | `project` | Report toolchain, Mathlib cache, checker binaries, and warm pool status |
| `subscribe` | `project` | `file`, `run_dir` | Subscribe UI client to pushed `file_progress`, `diagnostics`, `trace_step`, `verify_done`, and `pool_pressure` events |
| `unsubscribe` | `project` | — | End push subscription |
| `check` | `file` | `project`, `text` (unsaved buffer), `timeout_ms` | Elaborate via warm `lake serve` and wait for `$/lean/fileProgress` settle |
| `goal` | `file`, `line`, `col` | `project`, `text`, `timeout_ms` | Query `$/lean/plainGoal` + `$/lean/plainTermGoal` at 1-based `line`, 0-based `col` |
| `try` | `file`, `line`, `col`, `tactic` | `project`, `text`, `state_id`, `goal_id`, `timeout_ms` | Speculative non-destructive tactic probe (LSP overlay or Pantograph) |
| `hover` | `file`, `line`, `col` | `project`, `text` | LSP `textDocument/hover` |
| `def` | `file`, `line`, `col` | `project` | LSP `textDocument/definition` |
| `search` | `query` | `project`, `limit` | Search local declarations / configured lemma index |
| `verify` | `file` | `project`, `spec`, `theorem`, `run_dir`, `allow_classical`, `sandbox`, `timeout_ms` | Authoritative 3-stage kernel verification |
| `status` | — | — | List warm workspaces, RSS per worker, active subscriptions |
| `evict` | `project` | — | Explicitly stop a warm `lake serve` workspace to free RAM |
| `quit` | — | — | Shut down `oa-leanhost` after finishing in-flight requests |

### 3.3 Reply Envelope

Success:

```json
{
  "id": 1,
  "ok": true,
  "op": "goal",
  "elapsed_ms": 42,
  "engine": "lsp",
  "doc_version": 7,
  "goals": [
    {
      "id": "g0",
      "tag": "succ",
      "hypotheses": [
        {"name": "k", "type": "Nat"},
        {"name": "ih", "type": "k + 0 = k"}
      ],
      "target": "(k + 1) + 0 = k + 1",
      "raw": "case succ\nk : Nat\nih : k + 0 = k\n⊢ (k + 1) + 0 = k + 1"
    }
  ],
  "term_goal": null,
  "goals_accomplished": false
}
```

Refusal / Error (every refusal carries a typed `code` from `policy.coax`):

```json
{"id":1,"ok":false,"op":"check","code":"no_toolchain","error":"Lean 4 toolchain not found (checked ~/.elan/bin and PATH). Run `lean-run doctor` for setup steps."}
```

Canonical error codes: `no_toolchain`, `toolchain_missing`, `mathlib_cache_missing`, `bad_request`, `path_outside_workspace`, `file_too_large`, `timeout`, `lsp_crashed`, `pool_exhausted`, `sorry_used`, `forbidden_axiom`, `statement_mismatch`, `kernel_replay_failed`, `sandbox_unavailable`.

### 3.4 Pushed Events (to `subscribe` Clients)

```json
{"event":"file_progress","uri":"file:///.../Solution.lean","version":7,"ranges":[{"start_line":11,"end_line":16,"kind":1}]}
{"event":"diagnostics","uri":"file:///.../Solution.lean","version":7,"settled":true,"errors":0,"warnings":1,"sorry_count":1,"items":[{"severity":"warning","line":15,"col":4,"end_line":15,"end_col":9,"code":"sorry","message":"declaration uses 'sorry'"}]}
{"event":"trace_step","run_dir":"/Users/.../.openagents/lean/runs/r1","step":4,"node_id":"n4","parent_id":"n3","tactic":"simp [Nat.add_succ, ih]","goals_remaining":0,"ok":true}
{"event":"verify_done","run_dir":"/Users/.../.openagents/lean/runs/r1","verified":true,"receipt_sha256":"9f4a..."}
{"event":"pool_pressure","total_rss_mib":6410,"limit_rss_mib":6144,"evicted":"/Users/.../old-repo"}
```

---

## 4. Run Manifest (`lean-run.v1` — `lean-run.json`)

Written at run creation and updated atomically (`write .tmp` + `rename`) as verification completes:

```json
{
  "version": "lean-run.v1",
  "run_id": "sample-1732200000",
  "kind": "sample",
  "title": "add_zero_right",
  "project_root": "/Users/u/.openagents/lean/runs/sample-1732200000",
  "toolchain": "leanprover/lean4:v4.19.0",
  "uses_mathlib": false,
  "spec_file": "Spec.lean",
  "spec_sha256": "3a7b...",
  "solution_file": "Solution.lean",
  "target_theorems": ["OpenAgents.Sample.add_zero_right"],
  "trace_file": "trace.jsonl",
  "verify_file": "verify.json",
  "created_ms": 1763748600000,
  "updated_ms": 1763748601840,
  "status": "verified",
  "steps": 4,
  "goals_remaining": 0,
  "model": "deterministic-sample",
  "engine": "lsp"
}
```

Valid `status` values: `"running"`, `"unverified"`, `"failed"`, `"verified"`, `"stopped"`, `"no_toolchain"`. Note that `"verified"` is **only** valid when `verify.json` exists, has `verified: true`, and its `solution_sha256` matches `Solution.lean` on disk.

---

## 5. Trajectory & Proof-Tree Log (`lean-trace.v1` — `trace.jsonl`)

Append-only JSONL recording every proof state transition (both linear edit timelines from LSP and branching tactic DAGs from Pantograph/search agents). The Desktop Lean Gym Timeline Scrubber and Proof State Tree both project directly from this single file.

```json
{"version":"lean-trace.v1","step":0,"node_id":"n0","parent_id":null,"ts_ms":1763748600100,"kind":"init","file":"Solution.lean","snapshot":"snapshots/step_000.lean","line":12,"col":35,"action":"theorem add_zero_right","ok":true,"goals_remaining":1,"goals":["n : Nat\n⊢ n + 0 = n"],"errors":0,"sorry_count":1,"elapsed_ms":180}
{"version":"lean-trace.v1","step":1,"node_id":"n1","parent_id":"n0","ts_ms":1763748600310,"kind":"tactic","file":"Solution.lean","snapshot":"snapshots/step_001.lean","line":13,"col":2,"action":"induction n with","ok":true,"goals_remaining":2,"goals":["case zero\n⊢ 0 + 0 = 0","case succ\nk : Nat\nih : k + 0 = k\n⊢ (k + 1) + 0 = k + 1"],"errors":0,"sorry_count":2,"elapsed_ms":64}
{"version":"lean-trace.v1","step":2,"node_id":"n2","parent_id":"n1","ts_ms":1763748600405,"kind":"tactic","file":"Solution.lean","snapshot":"snapshots/step_002.lean","line":14,"col":14,"action":"| zero => omega_bad","ok":false,"goals_remaining":2,"goals":[],"errors":1,"error_message":"unknown tactic 'omega_bad'","sorry_count":1,"elapsed_ms":31}
{"version":"lean-trace.v1","step":3,"node_id":"n3","parent_id":"n1","ts_ms":1763748600520,"kind":"tactic","file":"Solution.lean","snapshot":"snapshots/step_003.lean","line":14,"col":14,"action":"| zero => rfl","ok":true,"goals_remaining":1,"goals":["case succ\nk : Nat\nih : k + 0 = k\n⊢ (k + 1) + 0 = k + 1"],"errors":0,"sorry_count":1,"elapsed_ms":48}
{"version":"lean-trace.v1","step":4,"node_id":"n4","parent_id":"n3","ts_ms":1763748600690,"kind":"tactic","file":"Solution.lean","snapshot":"snapshots/step_004.lean","line":16,"col":4,"action":"simp [Nat.add_succ, ih]","ok":true,"goals_remaining":0,"goals":[],"errors":0,"sorry_count":0,"elapsed_ms":72}
```

Notice how `parent_id` naturally captures **failed branches and backtracking** (above: `n2` failed off `n1`, then `n3` branched cleanly off `n1`), enabling the Proof State DAG to visualize dead ends vs winning proof paths without needing a second schema!

---

## 6. Authoritative Kernel Verification Receipt (`lean-verify.v1` — `verify.json`)

Produced exclusively by `coax/lib/lean/verify.coax` and judged by `#authority` `coax/lib/lean/policy.coax`:

```json
{
  "version": "lean-verify.v1",
  "verified": true,
  "refusal_code": "",
  "refusal_reason": "",
  "toolchain": "leanprover/lean4:v4.19.0",
  "checker": "lean4checker-v4.19.0+oa-statement-audit-v1",
  "sandbox": "seatbelt-no-net",
  "spec_sha256": "3a7b09...",
  "solution_sha256": "81cc42...",
  "olean_sha256": "52e19a...",
  "verified_at_ms": 1763748601840,
  "elapsed_ms": 1120,
  "gates": {
    "parse": true,
    "elaborate": true,
    "no_sorry": true,
    "axioms_ok": true,
    "target_match": true,
    "kernel_replay": true
  },
  "theorems": [
    {
      "name": "OpenAgents.Sample.add_zero_right",
      "matched_spec": true,
      "type_pretty": "∀ (n : Nat), n + 0 = n",
      "axioms": ["propext"],
      "forbidden_axioms": [],
      "uses_sorry": false,
      "uses_native_decide": false
    }
  ],
  "allowed_axioms": ["propext", "Quot.sound", "Classical.choice"],
  "receipt_sha256": "9f4a71..."
}
```

`receipt_sha256` is the canonical SHA-256 over the receipt fields (excluding `receipt_sha256` itself), binding `(spec_sha256, solution_sha256, olean_sha256, toolchain, gates, theorems)` together so any 1-byte edit to `Solution.lean` after verification immediately invalidates the `VERIFIED` badge in the Desktop UI (displaying `STALE — Re-verify`, matching the honest 1-byte tamper refusal discipline in `coax/lib/agent/desktop/executor-store.coax`).

---

## 7. Programmatic Gym Steering (`lean-gym-request.v1` & `lean-gym-state.v1`)

Modeled directly on `coax/lib/agent/desktop/gym.coax` (`ple-gym-request.v1`): every field is optional, unknown keys are refused by `dc_lean_gym_plan` (`oa_dc_lean_gym_plan`), and the same contract works over both `~/.openagents/lean/gym-request.json` (`lean-run gym`) and `GET/POST /v1/gym/lean` (`foreman-ctl gym lean`).

### Request (`lean-gym-request.v1`)

```json
{
  "version": "lean-gym-request.v1",
  "id": "lean-run-1763748602000",
  "open": true,
  "run": "latest",
  "file": "Solution.lean",
  "line": 14,
  "step": "last",
  "tab": "obligations",
  "diff": false,
  "live": true,
  "verify": false
}
```

- `run`: `"latest"`, a run name under `~/.openagents/lean/runs/`, or an absolute path to a run folder, Lake project, or `.lean` file.
- `line`: 1-based line number (`1..65535`).
- `step`: 1-based step index (`1..N`) or `"last"`.
- `tab`: `"obligations"` | `"diagnostics"` | `"tree"` | `"infoview"`.
- `verify`: when `true`, triggers `lean-run verify` on the active run and returns after verification settles.

### State Reply (`lean-gym-state.v1`)

```json
{
  "version": "lean-gym-state.v1",
  "ok": true,
  "id": "lean-run-1763748602000",
  "open": true,
  "gym_mode": "lean",
  "toolchain": "leanprover/lean4:v4.19.0",
  "runs": 3,
  "run": "/Users/u/.openagents/lean/runs/sample-1732200000",
  "title": "add_zero_right",
  "file": "/Users/u/.openagents/lean/runs/sample-1732200000/Solution.lean",
  "line": 14,
  "step": 5,
  "steps": 5,
  "goals_remaining": 0,
  "errors": 0,
  "sorry_count": 0,
  "verified": true,
  "axioms": ["propext"],
  "tab": "obligations",
  "live": true,
  "status": "Verified ✓ (0 sorry, 1 axiom)"
}
```

---

## 8. Benchmark Task Format (`lean-bench-task.v1`)

Extends the existing `evals/openagents-desktop/{tier2,tier4}/tasks/*.json` structure (`GymBenchmarkView.swift:1186-1204`) cleanly so both headless CLI evals (`evals/openagents-desktop/run.sh`) and the live Desktop Gym Benchmarks tab (`GymBenchmarkView`) execute the exact same tasks:

```json
{
  "version": "lean-bench-task.v1",
  "id": "l01_nat_add_zero_right",
  "suite": "lean_core_smoke",
  "tier": "lean",
  "fixture": "l01_nat_add_zero_right",
  "uses_mathlib": false,
  "toolchain": "leanprover/lean4:v4.19.0",
  "target_theorems": ["OpenAgents.Bench.l01_add_zero_right"],
  "allowed_axioms": ["propext", "Quot.sound", "Classical.choice"],
  "max_turns": 12,
  "timeout_s": 120,
  "steps": [
    {
      "send": "Replace `sorry` in `Solution.lean` with a complete Lean 4 proof of `OpenAgents.Bench.l01_add_zero_right`. Do not modify `Spec.lean`, do not introduce custom axioms, and do not use `native_decide`."
    }
  ],
  "graders": [
    { "type": "files_unchanged", "paths": ["Spec.lean", "lean-toolchain", "lakefile.*"] },
    { "type": "lean_verify", "solution": "Solution.lean", "spec": "Spec.lean", "theorem": "OpenAgents.Bench.l01_add_zero_right" }
  ]
}
```

---

## 9. ATIF-v1.8 Verified-Trace Export (`coax/lib/lean/atif.coax`)

To connect Lean proofs directly into OpenAgents' verified-trace distillation pipeline (`coax/lib/agent/desktop/proof-export.coax`, Psionic W3.2 `#13918`, and `openagents contribute submit` `#13926`):

- `lean-run export-atif RUN_DIR --out OUT.atif.jsonl` refuses to mark a trace `verified` unless `verify.json` passes `coax/lib/lean/policy.coax` (`verified == true` and `solution_sha256` matches).
- Each step in `trace.jsonl` (and, for agent runs, each LLM reasoning + tool call turn) maps onto an ATIF-v1.8 step via `coax/lib/nostr/atif/log.coax`:
  - `session_header`: `source_harness = "openagents-lean-gym"`, `model_name`, `cwd` (redacted via `coax/lib/agent/` secret/PII redactor before external contribution).
  - `user` step: frozen theorem statement (`Spec.lean`) + initial goal.
  - `assistant` / `tool` steps: tactic or file edit + resulting `$/lean/plainGoal` observation (`goals_remaining`, diagnostics).
  - Final verification metadata step: `verify.json` `receipt_sha256`, `toolchain`, `axioms`, and `atif_verify_log` SHA-256 `steps_digest`.

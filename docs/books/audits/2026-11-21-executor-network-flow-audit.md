# Audit: Aligning the Eight Books Around the OpenAgents Executor Network Capstone

- **Date:** 2026-11-21
- **Snapshot:** `origin/main` at `9c869b16ce` (after Master Epic `#13912` and `#13913`–`#13928` landed)
- **Scope:** Cross-book narrative flow, topic ownership, chapter inventory, registry health, and capstone alignment across all **8 books** (`coax`, `bitcoin`, `nostr`, `psionic`, `coder`, `openagents`, `metaverse-engine`, `everglade`) and the shared index (`docs/books/`).
- **Companion audit:** Extends [2026-11-20-holistic-books-audit.md](2026-11-20-holistic-books-audit.md) (which audited book hygiene and the first 6 chapters) by auditing how the entire 8-book arc builds up to—and closes into—the **OpenAgents Executor Network** ([master spec](../../../psionic/docs/executor-network-master-spec.md)).

---

## 1. Executive Thesis: The Missing Capstone Arc

The eight books already share a five-step logical spine ([`docs/books/README.md`](../README.md#the-argument)):

> **Bounded → Deterministic → Checkable → Payable → Ownable**

Until `#13912`–`#13928`, that spine ended at *individual* verified transactions (a single guard decision in Everglade, or a single Coder task paid via Breez Spark). With the **OpenAgents Executor Network** now built, verified on real Apple M5 Max Metal 4 and NVIDIA RTX 4080 CUDA GPUs, and settled live on Bitcoin mainnet via Breez Spark, the five-step foundation closes into a **sixth step**:

> **6. Compounding.** Because verified work produces provably correct traces and redacted real-world trajectories, a decentralized network of Pylons can run bounded training windows whose updates are Byzantine-filtered, quarantined, and promoted only past held-out gates—shipping better models and exact weight modules back to every user's agent automatically.

Right now, **the codebase is ahead of all eight books**:

| Executor Network Pillar (`#13912`–`#13928`) | Mentions across all 8 books (before this audit) | What is lagging in the books |
| --- | --- | --- |
| `wasm-exec-v1` / `wasm-core-i32-v1` (93-layer 784k-weight zero-fallback WASM transformer, 398,722 steps, 0 divergences) | **0** | Psionic Ch 231–232 still teach the 5-op register toy (`iw-v1`) and 64-entry shifted array (`hull-v1`) |
| `exec-gpu-v1` (Metal 4 & CUDA exact kernels + optimal liveness/slot-reuse scheduler cutting weights 26.4%) | **0** | `psionic/examples/exec-vm-gpu.coax` was unregistered in `examples.json` (breaking `book.py`); `chapters.json` still lists `qualified_targets: ["cpu"]` for all of Part XXIII |
| `module-v2` ("Agentic npm", `openagents module`, 4 Class-E coding-agent cores) | **0** | Psionic Ch 237 still teaches `module-v1` (capped at 64 slots / 4 seams / FFN-only); Coder & OpenAgents books never mention compiled weight modules |
| `hybrid-exec-v1` (`Inkling-2-Mini` GGUF + rank-8 LoRA routing, `11/48 → 48/48`, `0` silent hallucinations, KV cache reuse) & `H2_ROUTING_ONLY` distillation (`4.92M` tokens on Metal 4) | **0** | Psionic Ch 235–236 still teach tiny CPU table/linear students (`students-v1`, `core-v1`); Part XXVII (`Inkling 2 Mini`) is disconnected from Part XXIII |
| `NIP-PYLON` `cj-executor` / `cj-validate` / `cj-training` lanes & live two-leg Bitcoin mainnet Spark settlement | **0** in Nostr & Bitcoin; Ch 238 banner only in Psionic | Nostr Ch 82 still says paid receipts are not built; Bitcoin Part XIX (`ch123`) is still `planned` and mentions Spark once |
| Decentralized training (`TrainingWindow`, DiLoCo worker, 6-tier Byzantine verifier blocking 7/7 attacks, Quarantine Optimizer blocking 3/3 regressions) | **0** | **No chapter exists in any book** for W5 decentralized training or the Quarantine Optimizer |
| Coding-agent flywheel (`openagents mcp --domains executor`, `openagents contribute submit` with BIP-39/PII redaction, W6.3 shadow-to-promote rollout) | **0** | Coder Ch 8/12/16/18/23 and OpenAgents Ch 52/54/55/69/70/78 do not reflect the live flywheel |

Additionally, `python3 psionic/tests/book.py --book psionic` was failing on `origin/main` because three new Executor Network examples (`exec-vm-gpu.coax`, `hybrid-exec.coax`, `wasm-exec.coax`) were not registered in `psionic/book/examples.json`. *(Fixed in the same change as this audit.)*

---

## 2. Target Cross-Book Architecture (Building Up to the Capstone)

Each book should own **one crisp layer** of the buildup so readers (and agents) can walk from first principles to the live compounding network without duplicating material across books:

```text
Layer 0 — Language & Exact GPU Determinism
  Coax (Ch 16 Determinism, Ch 17 Metal/CUDA GPU, Ch 18 proto/, Ch 23 #authority)
          │
          ├───────────────────────────────────────────────────────────┐
          ▼                                                           ▼
Layer 1 — Money & Open Protocol                           Parallel World Consumers
  Bitcoin (Parts I–V, XVII, XIX Spark/two-leg)              Metaverse Engine ──► Everglade
  Nostr   (Ch 82/100 NIP-PYLON lanes & 30201)               (consume the same Pylon,
          │                                                  replay & Spark spine via
          ▼                                                  guard/maker capstones)
Layer 2 — Compiled Substrate, Hybrid Models & Decentralized Training
  Psionic (Part XXIII Exact Compiled Weights + Part XXVII Inkling 2 Mini
           + Part XXIV Eval/Gates/Shadow + New Part XXVIII Decentralized Training)
          │
          ▼
Layer 3 — Coding Agent Harness & Trajectory Flywheel
  Coder   (Ch 8/9/12 Hybrid + Weight Modules, Ch 16 MCP executor,
           Ch 18 Redacted ATIF-v1.8 contribute, Ch 21/23 CLI)
          │
          ▼
Layer 4 — Product & Compounding Network Capstone
  OpenAgents (Ch 52 Gym, Ch 54–58 Signed releases & paid calls,
              Ch 69–70 Served models & Pylons, Ch 78 What 1.0 means)
          │
          ▼
  MASTER CAPSTONE: docs/books/capstones/executor-network-flywheel.md
```

---

## 3. Book-by-Book Detailed Audit

### 3.1 `coax/book` — Layer 0: The Deterministic, Multi-GPU, `#authority` Foundation

**Role in the buildup:** Coax is the reason one source file can compile to bit-exact `i64`/`f64` kernels on both Apple Metal 4 and NVIDIA CUDA, enforce `#authority` zero-mutant policy rules, and share one registered `proto/` wire codec across every host.

**What's solid:**
- Parts I–II (`ch01`–`ch12`), `ch18` (protocol boundary), and `ch23` (verification boundaries) already establish bounded capacity, checks, mutation, and `#authority`.

**What needs updating:**
1. **`ch16-determinism.md` (`partly implemented`):** Currently frames determinism primarily around game-world replay and C compiler float traps. Add a concrete section linking directly to Psionic Part XXIII's cross-hardware bit-exactness invariant: how `binary64` / compensated `f32×2` / exact `i64` arithmetic keeps Apple Metal 4 and NVIDIA RTX 4080 CUDA bit-identical across 398,722 steps (`exec-gpu-v1`).
2. **`ch17-gpu.md` (`partly implemented`):** Currently opens with the old iOS Metal home screen shaders (`f32`/`bool`). Reframe around single-source Coax compute kernels targeting **both** Metal 4 and CUDA, citing `psionic/platform/exec-vm-gpu-bridge.coax` and `docs/verification/executor-w13-gpu-sched.md` as the flagship real-world proof.
3. **`ch23-verification-boundaries.md`:** Add a short cross-reference showing how `#authority` zero-mutant policy files protect real Bitcoin mainnet settlement (`pay/executor-settlement.coax`, `psionic/lib/peer/pay.coax`) and Byzantine training verification (`psionic/lib/peer/train-verify.coax`).
4. **Tooling gap (from 2026-11-20 audit):** Register `coax` in `psionic/tests/book.py` so its listings and links are checked automatically.

---

### 3.2 `bitcoin/book` — Layer 1A: Custody, Lightning & Live Spark Two-Leg Settlement

**Role in the buildup:** Bitcoin From Scratch owns the cryptographic primitives (Parts I–V), Lightning protocol (Part XVII), and Bitcoin-in-OpenAgents custody & payment mechanics (Part XIX).

**What's solid:**
- 100/125 chapters implemented; gold-standard four-rule rigor; `ch121` (One seed for Nostr and Bitcoin) and `ch122` (Agent treasuries) are implemented.

**What needs updating:**
1. **Part XIX (`ch123-paying-for-requests.md`, currently `planned`):** This is the natural bridge between the from-scratch Lightning chapters (`ch104`–`ch115`) and the real Breez Spark wallet (`apps/shared/host/apple/SparkWallet.swift`, `pay/wallet.coax`, `pay/executor-settlement.coax`). Expand `ch123` to cover:
   - The separation between `#authority` Coax money policy (`pay/`) and the external Spark SDK host boundary.
   - **Two-leg settlement (`WORKER` + `VALIDATOR`)** keyed by `(receipt_digest, leg)` with idempotency across restarts (`0` double-spend), rolling 24h UTC caps, and default-OFF mainnet arming (`NEEDS_OWNER.md`).
   - Explicit link to `docs/verification/executor-w43-mainnet-settlement.md` (live `200 sats` worker + `50 sats` validator, `0` sat fee on Bitcoin mainnet).
2. **Keep Parts I–XVIII untouched in scope:** Do not pull ML or Pylon orchestration into Bitcoin From Scratch; keep its boundary strictly at keys, signatures, Lightning/Spark settlement, and policy-gated payouts.

---

### 3.3 `nostr/book` — Layer 1B: `NIP-PYLON` Lanes, Replay Verdicts & Reconciled Pool Feeds

**Role in the buildup:** Nostr From Scratch owns the signed event model, relays, and `NIP-PYLON` wire events that coordinate untrusted workers, independent validators, and training windows without a central account system.

**What's solid:**
- 88/104 chapters implemented; `ch082` (Pylons, environments and deploys) and `ch100` (Pylons and the mesh) already cover kind `30200` beacons, kind `3201` service receipts, NIP-42 auth, and NIP-44 sealing.

**What needs updating:**
1. **`ch082-pylons-environments-and-deploys.md`:**
   - Remove the stale *"paid receipts as not built"* caveat.
   - Document the three new `NIP-PYLON` lanes (`cj-executor`, `cj-validate`, `cj-training`) added in `#13919` / `#13921`, digest self-test admission before advertising a slot, NIP-32 kind `1985` `ReplayVerdict` events (enforcing `validator_pubkey != worker_pubkey` and `validator_device_ref != worker_device_ref`), and reconciled kind `30201` `PoolAggregate` settled feeds (`paid_msat.bitcoin` strictly excluding `real_bitcoin_moved == false`).
2. **`ch100-pylons-and-the-mesh.md`:**
   - Extend the single-job walkthrough to show the **two-device** live flow: worker publishes `ExecutionTraceReceipt` → independent validator on a second physical machine replays via Ch 234 and publishes signed `ReplayVerdict` → settlement publishes reconciled kind `30201`.
3. **`ch078-runs-sessions-terminals-and-trajectories.md` & `ch081-decisions-evaluations-optimization-and-programs.md`:**
   - Cross-link `ATIF-v1.8` redacted trajectory digests (`openagents contribute submit`) and `CheckpointLineageRecord` / `TrainingWindowClaim` schemas (`proto/openagents/executor/v1alpha1/`).

---

### 3.4 `psionic/book` — Layer 2: The Technical Heart of the Executor Network

**Role in the buildup:** Psionic owns the compiled transformer substrate, GPU exact kernels, composable weight modules, hybrid LLM + exact core architecture, falsifiable distillation, evaluation gates, and decentralized training.

**What's solid:**
- Part XXIII (`ch226`–`ch238`), Part XXIV (`ch239`–`ch248`), and Part XXVII (`ch266`–`ch271`, Inkling 2 Mini) already establish the exact conceptual ladder—and `ch238`'s status banner already records the live W4.3 mainnet qualification.

**What needs updating (highest leverage in the entire library):**
1. **Fix `examples.json` & `chapters.json` metadata:**
   - Register `exec-vm-gpu.coax`, `hybrid-exec.coax`, and `wasm-exec.coax` in `psionic/book/examples.json` *(done in this change so `book.py --book psionic` passes)*.
   - Update `qualified_targets` in `psionic/book/chapters.json` where real GPU/mainnet evidence now exists (`ch229`, `ch231`, `ch232`, `ch234`–`ch238`, `ch269` currently all say `["cpu"]` despite live Metal 4, RTX 4080 CUDA, and Bitcoin mainnet verification records).
2. **Upgrade Part XXIII chapters (`ch229`–`ch237`) from bounded CPU prototypes to the real W1–W3 artifacts** (keep the small CPU hand-calculations as the opening teaching step, then land the real artifact as the chapter's culmination):
   - **`ch229` (Compile a gate graph into a block):** Add W1.3's optimal liveness/slot-reuse scheduler (26.4% weight reduction over greedy) and single-source Metal 4 / CUDA `exec-gpu-v1` exact kernels (`57.5k steps/s` Metal 4 `i64`, `91.9k steps/s` RTX 4080 CUDA `i64`, `63.8k steps/s` native `f64`).
   - **`ch231` (An interpreter in weights, or one program in weights):** Upgrade from the 5-op register toy (`iw-v1`) to `exec-vm-v1` (54/54 Ch 226 VM programs) and the full 93-layer, 784k-weight zero-fallback dense `binary64` WASM transformer (`wasm-exec-v1`, passing 9×9 Sudoku, 10×10 Hungarian, and Coder Ch 8 programs across 398,722 steps with 0 divergences; always printing native CPU speed beside compiled-weight speed).
   - **`ch232` (Decode in logarithmic time):** Upgrade from the 64-entry shifted array (`hull-v1`) to the balanced AVL parabolic convex-hull index (`hull-v2`, `O(log t)` reads *and* inserts to `1,048,576` writes).
   - **`ch234` (Exact replay as a verification class):** Connect local replay to W4.2's multi-device Pylon validator (`cj-validate`, distinct key + distinct device ref) and the 4-tier trace quarantine ladder before admission into Ch 247.
   - **`ch235` (Can gradient descent learn exact execution?):** Add the live Metal 4 `4.92M`-token distillation result (`docs/verification/executor-distill-h1-h3-h5.md`) proving `H2_ROUTING_ONLY`: neural distillation should train routing, while execution stays in frozen compiled cores.
   - **`ch236` (A frozen exact core with a learned interface):** Upgrade from the 12-source toy (`core-v1`) to `hybrid-exec-v1` wired live to the real 16.38 GB `Inkling-2-Mini` GGUF on Metal 4 with a rank-8 LoRA routing adapter (`11/48 → 48/48` on held-out tasks, `0` silent hallucinations, KV cache reused across exact calls; `docs/verification/executor-w31-hybrid-exec.md`). Explicitly link Part XXVII (`ch266`–`ch271`) here!
   - **`ch237` (Composable weight modules):** Upgrade from `module-v1` (64 slots / 4 seams / FFN-only) to `module-v2` (`~/.openagents/modules/<sha256>/`, `openagents module` CLI, attention+FFN linking, and the 4 Class-E coding-agent cores: `oa-exec-diff-verify`, `oa-exec-json-schema`, `oa-exec-dep-sched`, `oa-exec-arith-budget`).
3. **Add Part XXVIII: Decentralized Training on Pylons (3 new chapters: `ch272`–`ch274`):**
   W5 currently has **zero** chapters in any book, despite having complete `#authority` Coax implementations and end-to-end attack/regression harnesses (`#13923`, `#13925`, `#13927`). Add:
   - **`ch272` — Bounded training windows and Local-SGD / DiLoCo:** `TrainingWindow`, Dataset Shard Authority (building decontaminated shards from Ch 247's verified traces & redacted `ATIF-v1.8` coding trajectories), `openagents pylon train`, and compact LoRA/pseudo-gradient deltas over consumer internet.
   - **`ch273` — Six-tier Byzantine verification for public training:** `#authority` `psionic/lib/peer/train-verify.coax` (admission, deterministic step spot-check, redundant replication, statistical/Byzantine filtering, quarantine canary, survival bonus) with the 7-attack falsifier suite (`7/7` attacks blocked).
   - **`ch274` — The Quarantine Optimizer and atomic lineage rollback:** `#authority` `psionic/lib/peer/quarantine-opt.coax` (`canonical → quarantine → promoted`), requiring Ch 239–241 decision/probability gates + Ch 234 exactness preservation (`3/3` regressions blocked), signed `CheckpointLineageRecord`, and atomic rollback.
4. **Rationalize book navigation (from 2026-11-20 audit):** Separate the **Core ML + Executor Spine** (Parts I–IV, X–XII, XX, XXII–XXIV, XXVIII) from the **Model Targets Appendix** (Parts VI–IX, XIII–XVIII, XXVII) in `psionic/book/README.md` so readers see the direct line from Chapter 1 to the Executor Network without wading through 100+ model-target chapters.

---

### 3.5 `coder/book` — Layer 3: Real Coding Agents Consuming Modules & Feeding the Flywheel

**Role in the buildup:** Coder turns the Executor Network from an ML infrastructure story into immediate daily value for developers—and closes the data loop safely via opt-in, redacted `ATIF-v1.8` trajectories.

**What's solid:**
- Clean spine (`ch001`–`ch023`), strong real-world desktop/worker chapters (`ch036`–`ch041`), and an explicit note in `PLAN.md` for recent desktop additions.

**What needs updating:**
1. **`ch008-programs-as-tool-calls.md` & `ch009-microcoder.md`:** Connect Microcoder's edit/tool loop directly to the 4 W2.2 Class-E compiled weight modules (`oa-exec-diff-verify`, `oa-exec-json-schema`, `oa-exec-dep-sched`, `oa-exec-arith-budget`), showing how `oa-exec-diff-verify` eliminates silent patch-offset failures without an external sandbox.
2. **`ch012-a-psionic-served-model.md`:** Update to show Microcoder running on `hybrid-exec-v1` (local Metal 4 `Inkling-2-Mini` + rank-8 LoRA router + frozen compiled cores) alongside Pylon-routed execution (`docs/verification/executor-w61-coding-agent.md`).
3. **`ch016-speak-the-protocols.md`:** Document exposing compiled weight modules to external coding agents (Claude Code, Codex, Cursor, Antigravity) via `openagents mcp --domains executor` (`#13924`).
4. **`ch018-consent-before-sharing.md`:** This chapter was *written for* W6.2 before W6.2 existed! Upgrade it with the live `#authority` secret/PII/BIP-39 redaction pipeline and `openagents contribute submit` (`#13926`), showing how opt-in, user-previewed `ATIF-v1.8` trajectories with real compiler/test/replay labels (never LLM self-summaries) emit `DatasetShardManifest` records for Psionic's W5.1 Shard Authority.
5. **`ch023-openagents-coder.md`:** Cross-link the sibling CLI domains (`openagents module`, `openagents contribute`, `openagents pylon {executor,validate,train}`) so the CLI spine reflects the full loop.

---

### 3.6 `openagents/book` — Layer 4: The Unified Product & Compounding Network Book

**Role in the buildup:** The OpenAgents Book is the product book that puts the Desktop/iOS apps, wallet, Gym, signed releases, and Pylon compute network together into one coherent system.

**What's solid:**
- The 78-chapter map already has exact slots for the entire flywheel (`ch052` The Gym, `ch054`–`ch055` Signed releases & "Everyone gets it", `ch056`–`ch058` Paid calls/metering/payouts, `ch069`–`ch070` Served models & Pylons, `ch078` What 1.0 means).

**What needs updating:**
1. **Re-anchor `ch070-pylons.md` (currently a stale design draft):**
   - `ch070` currently ends with open questions (*"What does a Pylon earn?"*, *"How large a sample do we recompute?"*, *"Who is the first outside buyer?"*) and only mentions decision models.
   - Rewrite `ch070` around the live **OpenAgents Executor Network**: the three Pylon lanes (`cj-executor`, `cj-validate`, `cj-training`), digest self-test admission, distinct-device replay, two-leg Bitcoin mainnet Spark settlement (`200 sats` worker + `50 sats` validator, `0` sat fee), and reconciled kind `30201` pool aggregates.
2. **Connect `ch052` (The Gym), `ch054` (Releases as signed events), and `ch055` (Everyone gets it):**
   - Ground these chapters in W6.3 (`#13928`, `docs/verification/executor-network-effect-h7.md`): signed manifests, Ch 224 shadow-to-promote rollouts of newly promoted LoRA routers and compiled weight modules, and live dashboards showing network-effect gains without silent regressions.
3. **Connect `ch069` (Models we serve):**
   - Add `Inkling-2-Mini` + `hybrid-exec-v1` + `openagents module` alongside Elephant/Mammoth decision models.
4. **Update `ch078-what-one-point-zero-means.md`:**
   - Update the "what we have shown vs. not yet shown" ledger: the end-to-end compounding loop (exact compiled weights → hybrid routing → multi-device Pylon verification → live Bitcoin mainnet two-leg settlement → Byzantine-safe DiLoCo training → shadow-to-promote rollout) is now **proven on our own hardware**; what remains for 1.0 is independent outside operators at scale and completing the book's own Desktop/iOS implementation chapters (`ch001`–`ch011`).

---

### 3.7 `metaverse-engine/book` & `everglade/book` — Parallel Consumers (Keep Cleanly Scoped)

**Role in the buildup:** The two world books should **not** be bloated with WASM-in-weights, LoRA routing, or DiLoCo training. Their job is to prove that the exact same bounded/deterministic/checkable/payable/ownable spine also powers real-time 3D realms.

**What needs updating (lightweight, high-clarity):**
1. **Update `docs/books/capstones/a-guard-decision-is-paid-for.md`:** Note that Steps 7–8 (escrow ledger → real Bitcoin/Spark payment) are no longer blocked on an unbuilt settlement bridge—Ch 238 W4.3's live two-leg Breez Spark settlement (`pay/executor-settlement.coax`) solves the exact payment boundary this capstone needed.
2. **Keep `everglade/book` and `metaverse-engine/book` focused on their own completion gates** (from the 2026-11-20 audit: register `everglade` in `book.py`, rename `aethyr` alias, add missing transcripts to `metaverse-engine` `ch002`/`ch004`/`ch006`).

---

## 4. Phased Execution Plan

| Phase | Status | Scope | Key Deliverables | Exit Gate |
| --- | --- | --- | --- | --- |
| **Phase 0** | **Done (`927815bb18`)** | Index, Capstone, Glossary, `book.py` green | 1. This audit<br>2. New master capstone (`executor-network-flywheel.md`)<br>3. Updated `docs/books/README.md`, `paths.md`, `glossary.md`<br>4. Registered `exec-vm-gpu.coax`, `hybrid-exec.coax`, `wasm-exec.coax` in `psionic/book/examples.json` | `book.py --book <ALL>` and `book_routes.py --check` passed |
| **Phase 1** | **Done (`851ae8a388`)** | `psionic/book` Part XXIII & new Part XXVIII | Upgraded `ch229`, `ch231`, `ch232`, `ch234`–`ch237`; updated `chapters.json` `qualified_targets` (`metal`/`cuda`); added **Part XXVIII (`ch272`–`ch274`)** with 3 runnable examples & verified debug/release transcripts; split Core ML & Executor Spine vs. Model Targets Appendix in `psionic/book/README.md` | All Part XXIII & XXVIII chapters verified by `book.py --book psionic` |
| **Phase 2** | **Done (`e00af5795f`)** | `nostr/book` Ch 78, 81, 82, 100 & `bitcoin/book` Ch 123 | Updated Nostr Ch 78, 81, 82, 100 for `cj-executor`/`cj-validate`/`cj-training`, kind `1985` verdicts, reconciled kind `30201` feeds & ATIF/lineage contracts; implemented Bitcoin Ch 123 (`paying-for-requests.coax` + `pay/executor-settlement.coax`) | `book.py --book nostr` and `book.py --book bitcoin` passed |
| **Phase 3** | **Done (`424345b388`)** | `coder/book` Ch 8, 9, 12, 16, 18, 23 & `openagents/book` Ch 1, 52, 54–55, 69–70, 78 | Wired W2.2 weight modules, `hybrid-exec-v1`, `openagents mcp --domains executor`, and W6.2 redacted `ATIF-v1.8` `contribute` into Coder; fixed Ch 1 dead `chat_screen.coax` refs, rewrote OpenAgents Ch 70 (Pylons), and updated Ch 52/54–55/69/78 around live W6.3 rollouts | `book.py --book coder` and `book.py --book openagents` (with `provenance.json`) passed |
| **Phase 4** | **Done (this change)** | `coax/book` Ch 16, 17, 23, guard capstone & `docs/books/routes.json` | Updated Coax Ch 16/17/23 with cross-GPU exactness (`exec-gpu-v1`) and `#authority` settlement/Byzantine proofs; updated `a-guard-decision-is-paid-for.md`; added and executed runnable route `executor-network-flywheel` in `docs/books/routes.json` | `coax/book/check-structure.py`, `book_routes.py --check`, and `book_routes.py --route executor-network-flywheel` all passed end-to-end |

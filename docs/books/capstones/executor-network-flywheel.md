# Capstone: the Executor Network closes the loop

> **Status: verified on real GPUs and live Bitcoin mainnet (`#13912`–`#13928`); Phases 0–4 book alignment complete with runnable route `executor-network-flywheel`.**
> Every step below maps to live, checked Coax code, updated book chapters across all six books, and recorded verification artifacts on `main` (Apple M5 Max Metal 4, NVIDIA RTX 4080 CUDA, live Coax Nostr relay, and live Breez Spark Bitcoin mainnet). See [the 2026-11-21 Executor Network flow audit](../audits/2026-11-21-executor-network-flow-audit.md) for the full cross-book mapping.

A developer runs a coding task in **OpenAgents Desktop** or **Microcoder**. When the agent needs exact computation—verifying a unified diff, checking a JSON schema, scheduling a dependency DAG, or computing a satoshi/token budget—its hybrid model (`Inkling-2-Mini` + a rank-8 LoRA router) routes to a **frozen, digest-pinned compiled weight module** (`exec-vm-v1` / `wasm-exec-v1`) with KV cache preserved and zero silent hallucinations. Across the network, consumer Macs and NVIDIA GPUs running as **Pylons** (`NIP-PYLON`) execute compiled workloads, independently replay each other's traces on distinct physical devices, and settle **both** the worker and the validator automatically on **Bitcoin mainnet via Breez Spark**. Opt-in, secret-redacted `ATIF-v1.8` coding trajectories and verified execution traces feed bounded **DiLoCo** training windows on Pylons, whose updates pass a **6-tier Byzantine verifier** and a **Quarantine Optimizer** (`canonical → quarantine → promoted`) before shipping back to every user's coding agent via signed shadow-to-promote rollouts.

This capstone is the unifying arc of the OpenAgents library: it turns the five-step argument (*Bounded → Deterministic → Checkable → Payable → Ownable*) into a sixth step—**Compounding**.

## Why this route

- It crosses **six books** in order: **Coax** (bit-exact multi-GPU kernels, `proto/`, `#authority`), **Psionic** (compiled transformer weights, hybrid models, replay, Byzantine-safe DiLoCo training), **Nostr** (`NIP-PYLON` lanes, signed verdicts, reconciled kind `30201` pool aggregates), **Bitcoin** (Breez Spark two-leg mainnet settlement under rolling caps), **Coder** (weight modules in Microcoder, MCP executor tools, redacted `ATIF-v1.8` contribution), and **OpenAgents** (Desktop/CLI UX, Gym, signed shadow-to-promote rollouts).
- Every stage is **falsifiable**: tampered weights, planted compiler bugs, same-device self-validation, duplicate payouts, 7 Byzantine training attacks, and 3 held-out regressions are all explicitly tested and blocked.
- It obeys the standing honesty rule: a bare CPU is orders of magnitude faster for standalone loops, and every benchmark prints native CPU speed beside compiled-weight speed. The point is verifiable, composable, exact execution native to neural agents and open compute markets.

## The trace

| # | Step | Book Owner | Live Code & Verification Evidence Today | Book Status |
| --- | --- | --- | --- | --- |
| 1 | **Exact compiled transformer substrate:** Compile the full VM (`54/54` programs) and 93-layer 784k-weight zero-fallback WASM transformer (`wasm-core-i32-v1`, 9×9 Sudoku, 10×10 Hungarian, Coder Ch 8 programs: `398,722` steps, `0` divergences) into dense `binary64` safetensors weights with `O(log t)` balanced AVL parabolic attention (`hull-v2`). | [Coax 16–17](../../../coax/book/ch16-determinism.md); [Psionic 226–233](../../../psionic/book/ch231-an-interpreter-in-weights-or-one-program-in-weights.md) | `psionic/examples/wasm-exec.coax`, `psionic/lib/wasm-transformer-benchmarks.coax`, [W1.2 record](../../verification/executor-w12-wasm-exec.md), [2026-11-21 record](../../verification/2026-11-21-wasm-exec-v1.md) | Implemented (`cpu`, `metal`, `cuda`); Ch 231–232 & Coax 16–17 updated |
| 2 | **Cross-GPU exact execution & optimal scheduler:** Optimal liveness/slot-reuse scheduler cuts weights by `26.4%`; single-source Coax kernels run bit-exact on **Apple M5 Max Metal 4** (`57.5k steps/s` batched `i64`) and **NVIDIA RTX 4080 CUDA** (`91.9k steps/s` `i64`, `63.8k steps/s` native `f64`). | [Coax 17](../../../coax/book/ch17-gpu.md); [Psionic 229–230](../../../psionic/book/ch229-compile-a-gate-graph-into-a-block.md) | `psionic/examples/exec-vm-gpu.coax`, `psionic/platform/exec-vm-gpu-bridge.coax`, [W1.3 record](../../verification/executor-w13-gpu-sched.md) | Implemented (`cpu`, `metal`, `cuda`); Ch 229 & Coax 17 updated |
| 3 | **Composable weight modules ("Agentic npm"):** Link and digest-verify compiled weight modules in `~/.openagents/modules/` via `openagents module`, shipping 4 Class-E coding-agent cores (`oa-exec-diff-verify`, `oa-exec-json-schema`, `oa-exec-dep-sched`, `oa-exec-arith-budget`) over `proto/openagents/executor/v1alpha1/`. | [Coax 18](../../../coax/book/ch18-protocol.md); [Psionic 237](../../../psionic/book/ch237-composable-weight-modules.md) | `psionic/lib/weight-module.coax`, `proto/openagents/executor/v1alpha1/`, `openagents module` CLI (`#13914`, `#13917`) | Implemented; Ch 237 updated with `module-v2` & 4 cores |
| 4 | **Hybrid LLM + exact cores & routing-only distillation:** Wire the real 16.38 GB `Inkling-2-Mini` GGUF on Metal 4 with a rank-8 LoRA routing adapter (`11/48 → 48/48` on held-out tasks, `0` silent hallucinations, KV cache reused across exact calls); prove on `4.92M` tokens that distillation should train routing (`H2_ROUTING_ONLY`), keeping execution in frozen compiled cores. | [Psionic 235–236](../../../psionic/book/ch236-a-frozen-exact-core-with-a-learned-interface.md), [266–271](../../../psionic/book/PLAN.md#part-xxvii-looped-transformers-inkling-2-mini) | `psionic/examples/hybrid-exec.coax`, `psionic/lib/hybrid-exec.coax`, [W3.1 record](../../verification/executor-w31-hybrid-exec.md), [W3.2 H1/H3/H5 record](../../verification/executor-distill-h1-h3-h5.md) | Implemented (`cpu`, `metal`); Ch 235–236 updated & linked to Part XXVII |
| 5 | **Multi-device Pylon execution & independent replay:** Workers advertise `cj-executor` only after passing a digest self-test; a second distinct physical device replays traces (`cj-validate`, Ch 234) and emits signed NIP-32 kind `1985` verdicts; raw traces stay quarantined until verified. | [Nostr 82, 100](../../../nostr/book/ch082-pylons-environments-and-deploys.md); [Psionic 218, 234](../../../psionic/book/ch234-exact-replay-as-a-verification-class.md) | `nostr/nips/openagents/NIP-PYLON.md`, `psionic/lib/pylon/`, `psionic/lib/peer/audit.coax` (`#13919`, `#13921`) | Implemented; Nostr 82/100 & Psionic 234 updated |
| 6 | **Live Bitcoin mainnet two-leg settlement:** Verified across Apple M5 Max worker + `coderos-4080` Linux validator over a live Coax Nostr relay, paying both legs on **Bitcoin mainnet via Breez Spark** (`200 sats` worker + `50 sats` validator, `0` sat fee, zero double-spend on restart), gated by `#authority` rolling 24h UTC caps and default-OFF mainnet arming, reconciled into signed `NIP-PYLON` kind `30201` pool aggregates. | [Psionic 238](../../../psionic/book/ch238-pay-for-verified-work.md); [Bitcoin 121–123](../../../bitcoin/book/ch123-paying-for-requests.md); [Nostr 82](../../../nostr/book/ch082-pylons-environments-and-deploys.md) | `psionic/lib/peer/pay.coax`, `pay/executor-settlement.coax`, `psionic/lib/peer/settled-feed.coax`, [W4.3 mainnet record](../../verification/executor-w43-mainnet-settlement.md) | Implemented: Psionic 238, Bitcoin 123 (runnable), Nostr 82 |
| 7 | **Quarantined decentralized training:** Pylons run bounded Local-SGD / DiLoCo training windows against deduplicated, exposure-ledger-checked shards; a `#authority` 6-tier Byzantine verifier blocks `7/7` planted attacks; the Quarantine Optimizer (`canonical → quarantine → promoted`) blocks `3/3` regressions and supports atomic lineage rollback. | [Psionic Part XXVIII (Ch 272–274)](../../../psionic/book/ch272-bounded-training-windows-and-diloco.md); [Psionic 239–241, 247](../../../psionic/book/ch241-gates-that-can-say-unverifiable.md) | `psionic/lib/peer/training-window.coax`, `psionic/lib/peer/train-verify.coax`, `psionic/lib/peer/quarantine-opt.coax`, `psionic/tools/diloco-round.coax` (`#13923`, `#13925`, `#13927`) | Implemented: Ch 272, 273, 274 with runnable examples & verified transcripts |
| 8 | **Coding-agent integration & network-effect flywheel:** Microcoder, OpenAgents Desktop, and `openagents mcp --domains executor` invoke compiled weight modules and promoted hybrid models; opt-in, BIP-39/PII-redacted `ATIF-v1.8` sessions (`openagents contribute submit`) feed new shards; W6.3 signed shadow-to-promote rollouts deliver verified gains to every user. | [Coder 8, 9, 12, 16, 18, 23](../../../coder/book/PLAN.md); [OpenAgents 52, 54–55, 69–70, 78](../../../openagents/book/PLAN.md) | `coax/lib/microcoder/executor-guard.coax`, `apps/openagents-cli/executor-mcp.coax`, `apps/openagents-cli/contribute.coax`, [W6.1 record](../../verification/executor-w61-coding-agent.md), [W6.3 H7 record](../../verification/executor-network-effect-h7.md) | Updated across Coder & OpenAgents chapters |

## Broken / falsifier cases (checked)

1. **Planted compiler & scheduler bugs:** Extended Ch 230 differential fuzzing detects 100% of planted slot-reuse, scheduling, and emission bugs across attention+FFN blocks (`psionic/examples/exec-vm-gpu.coax`).
2. **Weight tampering & digest mismatch:** 1-byte weight tampering or mismatched `(base_model_sha256, core_module_sha256)` triggers atomic refusal before execution (`psionic/examples/hybrid-exec.coax`).
3. **Out-of-range inputs:** Typed `<exec_refuse>` traps replace silent wrong answers (`0` silent hallucinations across `48/48` held-out tasks).
4. **Same-device validation & duplicate payout:** `#authority` policy refuses `validator_pubkey == worker_pubkey`, `validator_device_ref == worker_device_ref`, and duplicate `(receipt_digest, leg)` payouts across restarts (`psionic/lib/peer/pay.coax`, `pay/executor-settlement.coax`).
5. **Byzantine training attacks & regressions:** Zero-delta freerider, random noise, inverted-sign poisoning, backdoor injection, and stale-checkpoint replay are 100% rejected (`7/7` attacks blocked), and checkpoints that regress held-out decision, probability, or exactness gates never leave quarantine (`3/3` regressions blocked).

## Verification records & runnable checks

The full GPU and mainnet receipts live in `docs/verification/`:
- [W1.2 WASM transformer (`executor-w12-wasm-exec.md`)](../../verification/executor-w12-wasm-exec.md)
- [W1.3 Metal 4 & RTX 4080 CUDA exact kernels (`executor-w13-gpu-sched.md`)](../../verification/executor-w13-gpu-sched.md)
- [W3.1 Hybrid executor on `Inkling-2-Mini` (`executor-w31-hybrid-exec.md`)](../../verification/executor-w31-hybrid-exec.md)
- [W3.2 Distillation falsification (`executor-distill-h1-h3-h5.md`)](../../verification/executor-distill-h1-h3-h5.md)
- [W4.3 Live Bitcoin mainnet two-leg settlement (`executor-w43-mainnet-settlement.md`)](../../verification/executor-w43-mainnet-settlement.md)
- [W6.1 Coding-agent integration (`executor-w61-coding-agent.md`)](../../verification/executor-w61-coding-agent.md)
- [W6.3 Shadow-to-promote & H7 network-effect qualification (`executor-network-effect-h7.md`)](../../verification/executor-network-effect-h7.md)

Representative runnable route & local `#authority` checks:

```shell
python3 scripts/book_routes.py --route executor-network-flywheel \
  --coax /absolute/path/to/coax --receipt /tmp/executor-network-flywheel.json
coax/bin/coax test psionic/lib/peer/pay.coax
coax/bin/coax test pay/executor-settlement.coax
coax/bin/coax test psionic/lib/peer/training-window.coax
coax/bin/coax test psionic/lib/peer/train-verify.coax
coax/bin/coax test psionic/lib/peer/quarantine-opt.coax
```

## Chapters added or upgraded for this capstone

All four phases in [the 2026-11-21 Executor Network flow audit](../audits/2026-11-21-executor-network-flow-audit.md) have landed on `main`:

| Book | Chapters added or upgraded |
| --- | --- |
| **Coax** | Upgraded Ch 16 (cross-GPU exactness), Ch 17 (Metal 4 + CUDA exact kernels), Ch 23 (`#authority` settlement & Byzantine proofs) |
| **Bitcoin** | Implemented Ch 123 (Spark bridge, two-leg settlement, rolling UTC caps) with runnable example `bitcoin/examples/paying-for-requests.coax` |
| **Nostr** | Upgraded Ch 78, 81, 82 & 100 (`cj-executor` / `cj-validate` / `cj-training` lanes, kind `1985` verdicts, reconciled kind `30201` feeds, redacted `ATIF-v1.8` & lineage schemas) |
| **Psionic** | Upgraded Ch 229, 231, 232, 234–237 (`qualified_targets` + W1–W3 artifacts); added **Part XXVIII (Ch 272–274)** with runnable examples & verified transcripts for DiLoCo, 6-tier Byzantine verification, and the Quarantine Optimizer |
| **Coder** | Upgraded Ch 8, 9, 12, 16, 18, 23 (W2.2 weight modules, `hybrid-exec-v1`, MCP executor domain, redacted `ATIF-v1.8` `contribute`) |
| **OpenAgents** | Rewrote Ch 70 (Pylons), fixed Ch 1 dead paths, and updated Ch 52, 54–55, 69, 78 around the live Executor Network flywheel |

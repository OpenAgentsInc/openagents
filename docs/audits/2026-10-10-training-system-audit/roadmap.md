# Training system roadmap

Date: October 10, 2026. Baseline: `3168c986aa`. This roadmap acts on the
[training system audit](README.md). It is a proposal for the owner and does not
yet commit scope. Each item becomes an issue in project 22 when it is filed.
Finding numbers (G1–G20) refer to the audit's
[Gaps and risks](README.md#gaps-and-risks) list.

## Decisions

These are taken up front so that the work below does not branch.

1. **The first training customer is our own decision layer.** We train Clef and
   Kev decision heads, calibration maps, and LoRA adapters for questions Coder
   and the Verse already ask Jev:
   - file relevance
   - chat-router sub-decisions
   - task independence
   - delegation needing context
   - the six Verse/Alice questions

   Success is the Coder router and judges running locally or on a Pylon at
   Jev-level quality for less money. No Psion pretraining. No Tassadar learned
   executors.
2. **Labels come from outcomes first.** These include the files a fix commit
   changed, branch merge conflicts, delegation failures, paid-outcome
   acceptance (`inference/src/outcomes.rs`), and ext-eval verdicts. Jev answers
   are kept as *teacher* targets in their own field. They are never treated as
   ground truth. This matches the existing `tenancy::training` refusal of
   unconfirmed model labels.
3. **One trainer, in `crates/psionic`.** We port Kev's recipe (LoRA plus a
   pointer or joint head, with cross-entropy and calibration terms) into
   `psionic-train` and run it on `psionic-backend-cuda` (4080), and on the CPU
   for head-only training. Metal has no backward ops today, so Metal training
   is a separate Later item with its own backward-kernel deliverable and gate. The Apple toolkit stays a Lev-only path and becomes a recurring job
   keyed on `baseModelSignature`. Upstream `kev/train.py` is a reference only.
4. **Pylon order: decisions, then evals, then training.** Training on
   contributor machines comes last. It waits for signed eval receipts, real node
   keys, and an owner reversal of kitchen-sink ledger X6 (paid distributed
   training, paused). Settlement uses `pay-ledger` and
   `nostr::pylon` on Bitcoin rails only. It does not use the Treasury or Nexus
   path in `qwen_legal_pylon_*`.
5. **Cut serving loose from training.** `psionic-openai-server` stops linking
   `psionic-train`, `psionic-eval`, and `psionic-research`. The rest of the
   imported training code stays frozen and quarantined. We port lanes from it
   only when one of the decisions above needs them.
6. **Every number carries an evidence class.** Each class is one of
   `measured | authored | sample | contract_only`. Nothing below `measured`
   reaches a Verse board, `/promises`, the benchmark page, or copy.

## Now: 0–2 weeks

The goal is to make the existing code honest and safe, and to land the
inference prerequisites.

| # | Goal | Deliverable | Owning path | Gate | Depends on |
| --- | --- | --- | --- | --- | --- |
| N1 | Serving no longer builds training code (G5) | Put every `tassadar_*` and `psion_*` module in `psionic-serve/src/lib.rs` (about 45 of its `mod` lines) behind an off-by-default `legacy-research` feature, and make `psionic-train`, `psionic-eval`, and `psionic-research` optional dependencies enabled only by that feature. Move the symbols the core path needs (`load_qwen38_lm_head_adapter_safetensors`, used at `qwen35.rs:36`, and `GemmaE4bCudaAdapterCheckpoint`, `GemmaE4bCudaAdapterExportedArtifact`, and `GemmaE4bServedBaseModelBinding`, used at `gguf.rs:34`) into the existing `psionic-adapters` crate. Moving types alone cannot remove the dependency, because the Tassadar modules call about 100 `psionic_eval` report builders. | `crates/psionic/crates/psionic-serve`, `psionic-adapters` | `cargo tree -p psionic-serve --no-default-features` shows no `psionic-train`, `psionic-eval`, or `psionic-research`; `psionic-openai-server` builds on coderos-4080 and Clef tests pass; the CUDA build time before and after is recorded in the issue | — |
| N2 | Unsafe receipts cannot pay (G1) | Mark `qwen_legal_pylon_dispatch` `Production` mode and both swarm runtimes as refused unless a caller supplies a key. Remove or test-gate every `fn …signing_key…(…) -> SigningKey` in `psionic-train/src` that derives a key from public strings. | `psionic-train/src/qwen_legal_pylon_{dispatch,training_job}.rs`, `coordinator_live_buymode_dispatch.rs`, `swarm_first_live_runtime.rs`, `psion_google_two_node_swarm_runtime.rs`, `adapter_reference_program.rs`, `signed_node_identity_contract.rs` | A test asserts that `Production` dispatch with no node key returns a typed refusal; `git grep -n 'fn [a-z_]*signing_key[a-z_]*(.*-> SigningKey' crates/psionic/crates/psionic-train/src` returns only `#[cfg(test)]` items | — |
| N3 | Evidence classes (G2, G3, G19) | Add an `evidence_class` field to `ResearchExecutionReceipt`, eval reports, training-visualization bundles, and status packets. Backfill the authored-fixture table from the audit as `authored` or `sample`. `ResearchRunner` marks its 7 synthesized families `sample`. Add a row to `INVARIANTS.md`: "A training or eval number is shown only when it is `measured`." | `psionic-research/src/runner.rs`, `psionic-train/src/remote_training_visualization.rs`, the `examples/*_fixtures.rs` named in the audit, `INVARIANTS.md` | A unit test fails any `promoted`, `decision_grade`, or `*_green` packet whose evidence class is not `measured` | — |
| N4 | Retract overclaims (G3, G4) | Rewrite `reproduced_claim` in `tassadar_rust_only_article_closeout_audit.rs:99` to match the history doc: a bounded closeout (`green_bounded`, `arbitrary_c_or_wasm_not_claimed`) whose article parity the in-tree audits call "unearned". Rename the reference-linear gate's claim to "tokenizer round-trip". Relabel the `psion_executor_4080_*` packets "open-adapter LM-head host run". | `psionic-eval`, `psionic-train` | Grep shows no "reproduces the full" or "decision-grade" claim on a non-`measured` packet | N3 |
| N5 | Docs say what is in the tree | Fix every row of the audit's contradiction table: the Psionic READMEs, the history doc (its "no Tassadar code remains" predates the `737f94a17c` re-import: state what came back, list the deletion commits from the audit's [git history table](README.md#deletion-and-re-import-history-git) as recovery points, and correct the workspace guide's nonexistent `d7f53fccc` to `17aa21b544`), history "still live", `psionic-and-pylon.md`, glossary entries for NIP-PYLON and in-repo Psionic, `compute-for-bitcoin.md`, `roadmap.md` L303, `verse-compute.md:3`, `relevance-visualizer.md`, the Lev docs, the kev mesh-plan Treasury reference, the CoderOS audit, `size.md`, the kitchen-sink payment wording, and the transcripts README (289 row, Psionic/training row, Pylon disambiguation) | `docs/`, `crates/psionic/README.md` | Every contradiction row is resolved or annotated in a single docs commit | — |
| N6 | Hygiene (G17, G18) | Add `crates/psionic/target/` to `.gitignore`. Make the `cmp-*.json` files valid JSON. Mark GGUF-gated Clef tests `#[ignore = "needs PSIONIC_CLEF_GGUF"]`. Replace `expect_err` at `open_adapter.rs:1845` with a capability probe. Set the default for `PSIONIC_TRAIN_CPU_BUDGET` explicitly in the trainer CLI. Replace the stale `lev::calibrate::admit` references in `crates/gym/gates/probability-v1.json` and `probability-v2.json`. | as named | `git status` no longer shows `target/`; `jq . cmp-*.json` passes | — |
| N7 | Land Clef M2 CUDA (#11195), owned by the agent already on it | Commit the in-tree uncommitted prefill work. Clear the clippy `print_stderr` lint on its `eprintln!` calls. Make `auto` mode refuse, or report the CPU fallback in the response rather than only on stderr, when a weight type is unsupported (Q6_K), or add Q6_K support. | `psionic-backend-cuda/src/clef_prefill.rs`, `psionic-serve/src/qwen35/clef_cuda.rs` | `bench_latency.py` on the 4080 meets the M2 gate (≤ llama.cpp at 1k/4k/16k tokens, ≤ 7.5 GB VRAM at 16k), and the receipt shows `backend: cuda`; the N9 parity gate passes against the CPU lane | N9 (the gate can land after the commit, but M2 is not done until it passes) |
| N8 | Seed the outcome dataset (G8, G9) | Mine closed issues with `(#N)` fix commits into a file-relevance corpus. Each row holds the issue, candidate files (fix, sibling hard negatives, recent, random), and changed-file labels. Use the `tenancy::training` corpus format with training, calibration, development, and locked partitions, split by issue hash. Commit a manifest only (issue number, fix commit SHA, path, label, and content digest) and rebuild contents at the pinned commit, since the bench keeps file contents out of the repository. Exclude the 10 bench issues, which tuned thresholds in-sample, from development and locked. | `scripts/bench/clef-relevance-dataset.py` → `crates/gym/suites/file-relevance-v1*` (manifest), #11210 | At least 300 issues and 3,000 labelled files; `tenant-train check` passes (provenance, no cross-partition near-duplicates); the locked partition is read zero times | — |
| N9 | Kernel parity outranks speed | Port the deleted kernel-optimization parity gate to Rust (recover with `git show fae80bde79^:packages/tassadar-executor/src/kernel-optimization-parity.ts`, 340 lines). A new kernel is accepted only with an exact-replay (or stated-tolerance) verdict against an independent reference lane *and* a recorded tok/s gain over the baseline; a wrong-but-faster kernel is a typed rejection. Make it the acceptance gate for the Clef CUDA prefill work (N7) and every later backend kernel, with `fixtures/clef/tools/layer_parity.py` as the first harness it wraps. | `psionic-eval` (gate), `psionic-backend-cuda` (tests) | A test feeds a deliberately wrong but faster kernel and gets a rejection; the N7 prefill kernel passes against the CPU lane at 1k/4k/16k tokens with the tolerance written in the receipt | — |

## Next: 2–6 weeks

The goal is the first trained artifact that ships to a product, end to end.

| # | Goal | Deliverable | Owning path | Gate | Depends on |
| --- | --- | --- | --- | --- | --- |
| X1 | Calibrate Clef-Flash (G10) | Measure raw Clef-Flash F1 at threshold 0.5 on N8 development first and record it (the 0.44 bench figure is batch-prompt F1 on 56 files from 10 issues, not this dataset). Fit a per-question Platt or temperature map on the calibration partition of N8. Ship it as a versioned artifact and add a `calibration_digest` beside `head_digest` in the `/v1/systemone` provenance. | `crates/gym/src/{calibrate,gate}.rs`, `psionic-serve/src/clef/` | The map passes the `probability-v2` gate (`crates/gym/gates/probability-v2.json`, evaluated by `gym::gate`) on development; locked F1 at 0.5, read once, improves over the recorded baseline and reaches ≥ 0.75; ECE does not rise | N8 |
| X2 | One owned trainer for decision heads (G7) | Port the Kev recipe as a `decision-train` lane in `psionic-train`, in two steps. **X2a:** head-only training (joint or pointer head) on cached final hidden rows that `psionic-serve` exports to a file, with no crate dependency from train on serve. **X2b:** a rank-r q/k/v/o LoRA with backward through the qwen35 attention layers in `psionic-train`/`psionic-transformer`, started only after X2a beats the baseline; the serve qwen35 path is inference-only and cannot supply these gradients. The loss is cross-entropy + calibration term + teacher KL (when the teacher field is present). It runs on CUDA (4080) and on the CPU for head-only training. Gradient checks use `cs336_a1_real_gradient_reference` as the oracle pattern. The outside baseline is the upstream Kev `--init_from` warm start and the Unsloth `DecisionTrainer` recipe in [docs/training.md](../../training.md): run one of them on the same file-relevance-v1 splits first, and X2a must match or beat that number. | `psionic-train/src/decision_train.rs` (new), `psionic-backend-cuda` backward kernels | The gradient check on a tiny config is within 1e-4 of central differences; one 4080 X2a run on file-relevance-v1 completes, writing a `measured` receipt with recipe digest, data digest, seed, loss series, and wall time | N1, N8 |
| X3 | Serve adapters beyond the LM head on GPU (G7) | Extend `psionic-adapters` and serve so attention LoRA (q/k/v/o) and head overlays load on the CUDA and Metal qwen35 lanes. Add `/psionic/management/adapters/{promote,rollback}`, wired to `promote_exported_revision`, with `ServedModelRevisionIdentity` in every receipt. | `psionic-adapters`, `psionic-serve/src/{qwen35,gguf,openai_http}.rs` | The direct-forward and served-forward logits of a trained adapter differ by at most 1e-3; rollback restores the base digest; the receipt names the adapter digest | X2 |
| X4 | Gate and promote through the Gym | A candidate passes `decision-v1` (gain ≥ 2 standard errors computed per comparison) and `probability-v2` on development, does not raise ECE, and is confirmed once on locked. If a trial-variance floor is needed, file it as `decision-v2` after #9370 measures the spread. Sealing goes through `tenancy::training::seal_candidate`. `training/tenant-demo/run.sh` replaces `make_artifacts.py` with a real X2 run. The sealed candidate then passes `tenancy::admission` before X5 or X9 can bind it. | `crates/gym/gates/`, `crates/tenancy/src/training.rs`, `training/tenant-demo` | `tenant-demo` rerun with a real X2 run gives a sealed candidate with real tensor digests; the measurement doc lands in `docs/decision-models/measurements/` | X2 |
| X5 | Local door in Jev (#11191) | A `local-clef` door in `crates/jev/src/doors.rs` that uses the X1/X4-admitted head, with failover to hosted Jev on refusal or timeout. A fixed share (e.g. 5%) of local decisions goes to Jev as a shadow, and the agreement rate is written to Gym rows weekly. If agreement falls by more than 2 standard errors on 2 consecutive weeks, the door goes back to Jev automatically and an issue is filed. | `crates/jev` | On file-relevance-v1 locked, read once, in the same prompt mode as the Jev baseline: F1 within 0.03 of Jev; cost per 1k files ≤ ⅓ of Jev's measured $0.050 batched, with the local cost model (power plus amortization) stated in the measurement doc; Coder uses it by default with no setting to change | N7, X1, X4 |
| X6 | Gym training feed (G12) | `psionic-train` writes `openagents.gym-training-summary.v1` for every `decision-train` run (run id, evidence class, metric series, recipe, data and artifact digests). gym-bridge drops its "unverified declaration" label when the run receipt verifies. | `psionic-train`, `crates/gym-bridge/src/sources.rs` | `gym-bridge` loads the X2 summary with the verified label, as shown by a test in `crates/gym-bridge`, with no hand-written JSON. The Verse drawing is L3. | X2, N3 |
| X7 | Consented trace intake (G8) | Build `trace-admit`. It applies `secret-screen` to uploaded traces (#11109, #11178), uses validator tiers 0–3 from the revival doc, quarantines before admission, records the policy basis (the `privacy.md` section) and the account's opt-out state for each item, and refuses items from opted-out accounts at admission. Deleting a chat or trace tombstones its corpus items, and the next sealed candidate excludes them. | `crates/secret-screen`, the trace upload service, `crates/tenancy/src/training.rs` | No admitted item lacks a policy-basis record; 100% of planted test secrets are removed; deleting a planted trace removes it from the next corpus digest; one Coder-turn suite (`coder-turns-v2`) is rebuilt from admitted traces only | — |
| X8 | Training state in Postgres (G8) | Amend the Organization decision row in `docs/data/schema.md` to add a `training` schema (or place the tables under `registry`) in the same change, then migrate tables for corpora, partitions, recipes, trials, candidates, and locked-read ledger rows | `docs/data/schema.md` and its implementation | Two machines racing to read a locked partition: one succeeds, one gets a typed refusal | X4 |
| X9 | Pylon sells decisions (#11192) | Relax `decision_offer.rs` to accept an admitted adapter identity (`artifact_signature` from the seal) and Clef CUDA. Add a `pylon/decision` capability posting to `/v1/systemone`, with the artifact and head digests in the 3201 receipt. Add decision probes with pinned distributions to `pylon check`. A Pylon advertises a capability only after a self-test receipt for it (no overclaim), porting the Tassadar capability envelope (`psionic-provider/src/lib.rs:630`, the rule of the deleted `capability-envelope.ts`) into `crates/capability` and announcing it over NIP-CAP. | `crates/gateway/src/decision_offer.rs`, `crates/pylon`, `crates/nostr/src/pylon.rs` | coderos-4080 serves `decide` jobs; the probe catches a tampered head (fixture); league rows show pass rate and cost per accepted decision; a Pylon whose self-test fails or is missing cannot announce `pylon/decision` | N7, X4 |
| X10 | Local benchmark page (twitter item 11, #11131) | Re-runnable tokens/sec for Qwen3.5 0.8B/2B on 4080 CUDA and M5 Metal, Psionic against llama.cpp and Ollama, and GPT-OSS 20B against llama.cpp on the 4080, plus Clef decisions/s, with commands to re-run. If GPT-OSS 20B is not measured, remove that claim from the twitter ledger and from the copy. | `scripts/bench/`, the benchmark page | An independent re-run lands within 10% of the published numbers; every figure is `measured` | N7 |
| X11 | Exact-replay receipts gate corpus admission | Deterministic Psionic inference, Clef decisions, and Gym trial steps write a replay receipt (input digest, artifact and head digests, output digest). Before a row enters a sealed corpus (N8, X7) or a gate run (X4), a second run replays it and records `exact_replay | failed | unverifiable`; only `exact_replay` rows are admitted. Reuse `crates/plugin/src/replay.rs` (receipt `openagents.plugin-invocation-receipt.v1`, #9901) for the verdict shape and `psionic-compiler/src/tassadar_alm_trace_replay.rs` for windowed spot checks that name the divergent step, rather than writing a third replay format. | `psionic-serve` receipt module, `crates/receipts`, `crates/gym`, `crates/eval-runner`, `crates/tenancy/src/training.rs` | A planted nondeterministic or tampered row is refused at admission with the divergent step named; every row of the next sealed file-relevance corpus carries an `exact_replay` verdict | N8, X4 |

## Later: 6 weeks and beyond

The goal is to widen labels, put training on the Verse, and only then move
training onto Pylons.

| # | Goal | Deliverable | Owning path | Gate | Depends on |
| --- | --- | --- | --- | --- | --- |
| L1 | Router decisions run locally (#11193) | Split the router's ~24.5k-token main decision into sub-16k questions. Train on `chat-router-v5` plus outcome-labelled routes from `outcomes.rs`. | `crates/coder` router, `crates/gym/suites/chat-router-*` | Router A/B under `router-v1`: the `scripts/chat-goldens.sh` bar holds; p50 latency under 6 s; Jev is used only on refusal | X4, X5 |
| L2 | Verse/Alice questions calibrated | Run the six unmeasured question sets as one Gym batch, fit and admit thresholds, and train a short-state head (they fit Clef limits). Restore or close the missing owner entries. | `crates/coder/src/task/agent_*`, `questions/*.json` | Every set has an admitted map; townsfolk decisions run on `local-clef` or a Pylon | X4, X9 |
| L3 | Verse training scene (G12; ep 240; M4) | A Gym building object that draws a run from X6 receipts only: a "getting better" line drawn from the eval series, with practice rounds as steps (the loss series stays in the receipt link and is never labelled on screen), checkpoint promotions as events, Pylon decision jobs as beams. Users see "practice", "checked", and "got better". No system words. | `crates/verse-gym`, `crates/verse` | A capture where every drawn value links to a receipt; a `sample` bundle is refused at load | X6 |
| L4 | Trainer XP tied to model improvement | Implement the `trace-admit` NIP-XP rule, plus an `eval-adopt`-style rule that awards XP only when an admitted contribution moves a gated suite. XP never converts to money (NIP-XP). | `crates/xp-ledger`, `nostr::xp` | A test where a contributor's admitted traces raise a locked-suite score, and the XP award is derived from the gate receipt | X7, X4 |
| L5 | Pylon eval jobs | Gym suites run as `25920` execution jobs on Pylons, paid per accepted result through `Split::PylonJob`. Validators re-run a sample. | `crates/pylon`, `crates/eval-runner`, `crates/ext-eval` | On test sats: a forged result is caught by validator replay; settlement pays worker and validator | X9 |
| L6 | Training on Pylons (owner reversal of kitchen-sink ledger X6, paid distributed training, required) | A `decision_train` job kind: suite manifest hash, recipe allowlist, code digest, node identity keys (not derived), and a signed eval receipt. Payment is released only when a validator reproduces the loss delta. Port the worker receipt and settlement-proof schema from `qwen_legal_pylon_training_job.rs` onto `nostr::pylon` and `pay-ledger`, then delete the Treasury and Nexus types. Flip `trainingClaim` only behind this gate, with an `INVARIANTS.md` change. | `crates/pylon`, `crates/nostr/src/pylon.rs`, `nips/openagents/NIP-PYLON.md`, `psionic-serve/src/pylon_release_manifest.rs` | One paid job on test sats, end to end; a tampered gradient is rejected; mainnet stays behind the owner's `grant.json` ceilings | L5, owner decision |
| L7 | Personalization (B11) | Owner-machine-only adaptation of per-user or per-agent calibration maps from local traces and engrams. Opt-in. Nothing leaves in plaintext. | `crates/coder`, `crates/verse` engrams | A local A/B on the user's own held-out turns; a delete removes the adapter and its data | X4 |
| L8 | Quarantine or retire the remaining imported training code | Decide per lane. Keep Parameter Golf as the GPU trainer reference and `train_validator`/`train_checkpoint*` as libraries. For Tassadar (1,269 files, about 389k lines from `737f94a17c`), keep only the ALM compiler and `tassadar_alm_trace_replay.rs` (X11) and the capability envelope (X9) as libraries; feature-gate the rest behind `legacy` (N1 does this for the serving build) or delete it with the recovery commit noted in the history doc. The same applies to Psion actual-pretraining and research summaries. If the owner keeps it all, N5's history-doc correction is the minimum. | `crates/psionic`, `docs/history/2026-09-28-tassadar-percepta.md` | `cargo test` over the default features passes on a clean checkout with in-repo fixtures only; the build-time delta from N1 is recorded; the history doc names what remains and the recovery commit for what was removed | N1, N3, N5 |
| L9 | Metrics | Publish accepted outcomes per dollar for routed decisions (G9) and the local share of decision calls (ep 201), weekly from Gym rows | `crates/gym`, the stats page | Both figures come from receipts; no forecast appears in copy | X5 |
| L10 | Decision training on Metal (M5 Max) | Add the backward ops the X2 lane needs to `psionic-backend-metal` (it has none today), then run X2a on Metal | `psionic-backend-metal`, `psionic-train/src/decision_train.rs` | Metal backward ops match the CUDA ones within 1e-4 on the gradient-check config; one M5 X2a run writes a `measured` receipt | X2 |

## What not to do

- Do not restart Psion pretraining, DiLoCo, or "largest run" work. There is no
  buyer, no trainer that scales, and no data.
- Do not train learned exact executors (W3 H1). Do not market Tassadar. Do not revive the executor-weights direction, the paid compute-market run, or the module marketplace; reuse only its replay, parity, and self-test rules (N9, X9, X11).
- Do not surface `*_green`, `public_*_claim_allowed`, or any packet that is not
  `measured`.
- Do not route training through Treasury or Nexus, and do not add non-Bitcoin
  payout rails.
- Do not add a "Training" mode or screen to the conversation. Improvements show
  up as faster and cheaper answers and as Gym rows.
- Do not edit the standalone `psionic` repository.

## First five issues to file

1. **Psionic serve: gate the Tassadar/Psion modules behind `legacy-research`
   and drop the default train/eval/research dependencies** (N1). Acceptance:
   `cargo tree --no-default-features` is clean, Clef tests
   pass, and the build time delta is recorded.
2. **Psionic train: refuse Production dispatch without node keys; delete the
   derived signing keys** (N2). Acceptance: a refusal test and a clean grep.
3. **Evidence classes on every training and eval artifact, plus the
   `INVARIANTS.md` row** (N3 and N4). Acceptance: the gate test fails
   non-measured promotions, and the authored fixtures are relabelled.
4. **file-relevance-v1: mine fix commits into a partitioned decision corpus**
   (N8, extends #11210). Acceptance: ≥ 300 issues, `tenant-train check` passes,
   and the locked partition has not been read.
5. **Clef-Flash calibration map on file-relevance-v1, served with its digest**
   (X1). Acceptance: the map passes the `probability-v2` gate, and locked F1
   at 0.5 improves over the recorded development baseline and reaches ≥ 0.75.

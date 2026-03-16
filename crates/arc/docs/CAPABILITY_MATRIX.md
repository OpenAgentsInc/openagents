# ARC Capability Matrix
Status: canonical capability and drift matrix for the ARC subtree
Date: 2026-03-16

This file is the compact current-state matrix for `crates/arc/*`.

Use it when you need to answer:

- what is landed today vs only bounded?
- what is still blocked on Psionic vs missing in ARC itself?
- which ARC-AGI-3 preview surfaces are covered in Rust already?
- where does upstream preview drift still make release-readiness unknown?

## Status Legend

| Status | Meaning |
| --- | --- |
| `landed` | implemented in the retained Rust subtree and backed by owned tests or fixtures |
| `bounded` | implemented for the retained scope, but not yet a full claim of upstream release equivalence |
| `blocked` | intentionally waiting on named ARC or Psionic prerequisites |
| `unknown` | upstream preview/release behavior is still drifting or insufficiently frozen |

## Subtree Matrix

| Area | Static ARC-AGI-1/2 | Interactive ARC-AGI-3 preview | ARC-AGI-3 release readiness | ARC-ML readiness | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| shared contracts in `arc-core` | landed | landed | bounded | blocked | landed | grids, tasks, actions, frames, recordings, scorecards, refusal envelopes, canonicalization, object graphs, and correspondence candidates are real in Rust |
| datasets in `arc-datasets` | landed | n/a | n/a | bounded | landed | ARC JSON loading plus Psionic dataset-manifest export exist; larger train-class dataset programs remain later work |
| deterministic runtime in `arc-engine` | n/a | landed | bounded | n/a | landed | local game execution, replay-safe action handling, undo / `ACTION7`, and upstream parity fixtures are real |
| local / remote wrappers in `arc-client` | n/a | landed | bounded | n/a | landed | local runtime, compatibility server, scorecard lifecycle, recording transport policy, and local-vs-remote parity fixtures exist |
| benchmark truth in `arc-benchmark` | landed | landed | bounded | bounded | landed | exact-match scoring, interactive methodology / competition / preview-compat score policies, checkpoints, hygiene harnesses, and repeated-run aggregation are real |
| interactive runner in `arc-solvers` | n/a | landed | bounded | n/a | landed | typed agent trait, runner, registry, budget/reset/refusal semantics, checkpoint handoff, context retention, local-vs-remote parity, trajectory receipt export, and repeated eval bridging are real |
| baseline interactive agents | n/a | landed | bounded | n/a | landed | random, scripted, and ADCR-style baselines are reproducible, replayable, and checkpointable |
| static solver lanes | landed | n/a | n/a | n/a | landed | symbolic, transductive, recursive tiny-model, MDL/compression, verifier, and portfolio arbiter lanes are implemented for retained bounded scope |
| evidence / operator stack | landed | landed | bounded | n/a | bounded | public-eval hygiene is real; this matrix and the operator workflow doc are the current hardening layer, not a final competition-readiness claim |
| Psionic environment / eval bridge | n/a | landed | bounded | bounded | landed | ARC-owned trajectory bundles now export through `psionic-environments`, and repeated interactive eval now runs over `psionic-eval` without moving ARC scoring semantics into Psionic |
| `arc-ml` evaluator-first practice layer | n/a | bounded | bounded | bounded | bounded | `arc-ml` now exists in bounded synthetic-practice form: it scores ARC-AGI-3-style `ArcRecording` attempts through `arc-benchmark`, retains per-attempt run reports, and aggregates `pass@k` without claiming real dataset access or learned-model parity |
| HRM-class program | blocked | blocked | blocked | blocked | blocked | learned-model parity and train-class claims remain gated on the named Psionic roadmap items plus later `ARC-502` through `ARC-508` work |

## Solver Lane Matrix

| Lane | Scope today | Status | Evidence |
| --- | --- | --- | --- |
| random baseline | interactive preview smoke / reproducibility | landed | `crates/arc/solvers/tests/interactive_baselines.rs` |
| scripted baseline | interactive preview deterministic win path | landed | `crates/arc/solvers/tests/interactive_baselines.rs` |
| ADCR baseline | interactive preview bounded baseline lane | landed | `crates/arc/solvers/tests/interactive_adcr.rs` |
| symbolic lane | static search / repair substrate | landed | `crates/arc/solvers/tests/symbolic_lane.rs` |
| transductive lane | static candidate generation over Psionic serve adapter seam | landed | `crates/arc/solvers/tests/transductive_lane.rs` |
| recursive tiny-model lane | bounded static refinement loop | landed | `crates/arc/solvers/tests/recursive_lane.rs` |
| MDL / compression lane | bounded static simplicity-pressure ranking | landed | `crates/arc/solvers/tests/mdl_lane.rs` |
| verifier | typed falsification and common verification | landed | `crates/arc/solvers/tests/verifier.rs` |
| portfolio arbiter | verifier-first candidate selection and second-attempt gating | landed | `crates/arc/solvers/tests/arbiter.rs` |
| ARC-ML evaluator-first practice | synthetic ARC-AGI-3-style scoring and `pass@k` aggregation | bounded | `crates/arc/ml/tests/interactive_practice.rs` |
| ARC-ML learned lanes | model/training program | blocked | gated on `ARC-502` through `ARC-508` plus Psionic train/eval prerequisites |

## ARC-AGI-3 Preview vs Release Drift

| Topic | Upstream preview signal | Retained Rust state | Readiness |
| --- | --- | --- | --- |
| frame schema | `game-schema.mdx` says agents receive `1-N` frames and max `64x64` grids | `ArcSessionFrame` already carries `Vec<ArcFrameData>` and `arc-core` enforces the same coordinate / grid envelope; most retained fixtures still exercise single-frame episodes | bounded |
| `levels_completed` / `win_levels` naming | `ARC-AGI-3-Agents` `0.9.3` renamed `score` -> `levels_completed` and `win_score` -> `win_levels` | retained Rust already uses `levels_completed` / `win_levels` consistently across engine, client, recording, scoring, checkpoints, and solver reports | landed |
| `available_actions` | `ARC-AGI-3-Agents` `0.9.2` added explicit `available_actions` on frames | retained Rust uses typed `ArcActionKind` sets across engine/runtime, client parity, runner legality checks, and prompt/context surfaces | landed |
| `ACTION7` / undo | preview docs list `ACTION7` as undo | retained Rust owns `ArcAction::Action7`, engine undo behavior, compatibility transport, and parity / contract tests; per-game availability still depends on package data | bounded |
| scorecard lifecycle | preview docs separate local vs online scorecards and note closed / inflight restrictions | retained Rust has typed local and remote scorecard surfaces plus compatibility-server tests for competition-mode lifecycle restrictions; upstream hosted behavior beyond the documented preview is still external | bounded |
| recordings / replays | preview docs say local toolkit has no shareable recordings and online scorecards host replays | retained Rust has owned `ArcRecording`, checkpoint bundles, local canonical recording transport, remote JSONL compatibility, and ARC-owned trajectory bundles; hosted replay UX remains outside the subtree | bounded |
| checkpoint / resume | preview docs emphasize recordings and scorecards more than machine-readable resume | retained Rust already has typed checkpoint bundles, runner handoff state, resume context policies, and trajectory replay locators | landed |
| local vs online parity | preview docs say local is fast and offline while online yields scorecards and replays | retained Rust has a manifest-driven parity harness that compares local/offline and remote/online behavior while explicitly documenting expected divergences | landed |
| competition mode policy | preview docs require API interaction, one scorecard, one `make` per environment, level-reset-only semantics, and no inflight `get_scorecard` | retained Rust has explicit competition operation mode, competition scoring policy, and compatibility-server restriction tests; full release policy beyond the preview docs is still not frozen | bounded |
| environment receipt / eval bridge | upstream docs do not define ARC-specific typed receipt bundles or a Rust eval bridge | retained Rust now exports ARC-owned trajectory bundles through `psionic-environments` and runs repeated interactive eval rounds through `psionic-eval` | landed |
| final ARC-AGI-3 release contract | preview docs plus changelog still show field/policy drift | enough bounded substrate exists to track the preview honestly, but a final release-equivalence claim would be premature until upstream freezes the release contract | unknown |

## Blockers That Still Matter

| Item | Why it is not done yet | Owner |
| --- | --- | --- |
| HRM-class training / eval parity | still gated on the named Psionic roadmap items for interactive environments, collectives, train-class operator coverage, and model-state IO | Psionic + ARC |
| full ARC-AGI-3 release-readiness claim | upstream preview drift is still real enough that the final contract is not yet frozen | upstream + ARC |

## Maintenance Rule

When a roadmap item changes one of the matrix rows above, update this file in
the same change.

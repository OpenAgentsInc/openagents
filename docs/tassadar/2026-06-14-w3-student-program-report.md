# W3 Student Program Report

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-14

Issue: OpenAgentsInc/openagents#4749

## Decision

The W3 four-baseline sweep is complete on the W2 100M-token verified corpus
snapshot. The result is decisive:

- baselines A, B, and C trained to completion, but fail replay immediately;
- baseline D, the frozen analytic executor plus learned interface, passes the
  replay harness perfectly;
- H1 and H2 are supported by this sweep;
- H3 is falsified for this setup: analytic lookup initialization preserves the
  lookup auxiliary, but the backbone still diverges at rollout step zero.

This is Psion student evidence, not Tassadar proof evidence. Learned students
produce bounded statistics and first-divergence reports. Only the frozen
analytic executor path carries exact execution.

## Corpus

The W2 input bar is satisfied by:

- corpus id: `corpus.tassadar_trace.v0_2.w3_100m`
- verified tokens: `103,573,600`
- records: `12,548`
- families: `6`
- dataset snapshot digest:
  `d045a53d0cecbe6ffb1b4f0c1522ab76b02014491842f1770d34c12a885c8c3a`
- train prep SHA-256:
  `8095588b05ff1bc3b8a723431c35015882a25566f74d895b514071f5e1734350`
- eval prep SHA-256:
  `512830dcbdd4f8e4842adbf1960522c70e8609475581aa4936f6424b4981102b`
- OpenAgents corpus/report-schema commit on current `main`: `f54c9b6a9`

The corpus manifest is
`apps/openagents.com/workers/api/corpus/tassadar-trace-corpus.v0_2.w3_100m.manifest.json`.

## Artifact Refs

Psionic `main` now owns the executable W3 student harness and the resolvable
artifact bundle:

- Psionic artifact commit: `7497713e`
- student crate:
  `crates/psionic-tassadar-student/`
- fixture bundle:
  `fixtures/tassadar/w3_student_sweep_20260612/`
- fixture manifest:
  `fixtures/tassadar/w3_student_sweep_20260612/manifest.json`

The fixture bundle contains:

- A/B/C: `weights.bin`, `receipt.json`, `eval-report.json`
- D: `interface.json`, `receipt.json`, `eval-report.json`

The entrypoints default to a one-core CPU budget unless explicitly widened by
`--cpu-budget` or `PSIONIC_TRAIN_CPU_BUDGET`, per psionic#1123. The canonical
SHC sweep used explicit `--cpu-budget 5` per lane.

## Overall Metrics

| Baseline           | exact_rollout_pass@1 | replay acceptance | median first divergence | p90 first divergence | valid-prefix median tokens | branch accuracy | memory-read accuracy | output digest match | top divergence causes                                                                       |
| ------------------ | -------------------: | ----------------: | ----------------------: | -------------------: | -------------------------: | --------------: | -------------------: | ------------------: | ------------------------------------------------------------------------------------------- |
| A next-token       |                `0.0` |             `0.0` |                     `0` |                  `0` |                        `0` |  `0.9964216444` |       `0.4442458894` |               `0.0` | wrong_fetch `499`, memory_read `123`, output `60`, carry `56`, stack_depth `6`, branch `4`  |
| B auxiliary-state  |                `0.0` |             `0.0` |                     `0` |                  `0` |                        `0` |  `0.8952484131` |       `0.4426797129` |               `0.0` | wrong_fetch `316`, memory_read `306`, output `60`, branch `31`, carry `29`, stack_depth `6` |
| C lookup analytic  |                `0.0` |             `0.0` |                     `0` |                  `0` |                        `0` |  `0.9964216444` |                `1.0` |               `0.0` | wrong_fetch `499`, output `183`, carry `56`, stack_depth `6`, branch `4`                    |
| D frozen-interface |                `1.0` |             `1.0` |                   `512` |               `4096` |                    `10240` |             n/a |                  n/a |               `1.0` | none                                                                                        |

## Checkpoint, Config, And Eval Hashes

| Baseline           | checkpoint/interface SHA-256                                       | config digest                                                      | eval report SHA-256                                                |
| ------------------ | ------------------------------------------------------------------ | ------------------------------------------------------------------ | ------------------------------------------------------------------ |
| A next-token       | `961764732aefd74b8a019c804cb47a3d23b725fe528c55110170c6a93ae9dbf2` | `40fdd266ea84b22626d21d7c61503b98eae332a624c313d78a9ed466d9a5dbaf` | `c968c2fb44d890f51afd3e6cb192106afc21ec001f2b0548665df7cac02ec7c9` |
| B auxiliary-state  | `5f39d49ebd30bbc540ac9d7b4005de9f97eb250594e52213c6fb5bb3b71c5aef` | `af4af28c8216813c8280c74b9b803b22d8fc1c4d3849b1eb4996a6c8b448f142` | `8b23d27f1abb6774058492f01c4830e0866abde9d95e6e50df427a2baed488db` |
| C lookup analytic  | `961764732aefd74b8a019c804cb47a3d23b725fe528c55110170c6a93ae9dbf2` | `fcfdd1374627db3147669fd6a979ebca50e58170f4210b33627e1eed3f627375` | `d655729f686a02d5aa2cb1ac06e5bd42b99c905b3e61f9e3a568c9ed538043c7` |
| D frozen-interface | `9eb153e360f576770a6de0e50abd07fbb6ece1237c80b08d2bf6c4ffbb6d0217` | `9eb153e360f576770a6de0e50abd07fbb6ece1237c80b08d2bf6c4ffbb6d0217` | `a9b2bf9d95228d69f33f9dc4826d14536e3a70ddd15bb6d6b243888c3baebfd5` |

## Verdicts Against The Hypotheses

H1, "purely learned exactness fails": supported.

The next-token baseline trained on the 100M-token snapshot and still achieved
`0.0` exact rollout pass@1 and `0.0` replay acceptance. Every eval record
diverged before one replay step.

H2, "frozen exact cores plus learned control succeeds": supported.

The frozen analytic executor plus learned interface achieved `1.0` pass@1,
`1.0` replay acceptance, `1.0` output digest match, and no divergence causes
across all 748 eval records.

H3, "the 2D geometry is trainable only with help": falsified for this exact
setup.

The analytic lookup variant reports lookup accuracy `1.0` during training, but
the full rollout still has `0.0` pass@1 and `0.0` replay acceptance. The
lookup helper solved its local target; it did not make the learned backbone a
replay-safe executor.

## Boundary

This report closes the W3 sweep as a research/evaluation issue. It does not
make learned students production proof executors, does not create public
product claims, and does not affect the already-closed Autopilot MVP parent.

# `checks.oracle` at tier 0: protocol

Issue [#9656](https://github.com/OpenAgentsInc/openagents/issues/9656).
Written on 2026-09-25, after the component was built and tried on two
excluded tasks, and before any oracle result is joined with a verifier
reward.

## What is measured

`checks.oracle` (`crates/coder-one/src/checks/oracle/`) gets an acceptance
check that doesn't depend on the candidate:

1. Code looks for a checker the instruction names (a stated command that
   runs a file named like a checker, or a named runnable file named like
   one). When it finds one, that's the oracle; a case passes on exit 0.
2. Otherwise, Jev picks, one Noul per candidate, the instruction's
   sentences that define a correct result, the stated values that are
   parameters, and the sentences that name a boundary input. A Luna
   session that sees only that spec, the input and output formats, and the
   heads of the named input files writes `oracle.py`.
3. Code runs the oracle once on the untouched workspace and once on each
   retained workspace, each in its own container of the task's image with
   the network off.

## Population

The 23 tasks of the #9628 main population
(`bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/records/`)
less the 11 tasks Fable 5.1's winning runs were mapped from
(`embedding-drift-monitor`, `coq-block-bound`, `shadow-relay`,
`risk-scorer-replay`, `mp-checkpoint-consolidation`,
`payments-pipeline-fix`, `telecom-entity-resolution`, `fp8-rmsnorm-gemm`,
`distributed-dedup`, `intrastat-meldung`, `photonic-waveguide-routing`).
That leaves 17 tasks:

`bun-sourcemap-leak`, `cad-model`, `cargo-flight-dispatch`,
`fin-saccr-rwa`, `gsea-proteomics`, `heat-pump-warranty`,
`html-js-filter`, `interleaved-vigenere`, `ks-solver-cpp`,
`legacy-utility-triage`, `mvcc-lsm-compaction`, `production-planning`,
`protein-autointerp-disulfide`, `session-window-debug`,
`sound-change-cascade`, `uefi-bootkit`, `wal-recovery-ordering`.

Workspaces: every retained workspace that `coder-one accept offline`'s
reader finds under `~/.openagents/terminal-bench/jobs` for those tasks, with
the candidate grades under
`bench/terminal-bench/experiments/2026-09-24-candidate-evidence/records/candidate-grades`.
Jobs whose name contains `9584`, `truth-confirmation`, or `truth-control`
are excluded, so nothing here reads the #9584 cohort.

The per-task counts of graded passes and failures in the #9628 labels
were read while choosing the population. No oracle result existed then.

## Development before this freeze

The component was run on two excluded tasks only:
`photonic-waveguide-routing` and `coq-block-bound`. That run changed three
general things: fenced examples now travel with the sentence they follow,
input-file heads are 60 lines instead of 12, and the writer is told that a
shape-only check isn't an oracle and runs at high reasoning effort. No
population task was run before this freeze.

## Frozen now

- **The prompts.** The Jev questions `DEFINES`, `BOUNDARY`, and
  `PARAMETER` in `define.rs`; the writer's `TASK` and `PROTOCOL` in
  `write.rs`. SHA-256 of the files: `define.rs`
  `4df11925ae7805e5a4fe1b86266152793cf1a3eb0cbf32028fb7fde06c4517d5`,
  `write.rs`
  `281fbabf80d9817d43d16e2fbadd44d0c6e685cf7fe96276474ac0955f9d6344`,
  `find.rs`
  `edfb2ad8e2c2a5babfaa98cdcc8611c771ffde0bc1bbc9ef61c1aa1e8e6a02d3`,
  `mod.rs`
  `7fee1bf6301394685d6007bba55bc3ec4f5a1f28e854e34be286ab4c3d88636e`.
- **The rules.** Jev's bound is 0.5 for every Noul; at most 16 definition
  sentences, 12 parameters, and 8 boundaries are kept. One writer session
  per task with no checker: 30 turns, 600 seconds, $0.08, `gpt-6-luna` at
  high effort, in a writing boundary with no network, and no retry. Luna
  spend stops at $1.00 in total. Oracle runs are bounded to 300 seconds, a
  found checker to 180.
- **The calls.** An oracle's call on a workspace is `pass` when at least
  one case passed and none failed, `fail` when any case failed, and none
  otherwise. Its score is passed cases over cases that ran to an answer.
  An oracle is **trivially passing** when its call on the untouched
  workspace is `pass`, and **usable** when that call is `fail`.
- **The comparator.** The Luna self-score is the frozen evaluation script's
  score that a Microluna lean run recorded in `microluna-*.json`: for a
  final workspace, the score of the `lean.restore` move when there is
  one, otherwise of the last `lean` move with a score; for a retained
  candidate `….lean-1.session-N`, the score after session N. Its call is
  `pass` when every test passed. Only lean-loop workspaces have one.
- **The sets.** Primary: workspaces the verifier graded (Microluna
  finals, candidates with a reward from a grade record, and snapshots
  whose check candidates all share the snapshot's digest). A candidate
  whose reward source is `submitted` is left out, since its trial's final
  row counts it. Secondary: adds every snapshot with its trial's reward.
  Workspaces with no known reward are listed and never counted.

## Analysis

1. Per task: whether an oracle was found, written, or neither; whether it
   is trivially passing; how many workspaces it answered.
2. Within-task discrimination, on tasks with at least one graded pass and
   one graded failure: passing workspaces the oracle kept green, failing
   ones it called red, and pass-and-fail pairs its score ordered right
   (ties left out and counted). Primary on usable oracles; also reported
   with every oracle. The same three for the self-score where it exists.
3. Every rate with a 95% Wilson interval. The effective sample is the
   number of mixed tasks, not the number of workspaces.

The bar is the one #9629 set for a class that may hold the loop, read on at
least 2 mixed tasks: passes kept green with the interval's low end at 0.8
or more, failures called red with its low end at 0.2 or more. Meeting it
makes the component a candidate for matched mini-task runs; it doesn't
admit it. `checks::oracle::ADMITTED` stays `false` either way.

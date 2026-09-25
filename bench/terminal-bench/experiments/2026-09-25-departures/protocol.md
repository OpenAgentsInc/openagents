# Departure miners offline: protocol

Issue [#9634](https://github.com/OpenAgentsInc/openagents/issues/9634).
Written on 2026-09-25, after the miners' code and before any Jev answer on
a task workspace. The candidate labels in [`labels.json`](labels.json) were
written from the code-only candidate list (Jev off) and are committed with
this protocol.

## What is measured

`evidence.departures` runs three miners on a task's untouched workspace:
`rationale` (comments that justify a choice, the v13 miner), `docstring`
(functions and classes with a docstring), and `standard-method` (functions
and classes that name a method in the versioned list
`crates/coder-one/src/departures/standard-methods.json`). Jev ranks every
candidate with one Noul from the source's question set under `questions/`.
A row is listed when its probability is at or above its source's
threshold, at most 8 rows per source.

The miners read the instruction and the workspace only. The anatomy's
facts, the verifiers, and the reference solutions are used to label rows,
never as input.

## Population

The 18 tasks of `docs/terminal-bench/2026-09-24-task-anatomy.md`. Each
workspace is rebuilt from the Terminal-Bench task cache by
[`build_workspaces.py`](build_workspaces.py), which copies what each
Dockerfile puts in the agent's working directory. Files generated at image
build time aren't rebuilt (`shadow-relay`, `interleaved-vigenere`, and
`data-anonymization`'s input); none of them is source code. The miners scan
the same production source files the v13 miner scans (`py`, `js`, `ts`,
`go`, `rs`, `rb`, `java`, `c`, `cc`, `cpp`, `h`, and `sh`), so `.hpp`,
`.pm`, `.v`, and VBA files aren't read by any source.

`html-js-filter` has no decisive facts yet (the anatomy lists it as still
in analysis). It's run and counted for precision; it adds nothing to recall.

## Split, frozen now

The executed-contract-checks split (`sha256("openagents-9628:" + task)`)
puts five tasks this work may not tune on in its development half:
`embedding-drift-monitor` and four capability-gap tasks. So the split here
is by that rule instead:

- **Fit (10):** the tasks outside `embedding-drift-monitor` and the
  capability-gap log: `atrx-vep-crispr`, `biped-contact-dynamics`,
  `bun-sourcemap-leak`, `data-anonymization`, `html-js-filter`,
  `intrastat-meldung`, `ks-solver-cpp`, `layout-config-recreation`,
  `vba-userform-port`, and `vf2-speedup-networkx`.
- **Report (8):** `coq-block-bound`, `embedding-drift-monitor`,
  `fin-saccr-rwa`, `gsea-proteomics`, `interleaved-vigenere`,
  `session-window-debug`, `shadow-relay`, and `sound-change-cascade`.

## Threshold, frozen now

The code-only run found candidates in three workspaces:
`embedding-drift-monitor` (6 rationale, 15 docstring, 13 standard-method),
`session-window-debug` (14 docstring, 8 standard-method), and
`sound-change-cascade` (2 docstring). All three are report tasks. The fit
tasks have no candidate row for any source, so no threshold can be chosen
on them. The rule, fixed before any answer:

- When the fit tasks hold at least one labeled hit for a source, its
  threshold maximizes F1 (recall of `instruction` and `workspace` facts,
  precision of listed rows) on the fit tasks over 0.05 to 0.95 in steps of
  0.05, a tie going to the higher threshold.
- Otherwise the source keeps v13's threshold, 0.5.

So every source lists at 0.5. A sweep over thresholds on the report tasks
is shown for reading only; it doesn't choose anything.

## Scoring, frozen now

- A row names a fact per [`labels.json`](labels.json): it points at the
  code the fact says must change. A row not labeled names nothing.
- **Recall** of a source: the anatomy's `instruction` and `workspace`
  facts that at least one listed row of the source names, over all such
  facts in the 18 tasks. The `verifier-only` facts are reported the same
  way, separately, as the ceiling.
- **Precision** of a source: listed rows that name at least one decisive
  fact of any kind, over listed rows.
- Intervals are 95% Wilson intervals. Facts and rows cluster in three
  tasks, so they aren't independent; the intervals understate the
  uncertainty, and the write-up says so.
- The candidate ceiling, what each miner could name before Jev ranks it,
  is reported beside the listed numbers.

## Admission, frozen now

A source is admitted when, across the 18 tasks at the frozen thresholds:

1. the union of `rationale` and the source names more `instruction` and
   `workspace` facts than `rationale` alone, and
2. the source's precision is at least `rationale`'s.

Both conditions compare point estimates; the write-up reports the
intervals beside them. Jev runs live on `jev-1.13.0`, answers are saved
beside the fixtures outside the repository, and total Jev spend stays under
$0.05.

# `verify.method_conformance` offline: protocol

Issue [#9653](https://github.com/OpenAgentsInc/openagents/issues/9653).
This protocol is committed with the component, before any Jev answer on a
retained workspace and before any label is joined. It fixes the
population, the split, the rules, and the admission rule.

## What is frozen

- The registry: the 11 files under `methods/`, digested one by one and
  together (`coder-one checks conformance registry`).
- The identifying question: `questions/method-conformance.json`.
- The runner (`crates/coder-one/src/checks/conformance/runner.py`) and the
  candidate rule: Python functions and methods of module classes in the
  first 60 non-test source files, with one to six parameters, at most 80
  lines, at most 60 per workspace, identified six to a Jev request.

None of these changes after the first Jev answer on a workspace outside
the source task. A fix to the harness is allowed only where a check
couldn't run at all (a host failure, a missing image, a restore error),
and every such fix is listed in the results.

## Population

Every retained workspace that `coder-one accept offline` reads
(`accept::offline::found`) under `~/.openagents/terminal-bench/jobs`:
Coder One snapshots, Microluna final workspaces, and retained lean-loop
candidates, for every task that has one. Workspaces are found by code;
none is picked by hand.

## Split

- **Source task:** `embedding-drift-monitor`. Every entry named in the
  issue lists it in `provenance.source_tasks`, and the component's pattern
  was learned from Fable's runs on it. Its results are reported apart and
  never counted as evidence. It may be used to debug the harness.
- **Report set:** every other task. Nothing is tuned on it; there is no
  threshold to choose.

## Procedure

1. `coder-one checks conformance offline TASK... --out records --jev live`
   restores each workspace, finds candidates by code, asks Jev once per
   distinct function (by digest, per task), and records every answer in
   `records/jev-recorded.json`. For each workspace with an identified
   function, it runs the entries' checks in a fresh container of the
   task's image with no network, then removes the container. Where a task
   has no image on this machine, its checks are `unknown` unless the image
   is built from the task's public `environment/`, one at a time, and
   removed afterward.
2. The Jev budget is $0.10. The run stops if the recorded input tokens
   reach it.
3. Only after every task's `conformance.json` is written, `measure.py`
   joins `labels.json`.

## Labels and units

- A workspace is **graded** when `labels.json` gives it a reward: a
  snapshot or final workspace's own verifier reward, or a candidate's when
  its files are the submitted workspace's. A reward of 1 is a pass; any
  other reward is a failure.
- The unit is a (workspace, method) pair. Its outcome is `fail` when any
  function tied to the method failed a property, `pass` when every tied
  function's properties all ran and passed, and `unknown` otherwise.
  Unknown pairs are counted and left out of the rates.

## Measures

Per method within each task, and pooled across report-set tasks:

- **Fail precision:** of graded `fail` pairs, the fraction whose workspace
  failed the verifier.
- **Pass agreement:** of graded `pass` pairs, the fraction whose workspace
  passed the verifier.

Both with 95% Wilson intervals, and the same two at the workspace level
(any method failing). Also reported: candidates, identified functions by
method, checks that couldn't run and why, and Jev's cost.

## Admission rule

The component may be switched on in a policy only if, on the report set,
graded `fail` pairs number at least 5 on at least 2 tasks and the lower
Wilson bound of the pooled fail precision is at least 0.6. An entry that
never fires on the report set stays in the registry with its admission
status `not admitted`.

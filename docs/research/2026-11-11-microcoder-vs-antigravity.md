# Microcoder against Antigravity on Gemini

Written 2026-11-11. GitHub #13231. Related: #13193, #13227,
[the Antigravity task-fit experiments](2026-11-11-antigravity-task-fit.md).

The owner's goal: make Microcoder work well on Gemini, bench it against
Antigravity (`agy`) and beat it, using only Microcoder's own small tool set.
Both harnesses run the same Gemini models on Vertex AI (project
`openagentsgemini`, location `us`), so this compares harness with harness.

- **Microcoder:** OpenAgents' own loop (`coax/lib/microcoder`): one structured
  model call a step, then the step's edits, shell commands and file views.
  Run by Coder: `coder run --engine microcoder --model M --effort low`.
- **Antigravity:** Google's `agy` CLI with about 60 tools, driven by Coder
  through ACP: `coder run --engine antigravity --model M-low`.

## The verdict, defined before the final runs

"Provably better" means: on the hold-out tasks plus T1–T6, with the same model,
thinking level and location, and at least two runs per task per harness,
Microcoder:

1. passes at least as many runs as Antigravity, and
2. has lower total cost and lower median wall time,

and the difference in cost and wall time holds on the hold-out tasks alone.

The final runs are at Argon low in `us`. A second setting (Flash low or
Carbon low) runs if the budget allows. With two runs a task, a difference of
one or two passes is within noise; the report gives counts, not rates.

## Tasks

- **T1–T6:** the six tasks of the task-fit experiments, each judged by its
  commit's own checks (`tasks.md` there): a rename, a bug fix with a failing
  check, a feature in one unit, a reading task, a five-file change, and a doc
  edit.
- **H1–H4 (hold-out):** built from this repository's history by a separate
  agent the same way (the parent of a real commit, its intent as the prompt,
  its own checks as the judge): a bug fix, a feature in one unit, a
  three-file change and a reading task. Nobody tuning Microcoder looked at
  them before the final runs.

Results follow below as they land.

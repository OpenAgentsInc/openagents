# CoderBench

A benchmark for **general coding-agent capability**, measured on real work
instead of authored puzzles. CoderBench grades how well an agent performs the
day-to-day coding this machine actually sees — implementing features across
Rust/TypeScript monorepos, porting modules, fixing flaky tests, migrating
package graphs, wiring verifiers, closing tracker issues with receipts — and
its task corpus, grading evidence, and optimization signal all grow out of the
real session traces that produced that work.

Building coding agents is the **first seeded domain**, not the definition: it
is the deepest slice of trace history on this machine today. The benchmark is
designed to grow domains as the corpus does — any real coding work with a
checkable outcome can become a CoderBench task.

Read in order:

1. [`plan.md`](plan.md) — the goal, non-goals, milestone plan, and success
   metrics.
2. [`terminal-bench-lessons.md`](terminal-bench-lessons.md) — what to copy,
   adapt, and reject from Terminal-Bench 2.0 and the Harbor framework, which
   this repository already runs through `bench/`.
3. [`process.md`](process.md) — the operating procedure: inventory every local
   trace, redact, upload the corpus to the OpenAgents Cloud, label it against
   the forge, distill pinned tasks, grade, and feed the autoimprovement loop.

## One-paragraph version

Every serious session on this machine — 2,548 Codex rollouts, 1,327 Claude
sessions, and every `openagents coder` run's native ATIF export — is a record
of real coding work with a checkable outcome attached: a commit, a closed
issue, a green gate. CoderBench uploads those traces (redacted, deduplicated,
pinned by digest) into a domain-tagged corpus, turns the outcome pairs into
pinned, verifiable benchmark tasks across every domain the traces cover —
agent-building first, because that is where the history is — and runs the
coder against them on the same receipted rails as the tb2 lanes: Harbor
adapter, pinned suite manifest, thresholds, append-only hash-chained results.
Terminal-Bench proved the methodology on contrived terminal tasks; CoderBench
applies it to real work as it actually happens, across whatever code the
corpus honestly contains.

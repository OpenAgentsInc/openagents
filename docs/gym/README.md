# OpenAgents Gym — docs

The **Gym** is the interactive experimentation surface and eval+reward factory
that trains [Khala](../inference/khala.md). Like OpenAI's original Gym (standard
environments + one interface so policies can be compared), it lets you configure a
Khala **policy** — coordinator candidate × provider fan-out × tool set ×
plugin/module composition × sampling × quantization/speculation — run it against a
registered **environment** (Terminal-Bench, `khala-code`, long-context QA, the M8
head-to-head, …), and score it on the **executed verification verdict +
cost-per-accepted-outcome**. The `/gym` web route is the knobs-and-dials surface
in Foldkit + `@openagentsinc/three-effect`; paid runs ride the existing
metering/settlement spine so people can pay to run benchmarks.

It is **not** a new inference engine or metric vocabulary — it compiles down to
the already-landed Khala benchmark harness, coordinator/`ModelRouter`,
provider-adapter registry, verification-class registry, and the
`openagents.khala.telemetry.v1` schema.

## Contents

- [`openagents-gym.md`](openagents-gym.md) — the spec & roadmap (start here).

## Related (in this repo)

- [`../inference/khala.md`](../inference/khala.md) — the Khala model the Gym trains.
- [`../inference/2026-06-23-khala-benchmark-harness-book-p1-5.md`](../inference/2026-06-23-khala-benchmark-harness-book-p1-5.md)
  — the typed benchmark matrix/runner/report the Gym compiles to (book P1-5 / #6088).
- [`../inference/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md`](../inference/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md)
  — typed programs + the plugin/marketplace layer the Gym composes (and its
  no-public-marketplace boundary).
- [`../inference/khala-in-the-world.md`](../inference/khala-in-the-world.md) — the
  Verse fan-out/verdict/cost visual language the Gym run scene reuses.
- [`../inference/2026-06-23-khala-head-to-head-m8-status.md`](../inference/2026-06-23-khala-head-to-head-m8-status.md)
  — the M8 head-to-head, a first Gym environment.
- [`../inference/khala-buildout-roadmap.md`](../inference/khala-buildout-roadmap.md)
  — the M0–M8 buildout the coordinator candidates come from.

> Status: initial design spec, honest-scope. Not a product promise, served
> capability, or public-claim copy.

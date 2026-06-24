# OpenAgents Gym — docs

The **Gym** is the interactive experimentation surface and eval+reward factory
that trains [Khala](../khala/khala.md). Like OpenAI's original Gym (standard
environments + one interface so policies can be compared), it lets you configure a
Khala **policy** — coordinator candidate × provider fan-out × tool set ×
plugin/module composition × sampling × quantization/speculation — run it against a
registered **environment** (Terminal-Bench, `khala-code`, long-context QA, the M8
head-to-head, ...), and score it on the **executed verification verdict +
cost-per-accepted-outcome**. The public `/gym` web route is the Phase 0
fixture-only knobs-and-dials surface in Foldkit + `@openagentsinc/three-effect`;
the owner-gated `/gym/oss` surface is the GPT-OSS 20B latency playground for
hammering the hourly Hydralisk L4 lane without exposing a public load generator.
Future paid runs ride the existing metering/settlement spine so people can pay to
run decision-grade benchmarks.

It is **not** a new inference engine or metric vocabulary — it compiles down to
the already-landed Khala benchmark harness, coordinator/`ModelRouter`,
provider-adapter registry, verification-class registry, and the
`openagents.khala.telemetry.v1` schema.

## Status

- Phase 0 public fixture Gym is landed and intentionally spend-free:
  `#6164`, `#6165`, `#6166`, and the closeout epic `#6163`.
- GPT-OSS owner/internal latency playground is landed at `/gym/oss`:
  `#6167`. It is auth/owner-gated, capped at eight in-flight requests, streams
  against `openagents/khala-oss-20b`, and keeps `not_measured` distinct from
  fabricated zeroes.

## Contents

- [`openagents-gym.md`](openagents-gym.md) — the spec & roadmap (start here).

## Related (in this repo)

- [`../khala/khala.md`](../khala/khala.md) — the Khala model the Gym trains.
- [`../khala/2026-06-23-khala-benchmark-harness-book-p1-5.md`](../khala/2026-06-23-khala-benchmark-harness-book-p1-5.md)
  — the typed benchmark matrix/runner/report the Gym compiles to (book P1-5 / #6088).
- [`../khala/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md`](../khala/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md)
  — typed programs + the plugin/marketplace layer the Gym composes (and its
  no-public-marketplace boundary).
- [`../khala/khala-in-the-world.md`](../khala/khala-in-the-world.md) — the
  Verse fan-out/verdict/cost visual language the Gym run scene reuses.
- [`../khala/2026-06-23-khala-head-to-head-m8-status.md`](../khala/2026-06-23-khala-head-to-head-m8-status.md)
  — the M8 head-to-head, a first Gym environment.
- [`../khala/khala-buildout-roadmap.md`](../khala/khala-buildout-roadmap.md)
  — the M0–M8 buildout the coordinator candidates come from.

> Status: implementation-linked spec, honest-scope. Not a product promise,
> served public capability, or public-claim copy.

# Khala docs

Khala is the OpenAgents inference product — one OpenAI-compatible endpoint
(`openagents/khala`, base `https://openagents.com/api/v1`) over a network of
agents. This folder collects the Khala-specific design, brain, and buildout docs.

## Start here

- [**2026-06-24-khala-brain-and-blueprint-hookup-audit.md**](2026-06-24-khala-brain-and-blueprint-hookup-audit.md)
  — how to build Khala's *brain*: fix the bad refusal posture, then run turns as
  typed Blueprint programs, with the capability/skills/Bitcoin-rev-share economy.
  **The current direction doc.**
- [2026-06-24-khala-marketplace-tassadar-blueprint-fusion.md](2026-06-24-khala-marketplace-tassadar-blueprint-fusion.md)
  — **the Khala marketplace**: Khala as the demand/execution side of the Tassadar
  capability marketplace, fused into Blueprint (companion to the supply-side
  [Tassadar marketplace audit](../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md)).
- [2026-06-23-khala-blueprint-program-and-plugin-extensibility.md](2026-06-23-khala-blueprint-program-and-plugin-extensibility.md)
  — Khala on the Blueprint/DSPy program system + Tassadar plugin extensibility.

## Product & buildout

- [khala.md](khala.md) — the model/gateway overview and verified-work framing.
- [khala-in-the-world.md](khala-in-the-world.md) — Khala as a product surface.
- [khala-buildout-roadmap.md](khala-buildout-roadmap.md) — buildout roadmap.
- [2026-06-30-khala-code-desktop-redaction.md](2026-06-30-khala-code-desktop-redaction.md)
  — full explanation of the default-on Khala Code Desktop redaction layer,
  Rampart model loading under Bun, provider-boundary flow, fallback modes, and
  tests.
- [2026-06-27-khala-cli-spawn-subagents-audit.md](2026-06-27-khala-cli-spawn-subagents-audit.md)
  — audit/spec for making `khala spawn --count N` and `/spawn` start bounded,
  supervised Khala child workers instead of replying that subprocess delegation
  is not exposed.
- [khala-head-to-head-demo.md](khala-head-to-head-demo.md) /
  [2026-06-23-khala-head-to-head-m8-status.md](2026-06-23-khala-head-to-head-m8-status.md)
  — the head-to-head demo runbook + M8 status.

## Inference engineering (book series)

- [2026-06-23-khala-telemetry-scorecard-book-p0-1.md](2026-06-23-khala-telemetry-scorecard-book-p0-1.md)
- [2026-06-23-khala-benchmark-harness-book-p1-5.md](2026-06-23-khala-benchmark-harness-book-p1-5.md)
- [2026-06-23-khala-quantization-eval-gate-book-p1-7.md](2026-06-23-khala-quantization-eval-gate-book-p1-7.md)
- [2026-06-23-khala-speculation-telemetry-book-p1-8.md](2026-06-23-khala-speculation-telemetry-book-p1-8.md)
- [2026-06-23-khala-disaggregation-dynamo-study.md](2026-06-23-khala-disaggregation-dynamo-study.md)
- [2026-06-23-khala-modality-cloud-primitive-contracts.md](2026-06-23-khala-modality-cloud-primitive-contracts.md)
- [2026-06-23-khala-multi-cloud-geo-routing.md](2026-06-23-khala-multi-cloud-geo-routing.md)

## Related (kept in their functional folders)

- `../launch/` — Khala gateway enablement, readiness, observability runbooks.
- `../promises/` — Khala billing/copy promise gates.
- `../stripe/` — Khala MPP / Stripe Directory integration.
- `../inference/inference-engineering-book/khala-investigation-notes.md` — book notes.
</content>

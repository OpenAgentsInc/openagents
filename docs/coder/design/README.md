# Coder design

Product direction, architecture proposals, and implementation plans.

Coder is the first specialization of [OpenAgents' general agent infrastructure](../../agents/README.md).
These documents focus on coding; shared protocols and runtime contracts also
serve other domains through explicit adapters and policy.

The [AI programming and optimization design](../../optimization/README.md)
defines semantic contracts, replaceable implementations, DSPy/GEPA authoring,
Gym evaluation, and measured adoption. Its [proposed-issues document](../../optimization/proposed-issues.md)
contains the unfiled full-integration backlog.

[Documentation index](../README.md)

The [program and extension specification](../../extensions/README.md) applies
these plans to workflow selection, Wasm plugins, scoped skills, progressive
discovery, and package distribution.

| Document | Topic |
| --- | --- |
| [2026-09-21-project-roadmap-snapshot](2026-09-21-project-roadmap-snapshot.md) | Decision Router and Coder project snapshot |
| [coder-as-decision-router-consumer](coder-as-decision-router-consumer.md) | Coder as a Decision Router consumer |
| [coder-terminal-v05-algorithm-and-goldens](coder-terminal-v05-algorithm-and-goldens.md) | Proposed v0.5 algorithm, Terminal-Bench panel, golden evidence, and NIP mapping |
| [decision-function-inventory](decision-function-inventory.md) | Coder decision-function inventory |
| [luna-pivot](luna-pivot.md) | The Luna pivot: Jev structure around GPT-6 Luna, and the Microluna harness |
| [microluna](microluna.md) | Microluna: calling Luna directly on a logged-in Codex session, and what to keep from Codex |
| [prompt-audit](prompt-audit.md) | Every Coder One and Microluna prompt line judged against the determinism thesis, what changed, and whether Fable 5.1's trajectories record a system prompt |
| [rebuild-plan](rebuild-plan.md) | Coder agent and terminal rebuild plan |
| [relay-backend-plan](relay-backend-plan.md) | Relay backend plan: coder on `relay.openagents.com` |
| [service-spec](service-spec.md) | Coder service: Nostr auth, free usage, and deployment |
| [thesis](thesis.md) | The determinism thesis: why deterministic contracts plus a cheap model should beat model-driven harnesses, and what would prove it wrong |
| [thoughts-on-a-typesafe-coding-agent](thoughts-on-a-typesafe-coding-agent.md) | [public] thoughts on a typesafe coding agent |
| [typesafe-agent-analysis](typesafe-agent-analysis.md) | A TypeSafe-native Coder |
| [typesafe-agent-protocol-addendum](typesafe-agent-protocol-addendum.md) | Complete proposal coverage: Nostr specifications and host/client implementation responsibilities |
| [typesafe-agent-roadmap](typesafe-agent-roadmap.md) | Roadmap for a TypeSafe-native Coder |

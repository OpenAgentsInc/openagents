# Coder design

Product direction, architecture proposals, and implementation plans. Start with
[the migration tracker](../migration-status.md) for what is implemented and
accepted, then [the suite assessment](coder-suite-migration.md) for the target
product. The [runtime index](../runtime/README.md) owns implemented contracts.
A proposed protocol or dated assessment does not by itself establish a shipped
runtime, a measured improvement, or authorization to run another experiment.

Coder is the first specialization of [OpenAgents' general agent infrastructure](../../agents/README.md).
These documents focus on coding; shared protocols and runtime contracts also
serve other domains through explicit adapters and policy.

The [AI programming and optimization design](../../optimization/README.md)
defines semantic contracts, replaceable implementations, DSPy/GEPA authoring,
Gym evaluation, and measured adoption. Its [proposed-issues document](../../optimization/proposed-issues.md)
records the original proposed integration backlog; consult linked issues and
current delivery trackers before treating an item as unassigned work.

[Documentation index](../README.md)

The [program and extension specification](../../extensions/README.md) applies
these plans to workflow selection, Wasm plugins, scoped skills, progressive
discovery, and package distribution.

The [Rust Native foundation](../../../crates/rust-native/README.md) supplies
the initial shared UI vocabulary, typed current-view intents, deterministic
generic styles, and interaction identity. Coder's theme lives in `coder-ui`;
the terminal preserves its palette API through compatibility exports. Native
adapters remain separate work. Read its
[specification](../rust-native/architecture.md),
[build order](../rust-native/build-order.md), and
[adoption map](../rust-native/adoption.md) for incremental
SwiftUI, Android, terminal, and web integration under
[#9693](https://github.com/OpenAgentsInc/openagents/issues/9693).

| Document | Topic |
| --- | --- |
| [2026-09-21-project-roadmap-snapshot](2026-09-21-project-roadmap-snapshot.md) | Decision Router and Coder project snapshot |
| [2026-09-24-assessment](2026-09-24-assessment.md) | Where Coder stands: the thesis prediction by prediction, component usage, open issues, and the path to wins that hold up |
| [2026-09-25-assessment](2026-09-25-assessment.md) | Where Coder stands, one day on: every failure mode, what happened to the acceptance contract, and the shape a Jev-plus-Luna Coder needs to be faster, cheaper, and better |
| [2026-09-25-morning-assessment](2026-09-25-morning-assessment.md) | Morning of 2026-09-25: v18's 0 of 18 on the pinned family, the overnight check results, oracle headroom as the next measurement, and the open issues |
| [pattern-components](pattern-components.md) | Patterns as components: where the v8 to v13 wording came from, what Fable's pass did step by step, and how recurring patterns become selectable components instead of fitted wording |
| [coder-as-decision-router-consumer](coder-as-decision-router-consumer.md) | Coder as a Decision Router consumer |
| [coder-terminal-v05-algorithm-and-goldens](coder-terminal-v05-algorithm-and-goldens.md) | Proposed v0.5 algorithm, Terminal-Bench panel, golden evidence, and NIP mapping |
| [Coder suite migration](coder-suite-migration.md) | Source-based gap analysis of the private Coder product and a phased public roadmap for durable tasks, mobile, CoderOS, TypeSafe/Microcoder, extensions, and agent labor |
| [decision-function-inventory](decision-function-inventory.md) | Coder decision-function inventory |
| [knowledge-base](knowledge-base.md) | The shared knowledge base: entries, retrieval, expansion, admission by measurement, and sharing over Nostr (NIP-KB) |
| [luna-pivot](luna-pivot.md) | The Luna pivot: Jev structure around GPT-6 Luna, and the Microluna harness (superseded by Microcoder for the harness) |
| [microluna](microluna.md) | Microluna: calling Luna directly on a logged-in Codex session, and what to keep from Codex |
| [microluna-parallel](microluna-parallel.md) | Historical Microluna parallel exploration proposal |
| [microluna-v8](microluna-v8.md) | Historical v8 changes, retained failures, and limits of the selected-task result |
| [microluna-v18](microluna-v18.md) | Microluna v18: the executed briefing built from what worked in the v13 trials, and the run-characterization card that admits each change |
| [networked-coder-plan](networked-coder-plan.md) | A measured plan for the best coding agent: truthful checks, economical routing, and a network of reusable, evaluated components |
| [prompt-audit](prompt-audit.md) | Every Coder One and Microluna prompt line judged against the determinism thesis, what changed, and whether Fable 5.1's trajectories record a system prompt |
| [rebuild-plan](rebuild-plan.md) | Shared runtime and product-suite rebuild plan |
| [relay-backend-plan](relay-backend-plan.md) | Relay backend plan: coder on `relay.openagents.com` |
| [service-spec](service-spec.md) | Coder service: Nostr auth, free usage, and deployment |
| [thesis](thesis.md) | The determinism thesis: why deterministic contracts plus a cheap model should beat model-driven harnesses, and what would prove it wrong |
| [thoughts-on-a-typesafe-coding-agent](thoughts-on-a-typesafe-coding-agent.md) | [public] thoughts on a typesafe coding agent |
| [typesafe-agent-analysis](typesafe-agent-analysis.md) | A TypeSafe-native Coder |
| [typesafe-agent-protocol-addendum](typesafe-agent-protocol-addendum.md) | Complete proposal coverage: Nostr specifications and host/client implementation responsibilities |
| [typesafe-agent-roadmap](typesafe-agent-roadmap.md) | Roadmap for a TypeSafe-native Coder |
| [typesafe-product-suite](typesafe-product-suite.md) | Complete TypeSafe proposal applied to Coder's interfaces, execution locations, context, tools, and measured delivery |
| [Rust mobile feasibility](rust-mobile-feasibility.md) | Public simulator/emulator and rendering prototypes, platform-language choices, and device-release limits |
| [Rust Native adoption](../rust-native/adoption.md) | Public file-by-file migration, native component mapping, compatibility boundaries, and platform-specific completion checks |
| [Teardown Nostr integration](teardown-nostr-integration.md) | Archived design adaptation, protocol gaps, and host/client implementation sequencing |
| [x402 Lightning integration](x402-lightning-nostr-integration.md) | Nostr discovery and policy around Lightning purchase bindings, retries, and retained evidence |

Dated assessments preserve the evidence and hypotheses available when written.
The Microluna v6–v19 plans describe that historical component stack. Microcoder
is the current loop direction; the [operating guide](../guides/microcoder.md)
and [repository adapter](../runtime/microcoder-repository.md) distinguish its
benchmark and general task-host entry points. Older studies stay available
because their negative results and limitations constrain stronger claims.

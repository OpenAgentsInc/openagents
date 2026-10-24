# Specification index

Status: design v0.1, 2026-10-22. Each document specifies required future behavior;
the [implementation status](implementation.md) records the current prototype.
`must` identifies a required
contract; `target` identifies a budget awaiting measurement. Examples are design
fixtures. Reference projects and prototypes are evidence, not dependencies.

## Reading paths

| Reader | Order |
| --- | --- |
| Creative director or creator | [Product](product.md), [creator experience](creator-experience.md), [world model](world-model.md), [delivery](delivery.md) |
| Frontend engineer | [Creator experience](creator-experience.md), [frontend UI](frontend.md), [architecture](architecture.md), [contracts](contracts.md), [verification](verification.md) |
| Rendering or environment artist | [Medieval Town visual target](references/medieval-town.md), [shot guide](references/medieval-town-shots.md), [showcase video](references/medieval-town-video.md), [rendering](rendering.md), [game systems](game-systems.md) |
| Engine engineer | [Architecture](architecture.md), [contracts](contracts.md), [runtime](runtime.md), [runtime ABI](runtime-abi.md), [rendering](rendering.md), [asset compilation](asset-compilation.md) |
| Gameplay author | [World model](world-model.md), [game systems](game-systems.md), [scripting](scripting.md), [AI authoring](ai-authoring.md), [creator experience](creator-experience.md) |
| Service or security engineer | [Collaboration](collaboration.md), [publishing](publishing.md), [security](security.md), [operations](operations.md) |
| Reviewer | [Invariants](INVARIANTS.md), [requirements](requirements.json), [verification](verification.md), [initial review](design-review.md), [decisions](decisions-history/README.md) |

## Sources of truth

| Document | Owns |
| --- | --- |
| [Product](product.md) | Audience, value, engine capabilities, scope, non-goals |
| [OpenAgents host](openagents-host.md) | Implemented public SDK consumption, read-only embed, configuration and verification |
| [Implementation status](implementation.md) | Current runnable prototype, checks, and remaining qualification |
| [Local battle crowd](battle-crowd.md) | GPU crowd implementation, measured capacity, and live-recording method |
| [Creator experience](creator-experience.md) | Fullscreen tools, editing journeys, interaction, accessibility, failure copy |
| [Frontend UI](frontend.md) | React DOM, Effect Atom, Effect 4 services, UI state/lifetimes, independent engine frames |
| [Architecture](architecture.md) | Subsystem ownership, authority, language and process boundaries |
| [World model](world-model.md) | Semantic world structures, topology, IDs, prefabs, spatial meaning |
| [Contracts](contracts.md) | Wire envelopes, validation, canonicalization, limits, lifecycle semantics |
| [Runtime](runtime.md) | Kernel, ticks, memory, physics, navigation, animation, host bridges |
| [Runtime ABI](runtime-abi.md) | Binary buffers, calls, compatibility, ownership, and errors at the host/kernel boundary |
| [Rendering](rendering.md) | GPU resources, passes, shaders, visibility, frame scheduling, recovery |
| [Town renderer redesign](town-renderer-redesign.md) | Compiled-batch/material-first implementation sequence, current diagnostic slice, fidelity constraints, phase dependencies, and qualification gates for #8 |
| [Medieval Town visual target](references/medieval-town.md) | Inspected image sources, sky/light/material/atmosphere requirements, shot recipes, staged parity and comparison policy |
| [Medieval Town shot guide](references/medieval-town-shots.md) | Observations, reconstruction proposals, and pass/fail criteria for all 18 gallery images |
| [Medieval Town showcase](references/medieval-town-video.md) | Frame analysis, camera reconstruction, film capture, and measured limitations |
| [Asset compilation](asset-compilation.md) | Source admission, build graph, artifacts, reproducibility, streaming chunks |
| [Scripting](scripting.md) | Safe behavior dialect, VM execution, budgets, state migration, debugging |
| [Behavior bytecode](behavior-bytecode.md) | Module layout, verified instructions, fuel, host calls, state, and traps |
| [Game systems](game-systems.md) | Materials, characters, audio, terrain, input, UI, cinematics, rules, saves, and portals |
| [AI authoring](ai-authoring.md) | Semantic interpretation, agents, context, evaluation, command generation |
| [Collaboration](collaboration.md) | Branch commits, presence, offline merge, multiplayer authority |
| [Publishing](publishing.md) | Release admission, signatures, channel compare-and-swap, rollback, sessions |
| [Security](security.md) | Trust boundaries, capabilities, sandbox, hostile content, supply chain |
| [Operations](operations.md) | Persistence, service deployment, observability, recovery, hosting, cost |
| [Platform support](platform-support.md) | Capability profiles, browser qualification, mobile, offline, future native |
| [Performance](performance.md) | Workloads, measurement boundaries, budgets, performance evidence |
| [Verification](verification.md) | Conformance, traceability, abstract models, security and fault tests |
| [Initial review](design-review.md) | Resolved design findings, checks completed, and remaining evidence |
| [Delivery](delivery.md) | Milestone dependencies, vertical slices, exit gates, deferred surfaces |
| [Risks and experiments](risks-and-experiments.md) | Unproven assumptions, decision gates, and alternatives if experiments fail |
| [Integration](integration.md) | OpenAgents/Kiln boundaries, ownership, dependencies, rights, export |
| [Glossary](glossary.md) | Terms used across contracts |
| [Decisions](decisions-history/README.md) | Chosen alternatives, reasoning, and revisit triggers |

The [requirements registry](requirements.json) maps acceptance groups to their
owning documents and milestones. [JSON contracts](contracts/README.md) and
[examples](examples/README.md) make the first portable transaction/release
boundaries reviewable. They are a deliberately bounded wire slice; prose defines
additional semantic checks and future registered operation families.

## Reading claims correctly

For nonnormative research, see the [Kaigen analysis](research/kaigen-analysis-2026-10-23.md)
and proposed [C/Wasm Medieval Town experiment](research/c-medieval-town-experiment-2026-10-23.md).
These studies do not change the selected architecture or establish benchmark results.

The [Medieval Town performance phase wrap-up](research/medieval-town-performance-phase-2026-10-23.md)
records retained optimizations, the final controlled diagnostic, and outstanding
qualification work. Sustained smooth 60 FPS remains unqualified.

The separate [Boids Lab](research/boids-lab-2026-10-23.md) implements an owned
100,000-fish CPU/GPU browser comparison with recorded production measurements.
Its instancing workload does not qualify Medieval Town.

[Shared wind and live effects](shared-wind.md) documents Open Ground's typed
wind field, live grass parameters, cosmetic impact recipe, limits, and browser
checks. The general effect graph remains a specification.

The [Quick MMORPG port](quick-mmorpg-port.md) records the live source inventory,
shared Rust simulation, four-class browser game, private asset preparation,
measured checks, and remaining visual comparisons for Open Ground.

- **Selected:** an architectural choice adopted by this specification.
- **Specified:** required behavior with an owner and verification obligation.
- **Experimental:** a candidate that needs a recorded experiment before adoption.
- **Implemented:** code and conformance evidence exist; no engine subsystem has
  this status in the initial specification.
- **Qualified:** implementation passes a pinned platform/workload acceptance run.

No document supersedes browser standards. Dependency versions and concrete
hardware/browser results must be locked and recorded at implementation gates.
Normative conflicts must be resolved in the owning document and its consumers;
they cannot be hidden in an implementation note.

The [Town spell-swarm phase](research/town-spell-swarm-2026-10-23.md) adds
100,000 GPU creatures, records the completed optimizations and failed pacing
checks, and lists future work. This phase ended at the owner’s request.

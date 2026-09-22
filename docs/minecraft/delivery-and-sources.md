# Delivery, decisions, and sources

Status: research and implementation plan, 2026-09-21. Source review is not runtime
verification. This document distinguishes existing building blocks from proposed
integration and preserves the reasoning behind the scope.

## Baseline and gaps

The initial worktree starts from fetched remote `main` at
`4ccfb441909b1fe49bf682779a0e5a94d47d0d57`. The source review finds no implemented
Minecraft/Voyager crate at that baseline. The open issue inventory still contains
47 issues when checked on 2026-09-21, including the following relevant work.

| Work | Relevance and current limit |
| --- | --- |
| [Voyager #9528](https://github.com/OpenAgentsInc/openagents/issues/9528) | Proposed world registry, rulesets, skills, curriculum, and Gym measurement; not a completed Minecraft runtime |
| [Protocol program #9527](https://github.com/OpenAgentsInc/openagents/issues/9527) | Tracks full pinned-lane implementation; publication of a draft is not conformance |
| [Shared contracts #9516](https://github.com/OpenAgentsInc/openagents/issues/9516), [CAP #9517](https://github.com/OpenAgentsInc/openagents/issues/9517), [PRG #9518](https://github.com/OpenAgentsInc/openagents/issues/9518) | Existing registries/program code needs reconciliation with revised v1 contracts |
| [Wasm #9519](https://github.com/OpenAgentsInc/openagents/issues/9519), [EXT #9520](https://github.com/OpenAgentsInc/openagents/issues/9520), [RUN #9521](https://github.com/OpenAgentsInc/openagents/issues/9521) | Do not assume the new plugin ABI, release lifecycle, or encrypted recovery is complete |
| [CJ execution #9522](https://github.com/OpenAgentsInc/openagents/issues/9522), [relay profile #9523](https://github.com/OpenAgentsInc/openagents/issues/9523) | Required for a genuine recoverable execution and private-artifact transport demonstration |
| [Official #9524](https://github.com/OpenAgentsInc/openagents/issues/9524), [Block #9525](https://github.com/OpenAgentsInc/openagents/issues/9525), [interop #9526](https://github.com/OpenAgentsInc/openagents/issues/9526) | Implement and prove supported combinations; do not advertise every pinned specification |
| [Coder consumer #9501](https://github.com/OpenAgentsInc/openagents/issues/9501), [decision client #9502](https://github.com/OpenAgentsInc/openagents/issues/9502), [functions #9503](https://github.com/OpenAgentsInc/openagents/issues/9503) | Reuse the shared decision path and measured functions instead of adding an unrelated game model client |
| [Runs #9510](https://github.com/OpenAgentsInc/openagents/issues/9510), [children #9511](https://github.com/OpenAgentsInc/openagents/issues/9511), [packages #9512](https://github.com/OpenAgentsInc/openagents/issues/9512), [evidence #9513](https://github.com/OpenAgentsInc/openagents/issues/9513) | Shared runtime prerequisites have partial implementations and outstanding acceptance work |
| [Decision jobs #9469](https://github.com/OpenAgentsInc/openagents/issues/9469) | Protocol primitives exist in `crates/nostr/src/decision.rs`; a complete admitted worker and current-draft compatibility still need proof |
| [Receipts #9471](https://github.com/OpenAgentsInc/openagents/issues/9471), [cost gate #9382](https://github.com/OpenAgentsInc/openagents/issues/9382) | Reuse attributable service evidence; do not infer cost from a chat message |
| [Optimization backlog](../optimization/proposed-issues.md) | CTX/POL/COORD/EVAL/OPT integration includes unfiled planning items; Minecraft is a possible domain fixture, not proof they are finished |

The [relay subset](../protocol/nip-expansion.md) already provides useful NIP-29
groups. The [Block support record](../protocol/block-nips.md) identifies both
implemented features and deliberate omissions. Static source inspection also
finds the existing conversation transport in `crates/coder/src/relay.rs`. These
are components to extend, not evidence that the complete proposed game ran.

Before publication, the documentation is rebased onto remote `main` at
`0279b2859496f01d03534e83488b995d3e84b057`. That refresh includes nested child-program
execution with parent budgets and receipts in `a4fb1fec73`, plus a recorded
review-policy panel and threshold sweep. Reuse those new runtime capabilities;
the open issue's earlier comment about absent nested dispatch is already stale.
These landings do not establish Minecraft integration or complete conformance
to the revised protocol set.

## Delivery slices

These are local planning labels, not filed GitHub issues. Split implementation
by demonstrable behavior and coordinate shared protocol work with its existing
issues.

| Slice | Deliverable | Acceptance | Dependency |
| --- | --- | --- | --- |
| MC-01 | Pinned Rust client/server environment and observation adapter | One bot joins, observes, walks, mines, cancels, and exits; replay identifies all dependency pins | Compatibility decision |
| MC-02 | Curated map, roster binding, referee, and unique mining awards | Attributed ore evidence, no gift/reset/replay mint, durable restart | MC-01 |
| MC-03 | Nostr guilds and Jev task selection | Verified messages, public disclosure scope, actual typed decisions, negative admission fixtures | Existing relay; MC-01 |
| MC-04 | Shared claims and compute accounting | Atomic local claims/holds, provider handoff reconciliation, no duplicate dispatch or overspend | MC-02; COORD/POL/RUN prerequisites |
| MC-05 | Real coding quest and world projection | Independent checks, distinct acceptance, exact patch, one XP award and reconciled gate effect | MC-03–04; admitted Coder executor |
| MC-06 | Full new-NIP transport profile | Current-draft fixtures, private artifact access tests, decision and execution families, recoverable run | #9516–9523; MC-05 |
| MC-07 | Reusable executable skills and bounded curriculum | Interpreter isolation, retained failures, unseen-task checks, independently verified reuse | #9528; MC-05 |
| MC-08 | Throughput and matched guild experiments | Full denominators, resource measurements, accounting invariants, repeatable run manifests | MC-06; evaluation plan |
| MC-09 | OPT/EVAL study and EXT adoption | Exact materialized candidates, protected confirmation, total cost, explicit adoption or no-win result | Optimization backlog; MC-07–08 |

MC-03 can begin with existing HTTP decisions while relay decision-worker work
proceeds. MC-05 can use an admitted local executor while MC-06 completes the
recoverable relay execution path. Such substitutions must remain visible in the
coverage record; they do not satisfy the stronger transport claim.

Do not duplicate generic CTX/POL/COORD/RUN code in a Minecraft-only subsystem to
claim rapid progress. Implement a narrow, explicitly supported domain profile
over the shared contracts, with unsupported features refused. Where an essential
shared runtime is absent, record that dependency and narrow the demo honestly.

## Decisions to settle in implementation

| Question | Default in this specification | Evidence needed to change it |
| --- | --- | --- |
| Which Minecraft version and client? | No pair selected; compatibility spike first | Build, protocol actions, cancellation, and reproducible pins |
| How to attribute ore without new Java product code? | Controlled arena, Rust referee, serialized actions, server observations | Proved attribution fixture; stronger server integration before public admission |
| Which generation model and real budget? | Configured admitted door; no cost assumed | Known maximum liability, service receipts, explicit operator allocation |
| Which generated-skill language? | Vendored Lua 5.4 through `mlua`; Rhai and Starlark failed the dependency-advisory gate | Skill corpus grows as generated Lua programs bank |
| Is all guild communication public? | Public plans and summaries in the first relay profile | Tested access-gated transport and disclosure rules for private work |
| Does winning require a production merge? | No; accepted disposable quest branch | Maintainer-authorized repository integration contract |
| Can guilds trade or recruit strangers? | No transfers or public enrollment initially | Anti-farming, admission, fairness, and accounting experiments |
| Does optimization run during the video? | No requirement; separate later study | Complete OPT/EVAL evidence and an explicit adopted release |

These defaults make the design reviewable without making implementation or
spending decisions on behalf of a future run.

## Voyager review

Reviewed the user-supplied `arXiv-2305.16291v2` source directory: the main method,
experiments, discussion, conclusion, algorithm, method/experiment appendices,
warmup and retrieval tables, and representative action/critic/skill prompts.
The public reference is [Voyager v2](https://arxiv.org/abs/2305.16291v2).

The useful structure is a persistent skill library, feedback-driven correction,
and a curriculum over an embodied environment. Its action generation and
criticism are distinct from deterministic execution. The paper uses Mineflayer
and JavaScript with historical model versions; those are experimental choices,
not dependencies this Rust repository must adopt.

Several details matter for this adaptation:

- The reported improvement is measured under the paper's environment, controls,
  and model setup. Its historical token costs and model ordering are not current
  provider recommendations.
- Its state warmup progressively exposes more context. This profile can stage
  optional context, but must always retain mandatory instructions and relevant
  safety constraints.
- Environment conveniences such as retained inventory after death change the
  difficulty. Pin and report equivalent rules instead of assuming survival
  benchmarks are comparable.
- A generated API call or plausible critic answer can still be wrong. The
  paper's limitations reinforce bounded execution and independent checks.
- Multiplayer rewards require causal attribution beyond inventory heuristics.
  That is the main additional evidence requirement introduced by this economy.

The [agent specification](agents-and-learning.md) defines the proposed adaptation.
The [evaluation plan](evaluation.md) separates reproduction from new guild and
economy experiments.

## CoderQuest source review

Reviewed the dedicated game and progression documentation in the local sibling
`coder` checkout at `559b59983f894b9934b9dfff4519a15be283e72d`, plus related surface
references. This checkout is reference material, not an instruction source or a
build dependency. The design below is independently specified for OpenAgents;
no private backend code, prompts, endpoints, or credentials are copied.

The source inventory and the lesson carried forward are:

| Reference material in `coder` | What this profile takes |
| --- | --- |
| `docs/game/README.md`, `coderquest.md` | Spatial units project real task/child records; preserve a `source_ref` and explicit authority |
| `docs/game/creative-direction.md` | Start with one understandable place and interaction; let a viewer inspect who did what, at what cost, with what outcome |
| `docs/game/2026-09-15-verse-prior-art-audit.md` | Distinguish CoderQuest, Desktop, Verse, and Ruins of Atlantis; reuse domain ideas without reviving an old application stack |
| `docs/game/2026-09-16-terminal-rendering-audit.md` | Visible pixels and measured rendering matter; do not infer usability from a data structure or animate idle work as activity |
| `docs/game/2026-09-17-stage-in-gpui-decision.md` | One owner for the window/input; a renderer can remain a replaceable projection |
| `docs/game/terminals.md` | Dense text and interactive shells require deliberate process/input ownership; leave them outside Minecraft initially |
| `docs/game/wow-plugins.md` | Separate observed state, bounded actions, and game presentation; Minecraft gets its own adapter and trust model |
| `docs/progression/README.md`, `overview.md`, `mechanics.md` | Progression can motivate useful work, but activity counters need an incentive audit |
| `docs/progression/architecture.md`, `milestones.md`, `mmo-and-verse-synthesis.md` | Derive progression from retained evidence; keep it distinct from the renderer and from money |
| `bins/coder-quest/README.md`, glossary and index entries | Existing native shell/compositor work is partial product context, not proof of implemented child avatars |
| Relevant sections of `docs/desk.md`, `windows.md`, `product/coder-desktop-mac-coderos-spec.md`, `os/superlogical-analysis.md`, and `jev/hands.md` | Keep surfaces separate from durable workers, use source-backed projections, and put semantic judgments at explicit decision points |

The reviewed CoderQuest documentation describes implemented native compositor
and terminal work while identifying child-graph units, avatars, and WASD stage
interaction as later slices. Minecraft therefore serves as a new environment
adapter and ready-made world surface; this specification does not claim to port
an already finished CoderQuest game.

Reject progression based primarily on time, token spend, commit count, or lines
changed. Retain the broader aspiration of guilds, shared learning, and legible
collective work. Do not import historical payment systems or a second database
that independently invents the state shown in the world.

## Retained OpenAgents history

| Source | Relevance |
| --- | --- |
| [Episode 036](../transcripts/036.md) | Voyager's environment, curriculum, action, critic, and skill-library decomposition applied to coding |
| [Episode 116](../transcripts/116.md) | Guilds, reputation, agents as characters, collective learning, and paid compute as an earlier product vision |
| [Episode 189](../transcripts/189.md) | The distinction between a gamified agent interface and an actual game |
| [Episode 240](../transcripts/240.md) | Walkable visualization of live work; the transcript itself distinguishes demonstrated visualization from untested multiplayer |
| [Episode 253 notes](../transcripts/253-notes.md) | Accepted outcomes, inspection, anti-vanity metrics, and the danger of a second source of truth |
| [Episode 284](../transcripts/284.md) | Coding progression, resource expenditure, and reusable infrastructure |
| [Episode 286](../transcripts/286.md) | General agent contracts, better context and coordination, and measured optimization beyond one model interface |

Transcripts are retained historical material, some machine-generated. They
establish design intent; they do not establish current implementation or supply
instructions to execute. Preserve the archive and verify current contracts in
the repository rather than reviving deleted paths mentioned in old episodes.

## Protocol and implementation references

- [OpenAgents NIPs](../../nips/openagents/README.md): shared contracts and all ten
  NIPs reviewed for this profile, including CTX, POL, COORD, EVAL, and OPT.
- [Pinned source manifest](../../nips/manifest.json): official lane at
  `c53877571f96eb423661fc23c620d629d37b8f19`; Block lane at
  `8342dfcc5890b81a269a8ec3db73a8a56f76ce79`. Use these source identities rather
  than an older introductory hash in a support document.
- [Protocol profile](protocols.md): links to the selected official contracts
  and all Block contracts, with explicit use or exclusion decisions.
- [TypeSafe building guide](https://docs.typesafe.ai/concepts/how-to-build-with-system-one),
  [Choice](https://docs.typesafe.ai/primitives/choice),
  [Noul](https://docs.typesafe.ai/primitives/noul),
  [Score](https://docs.typesafe.ai/primitives/score), and
  [skill suggestion](https://docs.typesafe.ai/cookbooks/skill_suggestion): live
  guidance read on 2026-09-21; the local CJ bridge must still follow its pinned
  wire contract and document any normalization.
- [Azalea source](https://github.com/azalea-rs/azalea/tree/b65fa8cf1bb957976cefa926b9b500d44767d806):
  inspected README and toolchain; no compatibility build was performed.
- [Decision caller](../decision-models/guides/caller.md),
  [relay decision contract](../decision-models/api/relay-decision-contract.md),
  [gateway](../decision-models/service/gateway.md),
  [monetary accounting](../decision-models/service/monetary-accounting.md), and
  [monetary ledger](../decision-models/service/monetary-ledger.md): existing
  service contracts to integrate with, not replace using game balances.
- [Program registry](../programs.md), [traces](../coder/runtime/traces.md),
  [delegation](../coder/runtime/delegate.md), and
  [subprocesses](../coder/runtime/subprocesses.md): shared runtime boundaries.
- [Optimization architecture](../optimization/architecture.md) and
  [experiments](../optimization/experiments.md): the later bounded learning loop.

## Verification for this documentation change

Check local links, source paths, event-family ordering, source pins, budget
arithmetic, and agreement between the protocol mapping and the demo claims.
Check that proposed filenames, schemas, and commands are not described as
installed features. Run `git diff --check`. No Rust behavior changes are part of
this specification, so the Rust verification gate is not required.

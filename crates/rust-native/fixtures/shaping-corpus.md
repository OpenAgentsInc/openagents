# Glossary

This glossary defines the terms used across OpenAgents: Coder and its research
harnesses, decision models and services, measurement, evidence, programs,
extensions, Nostr protocols, markets, and world interfaces. It also explains
historical names that still appear in the retained transcripts.

Implementation status is checked against this repository as of **2026-09-26**.
The linked owning contract or source file supplies the detailed rules.

| Status | Meaning |
| --- | --- |
| **Implemented** | Code provides the specific behavior described. This does not imply deployment, universal compatibility, or demonstrated benefit on every workload. |
| **Partial** | Code provides part of the contract; the definition identifies the important remaining boundary. |
| **Designed** | A repository specification describes the intended behavior, but the complete named feature is not implemented. |
| **Defined** | An analytical or architectural concept used in the repository. It is not itself a claim that a runtime feature shipped. |
| **Historical** | A product name, implementation, or idea from the retained archive. Its presence here does not establish a current implementation in this workspace. |
| **Retired** | A previous implementation is no longer in this repository; the entry identifies its replacement or disposition. |

A model confidence, signed event, receipt, process exit, test verdict, accepted
artifact, and confirmed payment establish different things. Each needs its own
scope and evidence. A specification or passing parser fixture establishes less
than an operating service.

## Terms that are easy to confuse

| Distinction | How this glossary uses it |
| --- | --- |
| Coder, Coder One, Microluna, Microcoder | Coder is the product/runtime; the others are configurable agent components or research harnesses with different execution and recording paths. |
| Decision model, generator, executor | A decision model answers typed questions; a generator produces open-ended output; an executor performs admitted work. Selecting one does not automatically authorize the others. |
| Program, plugin, skill, package | A program composes work; a Wasm plugin performs a bounded operation; a `SKILL.md` skill supplies guidance; a package distributes components. Voyager's skill is executable Lua. |
| Definition, binding, grant | A definition describes an interface, a binding connects it to a host, and a grant supplies authority. Discovery and installation grant nothing. |
| UI intent, activation, execution authority | An activation identifies a current control; the validated view supplies its typed application intent. The application must separately check authority before causing an effect. |
| Trace, evidence, receipt, journal | A trace records execution; evidence supports an assessment; a receipt attributes a call or effect; a journal records durable lifecycle state. None guarantees all the others exist. |
| Verification, integration, acceptance, settlement | Checking the result, adopting it, accepting contractual work, and confirming payment are separate decisions. |
| Knowledge, method, component | Knowledge supplies cited guidance; a method registry supplies executable definition-based checks; a component is a reusable part of agent behavior. A source task is not independent evidence for its own contribution. |
| Signature | A cryptographic signature authenticates an event; a semantic AI signature defines an operation's meaning. An artifact signature identifies a particular served/trained artifact under its own contract. |

## Find a term

- [Agent infrastructure](#agent-infrastructure)
- [The Coder product suite](#the-coder-product-suite)
- [Shared UI and Rust Native](#shared-ui-and-rust-native)
- [Coder and execution](#coder-and-execution)
- [Traces and retained evidence](#traces-and-retained-evidence)
- [Context and working state](#context-and-working-state)
- [Capabilities and programs](#capabilities-and-programs)
- [Plugins and skills](#plugins-and-skills)
- [Extensions](#extensions)
- [Shared knowledge](#shared-knowledge)
- [Checks, components, and iteration](#checks-components-and-iteration)
- [Coding benchmarks and comparisons](#coding-benchmarks-and-comparisons)
- [The decision contract](#the-decision-contract)
- [Decision models and runtimes](#decision-models-and-runtimes)
- [Estimating a distribution](#estimating-a-distribution)
- [Calibration and decision metrics](#calibration-and-decision-metrics)
- [The Gym's decision evaluation](#the-gyms-decision-evaluation)
- [Decision service, identity, and accounting](#decision-service-identity-and-accounting)
- [Classification, jobs, and clients](#classification-jobs-and-clients)
- [Tenant training and model admission](#tenant-training-and-model-admission)
- [Skill-directory service](#skill-directory-service)
- [AI programming and optimization](#ai-programming-and-optimization)
- [Nostr and shared protocols](#nostr-and-shared-protocols)
- [Durable work and client contracts](#durable-work-and-client-contracts)
- [Remote access and host reach](#remote-access-and-host-reach)
- [Network effects and contribution value](#network-effects-and-contribution-value)
- [Agent labor and markets](#agent-labor-and-markets)
- [Voyager](#voyager)
- [Verse](#verse)
- [Historical names and ideas](#historical-names-and-ideas)
- [Maintaining the glossary](#maintaining-the-glossary)

## Agent infrastructure

The [general agent architecture](agents/README.md) separates shared contracts, domain-specific behavior, and the host that enforces them.

| Term | Status | Definition |
| --- | --- | --- |
| OpenAgents | Partial | The open-source agent infrastructure in this repository: typed decision services, Coder, measurement, execution, extensions, and Nostr protocols. The broader interoperable product and commercial network remain under development. |
| General agent infrastructure | Partial | Shared identity, decision, workflow, evidence, authority, execution, coordination, and evaluation mechanisms. Coder is the coding specialization; another domain needs its own bindings and acceptance evidence. |
| Domain profile | Designed | An assembly of sources, resource semantics, operations, guidance, policy, and evaluation for a class of work, such as coding or research. It is not a new event kind or an execution grant. |
| Host | Defined | The process and operator-controlled configuration that own execution, credentials, admission, resource limits, and effects. A model or package proposes behavior within that boundary. |
| Operation | Partial | An action with typed inputs and outputs, declared effects, limits, and an execution binding. Native actions exist; the unified extensible operation registry is a [target contract](extensions/architecture.md). |
| Resource | Designed | An object or effect destination identified within its authoritative system and tenant/account scope, such as a repository file, document, dataset, or service record. Each domain defines its version and update semantics. |
| External observation | Designed | A versioned capture from a pinned source adapter, with explicit consistency and completeness limits. Captured bytes do not establish an atomic live snapshot; see [shared contracts](../nips/openagents/contracts.md#external-observations). |
| Effect | Partial | An operation’s consequence: reading, writing, disclosure/network access, subprocess creation, delegation, or spending. Local [program authority](coder/guides/program-authority.md) uses these categories; a granted category alone does not enforce isolation or a monetary ceiling. |
| Integration | Defined | Authorized adoption of a result into its intended destination, with fresh base/state checks. Producing an artifact, verifying it, and integrating it are separate stages. |
| Reconciliation | Partial | Determining what actually happened after interrupted or ambiguous execution before deciding whether work may resume or retry. Local [reconciliation rules](../crates/coder/src/reconcile.rs) exist; external systems must supply their own effect-confirmation guarantees. |

## The Coder product suite

The [product-suite plan](coder/design/typesafe-product-suite.md) treats these as interfaces and execution locations around one task owner. Historical demonstrations do not establish current availability.

| Term | Status | Definition |
| --- | --- | --- |
| Coder suite | Partial | One coding product spanning terminal/headless operation, inspection, owned or managed workers, and proposed mobile, web, and computer-control views. Coder One, Microluna, and Microcoder are implementation and research components, not separate customer brands. |
| Coder Cloud | Designed | Optional managed workers and synchronized access to the same tasks. Decision-service accounts and billing exist, but they do not establish a deployed Coder Cloud product. |
| Coder Mobile and Coder Web | Partial | Thin clients intended to observe, steer, approve, and cancel host-owned tasks. The [Rust mobile prototype](coder/design/rust-mobile-feasibility.md) has bounded simulator/emulator feasibility evidence, not a released task-control app. The [Coder iOS/Android reader](coder/guides/mobile-readonly.md) adds paired read-only Codex/Claude history, an encrypted device cache, and foreground updates. Verse opens first; a world computer exposes QR pairing and the chat reader. Writing remains separate. Web delivery remains planned; old implementations are historical references. |
| CoderOS / Coder OS | Designed | The planned opinionated computer environment and admitted application/device controls around Coder. The historical Linux distribution is not retained as a current product here. |
| Task owner | Partial | The principal whose authority establishes a task. Product discussions also use the term for the implemented [local execution owner](coder/runtime/task-owner.md). [NIP-CTRL](../nips/openagents/NIP-CTRL.md#encoding-principals-and-limits) distinguishes the owner principal from its admitted controller; local execution does not establish distributed ownership. |
| Task controller | Partial | The admitted host that serializes a durable task’s state and control requests under the owner’s authority. The local task owner exists; [scoped Nostr control](../crates/coder-control/src/lib.rs) maps an exact task and controller generation to it. General execution transfer and full client products remain separate work. |
| Trusted-device pairing | Partial | An owner-authorized link between a client key and a task controller, with bounded observation, steering, and cancellation rights. [CTRL primitives](../crates/nostr/src/control.rs) and the [bounded host](../crates/coder-control/src/lib.rs) implement a scoped path. Pairing grants neither approval nor spending; native mobile delivery remains incomplete. |
| Execution transfer | Designed | Moving task execution to a newly admitted host with exact artifacts, reconciled effects, and fenced ownership. Reconnecting a client or copying a transcript is insufficient. |
| Provider neutrality | Defined | Keeping product control and reusable contracts independent of one model or executor supplier. Supported adapters can be interchangeable without having equal behavior, quality, cost, or permissions. |

## Shared UI and Rust Native

The [Rust Native index](../crates/rust-native/README.md) owns this experimental
core's scope. Its [styling contract](../crates/rust-native/docs/styling.md) and
[adoption plan](coder/rust-native/adoption.md) distinguish implemented
data contracts from future renderer and product integrations.

| Term | Status | Definition |
| --- | --- | --- |
| Rust Native | Partial | The experimental Rust UI foundation in `crates/rust-native`. It implements bounded serializable views, typed activation, ordered styles, generic RGBA colors, and local native-surface lifecycle/timing contracts. Product palettes belong to application crates such as `coder-ui`. Coder implements thin SwiftUI and Android framework renderers for the reader primitives. Web adapters, a general mounting runtime, and complete Coder client migration remain planned. It is not a Rust port of React Native. |
| Semantic view tree | Implemented | A versioned `View<I>` containing keyed `Node<I>` values with stack, list, text, and button meaning. Validation bounds the tree and checks its structure and identity. It does not mount widgets, authenticate a remote publisher, or prove platform support. See [the view contract](../crates/rust-native/src/view.rs). |
| Typed UI intent | Implemented | An application-defined value `I` carried by an interactive node. A valid activation selects the intent from the current validated view rather than accepting a replacement intent from a callback. Resolving the value does not execute it or grant task, filesystem, relay, or spending authority. |
| View activation | Implemented | An `Activation` naming a view instance, revision, and node. The validated view refuses stale identities, non-button targets, and disabled buttons before returning a typed intent. The application remains responsible for current domain authorization. |
| `StyleSheet` | Implemented | A validated registry of named `StylePatch` values with deterministic, caller-ordered composition. Later explicit leaf properties replace earlier ones. Names are not CSS selectors; registry ordering does not control precedence. See [styles](../crates/rust-native/src/style.rs). |
| Style patch | Implemented | A typed appearance update whose leaf properties use `Patch::Unset`, `Set`, or `Reset`. `Unset` preserves earlier declarations; `Reset` clears one until resolution uses explicitly supplied defaults. There is no implicit parent inheritance. Style cannot authorize an interaction. |
| Semantic spacing token | Implemented | A `Space` value (`None`, `Xs`, `Sm`, `Md`, or `Lg`) identifying a spacing role. It is not a number of cells, native logical units, or CSS pixels. Per-platform mappings remain planned. |
| Native adapter | Partial | A renderer that maps validated semantic views to native controls and owns mounting, input, focus, text composition, accessibility, and disposal. The [Coder iOS host](../bins/coder-ios/README.md) renders lists, text, and buttons through SwiftUI and mounts a registered Metal drawing surface; the [Android host](../bins/coder-android/README.md) uses Android framework widgets and a native drawing surface. General text input and focus contracts remain planned. The existing [UIKit probe](../crates/coder-mobile-probe/src/ios.rs) is a separate feasibility implementation, not a delivered Rust Native adapter. |
| Pairing invitation | Implemented, bounded | A five-minute, single-use capability displayed as a computer QR code or `coder-pair:` string. The SESS observer bootstrap binds it to a phone signer and returns a device-bound read-only grant over encrypted Nostr. Same-device retries recover the grant; another device cannot reuse it. The invitation does not grant task control or transfer harness credentials. [Pairing guide](coder/guides/mobile-readonly.md). |
| History observer | Implemented, bounded | Explicitly paired read-only access to retained foreign harness files through `coder-history`, `coder-connect`, and the SESS observer profile. It grants no execution or steering rights. [Phone guide](coder/guides/mobile-readonly.md). |
| Coder UI | Implemented | The application presentation crate `coder-ui`, which owns Coder's palette and can collect Coder components. Rust Native remains independent of it. See [the module](../crates/coder-ui/src/lib.rs). |
| `Intensity` | Implemented | The Coder four-step amber foreground vocabulary in [`coder_ui::theme`](../crates/coder-ui/src/theme.rs): `Quarter`, `Half`, `ThreeQuarters`, and `Full`. `coder_terminal::Intensity` re-exports the same type; the existing colors, class names, digits, and near-black constants are preserved. Terminal capability mapping stays in `coder-terminal::Ladder`. |

## Coder and execution

| Term | Status | Definition |
| --- | --- | --- |
| Coder | Implemented | OpenAgents' coding specialization and the `crates/coder` agent. Interactive and headless use share `coder::turn::run`, with typed routing, generation, and host-controlled execution. The broader product suite has additional planned surfaces. See [Coder](coder/README.md). |
| Coder Terminal | Implemented | Coder's terminal interface. `crates/coder-terminal` supplies the terminal design system and renderer and re-exports the Coder amber palette from `coder-ui`; the `coder` binary supplies the conversation and runtime. Shared palette ownership does not mean the terminal has migrated to semantic view rendering. See [terminal behavior](coder/runtime/terminal.md). |
| Coder One | Implemented | The standalone agent and reusable controller in `crates/coder-one`: Jev judgments, evidence preparation, policy-selected executors, checks, and retained episodes. Its original issue-to-PR loop and experimental benchmark compositions coexist. See [the implementation](../crates/coder-one/src/lib.rs) and [tunable composition](coder/guides/coder-one-tunable.md). |
| Microluna | Implemented | The small generative-session harness in `crates/microluna`, with five native tools, supervised commands, workspace boundaries, and ATIF recording. Its Codex-login transport reads credentials without refreshing them. Coder's delegate door still uses it. See [Microluna](coder/design/microluna.md) and [the delegate door](coder/runtime/delegate-door.md). |
| Luna pivot | Defined | The development direction that moved Coder One toward small, short Luna executor sessions surrounded by typed decisions, explicit state, and checks. It is an architectural strategy to evaluate, not evidence of an automatic quality or cost advantage. See [the Luna pivot](coder/design/luna-pivot.md). |
| Microcoder | Implemented | The newer loop in `crates/microcoder`: rebuild state, retrieve knowledge, ask Jev, make one structured generation through the Codex login by default (or explicitly selected OpenRouter), and execute its commands. Its benchmark CLI runs and grades local Terminal-Bench tasks. Its separate [repository adapter](coder/runtime/microcoder-repository.md) uses the common owner; that adapter’s stopped live acceptance remains unverified. It does not replace every existing Coder or Microluna integration. See [Microcoder](coder/guides/microcoder.md). |
| Headless mode | Implemented | `coder -p` runs one turn without the terminal UI, optionally streaming structured events and recording an explicitly named trace. It uses the same turn and permission decisions as interactive Coder. See [headless mode](coder/guides/headless.md). |
| Delegate door | Implemented | The Coder response path that prepares a Jev-selected briefing and hands the turn to Microluna or an available CLI executor. Door selection is host logic, separate from a program's `delegate` step. See [the delegate door](coder/runtime/delegate-door.md). |
| Issue flow | Implemented | Coder One's issue-specific path: resolve the requested issue, create an isolated clone, work under a selected policy, review and check the result, then prepare a draft PR when the host's conditions permit. See [issue turns](coder/runtime/delegate-door.md#a-turn-that-works-an-issue). |
| Shell round | Implemented | One iteration of the ordinary Coder shell loop: validate a generated plan, run permitted commands, retain outcomes, and judge the next step. This is distinct from a Microluna session or Microcoder step. See [the shell loop](coder/runtime/shell-loop.md). |
| Plan | Implemented | In Coder's ordinary shell loop, a complete JSON reply under the supported plan schema, containing commands and their reasons. Quoting plan-shaped text does not make a reply executable. See [the shell loop](coder/runtime/shell-loop.md). |
| Permit | Implemented | The host's execution decision for one turn, derived before generation from the route and operator settings. It can narrow but cannot be widened by a model response. See [execution intent](coder/runtime/shell-loop.md#execution-intent). |
| Execution boundary | Implemented | The `coder-boundary` filesystem write policy, enforced with `sandbox-exec` on macOS or `bwrap` on Linux. A write boundary does not, by itself, restrict network access or every filesystem read. See [boundary verification](coder/verification/2026-09-20-execution-boundary.md). |
| Supervised job (job) | Implemented | One subprocess job in `crates/supervise`, with process-group ownership, deadline and cancellation handling, and bounded output capture. Job completion includes cleanup of the supervised child. See [subprocesses](coder/runtime/subprocesses.md). |
| Policy manifest | Implemented | Coder One's digested `openagents.coder-one.policy.v1` configuration: component choices, executor settings, and protected constraints. Human labels do not identify a configuration; the policy digest does. See [the manifest contract](../crates/coder-one/src/policy.rs). |
| Probe battery / Jev probes / Jevprobe | Implemented | Host-selected read-only operations that gather task and workspace evidence before execution; Jev can select useful outputs and files for the briefing. A probe observation is different from a capability-presence probe. See [evidence components](coder/guides/coder-one-components.md) and [the Jev-probe arms](terminal-bench/runbook.md). |
| Briefing | Implemented | The bounded input Coder One builds from the task, selected observations, requirement coverage, and any handoff state for an executor. A useful briefing is measured by delivered evidence and task outcomes, not its length alone. See [evidence packing](coder/guides/coder-one-components.md). |
| Requirement map | Implemented | Coder One's classification of instruction spans as deliverables, behaviors, constraints, checks, or context. It links later evidence to the task's words; extraction does not prove that every requirement was captured or satisfied. See [task components](coder/guides/coder-one-components.md). |
| Requirements loop | Implemented | Coder One's Microluna controller that works through requirement groups in short sessions, rebuilds context, and chooses subsequent work from judgments and checks. It remains the terminal and issue-flow default. See [Microluna turns](coder/runtime/delegate-door.md#a-microluna-turn). |
| Lean loop | Implemented | An alternative Microluna controller using short work sessions, a frozen evaluation script, retained candidates, and review. Policy settings determine its safeguards; it is not the issue-flow default. See [policy selection](coder/runtime/delegate-door.md#which-manifest-a-microluna-turn-runs). |
| Keep-best | Implemented | Retain an earlier workspace when the configured score or check rule prefers it to a later candidate. “Best” means best under that rule, not independently correct; copy bounds and missing snapshots limit the safeguard. See [tunable composition](coder/guides/coder-one-tunable.md). |
| Finish claim | Implemented | A model's typed request to end work. Host rules can reject or qualify it, but accepting the request does not itself establish task success. Microluna can explicitly record an unverified finish. See [Microluna's finish rule](../crates/microluna/src/finish.rs) and [Microcoder](coder/guides/microcoder.md). |
| Files in view | Implemented | Microcoder's bounded set of selected file contents, reread after commands and included in the next freshly built prompt. This is current loop state, not a persistent shared evidence store. See [Microcoder state](../crates/microcoder/src/state.rs). |

## Traces and retained evidence

| Term | Status | Definition |
| --- | --- | --- |
| ATIF | Implemented | Agent Trajectory Interchange Format, version `ATIF-v1.7`, implemented in `crates/atif`. It represents a session as ordered steps with calls, observations, and metrics; decision calls have structured metadata. See [traces](coder/runtime/traces.md). |
| Trace / trajectory | Implemented | An ordered record of an agent session. Coder writes ATIF logs; native executor streams and Microcoder event logs have different formats and capture limits. A readable trace is not necessarily complete. See [Coder traces](coder/runtime/traces.md) and [Microcoder records](coder/guides/microcoder.md). |
| Session | Implemented | A recorded interaction with a particular runtime or executor. A Coder trace session is one terminal/headless invocation; one Coder One episode can contain several executor sessions. Session IDs must retain their source meaning; the designed [engine session](#durable-work-and-client-contracts) is a separate portable contract. See [trace sessions](coder/runtime/traces.md#a-session-is-one-terminal-invocation) and [run cards](gym/run-card.md). |
| Decision call | Implemented | A recorded System One request: the state, questions, door identity, typed answers, and consuming route, carried as `openagents.decision-call.v1` metadata in ATIF. It records a judgment, not independent proof of its correctness. See [recorded steps](coder/runtime/traces.md#what-is-recorded). |
| Component invocation | Implemented | One Coder One component execution, identified by component, implementation and input digests, parent invocation, evidence revision, timing, outcome, and cost. A start without an end leaves the result unknown. See [the episode bundle](coder/terminal-bench-contract.md#the-episode-bundle). |
| Native stream | Implemented | The executor's original machine-readable event stream, retained alongside normalized episode records where available. Record and file caps can leave explicit gaps; normalization cannot recreate missing source events. See [executor control](coder/guides/coder-one-minitasks.md#the-capability-matrix). |
| Retained evidence | Defined | Saved observations and artifacts sufficient to inspect a stated claim: inputs, commands, outputs, candidate bytes, checks, identities, and capture limits. A summary or digest alone cannot replace missing source content. See [retained files](gym/retained-files.md) and [the episode contract](coder/terminal-bench-contract.md). |
| Complete trace | Implemented | For Coder's ATIF evidence reader, a log with a valid lifecycle, an end record, and no reader faults. The recovery reader can display an interrupted prefix; `read_whole` refuses it as complete evidence. See [trace recovery](coder/runtime/traces.md#steps-are-appended-the-document-is-rendered-on-read). |
| Capture limit | Implemented | A bound applied while collecting output or records. Trace storage preserves what the collector retained, not bytes already discarded. Microcoder retains command-output heads and tails; Coder's shell also has an upstream capture cap. See [trace limits](coder/runtime/traces.md#nothing-in-the-trace-is-capped) and [Microcoder state](../crates/microcoder/src/state.rs). |
| Repository source reference | Implemented | An observed path, line, file-content digest, excerpt digest, and coverage metadata attached to Coder's repository context. It identifies the bytes read, including uncommitted changes, without claiming an atomic cross-file snapshot. See [repository evidence](coder/runtime/repository-evidence.md). |
| Candidate workspace | Implemented | The actual file state produced by an executor or selected by its controller. Checks and grades belong to that state; an earlier writer's report or later review cannot stand in for it without verified attribution. See [candidate attribution](terminal-bench/2026-09-25-truthful-checks-microluna.md). |
| Retained file version | Implemented | A saved read, write request, patch, candidate snapshot, or final artifact export displayed by Gym. The viewer labels provenance and time limits: a write request does not prove success, and a final snapshot is not earlier historical state. See [retained files](gym/retained-files.md). |
| Artifact verification | Implemented | Host-prepared checks against an inspected, pinned artifact, with output and unchanged-workspace evidence retained separately from the delegate's report. A passed verification workflow does not authorize integration. See [artifact verification](coder/guides/artifact-verification.md). |
| Integration acceptance | Partial | The operator's decision to adopt a verified artifact, distinct from execution completion and checks. Existing project workflows retain these states separately; broader automated completion and presentation remain incomplete. See [project supervision](coder/guides/project-supervision.md) and [verification limits](coder/guides/artifact-verification.md#enforcement-and-limits). |

## Context and working state

These are target concepts in the [TypeSafe analysis](coder/design/typesafe-agent-analysis.md), [suite plan](coder/design/typesafe-product-suite.md), and [NIP-CTX](../nips/openagents/NIP-CTX.md). Existing traces and local claim ledgers do not constitute the complete shared context system.

| Term | Status | Definition |
| --- | --- | --- |
| Evidence item | Designed | An addressable observation or derived artifact with source, version, scope, capture limits, and provenance. Summaries retain references to their sources. |
| Evidence snapshot | Designed | An immutable set of evidence versions read by a task or decision. Sharing it does not assume that the current working tree or external service remains unchanged. |
| Evidence representation | Designed | A raw, abbreviated, summarized, or structured view of an evidence item, with its own identity and a path back to the source. A shorter representation retains the original capture’s completeness limits. |
| Task frame | Designed | The current objective, binding constraints, acceptance references, attempted approaches, and unresolved questions. It distinguishes observed user instructions from inferred subgoals. |
| Context request | Designed | A request for evidence tailored to one operation or recipient, under task, input-size, freshness, and disclosure constraints. |
| Context manifest | Designed | The exact evidence and representation versions included in one input, plus omissions, unresolved coverage, and selection identities. It records what was supplied, not whether a provider cached it. |
| Meta-attention | Designed | A separate selection process that decides which evidence and representation an operation needs. Its value depends on preserved constraints, sufficient evidence, and complete-task cost, not tokens removed alone. |
| Retrieval recall and selection recall | Defined | Retrieval recall asks whether candidate search found the needed evidence; selection recall asks whether the later selector retained it. Ranking cannot recover evidence missing from the candidate set. |
| History index | Designed | A scoped task/topic index with references from summaries to original records and bounded expansion. The plan makes no guarantee of logarithmic retrieval of semantically useful evidence. |
| Recursive language model (RLM) approach | Designed | Keeping source material and typed intermediate values outside a single prompt, with bounded investigation and expansion. Generated context-processing code still needs host admission and resource limits. |
| Background view | Designed | An optional explanation or finding derived from a pinned snapshot under a separate resource allowance. It becomes stale when relevant sources change. |
| Mandatory instructions and optional guidance | Defined | Binding user, host, and scoped repository rules are resolved by precedence and scope. Relevance selection may choose optional guidance; it cannot discard applicable mandatory instructions. |
| Cache-aware routing | Designed | Choosing reuse, trimming, rebuilding, or a model change using actual cached/uncached input observations and expected remaining work. Selection, summaries, expansion, failed detours, and cache rebuilding all contribute to the cost. |
| Learned preference | Designed | Proposed optional guidance derived from authorized evidence, with owner, lineage, scope, counterevidence, expiry, and separate permission to learn from the sources. A current task's read permission does not authorize private-history mining. See [POL learned preferences](../nips/openagents/NIP-POL.md#learned-preferences-and-governed-activation). |
| Preference activation | Designed | An owner-admitted controller's revision-checked decision to activate, replace, suspend, reject, or withdraw an exact candidate. Only one revision per lineage is active, and application rechecks current scope and expiry. It cannot grant tools, spend, publication, or approval authority. See [POL activation](../nips/openagents/NIP-POL.md#learned-preferences-and-governed-activation). |

## Capabilities and programs

Sources: [program guide](programs.md), [runtime](../crates/coder/src/runtime.rs), [composition](../crates/coder/src/child.rs), and [capability trust](../crates/capability/src/trust.rs). Portable specifications can be ahead of local readers.

| Term | Status | Definition |
| --- | --- | --- |
| Capability | Partial | A portable description of operations, interfaces, required effects, and enforcement support. A host binding implements it; a separate grant authorizes use. Local manifests and probes exist, while the revised [NIP-CAP](../nips/openagents/NIP-CAP.md) runtime remains partial. |
| Executor | Implemented | The built-in runner or approved external-agent binding that performs a bounded task. Its supported effects and limits determine admission; [delegation](coder/runtime/delegate.md) records its actual outcome. |
| Program | Partial | A reusable host-interpreted workflow of named steps with separate sources, question sets, and bounds. The local runtime supports six step kinds and child composition; native `invoke` admission and fetching programs from relays remain unbuilt. |
| Program selection | Implemented | The `openagents.program.v1` decision proposes which admissible workflow a request asks for, or `none`. Selection does not install a plugin or grant execution authority. See [the target entry path](extensions/programs.md#what-the-program-decision-decides). |
| Step kind | Partial | What one step does: `query`, `check`, `decide`, `delegate`, `program`, `module`, or `invoke`. The runtime in `crates/coder/src/runtime.rs` runs the first six. It parses `invoke`, which the revised NIP-PRG adds for native and adapted operations, and refuses it as `not_admitted` because no host operation is admitted. Unsupported semantics refuse rather than being skipped. |
| Bounds | Partial | Declared ceilings on work, time, resources, or access. The runtime checks supported enforcement before dispatch and narrows child limits; unsupported required bounds refuse instead of becoming prompt suggestions. |
| Module | Partial | A Wasm guest invoked by a `module` step under `pure` or `snapshot-read` limits. Local bindings carry inline `bytes_base64`; Coder One’s [evidence-guest host](../crates/coder-one/src/guests.rs) verifies the declared byte pin, while the generic runtime lacks complete portable target/binding validation. Remote fetching remains unbuilt. |
| Module announcement | Designed | An optional `30183` event saying where a module's bytes can be found and what it requires. A locator, not an authority: it cannot change what a program runs, because the program names a hash. |
| Capability manifest | Implemented | A document that says how to drive an executor: transport, detection, bounds it enforces, bounds it ignores, whether it sees the repository, and who pays. Read from `capabilities/` today and published as Nostr `kind:30180` later. |
| Capability probe | Implemented | Running a manifest's `detect` through `crates/capability` under a recorded approval that pins the manifest and executable identity. The probe is bounded; reading a registry does not execute it. |
| Presence | Implemented | Capability status: **present**, **absent**, **present and unavailable**, **unprobed**, or **unknown**. A missing approval leaves a capability unprobed; a failed or incomplete probe cannot manufacture availability. |
| Program registry | Implemented | The programs a host resolved, read from `programs/` in `crates/coder`'s `program` module. The read records the Nostr filter it would have sent beside the answer it got from disk. |
| Run-state store | Partial | The append-only [local journal](../crates/coder/src/runstate.rs) for pinned run, step, and task-attempt identities, worktree/result references, and terminal outcomes. The [runtime](../crates/coder/src/runtime.rs) records into it when configured and consults recovery/reconciliation rules; this is not complete cross-process or NIP-RUN resume. |
| Task source | Implemented | Where a `query` step's work comes from, named in the program by slug and resolved from `sources/` in `crates/coder`'s `source` module. A program names a source and never a command, so a machine decides what the lookup reads. `request`, the work the request carried, is built in. |
| Selection | Implemented | What one `query` step looked up: the work that runs, the order it is in, what was dropped and why, and every path more than one selected item touches. A lookup answering with more than `max_results` truncates or refuses, as the step's `on_overflow` says, and the trace records which. |
| `cannot_enforce` | Implemented | A capability manifest’s explicit list of bounds its executor cannot enforce. Runtime admission refuses a required bound intersecting that list; the field reports a limitation rather than granting an exception. |
| Operator policy | Designed | The portable signed NIP-CAP `30181` document stating capability preferences, concurrency limits, and exclusions. Existing local program grants and executor settings implement narrower host controls; they are not this complete portable policy runtime. |
| Program grant | Implemented | Operator authorization for named program slugs and an effect ceiling. The [grant](coder/guides/program-authority.md) is separate from semantic program selection and from the ordinary shell permit. |
| Child program | Implemented | A resolved `program` step whose dependencies, typed bindings, outcome propagation, and narrowing limits pass [composition admission](../crates/coder/src/child.rs). Children share the parent’s budget and deadline. |
| Host binding | Partial | Local configuration connecting a portable definition to real paths, adapters, executors, or credentials. Current program and capability bindings exist; the full revised portable contract remains under migration. |
| Local package lock | Implemented | The verified digests and dependency identities resolved by [`coder::package`](../crates/coder/src/package.rs). Resolution pins local bytes and records trust; it does not install, execute, approve a probe, or grant access. |

## Plugins and skills

Unless qualified, *plugin* means a Wasm guest and *skill* means a `SKILL.md` guide. Voyager uses *skill* for banked Lua code. Sources: [plugin contract and implementation status](extensions/plugins.md), [client packages](../plugins/README.md), and [Voyager](voyager/README.md).

| Term | Status | Definition |
| --- | --- | --- |
| Wasm plugin | Partial | The default meaning of *plugin*: an OpenAgents WebAssembly guest that `crates/plugin` runs, with typed operations and bounded host imports. It can implement a program's `module` step or, once host roles exist, a supported host role; it is not an executor or a workflow. The host core and the `module` step are built; the manifest, host roles, authoring commands, packaging, and a catalog are specified in [Wasm plugins](extensions/plugins.md) and not built. |
| Plugin host | Partial | The trusted Rust boundary in `crates/plugin`, using a fresh Wasmtime instance per call with no ambient WASI. It enforces profiles, fuel, memory, read/output limits, and cancellation. Full plugin-manifest admission, automatic host roles, and invocation-receipt integration remain designed. |
| Plugin packet | Implemented | The `openagents.plugin-packet.v1` request and response a host and a guest exchange, defined in `crates/plugin-pdk`, whose `guest` feature is the guest side of the ABI. `crates/plugin-outline` is a diagnostic guest built against it. |
| Evidence guest | Partial | A `snapshot-read` Wasm plugin that code runs as a program step to gather evidence, never offered to a model as a tool. Three are built, `crates/plugin-repo-map`, `crates/plugin-code-search`, and `crates/plugin-test-report`, and `programs/evidence-guests.json` runs them. Coder One's probe stage runs that program when a manifest turns it on; each guest stays only if it helps on the measured evaluation set. See [Evidence guests](extensions/plugins.md#evidence-guests). |
| Client plugin package | Implemented | A declarative package that teaches Claude Code or Codex to call the decision API: a client manifest (`.claude-plugin/plugin.json` or `.codex-plugin/plugin.json`), an `.mcp.json` that starts `oak-mcp`, and a `SKILL.md` skill. The packages live under [`plugins/`](../plugins/README.md), and `crates/discovery` serves their manifests. They run no Wasm and are not Wasm plugins. |
| Extism plugin | Retired | A module run by Extism in the earlier plugin marketplace and agent store. That runtime is absent from this repository; the current Wasmtime guest host replaces its role. See the [legacy map](roadmap.md#the-legacy-map). |
| `SKILL.md` skill | Implemented | The default meaning of *skill*: a Markdown guide in the Agent Skills format, with `name` and `description` frontmatter, that an agent reads before a task. It grants no authority and runs no code. Skills live in three places: [`.agents/skills/`](../.agents/skills/) holds the guides for agents working in this repository; [`plugins/skills/`](../plugins/skills/) and each client plugin package's `skills/` directory ship the decision API guide to clients; and the gateway's [skill directory](decision-models/service/skill-directory.md), `tenancy::skills`, publishes reviewed, versioned submissions. |
| Voyager skill | Implemented | A digested, versioned Lua program that `crates/voyager` banks after a critic passes it, and retrieves for later tasks. It is code the bounded Lua interpreter runs, not a `SKILL.md` guide. See the [skill library](#voyager) and [Voyager](voyager/README.md#the-critic-and-the-skill-library). |
| Pure profile | Implemented | The Wasm guest profile with no host imports or ambient filesystem, process, network, clock, or credential access. It transforms the supplied packet under host limits. |
| SnapshotRead profile | Implemented | The guest profile allowing bounded listing and reading through invocation-scoped snapshot handles. Stale handles, traversal, and symlink access are refused. Retained partial bytes remain readable; the [derivative helper](../crates/plugin/src/snapshot.rs) refuses a false completeness claim. |
| Plugin development kit (PDK) | Implemented | The shared packet types and optional guest-side ABI in [`crates/plugin-pdk`](../crates/plugin-pdk/), including allocation, handler exports, and typed host calls. |
| Plugin build receipt | Implemented | A record binding guest bytes, the PDK source digest, and the guest profile. Checked-in evidence-guest receipts add build provenance. This is byte attribution, not remote attestation or proof of task benefit. |
| Snapshot handle | Implemented | An opaque reference minted for one plugin invocation’s admitted snapshot objects. A guest cannot reuse it in another invocation or treat a path label as unrestricted file access. |
| Fuel | Implemented | The Wasmtime instruction allowance charged to a guest, including its start function. Fuel is separate from wall-time, memory, output, and read limits. |

## Extensions

Sources: [extension architecture](extensions/architecture.md), [packages and distribution](extensions/packages.md), and [local package implementation](../crates/coder/src/package.rs). Discovery, installation, enablement, grants, invocation admission, and promotion are separate decisions.

| Term | Status | Definition |
| --- | --- | --- |
| Operation descriptor | Designed | A digested discovery interface containing identity, purpose, typed schemas, execution binding, preconditions, effects, resources, and evidence references. It describes a component without replacing its execution contract. |
| Progressive discovery | Designed | Bounded retrieval and eligibility filtering followed, when useful, by semantic selection and loading of only the selected schemas or guidance. Discovery is inert and grants no authority. |
| Decision function | Partial | A typed semantic input/question/output contract with a consuming policy, limits, and admitted model scope. Coder has existing decision sites and question sets; the unified extensible function registry remains proposed. |

# Capabilities, executors, and programs

How a Coder instance learns what it can hand work to, how the decision
engine chooses, and how a **program** defines a reusable workflow.

The [program and extension specification](extensions/README.md) defines the
target integration of programs, Wasm plugins, skills, operation discovery,
and packages. This guide retains the original rationale and operational
history; the new specification distinguishes implemented behavior from
proposed composition and distribution.

The worked case throughout is delegating to the Devin CLI, because it is the
one an operator here actually has and wants used. It is a row in a table,
not a special case, and the last section says what generalizes.

## Current delivery direction

The [TypeSafe-native Coder roadmap](coder/design/typesafe-agent-roadmap.md) places
these programs over shared evidence and task-specific context. The earlier
reference-design discussion below explains the concepts; it is not a claim
that every proposed capability or composition rule is implemented here.
Use [the consumer inventory](coder/design/coder-as-decision-router-consumer.md)
for current scope.

Explicit [program grants](coder/guides/program-authority.md),
[scoped tracker intake](coder/guides/tracker-intake.md),
[project supervision](coder/guides/project-supervision.md), and a bounded typed
[`run-suite` host path](coder/guides/artifact-verification.md) now exist. Full
program recovery, typed composition, portable package resolution, and the
proposed evidence/context store remain delivery work. The new roadmap
extends these components; it does not introduce a separate private program
runtime.

## The word

Use **program** for a workflow and **plugin** for a bounded Wasm guest. A
Claude Code or Codex package under `plugins/` is a client plugin package, a
different thing; the [glossary](glossary.md#plugins-and-skills) separates the
meanings. Call the combined product surface **programs and extensions**. Selecting a program
answers which workflow the request asks for; it does not select a package to
install or grant permission to execute it.

The glossary in the reference implementation already allocates the terms,
and four of them matter:

| Term | What it already means there |
| --- | --- |
| **Capability** | A specifically granted ability — a tool, a readable directory. A capability a grant file does not declare is offered to no run. |
| **Executor** | The implementation that performs an agent session, including the built-in runner or an external agent adapter. `coder-devin` is listed among the executor adapters. |
| **Program (Jev)** | A state machine in code with named steps and per-step bounds, itself a signature. **The decision engine picks the program at run start** and the next signature inside a step. |
| **Plugin** | A sandboxed WebAssembly guest, with an authoring CLI, a package, an install, and a catalog. Explicitly distinct from a skill. |

So the words already exist and already fit:

- **Devin is an executor**, reached through an executor adapter. That term
  names it exactly, and `coder-devin` is already in the list.
- **The reusable, composable unit is a program.** That term was already
  claimed for the decision engine, which is precisely where this belongs.
- **A plugin is a Wasm guest**, and a Devin delegation cannot be one.

That last point is structural rather than a preference. The reference
design's plugin host sorts guests into three tiers: pure compute loads
without asking, a guest wanting read-only directories needs an operator, and
a guest declaring network access **never loads**. A Devin delegation spawns a process, reaches
the internet, writes files, and is not deterministic. It is the tier that
never loads. The host here, `crates/plugin`, implements only the first two
tiers, as its `Pure` and `SnapshotRead` profiles, and has no network
profile to load.

The
[capability-sockets review](decision-models/research/2026-09-19-capability-sockets.md)
found the same boundary for trained adapters: the manifest contract can
generalize beyond Wasm, while the sandbox applies to eligible guests.
Retain the bounded guest host for those operations and keep effectful
executors behind native adapters. A common operation descriptor supports
discovery without merging their execution contracts.

**A program is the workflow composition unit.** The target composition
contract connects typed step outputs to inputs under narrowed bounds. Two
automatic plugins eligible to replace the same call's output need explicit
host ownership so completion order cannot choose the result. The runtime
runs `module` steps; plugins can also compose through declared program
dataflow once typed bindings between steps are implemented. See
[Programs and decisions](extensions/programs.md) and [plugin host roles](extensions/plugins.md#host-roles).

## The three layers

**A capability manifest** says how to drive an executor: transport, how to
detect it, which bounds it enforces, **which bounds it will silently
ignore**, whether it can see the repository, and who pays.

**An operator policy** says what this operator wants: which capabilities to
prefer, how wide a fan-out may go, and what is never to be used.

**A program** says what to do: named steps, each with bounds, each either a
query, a decision, a deterministic check, or a delegation.

They are addressable Nostr events in two NIPs, split on purpose.
[NIP-PRG](../nips/openagents/NIP-PRG.md) defines programs (`30182`) and
[NIP-CAP](../nips/openagents/NIP-CAP.md) defines capabilities (`30180`,
`30181`). The relay is the workspace, which is the pattern `AGENTS.md` names
for application behaviour: event kinds and relay policy rather than a
private backend.

**A program is the general primitive and does not belong to Coder.** It says
nothing about an agent, an executor, a model, or a product — only what the
steps are and what bounds them. Filing it with capabilities implied it was
part of one product's surface, and it is not. Capabilities are specific to a
machine; programs are specific to nothing.

That split buys three properties a combined document could not state
cleanly: programs compose by reference under bounds that **narrow and never
widen**, a host **refuses** a program whose step kinds it does not recognize
rather than skipping them, and cycles are refused outright rather than
bounded by depth. Those are rules about programs, not about what a program
happens to reach.

**Local presence is not an event.** Whether `devin` is on *this* computer is
found by running the manifest's `detect` and stays on the machine.
Publishing it would broadcast an inventory of somebody's computer, and no
party here needs it.

## The local registry

Manifests and programs are files before they are events. The repository
carries both, and `crates/coder` reads them:

| Directory | What it holds |
| --- | --- |
| `capabilities/` | One `kind:30180` manifest per file. `devin-local` is the first; `coder-one-ask` is Coder One's read-only ask mode, and `gym` is the Gym's read-only binary for a command source. |
| `programs/` | One `kind:30182` program per file: `delegate-fan-out`, `burn-down`, `review-changes`, `answer-question`, `run-suite`, `review-runs`, `evidence-guests`. |
| `questions/` | One question set per file, addressed by identifier: `openagents.program.v1`, `openagents.independence.v1`, `openagents.independence.v2`, `openagents.completion.v1`. |
| `sources/` | One task source per file. `work-list` reads a file, `gym-runs` runs a command; `request` is built in. |

A host reads `CODER_CAPABILITY_DIR` first, then the repository's directory,
then `~/.openagents/capabilities`, and the same three for programs, for
questions, and for sources. The first definition of a slug wins, so an
operator overrides a checkout without editing it.

### A `decide` step names a question, and the wording lives elsewhere

A program keeps decision-question wording outside its steps. A `decide`
step names an identifier such as
`openagents.independence.v2`; the text behind it is a file in `questions/`,
digested as a whole, and the digest is recorded beside every answer.

Rewording a question changes what was asked. A program that inlined its
wording could not say which version produced a result, so `Program::load`
refuses a `decide` step carrying `instructions`, `criteria`, `questions`,
`text`, or `prompt`, and `Runtime::admit` refuses one whose identifier this
host has no wording for.

Some existing delegation programs have a `briefing` field for execution
guidance. That legacy field is not decision-question wording or authority.
The [portable program contract](extensions/programs.md#program-definitions-and-bindings)
separates guidance assets and requires an explicit migration rather than
changing the identity of historical programs.

A set fills in exactly two things at run time, and both are bounded fields
chosen after the route was:

- A Choice question declaring `"options": "supplied"` takes its options
  from the run, beside any it declares itself. The program-selection
  question's options are the programs this host would admit, which is how
  an operator without an executor gets a shorter option set rather than a
  broken one, plus the `none` the file declares. An option whose wording is
  the same on every host belongs in the set, where the digest covers it;
  only the slugs and summaries come from the run.
- A set declaring `per_requirement` is a template. The host asks it once
  per requirement and writes the requirement's name into the instructions,
  because a set of identical questions under different identifiers gives a
  model nothing to tell them apart with.

A question, or a template, may also carry a `decision` block: how its
answer becomes a decision, kept apart from what it asks. The block takes
`threshold` for a Noul (a probability at or above it reads as yes),
`cuts` for a Score (ascending boundaries that turn the
probability-weighted mean into a level), and `weights` for a Choice (the
decision is the option with the largest probability times weight):

```json
"per_requirement": {
  "type": "noul",
  "instructions": "The requirement the state lists under {requirement} landed.",
  "decision": { "threshold": 0.75 }
}
```

Every setting is optional. Without one, a Noul reads as yes at 0.5, a
Score is the level the model selected, and a Choice is the option the
model picked, which is what a host did before settings existed. The block
is never sent and is outside the set's digest, so changing a setting
leaves every request and every recorded answer unchanged. The host
records the settings' own digest as `decision_digest` beside the set's
digest, only when the set carries a block. Choice weights apply where an
answer is used and never change a calibration map, which never overrides
the model's pick. The Gym's question sets in `crates/gym/questions/` take
the same block, and Coder One's policy manifest records the digest of the
settings it reads Jev's answers under as `policy.jev.decision` when any
differs from its default.

The program registry read records the Nostr filter it would have sent
beside the answer it got from disk, because the query is the part that has
to keep working when the answer does not. Publishing to the relay changes
where the answer comes from and not what was asked.

### A `query` step names a source, and the command lives elsewhere

The same rule, one step over, and it is the rule that decides whether a
program can find work at all. A `query` step carries a **source slug** such
as `request` or `work-list`. What that slug reads is a file in `sources/`,
and a host that cannot resolve the slug refuses the step rather than falling
back to whatever work was handed in.

"Run `gh issue list`" is not a step kind. A program that carried a command
would be code, and a program that carries none is the one property
everything else rests on — it is what makes a program safe to read from a
stranger. So the program says *which* lookup, the machine says *what* the
lookup is, and the two can differ between machines running the same program
the way a `delegate` step's executor already does.

A source declares where its answer comes from and the order it is in:

```jsonc
{
  "v": 1,
  "slug": "work-list",
  "name": "The work list this checkout carries",
  "summary": "Reads an ordered work list from .coder/work-list.json, by identifier.",
  "from": {"file": {"path": ".coder/work-list.json"}},
  "order": "id"
}
```

`request` is the work the request carried, built in because its meaning
cannot be anything else, and a file source reads a work list under the
workspace. Neither runs a process.

A `command` source runs one, and only through the trust boundary and the
subprocess bounds
[#9427](https://github.com/OpenAgentsInc/openagents/issues/9427) is about. It
names a **capability**, never a binary, so the program it runs is the one
that capability's approval pins: `capability-trust approve gym` records the
manifest's digest and the binary's canonical path and content, and a
capability that isn't present refuses the step with `source_unavailable`.
The command runs in the workspace through the supervisor, bounded at 60
seconds and the probe's 64 KiB output cap, and its output must declare the
schema the source names. A lookup that executed an argv because it had read
a file is still the finding rather than the fix; the approval is what makes
this one different. `gym-runs` is the first:

```jsonc
{
  "v": 1,
  "slug": "gym-runs",
  "from": {"command": {"capability": "gym",
                       "args": ["runs", "--order", "learning", "--json", "--limit", "5"],
                       "schema": "openagents.gym.runs.v1"}},
  "order": "given"
}
```

This host reads one command schema, `openagents.gym.runs.v1`, and it becomes
one work item: the operator's request, with the Gym's totals and the runs it
listed as context. A question about runs is one question, so it's one
delegation, never one per run. A `query` step whose source is a command
declares the `subprocesses` effect as well as `reads`. An operator who wants
the open issues still writes them to a work list with one command of their
own.

A work list names what each item touches, what it comes after, and what
answer it expects back:

```jsonc
{
  "v": 1,
  "work": [
    {"id": "9391", "prompt": "…", "reads": "crates/gym/src/digest.rs", "writes": true,
     "expects": "3"},
    {"id": "9401", "prompt": "…", "touches": ["crates/gym/src/gate.rs"], "after": ["9391"]}
  ]
}
```

`expects` is the item's stated answer, the way a CoderBench task's
`expects` entry states one, and it is what the `accept` step judges: a
delegation whose output matches it passes, one whose output does not
fails, and an item that states none is **unverifiable**. An unverifiable
item is never counted as passed, however plausible its output reads; the
run's summary counts the three apart (`1 passed, 1 failed, 1
unverifiable`) and the acceptance state carries each requirement's
`expects` and `verdict`. The first burn-down episode reported "0 of 0
correct" because its items stated nothing to judge
([#9413](https://github.com/OpenAgentsInc/openagents/issues/9413)).

The answer is what follows the last `Final answer:` in the delegate's
output, or the whole output when it carries none. The Devin CLI prints
every text block the agent emits with nothing between them, so a writing
task's stdout runs narration and answer together
(`…Verifying the scratch repo is clean.done`); the briefing asks the
delegate to end with a `Final answer:` line, the narration before it is
recorded under the call's `transcript`, and `accept` judges only the
answer. The second episode reported "0 passed, 2 failed" for two items
that were done, because the whole stream was judged against `done`
([#9451](https://github.com/OpenAgentsInc/openagents/issues/9451)).

### An explicit list is a source, not a shortcut

`request` is reached through the same code every other source is, and that
is deliberate. The first real burndown will run on work chosen by
inspection, and work chosen by inspection has to exercise the ordering, the
bound, and the collision record that a queried list depends on later. A
second code path for the easy case is a second code path nobody tests.

### What a lookup does, in order

1. **Order.** Whatever the source answered with, in the order the source
   declares — `given` or `id` — and the identifiers are recorded. A burndown
   that silently reorders is not reproducible.
2. **Declared order is enforced.** An item whose `after` names work still in
   the same list is dropped from this batch and recorded as dropped, because
   running the two at once is wrong by construction. #9391 has to land before
   #9401, and that is a fact in the list rather than a judgment about it.
3. **The bound.** More items than `max_results` either truncates or refuses,
   as `on_overflow` says, and the trace records which happened along with
   everything dropped and why.

`delegate-fan-out` refuses. It is the stricter of the two and it is the one
the backlog needs: the work is not independent, the gate that should catch
that is the one #9414 measured at eleven of twelve wrong answers above the
floor, and a lookup that quietly chose six of twenty-one would be making a
selection nobody reviewed.

### Collisions are computed, not asked about

Work items that touch the same file cannot run beside each other, and that
is discoverable without a model: it is in the list. The lookup records every
path more than one selected item touches, and puts the collisions in front
of the decision that follows.

It records them rather than refusing on them, and the asymmetry is the
measurement's.
[#9414](https://github.com/OpenAgentsInc/openagents/issues/9414) put plans
whose tasks genuinely collide to four doors: eleven of twelve answers
cleared the 0.7 bound on the local ones, `kev-8b` at 0.96 and 0.97, and the
wrong answers sat above the right ones, so raising the bound does not help.
Only hosted Jev held. A gate in that state is not where a computable fact
belongs, so the fact is computed and recorded whether or not the gate reads
it. Which pairs genuinely collide is still the gate's question, and fixing
the gate is #9414's work rather than the lookup's.

A plan whose tasks touch six different files says nothing about collisions,
so the state those measurements were taken against reads the way it did.

### Three states, not two

A probe answers **present**, **absent**, or **present and unavailable**.

Absence is not an error. The capability is not an option, which is the
whole reason an operator without Devin loses nothing — the option set for
the program-selection decision is built from what the probe found, so an
absent executor is a route nobody was offered rather than one that fails
when it is taken.

The third state is the one a present-or-absent probe cannot report, and it
happened before it was implemented. Six of six delegations in the
[`coderbench` golden](coderbench.md) were declined with `Refusing to run in
an untrusted workspace` from a git worktree under `/private/tmp`, while the
executor stayed installed and kept reporting its version. A host that reads
that as present offers a route that fails every time.

The manifest states it: `refuses` names what the executor declines while
installed, and `workspace_probe` is the argv a host runs in a candidate
directory to ask. The argv stops short of starting a session, because a
host asks this whenever it considers a directory and a probe that did the
work would charge the operator for a question.

### The manifest drives the executor

`invoke` is the argv that hands one task over, with the prompt appended
last, so a delegation runs the binary the probe resolved under the
arguments the manifest names rather than a name written into the source.
A capability that is absent, or refusing this workspace, produces no
executor — which is how it drops out of a fan-out instead of failing in
one.

### `PATH` is a hint, not the answer

The probe resolves an absolute path and runs that path. `devin` was on the
operator's interactive `PATH` and not on the one a spawned subshell
inherited, and six delegations failed with `command not found` before the
full path was resolved. So the search reads `PATH` for candidate
directories, then keeps looking through the directories a login shell
usually adds, and what it reports — and what a delegation later runs — is
always the resolved path.

## What the decision engine is actually for

This is the part that is easy to get wrong, and we have this week's
measurements saying exactly how.

### The thing not to build

**Do not offer capabilities to the model as tools it may elect to use.**

The reference implementation ran that experiment and published the null: a
model-called capability got **zero calls across 18 attempts on six task
shapes**, in runs making 15 to 99 calls to the general tool it already had,
while the declaration cost **2,307 extra bytes on every request of every
turn**. A second one got zero calls in six of six.

**And do not ask "is this a delegation?" every turn.** On the real-turn
suite, six of seven production questions score no better than answering with
a constant, because most turns are the same kind of turn
([#9395](https://github.com/OpenAgentsInc/openagents/issues/9395)). "Is this
a delegation request" would join them: nearly always no, and a question that
is nearly always the same answer is a latency cost with a false-positive
risk.

The turn asks one anyway, because nothing else reaches the runtime from an
operator's sentence, and it does join them — measured rather than assumed in
[`decision-models/2026-09-19-program-selection.md`](decision-models/measurements/2026-09-19-program-selection.md).
On 32 real turns the constant scores 0.969 and hosted Jev scores 0.938,
which is the warning above coming true. What the measurement adds is the
shape of the error: **no program request was missed**, three ordinary turns
in 35 were answered with a program, every one of those involved
`answer-question` rather than `delegate-fan-out`, and none of them ran
anything, because a program cannot fan out over work the request did not
name. Read that report before changing the question, the option set, or the
programs' summaries.

Nor is keyword matching available, and not only because `AGENTS.md` forbids
it for intent routing. The reference's own capability search ranks a query
against a name and description by bag-of-words overlap — the one place a
carefully built system reached for exactly that, at the one point nobody was
measuring.

### The call site is the operator's sentence

When someone says *delegate six instances, one for each of the top six
issues*, they have already made the decision. Nothing needs to infer intent.
The count is stated, "top six open issues" is a **structured query**, and the
route was chosen by a person.

What the host does not know, and what is genuinely worth a decision model,
is **whether that is safe and admissible**. Those are bounded, typed
questions, asked at fixed points in a program, with option sets built from
what is actually present.

### The four questions

**1. Are these six tasks independent?** The one that matters. Six agents on
six issues that touch the same files produce six conflicting branches and a
mess that costs more than it saved. This is a Noul per pair, it has real
variance on real inputs, and — the rare part — **it has a mechanical outcome
label**: did the branches conflict on merge. That makes it trainable and
gateable rather than a matter of taste.

**2. Does this task need a bound the executor cannot enforce?** The
manifest's `cannot_enforce` is the field this reads. An executor that
ignores a bound is more dangerous than one that refuses it. Partly
deterministic — does the task name a tool restriction — and deterministic
parsing is allowed once the semantic route is already chosen.

**3. Can this task be done without the local checkout?** A cloud lane never
sees the working directory. A Noul, with an outcome label: did the
delegation fail for missing context.

**4. Did the delegated work actually land?** One Noul per stated
requirement, refusing to accept while any is below threshold. This is the
completion gate the
[terminal-bench audit](gym/terminal-bench.md) argues for, and it is the same
question as *is the task complete* asked where it has variance and
consequence instead of where it is 39-to-1 constant.

**None of these is "should I delegate?"** The operator said to delegate. The
engine's job is admission and safety.

The one question that *is* asked before the operator's sentence is read as
work — which program, or none — is the one the turn needs to reach a program
at all, and it is measured with the same suspicion:
[the program-selection report](decision-models/measurements/2026-09-19-program-selection.md)
publishes its baseline and headroom before its accuracy, and counts the two
errors apart.

## The honest risk

On 38 real decisions, routing through a decision model **changed what the
agent would otherwise have done twice, and both changes were wrong.**

That is the whole reason to build the independence question first rather
than all four: it is the one with a free outcome label, so it can be
measured rather than believed. Before it gates anything it needs the
treatment everything else got this week — a baseline, a headroom check, a
noise floor, and a gate that can refuse it. A decision model that fires on
one fan-out in seven and is wrong each time is worse than no decision model,
and
[#9397](https://github.com/OpenAgentsInc/openagents/issues/9397) is a live
example of exactly that shape.

## The first program

`delegate-fan-out`, whose steps are in [NIP-PRG](../nips/openagents/NIP-PRG.md):

```text
select  (query)     the work a named source answers with, ordered and bounded
        ↓
independence (decide)  are these N tasks disjoint?      ← the measured one
        ↓
admit   (check)     does any task need a bound the executor cannot enforce?
        ↓
fan_out (delegate)  one session per task, bounded, isolated per worktree
        ↓
accept  (decide)    per requirement, did each one land?
```

Two steps are deterministic and two are decisions, which is the right ratio:
the program is mostly mechanism, and the decision model is asked only where
a judgment is genuinely required.

`coder::runtime` runs all six from the file, and
[`coder/delegate.md`](coder/runtime/delegate.md) covers the `fan_out` step in
detail: what it records, how it is bounded, and why a refusal, a timeout,
and a failure are three outcomes.

Bounds are enforced, not declared. `concurrent_max` bounds the fan-out,
`isolation: worktree` gives each session a checkout of its own so a
collision is recoverable, and the `minutes` bound reaches the executor only
after the admission check has established that somebody is holding the
delegation to it.

## The runtime

`coder::runtime` is the interpreter. It takes a program, the work, and the
capability slug that is to do the work, and runs the steps the program
lists in the order the program lists them. Four rules make it a runtime
rather than a loop over a list, and all four are NIP-PRG's.

**A step whose bounds the host cannot enforce does not run.** Not a warning
and not a substitution. `Runtime::admit` checks every step's bounds against
the host before the first step runs, so a program this host cannot hold to
fails before it has done anything. The host keeps a table of the bound keys
it enforces per step kind, and it checks the values as well as the keys: a
step naming `isolation: "vm"` is refused, because running it in a shared
directory instead is the substitution the rule forbids.

**A step kind the host does not run refuses the whole program.** An
unrecognized kind is refused when the file is read. A kind this version
recognizes and does not run — `invoke`, which names a host operation this
host has not admitted — is refused. Neither is skipped.

**A `decide` step names a question, never its wording**, and **a `query`
step names a source, never a command.** See the previous section.

**A refused step stops the program**, and the reason it stopped is what the
run reports. An answer below a `refuse_below` floor, a check that will not
admit the delegation, and an executor this machine cannot reach are all
refusals, and each one stops the rest.

### What a step does here

| Kind | What the runtime does |
| --- | --- |
| `query` | Resolves the source the step names, orders the answer, enforces the order the work declares, and holds it to `max_results` — truncating or refusing, as `on_overflow` says. A step naming no source reads the work the request carried, which for a turn is the list the operator's sentence writes out. |
| `decide` | Puts the named question set to a decision door and records `openagents.decision-call.v1`, with the set's identifier and digest beside the answer. |
| `check` | Runs the admission test the `refuse_on` bound names. |
| `delegate` | Hands the work to the executor the capability probe resolved, at the width, isolation, and wall bound the step states. A step with nothing to hand over refuses rather than reporting that none of nothing answered. |
| `program` | Runs the child program the step's address resolves to, nested inside the parent's run. Admission checks the whole composition first: cycles, depth, step and call totals, and bounds that would widen. |

# Rust verification

## Choose checks for the change

Documentation-only changes do not require the Rust verification gate, including
before a commit or push. Do not run the full gate for prose edits, documentation
moves, navigation changes, or documentation references in code comments.

For documentation reorganizations, check local links, referenced paths, and
preservation of retained artifacts. If moving a document requires updating an
embedded-document path such as `include_str!`, check only the affected consumer
and any relevant formatting. A documentation path update does not justify
workspace-wide Clippy, tests, compiler compatibility checks, or PostgreSQL
acceptance. Unrelated test failures do not block a documentation-only push.

## Development checks and release verification

Day-to-day issue work uses targeted checks on the pinned toolchain. Check the
changed behavior and the relevant consumers, then commit and push. A full
workspace run is not required for ordinary development, integration, issue
closure, or a push. Never stop independent issues to wait for release checks.
Fix a relevant failure; record an unrelated failure separately and keep moving.

```sh
./scripts/verify-rust.sh                         # changed-package fmt, Clippy, tests
./scripts/verify-rust.sh --crates coder,gym       # explicitly affected packages
./scripts/verify-rust.sh --phases tests --crates coder
cargo test -p coder task::                       # focused regression checks are valid
./scripts/verify-rust.sh --print                 # inspect the plan without running it
```

The default comparison is `origin/main`, including uncommitted changes. Use
`--changed=REF` to choose another base. Changes to a lockfile or workspace
configuration do **not** silently expand development checks to every package.
Select affected consumers with `--crates` and record any coverage deferred to
release. Select feature, PostgreSQL, or infrastructure phases only when relevant
to the change. Documentation-only work needs link and artifact checks instead.

Before a full release, explicitly request the full manual matrix:

```sh
./scripts/verify-rust.sh --release
```

This opt-in runs the standard workspace, feature, dependency, and PostgreSQL
checks. It is release preparation, never a prerequisite for taking the next
issue. Nothing runs automatically: there are no verification git hooks or
GitHub workflows. Use `--list` to see phases, `--keep-going` to collect independent
failures, and `--with-metal` or `--with-soak` for optional coverage. A previous
standard full run took 1003.7 seconds (about 17 minutes); actual time depends on
cache warmth, changed dependencies, and machine load. Targeted checks avoid
paying that cost for every issue.

The cancellation fixtures require a current Python runtime. On macOS,
Apple's system Python 3.9 fails their known-good runner; Python 3.13 passes
the same check. Put the installed Homebrew runtime first for the gate:

```sh
export PATH="/opt/homebrew/opt/python@3.13/libexec/bin:$PATH"
python3 --version
```

This selects the test runtime without changing the system interpreter.

`--changed` maps `crates/<name>/` paths to packages and maps the data
directories `coder` loads (`programs/`, `questions/`, `capabilities/`,
`sources/`) to it. In release mode only, workspace-wide files expand that
scope to the workspace. Feature flags narrow to selected packages. A change
with no affected crates scopes the Cargo phases out. Scoped runs record
`partial` coverage; that label does not mean their selected checks failed or
that development must wait for a full run.

Every run writes `.coder/verification/<run-id>/run.json` (override with
`--record-dir`, disable with `--no-record`): run ID, start and end UTC,
elapsed, the tree it covered (HEAD, dirty flag, diff digest), the phases
requested, each phase's command, exit, elapsed, attempts, and log path, the
skipped phases and why, and the result. A pass binds to that tree; it is not
a standing fact about "the gate." Reuse it only for the coverage it names.

The `preflight` phase runs first and fails fast on the environmental
prerequisites the later phases assume: a file-descriptor limit of at least
2048 (worktree fan-out tests exhaust less; the script first tries raising
the soft limit itself), `cargo`, `python3`, `git`, and `rustup` on PATH, and
free disk. Fix what it names and rerun; it does not skip or weaken a check.

When a phase fails and its log shows resource exhaustion — file-descriptor
pressure, address reuse, or `EAGAIN` — the gate retries it once and records
both attempts. `--no-retry` disables that. A retry triggered by a signature
is not a pass over a defect; the log names why it ran.

[`rust-toolchain.toml`](../rust-toolchain.toml) pins Rust, Clippy, and rustfmt
to **1.97.1**. [`rustfmt.toml`](../rustfmt.toml) pins Rust and formatter style
editions to **2024**. Run the pinned formatter once for formatting-only
changes; do not mix a workspace reformat with behavioral fixes.

## Package policy and compiler version

The workspace compiles on one compiler: **1.97.1**, the version
[`rust-toolchain.toml`](../rust-toolchain.toml) pins and the root manifest's
`rust-version` declares. Every package inherits edition 2024,
`publish = false`, the pinned Rust version, and the workspace Rust and
Clippy lints — including Kev, Laya, Nostr, and the relay, which carry no
per-crate overrides. Package-license metadata remains pending the owner
decision documented in [the dependency policy](dependencies.md).

## Feature and infrastructure coverage

The release gate checks formatting, strict Clippy, and tests for both default features
and `kev/serve,lev/serve,gym/tui,jev/blocking`. It then checks dependency
policy and disposable PostgreSQL acceptance. It stops
on the first failed command; later commands have not run when that happens.
It also runs the small Python artifact-acquisition regression suite before
Rust checks; this suite needs Python 3 and no model weights or network.
Backup collection regressions also run before Rust checks. They force a blob
rename during collection and verify that missing or corrupt bytes prevent
publication. The PostgreSQL phase separately tests backup and restore under
concurrent uploads and deletions.
It also checks the delegation-result validator against completed, refused,
incomplete, and inconsistent execution records before the Rust checks.

The PostgreSQL script needs `initdb`, `pg_ctl`, `createdb`, `curl`, Python 3,
and ordinary shell tools on PATH. It creates disposable local databases;
never redirect its destructive-test environment to a production database.
Use a separate `CARGO_TARGET_DIR` per worktree. The acceptance script uses
that target directory for the relay binary it starts.

Optional coverage is explicit:

- `--skip-postgres` prints a skip and makes the run partial. It does not
  satisfy PostgreSQL release acceptance.
- `--with-metal` runs strict Kev Clippy and tests with `serve,metal` on a
  supported Apple host with the required Apple toolchain. A successful
  compile does not establish inference results for external weights.
- `--with-soak` runs the existing long-running relay soak against a disposable
  PostgreSQL cluster. Routine runs print that it was skipped.

FoundationModels tests require the Swift helper, an eligible Apple host, and
model availability. Build the helper with `./scripts/build-lev-bridge.sh` and
follow `docs/lev/` for model-backed verification. Tests that conditionally
return without an available model are not evidence of live inference.
Coder delegation tests need an enforceable filesystem boundary, `bwrap` on
Linux with user namespaces enabled. On a host without one, the cases in
`crates/coder/tests/program_run.rs` and
`crates/coder/tests/suite_questions.rs` that run or offer a `delegate` step
print `skipping:` and return, which is not evidence that delegation works.
Install `bubblewrap` to run them.
Kev checkpoint conformance replays every committed variant on CPU for
hours, so `crates/kev/tests/conformance.rs` runs only under
`KEV_CONFORMANCE=1` — an operator sets it by hand, and nothing automatic
does. `KEV_VARIANT=<id>` narrows the battery to one variant. Model
experiments, hosted Jev calls, and production relay/worker proofs
require their own documented inputs and records. The routine gate does
not download weights or authorize new paid measurements.

Shell orchestration and retained Python training/acceptance tooling are
infrastructure, not additional product implementation languages. Keep product
code in Rust, with the existing Swift FoundationModels bridge exception.
Preserve `docs/transcripts/` and do not add GitHub-billed automation.

## Dependency policy

[`deny.toml`](../deny.toml) is the dependency policy: advisories, licenses,
sources, and bans over the resolved workspace graph with all features and
development dependencies. The manual command that enforces it is:

```sh
./scripts/check-dependencies.sh
```

It requires `cargo-deny` 0.20.2 on the pinned toolchain; install it with
`cargo +1.97.1 install cargo-deny --version 0.20.2 --locked`. The script
refuses when the `RUSTSEC-2024-0436` exception is overdue for review or the
resolved `paste` version differs from the reviewed one, then runs
`cargo deny --locked check advisories licenses sources bans`. If `cargo-deny`
is absent, `./scripts/verify-rust.sh` prints `SKIPPED: dependency policy`
and continues; that result is a partial gate, not a pass.
[The dependency policy](dependencies.md) records the exception's owner,
reason, and review date, the `paste` dependency paths, and the license
review.

## Current verification record

On 2026-09-20, the workspace passed strict all-target Clippy on Rust 1.97.1
with the runtime feature combination after enabling lint inheritance for
Nostr and the relay. The workspace also compiled all targets with those
features on Rust 1.95.0. Kev's standalone library compiled on Rust 1.94.0 and 1.94.1.

The disposable PostgreSQL acceptance script also passed with a separate
`CARGO_TARGET_DIR`, including store, gateway, multiprocess, import, release-load,
and binary deployment checks. The long-running soak and Metal checks were not
run for this package-policy change.

On 2026-09-20 at `fc385a13a` and the Jev fix after it, the whole gate passed
on a Linux host (Ubuntu, x86-64, Rust 1.97.1): formatting, both strict
Clippy runs, both test runs, the Rust 1.95.0 and 1.94.0 checks, the
dependency policy, and the PostgreSQL acceptance script including the
release-load proof. Debian and Ubuntu install the PostgreSQL server binaries
under `/usr/lib/postgresql/<version>/bin`, which is not on PATH by default;
prepend it before running the gate. Soak and Metal were not run.

## Apple serving matrix, 2026-09-20

The [Apple serving verification record](lev/measurements/2026-09-20-apple-serving-matrix.md)
records the actual #9426 commands on an Apple M5 Max running macOS 26.4.
Lev default and `serve` tests and strict Lev/Kev Clippy passed. Weighted Kev
Metal F32 conformance passed for all four retained variants, but the full
`serve,metal` test command failed in its CPU HTTP round-trip test at the
unchanged 10-second client deadline. bf16 remains unmeasured. This is a partial
matrix, not a successful full gate; #9426 stays open.

## Progress during verification

Each manual-gate phase prints its name when it starts, an elapsed-time heartbeat
at least every 30 seconds while its command runs, and its elapsed time and exit
status when it finishes. Command output streams directly to the terminal and,
when a run record is being kept, is teed to that phase's log file. Cargo
tests use `--nocapture`, so test diagnostics appear while tests run rather than
only after a failure. The gate still stops at the first failing phase unless
`--keep-going` was passed; a heartbeat reports activity, not success or a
timeout extension.

The phase runner forwards interrupt, termination, and hangup signals to the
command's process group and waits for the direct child. It does not add a test
timeout, change feature or device settings, or skip an assertion. A test that


## Mixed scripts and odd text

Ελληνικά: η γρήγορη καφέ αλεπού πηδάει πάνω από τον τεμπέλη σκύλο, ενώ ο διακομιστής απαντά.
Русский: быстрая коричневая лиса перепрыгивает через ленивую собаку, пока сервер отвечает на запрос.
Français : « déjà vu », naïve façade, cœur, œuvre, garçon — l’élève a répondu très vite.
Deutsch: Die Straßenbahnhaltestellenüberdachung wurde gestern von der Verkehrsgesellschaft erneuert.

See https://github.com/OpenAgentsInc/openagents/blob/main/crates/rust-native/src/layout/rows.rs#L120 and https://example.com/a/very/long/path/that/keeps/going/and/going?query=value&other=thing for details.

A path like crates/openagents-mobile/src/coder_tab.rs, an identifier like `rust_native_layout_frame_display_with_a_long_name`, and a hash like 3943b45f62c9e8d1a0b7c6d5e4f3a2b1c0d9e8f7.

Numbers: 1,234,567.89; 3.14159; 42%; $1,000; 10:45 PM; 2026-09-28T13:45:00Z; v0.2.10; 1e-6; ±0.5; 90°.

Punctuation soup: (parentheses), [brackets], {braces}, "double quotes", 'single quotes', semicolons; colons: dashes – and — ellipses… slashes / backslashes \ pipes | ampersands & at signs @ hashes # tildes ~ carets ^.

Supercalifragilisticexpialidocious-antidisestablishmentarianism-pneumonoultramicroscopicsilicovolcanoconiosis is one very long hyphenated word.

AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA

**Bold text that runs on for a while to wrap across lines**, then *italic text that also runs on for a while*, then ***both at once***, then `inline code with spaces` and ~~struck text~~.

```rust
fn main() {
	let tabbed = "a tab starts this line";
	println!("{tabbed} -> != => <= >= == && || :: ...");
}
```

| Column one | Column two | A third column with a longer header |
|---|:-:|--:|
| short | a cell with several words in it | 1,234 |
| another row | x | 99.5% |

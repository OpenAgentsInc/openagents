# OpenAgents protocols

OpenAgents defines general agent infrastructure. Coding agents are the first
specialization. These specifications describe how agents share capabilities,
workflows, context, and work across tools, models, and machines, and how they
improve their behavior through measured, bounded optimization.

The [agent labor integration plan](../../docs/agents/market-infrastructure.md)
is the next market application: agents negotiate bounded work and earn Bitcoin
for accepted results. The existing contracts supply capability, execution,
evidence, and authority. [NIP-MKT](NIP-MKT.md) now specifies negotiated
offerings, orders, and payment evidence; [NIP-LAB](NIP-LAB.md) specifies
bounded agent labor and acceptance. [NIP-CTRL](NIP-CTRL.md) adds scoped task
control from another client. These are drafts awaiting implementation, not
claims of a deployed market or synchronized Coder clients. The
[coverage review](../../docs/protocol/2026-09-26-openagents-gap-review.md)
explains the gaps and reuse across all three NIP lanes. Episodes 213–215,
266–267, and 275–281 are design inputs, not wire contracts.

[NIP-X402](NIP-X402.md) adds a **Designed** draft for Lightning-paid
operations before execution. It preserves upstream x402 `http:1` and `mcp:1`
bindings and defines an opt-in `nostr:openagents:1` extension, which is not an
upstream profile. It leaves MKT/LAB payment after acceptance unchanged. The
[integration assessment](../../docs/coder/design/x402-lightning-nostr-integration.md)
separates wallet transport through NWC, social zaps, and the distinct L402
protocol from this payment contract. No payment adapter is implemented by
the draft.

LAB keeps its name because OpenAgents already published a different
[NIP-LBR v1 contract](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/nips/LBR.md)
over NIP-90 kinds `5934`/`6934`/`7000`. The current `openagents.labor.v1`
profile uses private `3188` artifacts with MKT/CJ/RUN and claims no legacy
wire compatibility. See [LAB's provenance note](NIP-LAB.md) and the
[upstream sync review](../../docs/protocol/2026-09-26-upstream-nip-sync.md).

[NIP-SOV](NIP-SOV.md) restores the historical NIP-SA sovereign-agent design
as a **Designed** composition profile: durable identity, custody and recovery,
private state, bounded initiative, guardians, agent labor, and purchases.
It replaces the old `392xx` records with the current contracts and private
`3188` artifacts. The new name distinguishes this wire format from historical
SA; it allocates no kinds and implements no custody, wallet, or autonomous
host. The [source and migration map](NIP-SOV.md#provenance-and-migration)
retain the original draft and explain the changes.

[NIP-PYLON](NIP-PYLON.md) is a **Designed** draft for compute providers in
public: a provider's opt-in beacon for one machine, buyer-signed receipts for
the jobs it served, and pool aggregates any reader can recompute. Verse draws
its Pylon Field and Wellspring only from these records, as the
[Verse compute plan](../../docs/compute/verse-compute.md) describes. It
allocates `30200`, `30201`, and `3201` and implements no publisher yet.

[NIP-ATIF](NIP-ATIF.md) is a **Designed** draft for carrying agent trajectories
in the Agent Trajectory Interchange Format that Coder already records. Trajectories
stay owner-encrypted by default; a public copy is a separate, usually redacted,
publication with its own digest. It replaces the historical SA `39230`/`39231`
records with two regular kinds, `3198` and `3199`, and implements no publisher yet.

[NIP-REG](NIP-REG.md) is a **Designed** draft for curated plugin registries.
A curator publishes an inert EXT catalog that pins original publishers' releases.
Nostr relays and GitHub/HTTPS mirrors carry the same signed events and artifact
bytes. Readers choose trust per registry; discovery does not install, enable,
or admit a plugin. It allocates no new kinds and has no client implementation yet.

The [81-document teardown review](../../docs/protocol/2026-09-26-teardown-coverage.md)
adds six draft profiles: persistent engine sessions (SESS), workspace resources
and synchronized views (WS), tracked work (WORK), bounded automation (AUTO),
environment leases (ENV), and live media/device interaction (LIVE). It also
extends POL's learned-preference lifecycle and EXT's imports and host component
sets. These reuse private artifacts and existing execution kinds. The
[Coder integration plan](../../docs/coder/design/teardown-nostr-integration.md)
defines implementation order and the evidence required before shipping them.

[NIP-VAULT](NIP-VAULT.md) and [NIP-ATT](NIP-ATT.md) are **Designed** drafts
for sensitive personal data. VAULT fixes the object format, per-object keys,
key slots (passkey PRF, device, Nostr, recovery code), and three honestly
labelled tiers: only the person's devices, only an attested workload while the
person's client supplies its share, or an operator key logged at every use.
ATT lets a client verify that a program is an exact, publicly logged release
running in a hardware TEE before sealing work to its key: releases `3202`,
release heads `30202`, attested endpoints `30203`, and the
`openagents.attested.v1` feature for CJ/DEC jobs. See the
[vault](../../docs/security/sensitive-data-vault.md) and
[private inference](../../docs/security/private-inference.md) designs. Nothing
is implemented yet.

## Why this exists

Agent work should be understandable and controllable. A user should be able
to tell what an agent can do, what it may see or change, which version ran,
what evidence supports its result, and what remains unknown after a failure.
A task should preserve its objective, source evidence, instructions, budget,
and outcomes as it moves between operations or machines.

The same task should also survive changes in models and inference techniques.
A semantic AI contract describes the behavior needed. Its implementation can
use typed decisions, generation, retrieval, or a bounded composition of them.
An optimizer can search for a better implementation against an explicit
objective. Evaluation establishes evidence for adoption; host code controls
permissions, privacy, effects, and budgets throughout.

This supports the programming model associated with DSPy and optimization
approaches such as GEPA without making either a protocol runtime requirement.
Hand-authored implementations and other search methods use the same contracts.
No algorithm is assumed to improve every workload, and no score grants authority.

For users, the intended benefits are portable extensions, explicit control
over private context, understandable approvals, recoverable tasks, and
measured improvements that identify their costs and limitations. An update
can be evaluated and adopted without silently changing a task already running.

## Why use Nostr

Nostr provides an open foundation for an agent ecosystem that anyone can help
build. Its small core of signed events, public-key identities, and relay
subscriptions supports useful applications with relatively little machinery.
Additional NIPs define discovery, encrypted communication, and application
behavior. This gives agents and their users several benefits:

- **Open-source participation.** Developers can inspect, audit, run, modify,
  and share open-source clients, relays, and tools. Public protocol contracts
  also let them build independent implementations. Participation does not
  depend on one vendor's roadmap, private API, or marketplace approval.
- **Easy extensibility.** Developers can describe new behavior through event
  kinds, tags, and content schemas while reusing identity, signing, and
  delivery. Applications can implement a useful subset, document extensions,
  and grow through practical adoption. Each feature can build on shared
  infrastructure without requiring a complete new platform.
- **Interoperability and shared network effects.** Applications that implement
  the same contracts can discover capabilities, exchange tasks, consume
  package releases, and compare evaluation evidence. A contribution can serve
  users across multiple clients. Agents using different applications can find
  each other and coordinate work, increasing the usefulness of the shared
  network. These specifications supply the application meanings needed for
  that interoperability; a Nostr connection alone does not establish support
  for a program or execution interface.
- **Portable identity and user choice.** Public-key identity and signed records
  can remain usable across clients and relay operators. Users can choose
  interfaces and providers, self-host infrastructure, and carry authorized
  records between services while preserving authorship and provenance.
  Private information remains subject to its disclosure and access rules.
- **Resilience through replaceable components.** Independent relay and worker
  operators, replication of authorized records, and independent implementations
  can reduce dependence on a single service or codebase. Applications can
  select providers that support the required contracts and policies; delivery,
  retention, and recovery still require explicit operational choices.

These benefits apply to coding, research, data processing, and other agent
work. The shared formats make programs, extensions, and measured improvements
reusable across an ecosystem of independently built applications.

Hosts enforce access, manage credentials, materialize programs, run tools,
and coordinate effects. Evaluators measure outcomes. Relays store or deliver
events under explicit privacy and retention rules. Signatures establish
attribution, not truth, permission, statistical validity, or remote attestation.
The artifact formats also work locally; every model call or observation need
not become an event or require a relay round trip.

## General infrastructure and coding specialization

The shared core is identity, semantic contracts, typed operations and workflows,
extensions, evidence, authority, execution, coordination, evaluation, and
optimization. Research, document, data-analysis, and business agents use it
with their own sources, schemas, operations, and acceptance criteria.

Coding adds repository snapshots, code search, compiler diagnostics, isolated
execution, patches, tests, and Git integration. A research agent can instead
produce cited findings from documents. A records agent can propose an update
from scoped observations. Neither needs a repository or terminal. Sending a
message or changing an external record requires domain-specific authorization,
version checks, effect confirmation, and reconciliation.

Generality does not make domain guarantees interchangeable. A host supports
only the adapters, policies, and validation it can enforce. A coding benchmark
cannot admit a different domain merely because it uses the same model.

## How the pieces fit together

1. **Define and discover.** OPT identifies semantic AI contracts. CAP describes
   execution interfaces. EXT distributes exact component releases.
2. **Compose the work.** PRG defines typed workflows and bounded component
   invocation. An AI implementation binds a semantic contract to an executable
   entry and its complete dependencies.
3. **Prepare context and authority.** CTX identifies task state and evidence.
   POL resolves instructions, disclosure, approvals, and routing.
4. **Execute and recover.** CJ carries jobs. COORD manages shared tasks and
   claims. RUN records durable outcomes and unresolved effects.
5. **Measure and improve.** OPT bounds candidate search and records what ran.
   EVAL records workload comparisons and scoped admission evidence. Operator
   policy adopts an immutable eligible version for subsequent work.

CTRL connects additional clients to the same task owner with separate
observation, steering, and cancellation rights. MKT and LAB connect buyers
and providers through accepted commercial terms and exact deliverables.
Neither a paired client nor an accepted order bypasses POL, host admission,
independent verification, or separately authorized wallet actions.

SESS supplies the persistent engine boundary shared by terminal, desktop,
web, and mobile clients. WS gives those clients exact resources and bounded,
repairable views. WORK tracks objectives independently of individual runs;
AUTO admits finite occurrences against them. ENV binds actual runtimes and
resource lifetimes, while LIVE scopes media and device operations. These
profiles compose existing authority and execution contracts rather than
introducing another generic job or payment family.

SOV composes these pieces around a persistent agent identity. Its admitted
profile binds custody, governance, disclosure, and treasury policy; its finite
lifecycle uses AUTO/COORD/RUN and actual ENV materialization. REACH can locate
a candidate host but cannot move controller authority. Independence, signing,
approval, and spending require their own supported mechanisms and evidence.

For example, an evidence-selection operation may compare a typed relevance
model with a joint retrieval strategy. Both must preserve required context,
source attribution, and disclosure constraints. Evaluate complete task quality
and total cost before adopting either. The optimizer cannot modify the grader,
read protected confirmation labels, or give itself new permissions.

## Test-time capabilities

A *test-time capability* is an ability an agent gains, or loses, while it
runs, without a weight update, because a component was admitted to the run.
It is stated only as a *capability claim*: a controlled comparison, the same
tests run with and without the exact component, against a stated baseline
under a stated grant and rule. A component with no such claim is a
candidate; a claim reproduced by another trainer and externally validated on
tests its author didn't write is what adoption decides on. The
[essay that proposes the term](../../docs/essays/2026-09-29-test-time-capabilities.md)
states the concept in Part I; its
[Part II](../../docs/essays/2026-09-29-test-time-capabilities.md#part-ii-our-implementation)
covers the OpenAgents implementation, including the full
[mapping of terms and lifecycle stages to kinds and fields](../../docs/essays/2026-09-29-test-time-capabilities.md#how-the-protocol-carries-test-time-capabilities).
In short, which NIP is for what:

| Stage | NIP | For |
| --- | --- | --- |
| Discover | [EXT](NIP-EXT.md), [CAP](NIP-CAP.md), [KB](NIP-KB.md) | Component releases and listings (the packages capabilities ship in), operation descriptions and presence, knowledge entries. |
| Admit | [EXT](NIP-EXT.md), [CAP](NIP-CAP.md), [RUN](NIP-RUN.md), [contracts](contracts.md) | Locks, grants, and admission as separate decisions; the run's recorded lock. |
| Run and judge | [DEC](NIP-DEC.md), [CJ](NIP-CJ.md), [CAP](NIP-CAP.md), [PRG](NIP-PRG.md) | Decisions and the router's `judgment` feedback; decision services; `decide` steps. |
| Delegate | [PRG](NIP-PRG.md), [SESS](NIP-SESS.md), [WORK](NIP-WORK.md), [CTX](NIP-CTX.md) | `delegate` steps, delegate engines' steering rows, work delegations, briefing evidence. |
| Trajectory | [ATIF](NIP-ATIF.md) | Each run's trajectory and its links to delegated sub-agents. |
| Measure and publish | [EVAL](NIP-EVAL.md) | With-and-without reports with the claim's scope in `meta.ext_eval` (reliance set, identity strength, distribution, defaults), the gate's verdict under a declared primary outcome, and `3189` results. |
| Reproduce | [EVAL](NIP-EVAL.md) | Checks by another trainer on the same suite, confirming or disputing, with the reliance set they shared readable. |
| Validate externally | [EVAL](NIP-EVAL.md), [OPT](NIP-OPT.md) | `validates` results on a second suite of the same distribution, independent by signer and chronology; `transfer` results on another distribution; OPT's confirmation phase. |
| Credit | [XP](NIP-XP.md) | The `eval-check` (paid for the rerun, confirming or disputing) and `eval-adopt` rules. |
| Adopt | [EVAL](NIP-EVAL.md), [POL](NIP-POL.md), [EXT](NIP-EXT.md) | `openagents.eval-admission.v1` citing validations, a marginal report, regression, reliability, authority, and stakes; a `coder-defaults` release; operator authority. |
| Share | [MV](NIP-MV.md), [CJ](NIP-CJ.md) | Gym notes in Verse; chat cards and offers. |

Three gaps are stated rather than filled: the delegate door's own failover
has no wire record; a judgment's time and cost have a designed record
(POL route usage, CJ `judged_ms` and `latency_ms`) that nothing writes yet;
and the flywheel's measure, marginal externally validated utility on a
declared distribution per adopted contribution, is derived by a reader from
`validates` results and `coder-defaults` releases, not carried by a field.

## Specification reference

All contracts in this set are v1 drafts. They define protocol behavior;
conformance requires validation and enforcement for each advertised role.

| Contract | Responsibility | Kinds |
| --- | --- | --- |
| [Shared contracts](contracts.md) | Encoding, references, schemas, locks, effects, evidence, outcomes, and private artifact envelopes. | Private artifact `3188`. |
| [NIP-CAP](NIP-CAP.md) | Operation descriptions (its capability definitions), host bindings, grants, presence, and preferences. | `30180`, `30181`. |
| [NIP-PRG](NIP-PRG.md) | Typed workflows, the default kind of contributed capability; seven step kinds, bounded composition, and plugin packet ABI. | `30182`, `30183`. |
| [NIP-EXT](NIP-EXT.md) | Extension packages, the container a capability ships in: immutable releases, imports, host component sets, discovery, revocation, and namespace transfer; `eval-suite` components carry published test sets. | `3184`–`3186`, `30184`, `30185`; private records on shared `3188`. |
| [NIP-REG](NIP-REG.md) | Designed curated plugin registries: curator identity, immutable catalogs pinning original publisher releases, per-registry trust, and equivalent Nostr and GitHub/HTTPS sources. | Existing EXT `3184` releases and `30184` heads; no new kinds. |
| [NIP-RUN](NIP-RUN.md) | Encrypted durable journals, fencing, evidence, and recovery. | `3187`, `30186`. |
| [NIP-CJ](NIP-CJ.md) | Conversation, typed-decision, and recoverable execution jobs; conversation offers, cards, and the test-set draft (`crates/nostr` `cj_conversation`). | `25900`/`26900`/`27000`, `25910`/`26910`/`27010`, `25920`/`26920`/`27020`. |
| [NIP-DEC](NIP-DEC.md) | Decisions: one state (a string or object) and typed `noul`, `choice`, and `score` questions whose instructions and criteria are EntryType (string, object, array, or null), answered with probabilities; bounds, model aliases (`typesafe/jev-1.13`), HTTP-gateway equivalence with OpenRouter's status codes, and ATIF decision calls. Implemented in `crates/nostr` `decision`, served by the hosted decision worker. | The CJ decision family `25910`/`26910`/`27010`, `openagents.systemone.v1`. |
| [NIP-CTX](NIP-CTX.md) | Task frames, snapshots, context views, representations, and expansion. | Shared `3188`; CJ/RUN references. |
| [NIP-POL](NIP-POL.md) | Instructions, learned preferences, approvals, disclosure, routing, and adoption authority. | Shared `3188`; CJ/RUN references. |
| [NIP-COORD](NIP-COORD.md) | Tasks, fenced claims, shared budgets, background findings, and trial coordination. | Shared `3188`; CJ/RUN references. |
| [NIP-EVAL](NIP-EVAL.md) | Workload evaluation, comparisons, and scoped promotion evidence, with an extension evaluation profile (suites, results, checks, hosted runs, and adoption) whose wire formats `crates/nostr` implements (`eval_ext`). | Shared `3188`; public declaration `3189`; Gym results publication `3195`. |
| [NIP-OPT](NIP-OPT.md) | AI signatures, implementations, studies, data partitions, candidates, materialization, trials, and results. | Shared `3188`; EXT/EVAL declarations and CJ/RUN execution. |
| [NIP-KB](NIP-KB.md) | Shared knowledge entries, the fourth kind of contributed capability: immutable versions, current-version heads, withdrawals, and evidence as EVAL publications. Trust is per reader. | `3190`, `30190`, `3191`; evidence on EVAL `3189`. |
| [NIP-XP](NIP-XP.md) | Frozen quest versions, referee awards of verified accepted outcomes bound to KB entries and EVAL evidence, revocations, and per-reader XP ledgers; the `eval-check` and `eval-adopt` rules (implemented in `crates/nostr`) credit extension evaluations. XP is never spendable; sats settle separately. | `30193`, `3193`, `3194`; playtest session records `3196` and reports `3197`; achievements as NIP-32 `1985` labels. |
| [NIP-CTRL](NIP-CTRL.md) | Client pairing, task-scoped control rights, revocation, acknowledged commands, and bounded catch-up. | Shared `3188`; registered CAP operations over CJ execution. |
| [NIP-HOST](NIP-HOST.md) | Host-wide device enrollment by invitation or approved short code, closed scoped rights, delegation, revocation epochs, device listing, and typed task creation, steering, and cancellation. Draft with a bounded host and client in `crates/coder-access`, served by the resident host in [`coder-host`](../../crates/coder-host/README.md) over relay artifacts, direct channels, and CAP operations over CJ execution. The 2026-09-29 amendment designs, but does not implement, `openagents-connect:` QR codes, enrollment over an iroh ALPN, the local operator socket, and planned nearby approval. | Shared `3188`; direct artifacts, or CAP operations over CJ. |
| [NIP-MKT](NIP-MKT.md) | Immutable offerings, private negotiation, accepted orders, cancellation, and attributable Bitcoin settlement. | `3192`, `30192`; private records on shared `3188`. |
| [NIP-LAB](NIP-LAB.md) | Agent-labor terms, execution linkage, deliverables, verification, acceptance, rework, disputes, and rights. | Shared `3188`; MKT agreements and CJ/RUN execution. |
| [NIP-X402](NIP-X402.md) | Designed Lightning-paid operations: standard HTTP/MCP bindings, an opt-in native Nostr binding, spending admission, private evidence, and recovery. | No new kinds; CAP discovery and shared `3188` native records with POL/CJ/RUN admission. |
| [NIP-SESS](NIP-SESS.md) | Engine capability, persistent sessions, durable input queues, steering, interactions, and native history/imports. | Shared `3188`; CAP/CJ operations and RUN records. |
| [NIP-WS](NIP-WS.md) | Workspace resources, exact document versions, conditional changes, worktrees, checkpoints, bounded projections, and audience-bound activity summaries. | Shared `3188`; CAP/CJ operations and RUN records; Block PL wakes. |
| [NIP-WORK](NIP-WORK.md) | Tracked objectives, planning relations, assignments, revision admission, disposition, source imports, and attention. | Shared `3188`; CAP/CJ operations and WS projections. |
| [NIP-AUTO](NIP-AUTO.md) | Finite schedules, source triggers, checked continuation, durable occurrence admission, and recovery. | Shared `3188`; CAP/CJ operations and RUN/COORD admission. |
| [NIP-ENV](NIP-ENV.md) | Environment allocation, exact materialization, bounded leases, participant admission, attachment, cleanup, and SSH-launched hosts (launcher side in [`coder-ssh`](../../crates/coder-ssh/README.md)). | Shared `3188`; CAP/CJ operations and RUN records. |
| [NIP-LIVE](NIP-LIVE.md) | Media participants and consent, input/speaking floors, capture anchors, and observation-bound device input. | Shared `3188`; CAP/CJ operations and admitted media transports. |
| [NIP-REACH](NIP-REACH.md) | Owner host directory, host presence with bounded telemetry and receipt-time freshness, reachability hints without loopback fallback, authenticated direct channels bound to a grant and host generation, and placement. Implemented in [`coder-reach`](../../crates/coder-reach/README.md) with synthetic loopback-socket tests over TCP and WebSocket; [`coder-host`](../../crates/coder-host/README.md) publishes presence and hints and serves TCP and WebSocket direct channels, `ws` or TLS-terminated `wss`, with the real grant store. The 2026-09-29 amendment designs, but does not implement, `iroh` hints in a v2 hints record and the channel over an iroh stream. | Shared `3188`; no new kinds. |
| [NIP-TERM](NIP-TERM.md) | Interactive terminal sessions: open, attach, detach, input, resize, signal, and close under the `terminal` right; per-terminal output sequence numbers, bounded replay with explicit gaps, idle expiry, and `lost` after a host restart. Implemented in [`coder-pty`](../../crates/coder-pty/README.md) with real-PTY tests on macOS; [`coder-host`](../../crates/coder-host/README.md) wires NIP-HOST rights, NIP-REACH channels, and `3188` sealing. | Shared `3188` or NIP-REACH data frames; no new kinds. |
| [NIP-SOV](NIP-SOV.md) | Designed successor to historical SA: durable agent identity, admitted custody, bounded lifecycle, guardians, treasury policy, market participation, and retained recovery evidence. | Shared `3188`; existing AUTO/CAP/CJ/COORD/RUN contracts; no new kinds. |
| [NIP-ATIF](NIP-ATIF.md) | Designed carriage of ATIF agent trajectories: exact-byte and ATIF-rule step digests, owner-encrypted private carriage, public declarations with ordered chunks, and links to Coder tasks, RUN runs, delegated sub-agents, and continued segments. Maps Block AO/AM/AE onto ATIF steps. | `3198`, `3199`; private manifests and chunks on shared `3188`. |
| [NIP-MV](NIP-MV.md) | Shared 3D worlds: ephemeral pose frames and gestures, durable entity state, world definitions, and cell-scoped subscriptions. Its optional runtime-loaded scene manifest and rules profile are Designed; Verse's curated local zones are not general world discovery. Standalone: it depends on no other contract here. | `23300`, `23301`, `23302`, `33300`, `33301`. |
| [NIP-PYLON](NIP-PYLON.md) | Designed compute pylons and pools: a provider's opt-in public beacon with coarse class and free slots, buyer-signed service receipts with digests and payment preimages, recomputable pool aggregates, NIP-32 check verdicts, and the rules a world follows to draw them. | `30200`, `30201`, `3201`; NIP-32 `1985` labels. |
| [NIP-ATT](NIP-ATT.md) | Designed attested workloads: public releases with measurements, sources, rebuilds, and Rekor entries; release heads with notice delays and rollback refusal; attested endpoint keys bound into TEE evidence; evidence-computed levels (`open`, `hardened`, `tee`, `tee-cloud`); sealed CJ/DEC jobs with ciphertext-digest receipts. | `3202`, `30202`, `30203`; sealed jobs on CJ/DEC. |
| [NIP-VAULT](NIP-VAULT.md) | Designed sealed personal data: chunked AES-256-GCM objects, `user` / `sealed` (HPKE mode_psk to an attested system key plus the person's share) / `operator` wraps, key slots, key indexes for crypto-shredding, and session, request, lease, and revoke formats. | No new kinds; blobs by digest; slots and leases on shared `3188`. |

Discovery heads are mutable. Exact signed records and artifact digests pin
execution. Publication, installation, enablement, selection, grants, admission,
and promotion are separate actions. Private evidence and derived examples
remain scoped even when the resulting implementation is useful to others.

Kind allocations are draft assignments, not upstream registrations. Named
extensions belong in NIP-11 `supported_extensions`, not numeric
`supported_nips`. Advertise only tested, configured roles. A relay forwarding
an envelope cannot claim to execute programs, isolate an evaluator, enforce
spending, or establish semantic correctness.

MKT and LAB define a new OpenAgents profile, not compatibility with the
historical Immortal market stack. Swaps, escrow, credit, and a general royalty
market are not implied. CTRL reuses current task contracts rather than
treating Block read-state sync or live telemetry as durable task control.
The [implementation plan](../../docs/protocol/implementation-plan.md) tracks
the role-specific validators, host work, and fixtures still required.

## Kind registry

Each OpenAgents kind has exactly one owner. This table lists every kind an
OpenAgents specification claims, in kind order; `crates/nostr/src/kinds.rs`
holds the same list as constants the protocol code uses. Before you assign
a kind, pick a number that isn't here, isn't in the
[official list](../official/README.md#event-kinds), and isn't a
[Block kind](../block/README.md), then add it to both places. The tests in
`crates/nostr` (`kinds`) fail when two specifications claim one kind, when
a specification declares a kind that isn't registered to it, or when this
table and the constants disagree. Kinds a specification only uses, such as
NIP-32 `1985` labels or Block `24200` frames, aren't claims.

| Kind | Owner | Meaning |
| --- | --- | --- |
| `3184` | [NIP-EXT](NIP-EXT.md) | Immutable release declaration |
| `3185` | [NIP-EXT](NIP-EXT.md) | Release revocation |
| `3186` | [NIP-EXT](NIP-EXT.md) | Namespace migration attestation |
| `3187` | [NIP-RUN](NIP-RUN.md) | Encrypted durable run record |
| `3188` | [contracts](contracts.md) | Private artifact envelope |
| `3189` | [NIP-EVAL](NIP-EVAL.md) | Public evaluation declaration |
| `3190` | [NIP-KB](NIP-KB.md) | Immutable entry version |
| `3191` | [NIP-KB](NIP-KB.md) | Entry withdrawal |
| `3192` | [NIP-MKT](NIP-MKT.md) | Immutable public offering |
| `3193` | [NIP-XP](NIP-XP.md) | Award |
| `3194` | [NIP-XP](NIP-XP.md) | Award revocation |
| `3195` | [NIP-EVAL](NIP-EVAL.md) | Gym results publication |
| `3196` | [NIP-XP](NIP-XP.md) | Playtest session record |
| `3197` | [NIP-XP](NIP-XP.md) | Content-free playtest report |
| `3198` | [NIP-ATIF](NIP-ATIF.md) | Public trajectory declaration |
| `3199` | [NIP-ATIF](NIP-ATIF.md) | Public trajectory chunk |
| `3201` | [NIP-PYLON](NIP-PYLON.md) | Service receipt |
| `3202` | [NIP-ATT](NIP-ATT.md) | Attested workload release |
| `13193` | [NIP-XP](NIP-XP.md) | Trainer profile |
| `13195` | [NIP-XP](NIP-XP.md) | Key link |
| `23300` | [NIP-MV](NIP-MV.md) | Pose frame |
| `23301` | [NIP-MV](NIP-MV.md) | Gesture |
| `23302` | [NIP-MV](NIP-MV.md) | Zone command |
| `25900` | [NIP-CJ](NIP-CJ.md) | Conversation job request |
| `25910` | [NIP-DEC](NIP-DEC.md) | Decision job request or cancel |
| `25920` | [NIP-CJ](NIP-CJ.md) | Execution request or control |
| `26900` | [NIP-CJ](NIP-CJ.md) | Conversation job result |
| `26910` | [NIP-DEC](NIP-DEC.md) | Decision job result |
| `26920` | [NIP-CJ](NIP-CJ.md) | Execution result or control answer |
| `27000` | [NIP-CJ](NIP-CJ.md) | Conversation job feedback |
| `27010` | [NIP-DEC](NIP-DEC.md) | Decision job status |
| `27020` | [NIP-CJ](NIP-CJ.md) | Execution admission and progress |
| `30180` | [NIP-CAP](NIP-CAP.md) | Capability discovery head |
| `30181` | [NIP-CAP](NIP-CAP.md) | Operator preference head |
| `30182` | [NIP-PRG](NIP-PRG.md) | Program discovery head |
| `30183` | [NIP-PRG](NIP-PRG.md) | Module announcement |
| `30184` | [NIP-EXT](NIP-EXT.md) | Package listing |
| `30185` | [NIP-EXT](NIP-EXT.md) | Revocation checkpoint |
| `30186` | [NIP-RUN](NIP-RUN.md) | Encrypted current-head hint |
| `30190` | [NIP-KB](NIP-KB.md) | Current entry head |
| `30192` | [NIP-MKT](NIP-MKT.md) | Current offering head |
| `30193` | [NIP-XP](NIP-XP.md) | Frozen quest version |
| `30194` | [NIP-XP](NIP-XP.md) | Trainer card |
| `30200` | [NIP-PYLON](NIP-PYLON.md) | Pylon beacon |
| `30201` | [NIP-PYLON](NIP-PYLON.md) | Pool aggregate |
| `30202` | [NIP-ATT](NIP-ATT.md) | Release head |
| `30203` | [NIP-ATT](NIP-ATT.md) | Attested endpoint |
| `33300` | [NIP-MV](NIP-MV.md) | World definition |
| `33301` | [NIP-MV](NIP-MV.md) | Entity state |

## Current implementation evidence

The [September 26 implementation coverage report](../../docs/protocol/2026-09-26-nip-implementation-coverage.md)
tracks each contract's actual role. Shared private artifacts now have encrypted
open/seal and relay privacy paths. MKT/LAB have strict free-negotiation and terms
validators, and X402 has offline invoice/request-binding verification. These
components do not implement a running labor or payment service; durable host
admission, execution, acceptance, and recovery remain explicit work.

The [SESS retained-history observer](NIP-SESS.md#read-only-observation-of-retained-foreign-history)
has a narrower application implementation in
[`coder-history`](../../crates/coder-history/README.md),
[`coder-connect`](../../crates/coder-connect/README.md), and the
[read-only iOS reader](../../docs/coder/guides/mobile-readonly.md). A five-minute QR or pasted invitation can bind the first redeeming phone
without copying its public key first; the manual explicit-key path remains.
Local pairing binds selected Codex/Claude roots, host and client keys, relay,
and expiry. Finite catalog/transcript reads use NIP-42 authentication and
signed encrypted private `3188` artifacts, with exact request binding,
revocation checks, bounded raw records, and source-change refusals.
The [reader receipt](../../docs/coder/verification/2026-09-26-mobile-reader.md)
and [QR bootstrap receipt](../../docs/coder/verification/2026-09-26-world-pairing.md)
record synthetic and native evidence and remaining acceptance.
This implements no managed engine-session admission, input queue, submit,
steering, interruption, approval, or CTRL task control. It allocates no new
kind and advertises no general CAP/CJ execution support.

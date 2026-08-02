# Porting Linear Agents into OpenAgents as a native Nostr-centric product system

- Date: 2026-08-02
- Lane: Fast Follow research
- Disposition: recommended native feature-port architecture; not implementation authority
- OpenAgents source pin: `20a7115694085b04bf9cc9b8bd486599fd26cb87`
- OpenAgents tree pin: `3e2c01565dbeacb951e7fcf3d0c38bd539637363`

## 1. Correct decision

OpenAgents should **port and independently implement Linear's useful agent-era
product functionality as native OpenAgents functionality**.

OpenAgents should not integrate with Linear, use Linear as a control surface,
mirror Linear data, or depend on Linear APIs. There is no Linear OAuth app,
Linear webhook bridge, Linear connector, Linear object mapping, Linear source
of truth, or Linear runtime in this architecture.

Linear is source evidence only. The implementation target is an OpenAgents-
owned product-work system with:

- native product spaces, teams, initiatives, projects, cycles, milestones,
  work items, subitems, dependencies, labels, documents, decisions, customer
  signals, and service-level state;
- native accountable human ownership and bounded agent delegation;
- native agent sessions, plans, visible activities, questions, actions,
  artifacts, errors, verification, and human disposition;
- native Skills, Guidance, Loops, triage intelligence, code intelligence,
  coding sessions, diffs, Guided Reviews, Inbox, notifications, and analytics;
- signed human and agent identities;
- Nostr-centric workrooms, causal threads, portable event projections, private
  messages, delegations, progress, code coordination, and evidence references;
  and
- OpenAgents authority for admission, policy, execution, budgets, evidence,
  verification, receipts, release, and settlement.

The shortest accurate statement is:

> Build OpenAgents' own Linear-class product-development system. Use Buzz's
> Nostr-centric collaboration lessons to make every person, agent, workroom,
> work item, session, decision, code change, and receipt part of one signed
> causal graph. Do not use Linear to provide any of it.

## 2. What “port” means

Linear is closed-source. “Port” here means a clean, target-native functional
implementation from public behavior and architecture evidence. It does not
mean copying Linear code, private APIs, visual assets, trademarks, prompts, or
proprietary data.

The port has four requirements:

1. **Feature completeness.** Reproduce the useful product behaviors, not only a
   chat bot or issue list.
2. **OpenAgents-native semantics.** Use OpenAgents identities, workrooms,
   ProductSpec packets, runs, grants, budgets, receipts, verification, Sync,
   Forge, Pylon, and Agent Computer.
3. **Nostr-centric collaboration.** Give work and agent activity stable signed
   identities and portable causal projections, as Buzz does for Slack-shaped
   collaboration.
4. **Stronger authority.** Keep relay events, model output, provider completion,
   and UI state separate from admitted commands, verified outcomes, release,
   payment, and settlement.

The result is not “Linear inside OpenAgents.” It is **OpenAgents Work**—a
placeholder name for a native product-planning, agent-execution, and review
system. Product naming is a later decision.

## 3. Authority and evidence status

This document is a Fast Follow research artifact. It recommends an architecture
and candidate packets. It does not itself admit implementation, change roadmap
priority, deploy a relay, create a public route, spend money, or make a parity
claim.

The current Sol and Omega roadmaps select only bounded Buzz-derived outcomes
and standard Nostr interoperability. Where older Buzz documents describe a
relay-primary company record, the later roadmaps control. A future decision to
make signed Nostr events the primary company record would need an explicit
ProductSpec, migration, authority, retention, privacy, and recovery decision.

This design therefore makes Nostr central to identity, collaboration, causal
provenance, portability, and interoperability while OpenAgents admission and
Cloud SQL projections remain canonical for product actions under current
authority.

### 3.1 Source manifest

| Evidence                                       | SHA-256                                                            | Use                                                                                                                 |
| ---------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
| [`FASTFOLLOW.md`](../../FASTFOLLOW.md)         | `62fa235232f77ab54389d40350e9c1281181465a001c5899038024bb88684122` | Admitted learning intent and source/lesson identities                                                               |
| [`INVARIANTS.md`](../../INVARIANTS.md)         | `77c8bcb6b8549b17c22aaa2a5886cc26687ab8162321deb366f35defc02e443f` | Current product, connector, workroom, authority, infrastructure, and privacy laws                                   |
| [Buzz teardown](./2026-07-21-buzz-teardown.md) | `25ae1fe51370297725ce621c10939fa95ccfe5cb1f91c7cb66789e937915b7cd` | Signed collaboration, agents, workflows, memory, attention, Git, search, and relay failure evidence                 |
| [Linear Agents teardown](./linear-agents.md)   | `cc3f2d59ab255e407acd803252d54c23fe93edffbd54fabacef96182ce4760d0` | Native agent, Agent Platform, Skills, Guidance, Loops, triage, code, sessions, governance, and reliability evidence |
| [Linear Diffs report](../forge/linear.md)      | `8b2235f31b0a6ccd61582e471ac1ff06e7f6211e421c283b108d111410ff2f4b` | Issue-context review, Git authority, Guided Reviews, and agent iteration evidence                                   |
| [Sol master roadmap](../sol/MASTER_ROADMAP.md) | `aecf21c6ceaad43394ee5bb051454341ea6d8cbdf4386147fbf189928f9e3cca` | Current sequencing and retired-path authority                                                                       |
| [Omega roadmap](../omega/ROADMAP.md)           | `a7371c6d5a601d3a7c0617293e261a53594e680d00b4ca31d3e2bf9108be37dc` | Current native surface and optional Nostr posture                                                                   |

### 3.2 Evidence disposition

| Axis                | Result                                                                                                       |
| ------------------- | ------------------------------------------------------------------------------------------------------------ |
| Source freshness    | Repository-current at the pinned target; upstream product details require revalidation before implementation |
| Evidence confidence | High for publicly documented product behavior; lower for undisclosed internal implementation details         |
| Target relevance    | High: OpenAgents already owns agent execution, workrooms, Sync, code, receipts, and Nostr mechanisms         |
| Portability         | High for product concepts; no source-code portability is claimed                                             |
| Implementation      | Substrate exists; the native product-work domain and complete UI do not                                      |
| Verification        | Architecture-only; no native Linear-class acceptance journey exists                                          |
| Disposition         | Build a native system through separately admitted, end-to-end packets                                        |

## 4. What Buzz teaches this port

Buzz did not bolt a bot onto Slack. It re-expressed Slack-shaped collaboration
as signed events, Nostr identities, relay-qualified rooms, agent participants,
private messages, workflows, memory, presence, attention, search, Git
coordination, and evidence.

OpenAgents should apply that same move to Linear-shaped product work.

### 4.1 Direct adaptations

| Buzz mechanism                      | Native OpenAgents Work adaptation                                                                 |
| ----------------------------------- | ------------------------------------------------------------------------------------------------- |
| Human and agent keypairs            | Every person and agent has a separate signed identity and explicit OpenAgents binding             |
| Channels and forum rooms            | Teams and projects have stable workrooms; initiatives and reviews can have bounded rooms or views |
| Threads and replies                 | Every work item, decision, review, and session has a causal thread                                |
| Agent roster and cards              | Agents are first-class product members with capabilities, runtime, state, owner, and activity     |
| ACP process pool                    | OpenAgents routes sessions through native runtimes, attached agents, Pylons, and Agent Computers  |
| One prompt in flight per channel    | Each work session is generation-fenced with explicit queue, steer, stop, retry, and replacement   |
| Owner-decryptable memory            | Durable agent memory is separately admitted and owner-readable through NIP-AE-compatible policy   |
| Workflow actions and approval gates | Loops compile triggers, conditions, agent steps, tools, approvals, budgets, and terminal fates    |
| Structural loop prevention          | A Loop's output event classes are excluded from its own trigger graph by construction             |
| Home/attention view                 | Inbox groups questions, mentions, blocks, failed runs, reviews, verification, and owner decisions |
| Signed Git events                   | Native work items link to NIP-34/Forge coordination while Git objects remain canonical            |
| Metrics and live state              | NIP-AO/NIP-AM-compatible projections show activity without becoming billing or verification truth |
| Search across event history         | Authorized structured search compiles operators over the native work graph and signed projections |
| Shared skill source                 | One versioned Skill definition compiles to each admitted runtime and client surface               |

### 4.2 Stronger OpenAgents boundaries

Do not copy Buzz's relay as general company-record, command, or receipt
authority. A Nostr event can prove signer and exact bytes. A relay response can
prove what that relay accepted. Neither proves:

- the signer had current OpenAgents authority;
- the work item, project, or membership mutation was admitted;
- an agent actually ran;
- a tool was allowed;
- provider output was correct;
- verification passed;
- a human accepted the result;
- Git merged the code;
- a release occurred; or
- payment or settlement happened.

OpenAgents should preserve signed events and relay receipts as evidence while
its typed admission and verification records decide product meaning.

### 4.3 Repository-wide Buzz disposition

The repository-wide Buzz audit distinguishes what OpenAgents already adapted
from what remains evidence or was canceled.

Already implemented or preserved:

- hermetic, read-only Forge conformance for pinned Buzz SDK and legacy Desktop
  NIP-34 dialects; this is fixture evidence, not live Buzz interoperability;
- the typed NIP-34 claim-ledger projection, signer, verified durable store, and
  subscription path retained from the canceled hosted-relay program;
- the standard NIP-29 public Agent Chat reader, relay identity checks, NIP-42
  authentication, reconnect, pagination, and bounded signer mechanisms; and
- selected Sarah owner identity, owner-decryptable memory, turn transport,
  persona, live-state, metric, and community records with their exact Product
  and Assurance limits.

Useful future outcomes, not complete shipped product:

- Omega-native attention, workrooms, threads, DMs, agent roster, existing-agent
  attachment, search, code rooms, decisions, governance, and cross-client
  journeys;
- complete NIP-29 membership, moderation, branching, multi-relay, and private-
  workroom behavior; and
- expanded signed memory and agent protocol promises only after compatible
  AssuranceSpec admission.

Canceled or rejected:

- a separate Buzz deployment, Buzz client, standalone relay service, or
  standalone Forge;
- the July Nostr-primary/full-parity plan where it conflicts with the later Sol
  and Omega roadmaps;
- the broad cross-app Nostr delegation program as implementation authority;
- Buzz's Tauri, Flutter, Postgres, Redis, MinIO, custom voice, and broad ACP
  product stack; and
- adoption of Buzz's complete vendor-specific custom-kind registry.

This port consumes the lessons and current OpenAgents implementations. It does
not restore any canceled Buzz product path.

## 5. Linear functionality to clone natively

### 5.1 Feature parity matrix

| Linear-class capability  | Native OpenAgents capability                       | OpenAgents advantage                                                  |
| ------------------------ | -------------------------------------------------- | --------------------------------------------------------------------- |
| Workspace                | Product Space                                      | Signed identities, portable projections, explicit authority           |
| Teams and subteams       | Teams and scoped workrooms                         | NIP-29-compatible room projection plus OpenAgents grants              |
| Initiatives and roadmaps | Initiative graph and roadmap views                 | Direct ProductSpec, promise, evidence, and receipt links              |
| Projects                 | Projects                                           | Native agent team, workroom, budget, verification, and release state  |
| Cycles and milestones    | Cycles and milestones                              | Typed timeboxes without treating dates as authority                   |
| Issues and subissues     | Work Items and subitems                            | Work packets, dependencies, acceptance, and execution lineage         |
| Dependencies             | Typed blocking graph                               | Causal work and verification gates, not text links                    |
| Documents                | Native Documents and Decisions                     | Versioned authority class, citations, signed projections              |
| Customer requests        | Customer Signals                                   | Private source provenance and product-impact links                    |
| Agent app users          | Agent Members                                      | Independent identity, owner attestation, runtime and capability truth |
| Assignee plus delegate   | Accountable Owner plus Agent Delegate              | Structural grant, budget, lease, verification, and disposition        |
| AgentSession             | Work Session                                       | Durable workroom/thread/run, placement, recovery, and receipts        |
| AgentActivity            | Work Activity                                      | Typed safe progress, question, action, result, error, and proof refs  |
| Skills                   | Skills                                             | One versioned source compiled to every runtime                        |
| Guidance                 | Guidance Bundles                                   | Explicit precedence, history, audience, and deny behavior             |
| Loops                    | Loops                                              | Typed triggers, budgets, approvals, circuit breakers, and replay      |
| Triage Intelligence      | Triage Engine                                      | Evidence/confidence proposals, optional admitted auto-apply           |
| Code Intelligence        | Code Context                                       | Repo-grant intersection, citations, symbols, commits, PRs, and tests  |
| Coding Sessions          | Coding Work Sessions                               | Local, Pylon, Agent Computer, managed, or attached runtime placement  |
| Diffs                    | Work Review                                        | Native Forge/Git projection beside intent, evidence, and verification |
| Guided Reviews           | Review Guides                                      | Regenerable evidence-linked chapters, never approval authority        |
| Reviews queue            | Review Inbox                                       | Priority from product graph, owner state, risk, and failed checks     |
| Pulse and Insights       | Product Pulse and Outcomes                         | Accepted-outcome, rework, cost, quality, and cycle-time metrics       |
| MCP                      | Typed API, generated client, CLI, and optional MCP | Durable contracts and receipts independent of model transcripts       |

### 5.2 Explicit non-capabilities

This port does not include:

- a Linear connector or import dependency;
- Linear-compatible OAuth, webhooks, API objects, URLs, or app users;
- a promise of pixel-identical UI;
- a copy of Linear's proprietary models or implementation;
- broad autonomous mutation before product contracts and assurance exist; or
- a Nostr relay treated as OpenAgents authorization or verification.

## 6. Native domain model

### 6.1 Product Space

`ProductSpace` is the top-level native work boundary.

```text
ProductSpace
  ref
  ownerScopeRef
  displayProfileRef
  memberPolicyRef
  defaultGuidanceBundleRef?
  defaultWorkroomRef
  initiativeRefs[]
  teamRefs[]
  projectRefs[]
  nostrIdentityRef?
  projectionPolicyRef
  createdAt
  archivedAt?
```

It is an OpenAgents object. A NIP-29 room or other Nostr address can project its
collaboration context, but does not create the Product Space or its authority.

### 6.2 Planning graph

The planning graph consists of:

- `Initiative` — strategic outcome with owners, evidence, projects, risks, and
  target horizons;
- `Project` — bounded deliverable with team, lead, status, scope, milestones,
  workroom, agents, budget, and verification policy;
- `Cycle` — a timebox and capacity view;
- `Milestone` — a meaningful project checkpoint;
- `WorkItem` — the basic unit of product intent, execution, and disposition;
- `WorkItemRelation` — parent, subitem, blocks, blocked-by, duplicates,
  relates-to, supersedes, or caused-by;
- `CustomerSignal` — private feedback or operational evidence linked to work;
- `Document` — versioned product or engineering context;
- `Decision` — an accountable choice with options, evidence, actor, and date;
  and
- `Label`, `State`, `Priority`, and `ServiceLevelPolicy` — typed configuration,
  not arbitrary display strings when they affect behavior.

### 6.3 Work Item

```text
WorkItem
  ref
  productSpaceRef
  teamRef
  projectRef?
  initiativeRefs[]
  cycleRef?
  milestoneRef?
  parentRef?
  relationRefs[]
  customerSignalRefs[]
  documentRefs[]
  title
  intentBodyRef
  stateRef
  priorityRef
  labelRefs[]
  accountableOwnerRef
  agentDelegateRef?
  activeWorkSessionRef?
  workroomRef
  threadRef
  repositoryRefs[]
  acceptanceContractRef?
  verificationPolicyRef?
  budgetPolicyRef?
  evidenceRefs[]
  resultRefs[]
  ownerDispositionRef?
  revision
  createdAt
  updatedAt
  archivedAt?
```

The body is not authority. Requirements, acceptance, grants, and verification
use their own typed records.

### 6.4 Accountable owner and agent delegate

The most important Linear behavior to port is the separation between the human
who remains accountable and the agent doing the current work.

```text
accountableOwnerRef  -> person or admitted accountable role
agentDelegateRef     -> current agent executor
delegationGrantRef   -> subject, tools, budget, time, disclosure, stop policy
workSessionRef       -> execution and activity
verificationRef      -> host or independent verification
ownerDispositionRef  -> accept, reject, revise, defer, or supersede
```

An agent is a first-class participant, but it does not become a legal person,
financial owner, release authority, employment principal, or accountable human.

### 6.5 Agent Member

```text
AgentMember
  ref
  profileRef
  ownerRef
  nostrPubkey?
  ownerAttestationRef?
  runtimeAdapterRefs[]
  capabilityRefs[]
  skillRefs[]
  allowedToolPolicyRef
  placementPolicyRef
  memoryPolicyRef
  budgetPolicyRef
  currentState
  activeSessionRefs[]
  activitySummaryRef
  revocationRef?
```

Agents appear in people and agent rosters, mentions, assignment pickers,
project membership, workroom presence, activity, filters, Inbox, and analytics.

### 6.6 Work Session

`WorkSession` is the native equivalent of the useful AgentSession concept.

```text
WorkSession
  ref
  workItemRef
  workroomRef
  threadRef
  runRef
  agentRef
  initiatingActorRef
  delegationGrantRef
  contextManifestRef
  planRef?
  placementRef
  requestedRuntimeRef
  effectiveRuntimeRef
  budgetRevision
  generation
  state
  activityCursor
  artifactRefs[]
  providerResultRef?
  verificationRef?
  ownerDispositionRef?
  blockerRefs[]
  startedAt
  terminalAt?
```

The session survives client disconnect and provider failure. Its state is not a
transient chat stream.

### 6.7 Work Activity

| Activity kind  | Meaning                                               | Privacy rule                                                    |
| -------------- | ----------------------------------------------------- | --------------------------------------------------------------- |
| `progress`     | Safe summary of current work                          | Never expose hidden chain of thought or raw provider trace      |
| `plan`         | Structured step or plan revision                      | Bind exact revision, actor, and reason                          |
| `elicitation`  | Question, approval request, or missing input          | Durable, deadline-aware, and answer-fenced                      |
| `action`       | Typed tool attempt and result                         | Include subject, grant, idempotency, and receipt refs           |
| `artifact`     | Commit, PR, report, screenshot, document, or evidence | Use digest and access-controlled reference                      |
| `result`       | Agent or provider result                              | Not verified or accepted by itself                              |
| `verification` | Test, review, replay, or oracle result                | Preserve producer and verifier distinction                      |
| `error`        | Typed failure or blocker                              | Distinguish retryable, denied, stale, interrupted, and terminal |
| `disposition`  | Human or admitted authority decision                  | Separate from agent output and verification                     |

## 7. Nostr-centric work graph

### 7.1 Core rule

Every important native collaboration object has:

- a stable OpenAgents ref;
- an owner and audience;
- a typed revision and causal parents;
- an optional signed Nostr event or addressable projection;
- exact source and admission refs;
- an OpenAgents canonical projection; and
- public-safe evidence and receipt refs where allowed.

Nostr is the default signed interoperability envelope for people, agents,
workrooms, messages, delegations, portable progress, code coordination, and
public-safe evidence. OpenAgents still evaluates whether an event has product
meaning.

### 7.2 Work graph shape

```text
Product Space identity
  ├── Team workrooms
  │   ├── Project workrooms
  │   │   ├── Work Item thread
  │   │   │   ├── accountable owner / agent delegate
  │   │   │   ├── plan and activity
  │   │   │   ├── questions and answers
  │   │   │   ├── code and artifact refs
  │   │   │   ├── review and verification
  │   │   │   └── owner disposition / closeout
  │   │   └── Project decisions and milestones
  │   └── Team documents and guidance
  ├── Initiatives and roadmap projections
  ├── Agent roster and signed profiles
  └── Inbox, search, and outcome projections
```

### 7.3 Protocol composition

Use standard NIPs before custom kinds. Do not depend on NIP-31 as the fallback
strategy; the current repository analysis records it as unrecommended.

| Need                           | Candidate protocol                             | Boundary                                                       |
| ------------------------------ | ---------------------------------------------- | -------------------------------------------------------------- |
| Event envelope and causal refs | NIP-01 and NIP-10                              | Signature proves key and bytes only                            |
| Team/project workrooms         | NIP-29                                         | Relay authority is room-scoped, not OpenAgents authority       |
| Private person/agent exchange  | NIP-17 over NIP-44 and NIP-59                  | Audience and product grants remain explicit                    |
| Addressable app data           | NIP-78 or another admitted addressable profile | Kind/profile selection requires collision review               |
| Handler discovery              | NIP-89                                         | Advertisement is not capability admission                      |
| HTTP proof                     | NIP-98                                         | One exact request, not a general session or permission         |
| Remote signer                  | NIP-46                                         | Signer consent is separate from product-action admission       |
| Relay preferences              | NIP-65                                         | Relay selection does not change product authority              |
| Expiration                     | NIP-40                                         | Best-effort, not guaranteed deletion                           |
| Code collaboration             | NIP-34                                         | Git objects, refs, checks, and merges remain Git-authoritative |
| Files and evidence             | Blossom with NIP-94 and NIP-92                 | Digest, access, retention, and audience remain required        |
| Agent ownership and auth       | Buzz NIP-OA and NIP-AA profiles                | Attestation and bounded auth, never ambient authority          |
| Agent persona                  | Buzz NIP-AP                                    | Persona is not a grant                                         |
| Owner-decryptable memory       | Buzz NIP-AE                                    | Separate admission; work text is not memory by default         |
| Live state and metrics         | Buzz NIP-AO and NIP-AM                         | Projections only, never billing or verification truth          |
| Agent task request/result      | NIP-90 or OpenAgents NIP-LBR                   | Host admits execution; result is not verification              |
| Read and attention state       | Buzz NIP-RS pattern                            | User-private projection, not work-item authority               |
| Reminders and push leases      | Buzz NIP-ER and NIP-PL patterns                | Canonical scheduler/delivery ledgers remain OpenAgents-owned   |

### 7.4 Event admission

```text
signed event received
  -> verify bytes, signature, kind/profile, timestamp, audience, and relay
  -> resolve hosted identity and owner scope
  -> resolve current Product Space, workroom, subject, and generation
  -> intersect product grant, role, tool policy, budget, and state transition
  -> admit or reject exact intent
  -> persist canonical ProductWorkEvent and projection transaction
  -> publish Khala Sync delta
  -> emit safe Nostr outcome/receipt projection
```

A relay acknowledgement is retained, but canonical state changes only after
OpenAgents admission.

### 7.5 Privacy

Public or third-party relays receive only public-safe material:

- opaque stable refs;
- public profiles and explicitly public work;
- state, causal, schema, revision, and digest fields;
- bounded public-safe progress;
- public Git refs; and
- dereferenceable public receipts.

Do not publish:

- private Product Space, work item, document, customer, or discussion bodies;
- raw prompts, model traces, chain of thought, provider payloads, or local
  histories;
- private repository source, paths, patches, logs, or artifacts;
- emails, phone numbers, customer identity, credentials, or account metadata;
- wallet, invoice, payment, or settlement material; or
- unrestricted bearer URLs.

Private work remains in the admitted OpenAgents store. Where Nostr carriage is
useful, use explicit audience encryption and still minimize content. NIP-29
`private` is relay access policy, not end-to-end encryption.

## 8. Native product features

### 8.1 Product planning

The system needs first-class views and commands for:

- initiatives and roadmap horizons;
- projects, leads, status, scope, risks, milestones, and progress;
- cycles, team capacity, carryover, and completion;
- work items, subitems, dependencies, duplicates, and supersession;
- documents, specifications, decisions, and comments;
- customer signals linked to product work without exposing customer identity;
- labels, priorities, service levels, and saved views; and
- exact history for every field and relation change.

Agent actions use the same commands as people, with a disclosed agent actor and
grant. No model writes storage directly.

### 8.2 Skills

A `SkillDefinition` is a versioned reusable work procedure:

```text
SkillDefinition
  ref
  ownerScopeRef
  revision
  sourceDigest
  intendedSubjects[]
  compatibleAgents[]
  requiredContext[]
  allowedToolPolicyRef
  expectedOutputSchemaRef
  stopConditions[]
  verificationPolicyRef
  budgetPolicyRef
  publicationState
```

One source compiles to Codex, Claude, ACP, native OpenAgents, and other admitted
runtimes. Harness text is a projection, not the skill's authority.

### 8.3 Guidance

`GuidanceBundle` stores durable organizational knowledge and operating policy.

Candidate precedence:

```text
repository invariants and admitted ProductSpec
  > owner policy
  > Product Space guidance
  > team guidance
  > project guidance
  > work-item instruction
  > skill defaults
  > agent defaults
```

The owning ProductSpec must select exact precedence. Deny conflicts fail
closed. A run records every Guidance revision it consumed. Natural language
cannot grant tools, budgets, repository access, release, or settlement.

### 8.4 Loops

`LoopDefinition` is a native recurring or event-driven agent workflow.

```text
LoopDefinition
  ref
  ownerScopeRef
  draftRevision
  publishedRevision
  triggerSpec
  conditionSpec
  agentRef
  skillRef?
  guidanceRefs[]
  toolPolicyRef
  approvalPolicyRef
  budgetPolicyRef
  concurrencyPolicyRef
  retryPolicyRef
  circuitBreakerRef
  selfTriggerExclusionRefs[]
  state
  nextEligibleAt?
  runHistoryRef
```

Supported triggers can include:

- work item created, changed, moved to triage, blocked, or stale;
- customer signal received;
- code check, deployment, incident, or monitoring event;
- work session completed, failed, or waiting;
- review requested or changes requested;
- scheduled time; and
- signed workroom event from an admitted actor.

Loops start in suggest/ask mode. Automatic mutation requires measured
confidence, exact tools, bounded subjects, budgets, circuit breakers, and an
admitted policy revision.

### 8.5 Triage Engine

The native Triage Engine proposes:

- team and project;
- duplicate relationships;
- labels and priority;
- accountable owner and possible agent delegate;
- issue type and service-level policy;
- related customer signals, documents, decisions, code, and prior work;
- whether the item is ready for an agent investigation; and
- required questions when context is insufficient.

```text
new native work item
  -> authorized context retrieval
  -> classification and duplicate candidates
  -> confidence + evidence refs
  -> human disposition or admitted auto-apply policy
  -> canonical mutation + receipt
```

The system measures proposal acceptance, correction, false routing, and
downstream outcome. Explanation is not proof.

### 8.6 Code Context

Code Context links product intent to repositories without making repository
access ambient.

```text
effective code access
  = owner grant
  ∩ Product Space and project policy
  ∩ repository-host permission
  ∩ work-item repository refs
  ∩ path/tool policy
  ∩ runtime containment
```

Answers return exact file, symbol, commit, PR, issue, test, and evidence refs.
Search does not grant mutation. Generated summaries retain source revisions and
can be invalidated when code changes.

### 8.7 Coding Work Sessions

A work item can launch a native coding session on:

- an owner-local Pylon;
- an admitted Agent Computer;
- an admitted managed environment;
- an attached existing agent preserving its home and credentials; or
- another separately admitted runtime lane.

The session records:

- requested and effective agent, runtime, model, account, and placement;
- repository, branch, pinned commit, worktree, and containment;
- context manifest, Skills, Guidance, tools, grants, and budget;
- plan, progress, questions, actions, artifacts, and failures;
- provider result separately from host verification;
- PR and review refs; and
- owner disposition and terminal closeout.

No private runtime data is copied into a public work event merely to make the
session visible.

### 8.8 Work Review and Guided Reviews

The review surface places code beside the native work item and product context.

It includes:

- changed files and structural diff views;
- commit, branch, PR, check, review, and merge state from the Git authority;
- work-item intent, project, customer signals, documents, and decisions;
- agent plan, actions, evidence, tests, screenshots, and verification;
- inline comments, replies, reactions, approvals, and change requests under
  exact user authority;
- agent follow-up for bounded fixes; and
- an evidence-linked Guide that groups changes by implementation purpose.

A Guide is a regenerable explanation. It never approves, verifies, or merges.
Git objects and refs remain canonical in Git or OpenAgents Forge.

### 8.9 Inbox and notifications

The Inbox is an attention projection, not another work store.

Group attention by stable subject and reason:

- assigned to me;
- delegated agent active;
- waiting for my answer or approval;
- mentioned;
- blocked or stale;
- agent failed or exceeded budget;
- verification completed or disagreed;
- review requested or checks failed;
- decision required;
- Loop paused or circuit-broken; and
- signer, relay, Sync, runtime, or repository degraded.

Support “by people,” “by agents,” “high-risk,” and “accepted outcome” filters so
automation does not bury human work.

### 8.10 Product Pulse and Insights

Measure outcomes rather than raw agent activity:

- attempted versus accepted work sessions;
- time from intake to triage, start, verified result, and accepted outcome;
- human review minutes;
- rework, rejection, rollback, and incident rate;
- cost and tokens per accepted result;
- agent contribution by task class;
- Loop proposal and auto-apply precision;
- blocked time and elicitation response time;
- cycle and project outcome accuracy; and
- verification disagreement and escaped-defect rates.

NIP-AM and provider counters can enrich views but never replace exact ledgers.

### 8.11 API, CLI, and MCP

The native contract should generate:

- TypeScript clients;
- headless transport;
- Pylon JSON commands;
- web/mobile/Desktop protocol bindings; and
- an optional MCP adapter.

MCP is a client of the native contract, not the product's storage or authority
boundary. Each command binds actor, Product Space, exact subject, operation,
expected revision, grant, budget, and idempotency key.

## 9. Service architecture

```text
Native clients and signed Nostr events
  - Omega / Desktop / web / mobile / Pylon / generated client
  - people, agents, workrooms, messages, delegations, reviews
                     |
                     v
Product Work command gateway
  - identity and audience resolution
  - grant, role, revision, transition, tool, and budget admission
                     |
                     v
Cloud SQL canonical Product Work event + projection transaction
  - spaces, planning graph, work items, sessions, activities
  - Skills, Guidance, Loops, triage, attention, review, outcomes
          |                              |
          v                              v
Khala Sync                         Durable Nostr outbox
  - desktop/web/mobile             - signed safe projections
  - snapshots/deltas/cursors       - retry and relay receipts
  - live agent graph               - multi-relay loss accounting
          |
          v
Agent execution and code
  - native runtime / attached agent / Pylon / Agent Computer
  - Forge or external Git authority
  - evidence, verification, owner disposition
```

### 9.1 Canonical event families

Candidate typed event families:

- Product Space created, member changed, policy changed, archived;
- initiative/project/cycle/milestone created or revised;
- work item created, classified, related, assigned, delegated, moved, closed,
  reopened, superseded, or archived;
- document/decision/customer-signal linked or unlinked;
- session admitted, started, progressed, waiting, steered, stopped, failed,
  provider-completed, verified, accepted, rejected, or superseded;
- Skill/Guidance/Loop drafted, published, paused, revoked, or archived;
- triage proposed, accepted, edited, rejected, or auto-applied;
- review requested, commented, approved, changes requested, or merged by the
  owning Git authority; and
- receipt/evidence added, invalidated, or superseded.

Each event has actor, subject, owner scope, audience, schema version, expected
revision, idempotency key, causal parents, source refs, and public-safe result.

### 9.2 Projection law

All clients derive from the same accepted event sequence and cursor. Optimistic
UI can show a pending intent, but it cannot show a mutation as confirmed before
the canonical event is admitted. Reconnect must report gaps, truncation, stale
generation, or partial history explicitly.

## 10. First-class user experience

### 10.1 Omega target

Omega is the future native primary surface:

```text
Navigation
  Inbox
  My work
  Initiatives
  Projects
  Cycles
  Reviews
  Teams / workrooms
  People / agents

Center
  work item / project / document / decision / diff / guide / artifact

Agent panel
  active Work Session
  plan / progress / questions / actions / artifacts / controls

Inspector
  owner / delegate / grants / revisions / evidence / verification / receipts
  signed Nostr identities and event refs
```

Buzz UI lessons to preserve:

- nested workroom tree;
- drag, drop, and reparent under typed commands;
- unread, mentions, questions, and blocked badges;
- fuzzy person/agent picker;
- explicit membership and capability inspector;
- presence and runtime health;
- compact thread and review navigation; and
- keyboard-first creation, assignment, delegation, and triage.

### 10.2 Current Desktop proof surface

Before Omega cutover, current Desktop can prove the shared contracts:

- product-space and project navigation;
- work-item list and detail;
- human owner and agent delegate;
- Work Session activity, elicitation, controls, evidence, and disposition;
- diff, Guide, checks, verification, and receipts; and
- Sync, runtime, signer, and relay degradation.

This is migration evidence, not a decision to keep two desktop products.

### 10.3 Web

Web can supply authenticated planning, work-item, document, Inbox, review,
agent, Loop, Guidance, and administration views only after the web ProductSpec
admits the route and information architecture. This document does not create a
new public route.

### 10.4 Mobile

Mobile uses the same contracts for:

- Inbox and project status;
- work-item read and edit;
- answer, approve, reject, steer, pause, and stop;
- agent/session visibility;
- review and Guide reading;
- lightweight diff and artifact inspection; and
- signed handoff to desktop or Omega.

Mobile is not an independent authority plane.

## 11. Security and reliability

### 11.1 Untrusted work content

Work-item bodies, comments, documents, customer signals, attachments, Nostr
events, imported logs, and code are untrusted data. They can contain prompt
injection.

The context compiler labels source, actor, audience, revision, and retrieval
time. It separates data from authority-bearing instructions and rejects any
attempt to widen tools, repositories, budgets, identities, disclosure,
release, or settlement.

### 11.2 Permission intersection

```text
effective action authority
  = hosted identity and owner scope
  ∩ Product Space membership and role
  ∩ exact subject grant
  ∩ agent delegation grant when applicable
  ∩ tool and transition policy
  ∩ repository or external-system permission when applicable
  ∩ current generation and expected revision
  ∩ remaining time and budget
```

A Nostr signature or workroom membership is additional evidence, never a
replacement for this intersection.

### 11.3 Idempotency and concurrency

- Every command has an app-owned idempotency key.
- Every mutation checks expected revision or generation.
- Duplicate signed events do not duplicate work.
- Reused ids with changed payloads are conflicts.
- One Work Session generation owns an active delegate lease.
- A replacement or retry produces an explicit successor relation.
- Loops exclude their own output classes and enforce concurrency limits.

### 11.4 Memory

Product work does not automatically become durable agent memory. Memory
promotion requires an admitted memory policy, owner-readable representation,
source refs, audience, retention, revocation, and tombstone behavior. Derived
embeddings and graphs remain rebuildable projections.

### 11.5 Deletion and retention

Native deletion must purge or tombstone canonical and derived OpenAgents state
according to policy, revoke dependent grants, rebuild search and memory, and
record what cannot be erased. Nostr events copied by independent relays or
peers may survive; the product must never promise global erasure it cannot
prove.

### 11.6 Failure honesty

- Relay down: canonical OpenAgents work continues; signed projection is queued
  or marked degraded.
- Sync gap: client shows last confirmed cursor and gap state.
- Agent crash: session becomes interrupted or stalled with recovery options.
- Provider complete but tests fail: provider result stays unverified.
- Verification disagrees: show both records and require disposition.
- Git unavailable: do not invent PR, check, review, or merge state.
- Signer unavailable: retain unsigned draft; do not silently change identity.
- Budget exhausted: pause or stop under the declared policy.
- Owner absent: follow the exact timeout/escalation rule; never infer approval.

## 12. Ordered native implementation program

These are candidate packets, not admitted implementation issues.

### OAW-00 — Product Work authority and schema freeze

Define the native domain vocabulary, event families, state machines, role and
grant matrix, audience classes, revisions, idempotency, projection rules,
retention, and receipt semantics.

Exit: conformance fixtures fail closed on unknown actors, subjects, roles,
events, transitions, revisions, audiences, and authority classes.

### OAW-01 — Product Space and planning graph

Implement Product Space, teams, initiatives, projects, cycles, milestones,
work items, subitems, relations, labels, documents, decisions, and customer-
signal refs in Cloud SQL and Khala Sync.

Exit: desktop and a headless client create, edit, relate, archive, reconnect,
and replay the same graph without optimistic confirmed state.

### OAW-02 — signed identity and workrooms

Bind people and Agent Members to explicit identities, add team/project
workrooms and work-item threads, implement the safe Nostr projection outbox,
and preserve relay-qualified cursors and loss accounting.

Exit: two authorized clients rebuild the same signed causal thread; relay loss
does not change canonical OpenAgents state or leak private content.

### OAW-03 — owner, delegate, Work Session, and Work Activity

Implement structural accountable owner, agent delegate, delegation grant,
session, plan, safe activities, elicitation, controls, provider result,
verification, and owner disposition.

Exit: one native work item completes a no-spend owner-local agent journey with
interrupt, resume, stale generation, denied action, and verification failure.

### OAW-04 — Skills and Guidance

Implement versioned Skill definitions, runtime compilation, Guidance Bundles,
precedence, conflict handling, history, audience, and run manifests.

Exit: every run proves the exact revisions it used; higher-authority deny
cannot be overridden by lower natural-language instruction.

### OAW-05 — Inbox, search, and agent roster

Implement attention reasons, grouping, unread/mentions/questions/reviews,
person/agent filters, structured authorized search, agent cards, presence, and
runtime health.

Exit: a user can find every item requiring action without scanning raw chat or
provider logs, and no private scope crosses search results.

### OAW-06 — Triage Engine

Implement duplicate, team, project, label, priority, owner/delegate, service-
level, and readiness proposals with confidence, evidence, disposition, and
measurement.

Exit: suggest/ask mode is usable and measurable; low-confidence or conflicting
proposals never mutate automatically.

### OAW-07 — Loops

Implement immutable draft/published revisions, event and schedule triggers,
conditions, Skills, Guidance, approvals, budgets, circuit breakers, run
history, self-trigger exclusion, pause, drain, revoke, and archive.

Exit: duplicate events, output echoes, retry storms, and external outages do
not create duplicate or unbounded work.

### OAW-08 — Code Context and Coding Work Sessions

Bind work items to exact repositories, commits, worktrees, context manifests,
runtimes, agents, budgets, tests, artifacts, and PRs. Integrate Pylon, Agent
Computer, attached agents, and native verification.

Exit: one native work item reaches a verified PR through an exact pinned
repository task without private runtime leakage or false completion.

### OAW-09 — Work Review, diffs, and Guided Reviews

Implement issue-context diff review, checks, comments, change requests,
approvals, agent follow-up, evidence-linked Guides, verification, and merge
disposition under Git authority.

Exit: reviewers can move from intent to changed code, proof, feedback, agent
revision, verification, and merge without losing actor or authority identity.

### OAW-10 — Product Pulse, cross-client parity, and live acceptance

Implement accepted-outcome analytics and the admitted Omega, web, mobile, and
Pylon surfaces. Exercise recovery, privacy, revocation, relay outage, Sync gap,
agent failure, budget exhaustion, Git drift, and multi-client control.

Exit: one user can capture work, delegate it, observe/steer from another
client, review verified code, and close the native work item with one canonical
history and signed portable evidence.

## 13. First proof to build

The first proof should demonstrate the corrected objective:

> In OpenAgents, a person creates a native Product Space, project, and work
> item. The person remains accountable and delegates the item to a native Agent
> Member. One signed workroom thread carries safe plan, progress, a question,
> answer, artifact, and closeout projections. An owner-local Pylon performs a
> no-spend fixture against a pinned repository. OpenAgents records provider
> completion, host verification, and human acceptance separately. Desktop and
> another client converge through Khala Sync. Relay outage, duplicate events,
> and revoked authority do not duplicate work or leak private material.

This proof uses no Linear service, account, API, token, webhook, object, or
runtime.

## 14. Acceptance and falsification matrix

| Claim                                       | Required proof                                                                                 | Falsifier                                                             |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Product work is native                      | All objects and commands exist and operate without Linear                                      | Any user journey requires a Linear account, API, object, or connector |
| Human accountability remains explicit       | Owner and agent delegate remain distinct through execution and review                          | Agent replaces or obscures the accountable person                     |
| Agents are first-class members              | Signed identity, profile, capabilities, session, activity, filters, and history                | Agent is only an invisible tool invocation                            |
| Nostr is central but not authority-confused | Signed causal rebuild plus separate admission records                                          | Relay event alone creates canonical work, verification, or acceptance |
| Sessions are durable and honest             | Disconnect, crash, resume, provider result, verification, and disposition tests                | Provider “done” appears as accepted work                              |
| Planning graph is complete                  | Projects, cycles, work items, dependencies, documents, decisions, and signals replay correctly | Critical product context survives only in prose or chat               |
| Skills and Guidance are reproducible        | Exact revisions and precedence appear in run manifest                                          | Runtime prompt drift changes behavior without a revision              |
| Loops are bounded                           | Budget, concurrency, replay, echo, circuit-breaker, pause, and revoke tests                    | One event produces recursive or unbounded runs                        |
| Triage is evidence-based                    | Proposal confidence, source refs, disposition, and outcome measurement                         | Unsupported low-confidence mutation occurs                            |
| Git authority is preserved                  | Commit, PR, check, review, and merge reconcile from Git authority                              | Work UI or Guide invents canonical Git state                          |
| Private content stays private               | Event, prompt, log, receipt, relay, search, memory, and export redaction tests                 | Private body, token, path, prompt, or source appears publicly         |
| Clients share one model                     | Desktop, Omega target, web/mobile target, Pylon, and Nostr projection converge                 | A client invents independent state or command authority               |

## 15. Current OpenAgents substrate and gaps

### 15.1 Landed substrate

- Cloud SQL and Khala Sync canonical projection, snapshots, deltas, cursors,
  conversations, attention, sessions, and live agent graphs;
- durable workroom packets, leases, evidence, verification refs, and owner
  disposition;
- stable agent profiles, safe agent cards, agent registration, and runtime
  adapters;
- background-agent budgets, tool policy, trigger, event-ledger, and circuit-
  breaker foundations;
- Pylon, owner-local execution, managed environment, Agent Computer, and
  attached-agent paths;
- Forge, Git admission, NIP-34 collaboration, and Git-object authority;
- NIP-29 public chat mechanics, NIP-42 auth, remote signer restrictions,
  reconnect, pagination, and relay identity checks;
- NIP-LBR ref-only requests, results, and content-addressed closeout receipts;
- selected owner-attested agent identity, owner-decryptable memory, persona,
  live-state, and metric protocol implementations; and
- current Desktop, mobile, web, and future Omega surface contracts.

### 15.2 Missing native product

- Product Space and team domain;
- initiative, project, cycle, milestone, work-item, relation, label, document,
  decision, customer-signal, and service-level schemas;
- complete native planning command and projection families;
- structural accountable owner and agent delegate on native work;
- Work Session and Work Activity product contracts;
- first-class Agent Member roster and project participation;
- versioned Skills and Guidance product surfaces;
- complete Loop builder, revisions, history, and safety controls;
- native Triage Engine and confidence/evidence measurement;
- issue-linked Code Context and coding-session composition;
- native diff, review queue, and Guided Review experience;
- Product Pulse and accepted-outcome analytics;
- complete signed Product Work Nostr profile and safe projection outbox;
- native Omega/web/mobile/Desktop/Pylon parity;
- end-to-end live acceptance and AssuranceSpec; and
- admitted roadmap packets and implementation issues.

## 16. Hard rejections

- No Linear integration, dependency, connector, import requirement, or source
  of truth.
- No attempt to make OpenAgents a thin client for Linear.
- No copying Linear source code, proprietary assets, prompts, or branding.
- No separate Buzz application or revival of the canceled Buzz deployment.
- No second Forum or standalone Forge.
- No relay as OpenAgents membership, command, work-item, execution,
  verification, receipt, release, payment, or settlement authority.
- No NIP-29 `private` claim presented as end-to-end encryption.
- No agent-as-accountable-human model.
- No hidden agent identity or ambient service token.
- No provider result, agent activity, Nostr event, or Guide treated as proof of
  completion.
- No natural-language Guidance treated as a capability grant.
- No broad repository or Product Space access inferred from a work-item body.
- No autonomous broad triage or Loops before suggest-mode evidence and bounded
  controls.
- No raw private work content, prompts, source, credentials, or payment data on
  public relays or receipts.
- No claim of global Nostr deletion.
- No public parity claim until the ProductSpec and AssuranceSpec prove it.

## 17. Final recommendation

Replace the mistaken integration framing completely with a native OpenAgents
product program.

Linear has assembled the strongest current vertical feature set for product
context, accountable agent delegation, shared sessions, recurring workflows,
coding, and review. Buzz demonstrates how collaboration can become a signed,
portable, agent-participatory Nostr graph. OpenAgents already owns the deeper
execution and authority substrate: multi-agent runtimes, workrooms, placement,
budgets, grants, receipts, verification, Sync, Forge, Pylon, and Agent Computer.

The correct move is to combine those lessons into OpenAgents' own system:

```text
Linear-class product functionality
  + Buzz-class Nostr collaboration
  + OpenAgents execution, authority, verification, and receipts
  = native OpenAgents Work
```

Build the native proof in section 13 first. It must work with Linear completely
absent.

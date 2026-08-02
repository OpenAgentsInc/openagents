# Omega and OpenAgents glossary

- Status: canonical vocabulary
- Owner: OpenAgents
- Date: 2026-08-02
- Scope: current contracts and target product architecture

This document is the single vocabulary for Omega and the OpenAgents product
system that Omega presents. It normalizes the current repository models and the
target Product Work, collaboration, coding, execution, identity, sync, and
interoperability models into OpenAgents-owned terms.

The glossary is a naming authority, not an implementation-status authority.
Code, tests, ProductSpec, AssuranceSpec, the Omega roadmap, the Sol roadmap,
admitted work packets, and receipts remain authoritative for behavior and
delivery state.

## 1. Status marks

Each term has one or more status marks:

| Mark | Meaning |
| --- | --- |
| **Current** | The term names a landed contract, data model, or supported product behavior. It does not claim that every intended surface is complete. |
| **Target** | The term is the chosen native name for an envisioned model or capability. It must not be presented as shipped without implementation evidence. |
| **Boundary** | The term states an authority, privacy, safety, or lifecycle distinction that current and target implementations must preserve. |
| **Interop** | The term is an optional wire or federation concept. It does not replace OpenAgents command, work, receipt, or settlement authority. |
| **Avoid** | The term is ambiguous, misleading, or retired in this vocabulary. Use the named replacement. |

When a term is both **Current** and **Target**, its core meaning exists now and
its product reach is expected to grow.

## 2. Canonical model spine

The target product-work relationship is:

```text
Product Space
├── Team
├── Initiative
│   └── Project
│       ├── Cycle
│       ├── Milestone
│       └── Work Item
│           ├── Work Item Relation
│           ├── Customer Signal
│           ├── Document / Decision
│           └── Workroom
│               └── Thread
│                   └── Work Session
│                       └── Run
│                           └── Work Activity
│                               ├── Artifact / Evidence
│                               ├── Verification
│                               └── Disposition
└── Product Pulse / Outcome Metric
```

The Omega application relationship is separate:

```text
App → Window → Workspace → Pane Group → Pane → Item
                              │
                              ├── Agent Panel / Thread View
                              ├── Project Panel / Editor / Terminal
                              └── Workroom / Product Work views

IDE Project Graph → Work Context → Project Root → Worktree → Project File Ref
Engine → Host Device → Placement → Agent Runtime → Session → Run → Turn
```

The separation is intentional. A product `Project` is not an IDE Project Graph;
a product `Work Item` is not a pane `Item`; a durable `Thread` is not a bounded
`Session`; and a filesystem `Worktree` is not a product `Workspace`.

## 3. Core naming rules

1. Use **Product Space** for the organizational root of product work. Use
   **Workspace** only for the Omega window/workbench container.
2. Use **Project** for a product outcome with scope and dates. Use **IDE Project
   Graph** for the editor's capability graph over repositories and roots.
3. Use **Work Item** for planned product work. Use **Item** only for a pane-hosted
   UI object.
4. Use **Thread** for durable conversational or causal context. Use **Session**
   for bounded interaction or execution. Use **Run** for one governed execution
   lifecycle inside a session.
5. Use **Workroom** for a collaboration container whose membership, threads,
   work, and agent participation are viewed together. A relay group can project
   a workroom but does not define its OpenAgents authority.
6. Use **Command Intent** for a requested mutation and **Command Outcome** for
   the durable result. Never call transport acceptance an outcome.
7. Use **Canonical State** for authoritative OpenAgents state and **Projection**
   for a derived read model. Signed events, local caches, and UI state are not
   canonical merely because they are durable.
8. Use **Agent Delegate** for an agent assigned to accountable work. An agent is
   never the human accountable owner by implication.
9. Use **Evidence** for produced proof material, **Verification** for an admitted
   evaluation of evidence, and **Owner Disposition** for the owner's separate
   acceptance or waiver decision.
10. A **Capability** describes what an actor or runtime can do. A **Grant**
    describes what it may do in a named scope. Capability never implies grant.

## 4. Product work and collaboration

| Term | Status | Definition |
| --- | --- | --- |
| **Product Work** | Target | The native OpenAgents system for planning, assigning, executing, reviewing, and learning from product work. It includes human and agent participation without making an external tracker the system of record. |
| **Product Space** | Target | The top-level tenant or organization container for teams, initiatives, projects, policies, members, and product knowledge. It is not an Omega Workspace. |
| **Team** | Target | A stable group of people and agent members with a shared work scope, workflow policy, and ownership boundary. |
| **Initiative** | Target | A strategic collection of projects and outcome metrics. It connects roadmap intent to delivery without serving as an execution session. |
| **Roadmap** | Current, Target | An ordered statement of intended outcomes and sequencing. A roadmap supplies priority context; it does not by itself grant implementation, release, spend, or external-action authority. |
| **Project** | Target | A bounded product outcome with owner, team, scope, dates, milestones, work items, documents, and status. It is distinct from an IDE Project Graph. |
| **Project Status** | Target | A time-stamped health and progress statement for a Project, with author, narrative, risk, and optional outcome metrics. |
| **Cycle** | Target | A time-boxed planning and delivery interval for a team. Work Items can join a Cycle without changing their identity. |
| **Milestone** | Target | A named checkpoint within a Project that groups expected Work Item completion or outcome evidence. |
| **Work Item** | Target | The canonical unit of product work. It has identity, title, description, state, priority, team, accountable owner, delegates, relations, labels, dates, and activity. |
| **Subitem** | Target | A Work Item whose parent relation decomposes another Work Item. It remains independently addressable and auditable. |
| **Work Item Relation** | Target | A typed edge between Work Items, such as parent, blocks, blocked-by, related, duplicate, or supersedes. Relations do not silently mutate either endpoint. |
| **Work State** | Target | The admitted lifecycle state of a Work Item under a team's workflow. State labels are policy data, not executable authority. |
| **Priority** | Target | The explicit ordering importance of a Work Item. Priority does not override safety, authority, or dependency gates. |
| **Label** | Target | A scoped classification attached to work, projects, or customer signals. Labels support discovery and automation but do not grant capabilities. |
| **SLA Policy** | Target | A policy that defines response or resolution expectations for a class of work. Timers derived from it are projections over canonical timestamps. |
| **Customer Signal** | Target | A normalized request, complaint, observation, or opportunity linked to product work. It preserves source and consent boundaries and is not itself a commitment. |
| **Document** | Current, Target | Durable authored product knowledge with identity, revision, authorship, and links to work. In the editor, the same word can name the text model behind a Buffer; context must make the layer clear. |
| **Decision** | Target | A durable record of a choice, its decision maker, context, alternatives, rationale, date, and affected work. A discussion message is not a Decision until recorded as one. |
| **Outcome Metric** | Target | A named measure used to judge an Initiative or Project outcome. It must carry definition, source, time window, and truth limitations. |
| **Product Pulse** | Target | A derived overview of projects, work flow, risks, customer signals, and outcome metrics. It is a projection, never a second product-work authority. |
| **Accountable Owner** | Target, Boundary | The human or explicitly admitted organizational role answerable for a Work Item or Project. Delegation does not remove this accountability. |
| **Agent Delegate** | Target | An Agent Member assigned bounded work under a Delegation Grant. It can execute and report within scope but does not inherit owner authority. |
| **Delegation Grant** | Target, Boundary | A revocable, scoped authorization that binds a principal, target work, allowed actions, constraints, issuer, validity, and evidence requirements. |
| **Agent Member** | Current, Target | An agent represented as a first-class participant in a Product Space, Team, or Workroom, with identity, owner relation, capabilities, and explicit grants. |
| **Membership** | Current, Target | A scoped relation between a principal and a Product Space, Team, Workroom, or interoperable group. Membership and role labels do not imply arbitrary command capability. |
| **Role** | Current, Target | A named bundle of expected responsibilities or presentation labels. Effective authority still comes from policy and grants. |
| **Workroom** | Current, Target | A collaboration container that combines members, agents, threads, work context, decisions, activity, and optional voice or code surfaces. Its canonical coordinate must identify both OpenAgents scope and any interoperable room. |
| **Community Workroom** | Current | A membership-scoped Workroom for invited people and their attested agents. It separates community coordination from private owner-agent work. |
| **Private Owner Workroom** | Current, Boundary | The private Workroom between an owner and owned agents. Private memory, credentials, raw traces, and owner-only controls stay here unless separately disclosed. |
| **Two-Room Rule** | Current, Boundary | The rule that private owner-agent work and community collaboration use distinct Workrooms, membership, disclosure, and evidence paths. |
| **Channel** | Target | A named topical lane within a Workroom. It organizes Threads but does not create a separate work or command authority. |
| **Thread** | Current, Target | A durable ordered context for messages, events, decisions, and sessions. A Thread can outlive many runtime Sessions and can be viewed on multiple devices. |
| **Work Item Thread** | Target | The primary Thread attached to a Work Item. It holds discussion and activity views while the Work Item remains the canonical state object. |
| **Thread Disclosure Intent** | Current, Boundary | A typed request to change who can read a Thread projection. It names the expected visibility revision, audience, administrator access, actor, and idempotency key. |
| **Thread Disclosure Receipt** | Current, Boundary | The durable result of a Thread Disclosure Intent, including the applied visibility version and exact target. It is required before publication or cross-surface disclosure. |
| **Thread Visibility Target** | Current, Boundary | The exact audience and administrator-access policy for a Thread, such as owner-only, internet-readable, workspace members, or a named group. It is not inferred from where a link appears. |
| **Thread Event Authority Relation** | Current, Boundary | A typed relation that records when one Thread event supersedes or reverts another. It preserves terminal authority lineage for export and replay. |
| **Message** | Current | A user, agent, or system-authored conversational record in a Thread. Messages can reference activities and artifacts; they do not replace those structured records. |
| **Reaction** | Interop, Target | A lightweight response to a message or event. It is presentation and attention data, not approval or owner disposition. |
| **Read State** | Current, Interop | A principal-scoped record of what has been seen in a Thread or Workroom. It is private by default and must not become work-state authority. |
| **Presence** | Current, Interop | Ephemeral availability or activity information for a principal or device. Presence can expire and must never prove durable completion. |
| **Attention Item** | Current, Target | A derived item that asks a principal to review, answer, approve, recover, or notice something. It retains source and required-action references. |
| **Attention Inbox** | Current, Target | The principal-scoped projection of Attention Items across runs, interactions, work, and collaboration. |
| **Review Inbox** | Target | The queue of work, artifacts, decisions, or agent results awaiting review. It is a specialized attention projection, not an approval ledger. |
| **Reminder** | Current, Interop | A principal-scoped scheduled attention record. Triggering a reminder does not trigger work unless a separate command is admitted. |

## 5. Agent work, sessions, and review

| Term | Status | Definition |
| --- | --- | --- |
| **Work Session** | Target | A bounded period in which one or more principals act on a Work Item under explicit context and grants. It links Runs and Work Activities back to the work. |
| **Run** | Current, Target | One governed execution lifecycle with identity, actor, objective, state, timestamps, bindings, outputs, and receipts. A Run belongs to a Session or other admitted work context. |
| **Work Activity** | Target | An append-only, typed record of meaningful progress within a Work Session. Activities form the auditable timeline without overloading chat messages. |
| **Progress Activity** | Target | A Work Activity that reports current progress, completed steps, remaining work, or a blocker. |
| **Plan Activity** | Target | A Work Activity that proposes or revises an execution plan. A proposed plan is not accepted work authority. |
| **Elicitation Activity** | Target | A Work Activity that asks for missing information or a decision and records the answer relation. |
| **Action Activity** | Target | A Work Activity that records an attempted or completed bounded action and its Command Outcome reference. |
| **Artifact Activity** | Target | A Work Activity that publishes or updates an Artifact reference. |
| **Result Activity** | Target | A Work Activity that summarizes a completed result and binds it to outputs and evidence. |
| **Verification Activity** | Target | A Work Activity that records an admitted verification attempt and its receipt or failure. |
| **Error Activity** | Target | A Work Activity that records a typed failure, affected scope, retry posture, and recovery reference. |
| **Disposition Activity** | Target | A Work Activity that records an authorized acceptance, rejection, waiver, cancellation, or supersession. |
| **Artifact** | Current, Target | A durable output of work, such as a file, patch, build, report, image, recording, or content-addressed blob. It carries provenance and does not prove its own correctness. |
| **Attachment** | Current | A file or content reference attached to a Message, Session, Work Item, or Artifact. Attachment identity and access are separate from display metadata. |
| **Skill Definition** | Current, Target | A versioned description of a reusable agent procedure, its triggers, inputs, boundaries, and verification. Availability is not execution authority. |
| **Guidance Bundle** | Target | Versioned product, team, repository, or workflow instructions supplied to an agent in a known precedence order. |
| **Loop Definition** | Target | A bounded recurring workflow with trigger, selection rule, stop conditions, budget, escalation, and evidence policy. |
| **Trigger** | Target | A typed event or schedule that makes a workflow eligible to run. Eligibility still requires admission and capacity. |
| **Triage Engine** | Target | A service that classifies and proposes routing, priority, labels, ownership, and duplicates for incoming work. It emits proposals, not silent authoritative mutations. |
| **Triage Proposal** | Target | A reviewable suggested change from a Triage Engine, with reasons, confidence, source facts, and expiry. |
| **Code Context** | Target | A structured link from product work to repositories, revisions, roots, files, symbols, diffs, checks, and coding policy. |
| **Coding Work Session** | Target | A Work Session specialized for repository work. It binds Work Items, Code Context, Worktrees, coding Sessions, Runs, reviews, and resulting Artifacts. |
| **Work Review** | Target | A structured evaluation of a Work Item result against requested outcomes, criteria, policy, and evidence. |
| **Review Guide** | Target | A versioned checklist or rubric for a Work Review. It guides the reviewer but cannot manufacture evidence or independence. |
| **Review Verdict** | Target | The reviewer's typed conclusion, such as approved, changes requested, rejected, or inconclusive, with evidence references. It is distinct from release authority. |
| **Blocker** | Current, Target | A typed condition that prevents admitted progress. It identifies scope, evidence, owner, and the next falsifiable recovery step when known. |
| **Reason Ref** | Current, Target | A stable reference to a structured reason, blocker, policy, or disposition explanation. It is preferred over unbounded status prose in machine contracts. |

## 6. Identity, principals, and agent representation

| Term | Status | Definition |
| --- | --- | --- |
| **Principal** | Current, Boundary | Any authenticated actor that can be the subject of identity, membership, policy, or grants: a person, agent, service, or device. |
| **Person** | Current, Target | A human principal. Product copy may say member or owner when the scoped role is more useful. |
| **Agent** | Current, Target | A software principal that can converse, use tools, execute work, and emit evidence under bounded authority. It is not synonymous with a model or provider. |
| **Actor** | Current | The principal or system component recorded as performing an action or transition. Actor identity must be specific enough for audit. |
| **Owner** | Current, Boundary | The principal with an admitted ownership relation over an agent, account, work context, or resource. Ownership does not bypass reserved actions or policy. |
| **Agent Owner Relation** | Current, Boundary | The verified relation binding an agent identity to its owner and any conditions. Self-asserted ownership is insufficient. |
| **Account** | Current | An authenticated OpenAgents user boundary with session, entitlements, and data scope. It is distinct from a public key and from a provider account. |
| **Application Identity** | Current | The installed application's signed identity, channel, bundle, and local data boundary. It is separate from the user's account and signing keys. |
| **Nostr Identity** | Current, Interop | A public-key identity used to sign interoperable records. It can represent a person or agent but does not by itself establish OpenAgents account or command authority. |
| **Public Key** | Current, Interop | The public identifier used to verify a signature. A public key is not a display profile, account, membership, or grant. |
| **Signer** | Current, Interop | A component authorized to produce signatures for a specific key under a custody policy. |
| **Sealed Signer** | Current | A signer whose secret material remains inside the protected local custody boundary and is not exposed to UI or ordinary application state. |
| **Remote Signer** | Current, Interop | A separately controlled signer reached through an authenticated signing protocol. The caller receives signatures, not raw secret material. |
| **Device Grant** | Current, Boundary | A revocable authorization for a named device to use a bounded account, signer, sync, or command capability. |
| **Owner Attestation** | Current, Interop | A signed statement that binds an agent key to an owner key and conditions. It supports verification but does not grant every OpenAgents action. |
| **Agent Authentication** | Current, Interop | Proof that an agent presenting to a relay or service holds an identity with an acceptable owner relation and conditions. Authentication is not authorization. |
| **Agent Profile** | Current, Target | The durable OpenAgents record of an agent's identity, owner relation, presentation, declared capabilities, runtime bindings, and policy references. |
| **Agent Card** | Current | A public-safe projection of agent identity, state, role, route disclosure, and selected activity for presentation. It excludes secrets and raw private traces. |
| **Safe Agent Card** | Current, Boundary | The bounded display model for one turn or delegated agent. It exposes safe lifecycle, provider, destination, usage-truth, and message-count facts and is never proof, acceptance, delivery, or release. |
| **Persona** | Current, Interop | A versioned presentation and behavior description for an agent. It can declare intended capabilities but cannot grant them. |
| **Managed Agent** | Current, Target | An agent instance whose runtime, credentials, placement, or lifecycle is operated through an admitted OpenAgents control plane. |
| **Attached Agent** | Current, Target | An existing external or local agent runtime attached to an OpenAgents Thread, Workroom, or work scope through an adapter and grants. |
| **Native Agent** | Current, Target | An agent runtime integrated directly with the OpenAgents conversation, tool, context, and authority contracts. |
| **Sarah** | Current | The named OpenAgents agent experience with private owner work, community participation, memory, voice, and signed identity boundaries. The name denotes a product persona and runtime composition, not a universal agent type. |
| **Memory Engram** | Current, Interop | An encrypted, addressable agent-memory record designed so the owning principal can decrypt it. It is not a plaintext relay log or a substitute for canonical work state. |
| **Agent Memory** | Current, Target | Durable context an agent can retrieve under owner, privacy, retention, and scope policy. Memory may use engrams or private canonical storage depending on the data class. |

## 7. Runtime, models, tools, and interactions

| Term | Status | Definition |
| --- | --- | --- |
| **Agent Runtime** | Current | The execution service that owns agent turns, tool interactions, interruption, continuation, and runtime events for a Session. |
| **Runtime Adapter** | Current, Target | An implementation that maps a provider or agent protocol into OpenAgents runtime, interaction, event, and authority contracts. |
| **Agent Client Protocol (ACP)** | Current | A protocol for connecting agent runtimes and clients. It is an adapter boundary; OpenAgents still owns product truth and grants. |
| **Model Context Protocol (MCP)** | Current | A protocol for exposing tools and contextual resources to an agent. Tool discovery does not imply permission to call a tool. |
| **Provider** | Current | A service or local runtime that supplies models, tools, compute, or agent execution. Provider state is not the product-work system of record. |
| **Model** | Current | A named inference capability offered by a Provider. A Model is one dependency of an Agent, not the Agent's identity or authority. |
| **Harness** | Current | The runner integration that launches and controls a coding or agent provider under a shared execution contract. |
| **Tool** | Current | A typed operation an Agent Runtime can request. A tool has schema, implementation, authority checks, and observable outcomes. |
| **Tool Policy** | Current, Boundary | The rule set that admits, rejects, scopes, or requires approval for tool use. |
| **Interaction** | Current | A runtime request for human or authorized-principal input, such as a question, approval, choice, or confirmation. |
| **Interaction Decision** | Current | The admitted response to an Interaction, including actor, choice, timestamp, and target. It is not inferred from ordinary chat text. |
| **Approval** | Current, Boundary | A typed authorization decision for a named action and scope. A reaction, message, or relay acceptance is not an Approval. |
| **Question Wizard** | Current | Omega's structured UI for runtime questions and choices. It projects Interactions and emits typed decisions. |
| **Turn** | Current | One bounded exchange in an agent Session, including input, model/runtime work, tool events, output, usage, and terminal state. |
| **Continuation** | Current | A request to resume a stopped or incomplete Turn or Run under its existing identity and updated constraints. |
| **Interruption** | Current | A typed request and resulting runtime state that stops or redirects active work without pretending it completed. |
| **Retry** | Current | A new attempt related to an earlier failed or interrupted operation. It preserves lineage and idempotency boundaries. |
| **Conversation** | Current | The synchronized read model of a Thread and its Messages, including loading, stale, access, and connection state. |
| **Agent Timeline** | Current | The ordered projection of agent events and runs associated with a Thread or selected agent context. |
| **Agent Timeline Event** | Current | A normalized runtime event for presentation and audit, with source, actor, time, kind, and related Run or Turn. |
| **Observed Agent Activity** | Current | Provider activity reduced to safe named fields before projection: bounded message text, a tool label, file-change count, or command-output byte count. Raw commands, output, paths, tokens, and secrets are excluded. |
| **Safe Message Chain** | Current, Boundary | The bounded, redacted sequence of Observed Agent Activities that read surfaces may present. It is a display projection, not the raw provider transcript. |
| **Route Disclosure** | Current, Boundary | The public-safe explanation of an agent routing decision, including outcome, reason, destination, cost class, local-only state, candidate dispositions, and context-manifest ref. It contains no credential or secret. |
| **Turn Recovery Facts** | Current | The safe terminal facts a client can show for a refused, failed, or cancelled Turn: card state, terminal status, and typed refusal reason. |
| **Live Agent Graph** | Current | A bounded graph projection of active and recent agents, Runs, Threads, relationships, states, routes, and usage truth. It can be live or historical. |
| **Agent Graph Node** | Current | A principal, Run, Thread, tool, or other typed vertex in a Live Agent Graph. |
| **Agent Graph Edge** | Current | A typed relation such as owns, delegates, runs, belongs-to, calls, or contributes-to between graph nodes. |
| **Usage Record** | Current | A durable measurement of model, tool, token, time, or cost usage with attribution and truth quality. It does not by itself authorize billing or settlement. |
| **Usage Truth** | Current | The declared quality of usage data, such as exact, partial, or unreported. Presentation must preserve this qualification. |
| **Budget** | Current, Boundary | A bounded allowance for tokens, money, time, actions, or concurrency. Reaching a Budget limit changes admission; it must not be hidden as a generic failure. |
| **Circuit Breaker** | Current, Target | A stateful guard that stops repeated calls after a defined failure threshold and supports explicit recovery or half-open testing. |

## 8. Full Auto, fleet, and Agent Computer

| Term | Status | Definition |
| --- | --- | --- |
| **Full Auto** | Current | The bounded multi-run product that coordinates concurrent agent work under an objective, autonomy policy, budgets, routing, guardrails, evidence, and owner controls. It is not unlimited autonomy. |
| **Full Auto Run** | Current | The canonical lifecycle record for one Full Auto execution, including objective revision, actors, autonomy plan, execution bindings, state transitions, receipts, and reports. |
| **Objective Revision** | Current | A versioned statement of what a Run is trying to accomplish. Revision changes preserve lineage and can invalidate earlier plans or claims. |
| **Autonomy Plan** | Current | The admitted plan that defines what a Full Auto Run may select and do without another owner decision. |
| **Autonomy Profile** | Current | Reusable default constraints for initiative, concurrency, spend, tools, provider routing, and stop conditions. A profile remains subordinate to live grants. |
| **Mission** | Current | A named objective package used to initialize or explain a Full Auto Run. It is not a grant by itself. |
| **Run State** | Current | The canonical lifecycle state of a Run. Terminal, paused, blocked, and active states must remain distinguishable. |
| **Run Transition** | Current | An append-only state change with previous state, next state, actor, time, reason, and transition evidence. |
| **Run Registry** | Current | The authoritative collection and lookup service for Runs and their current projections. |
| **Run Control Intent** | Current | An owner or admitted-actor request to pause, resume, cancel, retry, or close a Run. Dispatch and outcome are recorded separately. |
| **Self Claim** | Current, Boundary | A bounded agent claim on eligible work under an admitted loop. It does not supersede another valid claim or create new authority. |
| **Execution Binding** | Current | The durable relation between a Run and the selected agent, harness, provider account, environment, session, or device used to execute it. |
| **Fleet** | Current | A managed set of agent-capable execution slots or accounts that can receive bounded work. |
| **Fleet Run** | Current | The fleet projection of a Run, including authority freshness, state, attention, and supported control actions. |
| **Fleet Cockpit** | Current | The read-and-control projection that combines fleet authority freshness, Run cards, attention requests, and the actions currently eligible for admission. |
| **Pylon** | Current | The OpenAgents terminal and local execution front door that connects owned provider capacity, accepts bounded assignments, and returns public-safe proof. |
| **Assignment** | Current | A typed unit of delegated execution with objective, target Pylon or runtime, lease, pinned context, progress, outcome, and closeout proof. |
| **Lease** | Current, Boundary | A time- and scope-bounded exclusive right to execute an Assignment or Work Packet. Expiry or revocation ends the right even if a process remains alive. |
| **Capacity** | Current | A typed statement of available, ready, busy, and queued execution slots for a provider, device, Pylon, or fleet. |
| **Agent Computer** | Current | A managed execution environment that can run an admitted agent workload through OpenAgents control, credentials, capacity, and evidence contracts. |
| **Managed Environment** | Current | A provisioned and identified host environment with lifecycle, image, credentials, health, and policy controls. |
| **Environment Identity** | Current | The stable identity and revision of a Managed Environment, distinct from the agent, Session, or provider account running inside it. |
| **Control Plane** | Current | The authoritative service boundary that admits lifecycle and mutation commands for managed execution. Data-plane success cannot bypass it. |
| **Provider Lane** | Current, Target | A configured execution route for a provider or capacity class, including eligibility, account health, limits, and fallback policy. |
| **Placement** | Current, Target | The admitted selection of where a Session or Run executes: local device, remote host, managed environment, or another owned runtime. |
| **Portable Execution** | Current, Target | The ability to continue governed work across devices or hosts without losing canonical Session identity, command history, attachments, or outcomes. |

## 9. Sessions, devices, engines, and durable commands

| Term | Status | Definition |
| --- | --- | --- |
| **Session** | Current | A bounded interaction or execution context with identity, owner scope, host placement, state, attachments, commands, and outcomes. It is not the same as a durable Thread. |
| **Coding Session** | Current | A Session specialized for repository interaction, with Project, Repository, Worktree, navigation, agent, and runtime references. |
| **Portable Session** | Current | A synchronized Session whose canonical identity and command lifecycle can survive client disconnection and move between admitted hosts. |
| **Session Attachment** | Current | A versioned resource reference bound to a Portable Session, with generation and access metadata. |
| **Attachment Generation** | Current | A monotonic revision of a Session Attachment used to reject stale host or client writes. |
| **Portable Command** | Current | A durable command request associated with a Portable Session, including identity, sequence, admission, host execution, and terminal outcome. |
| **Command Ledger** | Current, Target | The ordered durable record of command intents, claims, execution, outcomes, and deduplication. It enables reconnect and replay without replaying side effects. |
| **Queue Command** | Current, Target | A durable command waiting for the eligible host or engine to claim it. Queue presence is not execution. |
| **Processed Command** | Current, Target | A Command Ledger entry with a terminal outcome or explicit rejection, safe to acknowledge without re-executing. |
| **Quiesce** | Current | A controlled transition that stops accepting new work and drains or checkpoints active work before a move, update, or shutdown. |
| **Checkpoint** | Current, Target | A recoverable snapshot of execution progress and relevant state. Restoration must detect conflicts and must not repeat uncommitted external side effects. |
| **Session Move** | Current | The admitted handoff of a quiesced Portable Session from one host placement to another, preserving identity and command generations. |
| **Engine** | Target | A long-lived host runtime that owns local execution, process supervision, command consumption, and outcome publication for one or more Sessions. |
| **Headed Engine** | Target | An Engine running with an attached visible Omega client and local interactive surfaces. |
| **Headless Engine** | Target | An Engine running without a visible client, suitable for remote or unattended work under the same command and evidence contracts. |
| **Detached Session** | Target | A Session whose Engine continues after the initiating UI disconnects. Detachment does not change owner, authority, or canonical identity. |
| **Device** | Current, Target | A registered client or host endpoint with identity, capabilities, grants, health, and last-seen state. |
| **Host Device** | Target | The Device currently admitted to execute a Session's commands and publish outcomes. Viewing a Session does not make a Device its host. |
| **Device Bridge** | Current, Target | The authenticated transport and command boundary between a client Device and a host runtime or managed environment. |
| **Host Claim** | Current, Target | A lease-like claim that identifies the Device or Engine eligible to execute the next commands for a Session. |
| **Host Outcome** | Current, Target | The signed or authenticated execution result returned by the admitted host and then validated into canonical state. |

## 10. Sync, state, and projections

| Term | Status | Definition |
| --- | --- | --- |
| **Canonical State** | Current, Boundary | The authoritative OpenAgents state accepted by the owning service and durable store. Clients and relays project it; they do not redefine it. |
| **Canonical Event** | Current, Target | An admitted append-only event that changes or explains Canonical State. It includes actor, scope, version, time, and idempotency. |
| **Projection** | Current | A derived read model built from canonical records, events, or validated external records. Projections can be stale, partial, rebuilt, or discarded. |
| **Projection Issue** | Current | A typed warning that a projection is partial, malformed, stale, conflicting, or unavailable. It must not silently convert uncertainty into truth. |
| **Confirmed State** | Current | State known to have been accepted by the canonical service and stored with an authoritative revision. |
| **Pending Intent** | Current | A local or transmitted mutation request awaiting confirmation or rejection. UI must not present it as confirmed truth. |
| **Optimistic View** | Current | A temporary client projection that includes Pending Intents while retaining their unconfirmed status and rollback path. |
| **Khala Sync** | Current | The OpenAgents synchronization system for scoped snapshots, deltas, client mutations, live updates, and local durable projections across desktop, web, and mobile. |
| **Sync Scope** | Current | The authorization and data boundary for one synchronized domain, such as conversations, runtime, attention, coding catalog, live graph, or portable sessions. |
| **Sync Session** | Current | A bounded connection and reconciliation lifecycle between a client identity and Khala Sync scopes. It is not an agent execution Session. |
| **Scope Sync State** | Current | The per-Sync-Scope state machine that distinguishes idle, bootstrap, catch-up, live, and must-refetch conditions while retaining the durable cursor. |
| **Snapshot** | Current | A complete bounded projection for a Sync Scope at a known version. |
| **Delta** | Current | An ordered change after a known version that advances a projection without replacing the entire Snapshot. |
| **Cursor** | Current | An opaque position in an ordered event, sync, or query stream. Clients must not infer authority or time solely from cursor shape. |
| **Revision** | Current | A monotonic version of a canonical object or document within its owning scope. |
| **Generation** | Current | A monotonic incarnation used to reject commands or attachments from an earlier host, Session, or binding. |
| **Epoch** | Current | A broader discontinuity marker after which earlier cursors, claims, or generations may no longer be comparable. |
| **Client View Record (CVR)** | Current | Server-maintained knowledge of what a client projection has seen, used to compute safe incremental updates. |
| **Mutation** | Current | A requested canonical state change with identity, arguments, actor context, and idempotency. |
| **Mutator** | Current | The typed client or service operation that constructs and submits a Mutation under a named contract. |
| **Mutation Gap Repair** | Current | The reconciliation path that detects a missing or rejected mutation sequence and restores the client from authoritative state. |
| **Local Store** | Current | Durable on-device storage for confirmed projections, pending intents, and sync metadata. It is not a second cloud authority. |
| **Live Subscription** | Current | A connection that delivers validated updates after bootstrap. Loss of liveness changes freshness, not the last confirmed revision. |
| **Live Conversation Envelope** | Current | The validated live transport envelope for conversation snapshots, updates, access changes, and connection signals. It advances the Conversation projection but is not durable truth by itself. |
| **Coding Catalog** | Current | The owner-scoped confirmed projection of coding projects, repositories, worktrees, Sessions, navigation, resolution, and catalog issues. Device-local rows never silently become hosted rows. |
| **Coding Navigation** | Current | The synchronized selection and recent-navigation record that resolves a coding Project, Repository, Worktree, and Session for presentation. |
| **Composer Draft** | Current | A private device-local snapshot of the coding composer document, selection, attachments, target, owner, and revision. It never enters hosted Sync unless a separate product contract admits that behavior. |
| **Ref** | Current | A stable typed reference to an entity, receipt, artifact, actor, or record. Human-readable labels must not replace refs in authority contracts. |
| **Digest** | Current | A cryptographic content identifier used to bind an exact document, artifact, event, or proposal revision. It proves bytes, not meaning or acceptance. |
| **Idempotency Key** | Current | A caller- or service-defined key that ensures retries of one logical mutation do not repeat its side effects. |
| **Tombstone** | Current, Interop | A durable deletion or redaction marker that preserves enough identity and authority context to prevent accidental resurrection. |

## 11. Omega application and native workbench

| Term | Status | Definition |
| --- | --- | --- |
| **Omega** | Current, Target | The native OpenAgents desktop destination and product workbench. It combines coding, agents, Product Work, collaboration, sync, identity, and managed execution. |
| **Current Desktop** | Current | The supported Electron desktop application that remains in service until Omega meets the admitted cutover gates. |
| **omega-effectd** | Current | The native companion service that owns extracted Effect workflows such as Full Auto and Agent Computer operations behind a bounded host protocol. |
| **Host Bridge** | Current | The protocol boundary between Omega's native UI process and omega-effectd or another admitted local service. |
| **GPUI** | Current | Omega's native Rust UI framework. It supplies application, window, entity, context, rendering, and action primitives; it does not own OpenAgents product truth. |
| **App** | Current | The process-level GPUI application context and service root. |
| **Window** | Current | One native application window with its own Workspace and focus tree. |
| **Entity** | Current | A GPUI-owned state object whose mutations and observations occur through framework contexts. It is a UI/runtime ownership primitive, not a Product Work entity category. |
| **Context** | Current | The typed GPUI handle used to read, mutate, observe, and schedule work for an Entity or App. It is distinct from agent Context or Code Context. |
| **Action** | Current | A typed UI command dispatched through focus and keybinding contexts. An Action can request a product Command Intent but is not itself canonical authority. |
| **Workspace** | Current | The Omega window-level workbench container for panes, items, projects, navigation, dock state, and persisted local layout. |
| **Workbench** | Current, Target | The user-facing composition of Workspace navigation, agent, code, work, review, and diagnostic surfaces. |
| **Pane Group** | Current | A layout group containing one or more Panes and split relationships. |
| **Pane** | Current | A tab container within a Workspace. It hosts pane Items and focus state. |
| **Item** | Current | A pane-hosted UI object such as an editor, terminal, thread, or settings view. Do not use this word for product work. |
| **Navigation History** | Current | Browser-style back and forward history over meaningful Workspace locations and selections. It does not own document history. |
| **Session Tab** | Current, Target | A pane tab representing an active or retained agent/coding Session, with status and host awareness. |
| **Zero Base** | Current | Omega's reduced shell mode that prioritizes the agent and essential work surfaces while retaining access to the full workbench. |
| **Direct Agent** | Current | The primary Omega mode for opening a Thread and working directly with one selected agent runtime. |
| **Agent Panel** | Current | The native Omega surface for Threads, agent interaction, runtime events, context, and related controls. |
| **Thread View** | Current | The presentation owner for the selected Thread's transcript, events, status, and scroll behavior. |
| **Message Editor** | Current | The composer component that owns draft input, mentions, attachments, editing, and submission affordances. |
| **Send Disposition** | Current | The typed decision that maps a composer submission to send a new message, steer active work, or reject the action. |
| **Send** | Current | Add a new user Message or start the next eligible Turn when no incompatible Turn is active. |
| **Steer** | Current | Add user direction to compatible active work without misrepresenting it as an unrelated new Turn. |
| **Stop** | Current | Request interruption of active work and expose the resulting runtime state. It is not a completed result. |
| **IDE Project Graph** | Current | The editor capability graph that coordinates roots, worktrees, buffers, language services, Git, tasks, terminals, remote state, and navigation. It is not a product Project. |
| **Workspace Service** | Current | A service exposed to Workspace items and actions, such as project, agent, terminal, settings, or navigation capabilities. |
| **Capability Store** | Current | A focused service interface over a capability domain, such as worktrees, buffers, language services, or agents. It avoids one all-powerful project object. |
| **Work Context** | Current | The selected repositories, roots, permissions, attachments, environment, and tools available to agent or coding work. It is referenced by ProductSpec Runs and coding Sessions. |
| **Project Root** | Current | A filesystem or remote root attached to the IDE Project Graph. One product Project can reference several Project Roots. |
| **Worktree** | Current | A concrete filesystem checkout and branch view used for code work. It is not an organizational Workspace or product Project. |
| **Project File Ref** | Current | A stable file reference composed from project/root identity and a normalized relative path, safe to adapt across local and remote placement. |
| **Buffer** | Current | The shared editable text state for a file or untitled document, including edits, versions, and collaboration hooks. |
| **Language Buffer** | Current | A Buffer enriched with parsed syntax, language identity, diagnostics, and language-service state. |
| **MultiBuffer** | Current | A composed text model that presents excerpts from multiple Buffers as one navigable view. |
| **Excerpt** | Current | A bounded range from a source Buffer embedded in a MultiBuffer, with mappings back to source coordinates. |
| **Editor** | Current | The pane Item that renders and edits a Buffer or MultiBuffer with selections, diagnostics, code actions, and navigation. |
| **Project Panel** | Current | The file and root navigation surface over the IDE Project Graph. |
| **Language Server** | Current | A process or service that supplies language intelligence through a language-server protocol and is scoped by project capability and placement. |
| **Git Model** | Current | The IDE projection of repositories, branches, status, diffs, commits, and operations. Git remains the authority for Git objects and refs. |
| **Diff** | Current | A structured comparison between content revisions. A Diff is review input, not proof that a change is correct. |
| **Terminal** | Current | A pane Item connected to a local or remote process environment under explicit placement and process policy. |
| **Task** | Current | A configured, repeatable process command launched from the IDE Project Graph. A Task is not a Product Work Item or a Codex task/thread. |
| **Remote Project** | Current, Target | An IDE Project Graph whose roots, language services, terminals, or tasks execute on a remote host while Omega remains the client. |

## 12. Diagnostics, experiments, and proof views

| Term | Status | Definition |
| --- | --- | --- |
| **Entropy** | Current, Target | A measure or model of uncertainty, divergence, instability, or unexplained variance in a run, system, or experiment. Its exact calculation must be named by the owning contract. |
| **Entropy Dashboard** | Current, Target | A native projection that helps compare uncertainty and convergence across targets, fixtures, arms, sessions, or runs. It is diagnostic, not canonical execution state. |
| **Forensic Bench** | Current, Target | A native inspection surface for replaying and comparing traces, model events, commands, outcomes, evidence, and contradictions. |
| **Experiment** | Target | A versioned comparison with hypothesis, arms, fixtures, target, metrics, policy, and result criteria. |
| **Arm** | Target | One configuration or treatment within an Experiment. |
| **Fixture** | Current, Target | A controlled input and expected constraints used for repeatable tests, evaluations, or agent proofs. |
| **Target Revision** | Current, Target | The exact repository commit, artifact digest, ProductSpec revision, or environment image against which work or proof runs. |
| **Trace** | Current | An ordered record of runtime, tool, command, and interaction events. A raw Trace can be private; public traces require redaction and explicit publication. |
| **Proof Replay** | Current, Target | A deterministic or bounded re-evaluation of recorded inputs, transitions, and evidence used to inspect whether a claim follows. Replay does not re-authorize side effects. |
| **Public-Safe Projection** | Current, Boundary | A deliberately redacted projection that contains only fields admitted for public disclosure. Absence from it does not imply absence from private canonical state. |

## 13. ProductSpec, assurance, evidence, and authority

| Term | Status | Definition |
| --- | --- | --- |
| **ProductSpec** | Current | The exact, versioned statement of product intent and acceptance criteria for a deliverable. It owns intent, not test execution or release. |
| **ProductSpec Identity** | Current | The tuple of spec ref, repository-relative path, positive revision, and content digest that binds work and receipts to exact intent. |
| **Acceptance Criterion** | Current | An independently addressable ProductSpec condition with stable identity, body, and order. |
| **ProductSpec Projection** | Current | The parsed `ready` or `invalid` view of a ProductSpec, including criteria, identity, warnings, and executable status. |
| **Plan** | Current, Target | A versioned proposal that decomposes intent into ordered or dependent work. A ProductSpec Plan becomes executable only after acceptance. |
| **Work Packet** | Current | The bounded executable unit in a ProductSpec Plan, linked to criteria, dependencies, allocation, state, evidence, verification, lease, and owner disposition. |
| **Packet State** | Current | One of the admitted Work Packet lifecycle states: planned, active, blocked, evidence present, verified, failed, superseded, or cancelled. |
| **Allocation** | Current | The choice that assigns a Work Packet to the root coordinator or a child execution lane. Allocation does not change evidence or verification requirements. |
| **Plan Reconciliation** | Current | The comparison between ProductSpec revisions that identifies retained, changed, added, and removed criteria and supersedes affected packets safely. |
| **Edit Proposal** | Current | A reviewable ProductSpec change with previous identity, next identity, diff, reconciliation, authoring context, and state. |
| **Evidence Attachment Proposal** | Current | A change proposal limited to attaching evidence references without altering normative ProductSpec intent. |
| **AssuranceSpec** | Current | The companion proof design that defines observers, evidence, independence, verification, and admitted claims. It does not replace ProductSpec intent or release authority. |
| **Evidence** | Current, Boundary | Material produced to support a criterion or claim, with provenance and exact references. Evidence is not Verification. |
| **Evidence Receipt** | Current | A durable record binding evidence ref, kind, producer, exact ProductSpec identity, criteria, and production time. |
| **Verifier** | Current, Boundary | The admitted principal or observer authorized and sufficiently independent to evaluate named evidence. The producer is not automatically the verifier. |
| **Verification** | Current, Boundary | The act of evaluating evidence against criteria under an admitted AssuranceSpec or equivalent contract. |
| **Verification Receipt** | Current | A durable passed verdict binding verifier, exact ProductSpec identity, criteria, evidence receipts, output, and time. |
| **Owner Disposition** | Current, Boundary | The owner's explicit acceptance or waiver after evidence and verification. It remains distinct from packet state, review verdict, release, and public claim. |
| **Admission** | Current, Boundary | The authoritative decision that a proposal, command, packet, event, or result satisfies the required policy and may enter canonical state or execution. |
| **Authority** | Current, Boundary | The recognized right to make a named decision or mutation in a named scope. Authority is always scoped and cannot self-amplify. |
| **Capability** | Current | A typed description of what a component, actor, device, or agent is technically able to do. |
| **Capability Grant** | Current, Boundary | The policy-backed authorization allowing a principal to use a Capability in a specific scope and time window. |
| **Command Intent** | Current | A typed request to perform a canonical or external mutation. It includes actor, target, arguments, policy context, and idempotency. |
| **Command Outcome** | Current | The durable accepted, rejected, failed, interrupted, or completed result of a Command Intent. Transport success is not a Command Outcome. |
| **Receipt** | Current | A durable, dereferenceable record that binds an action, producer, target, time, inputs, outputs, and relevant authority or evidence. |
| **Public-Safe Receipt** | Current | A redacted Receipt admitted for public disclosure. It preserves verifiable references without exposing secrets, raw prompts, private traces, or local paths. |
| **Release** | Current, Boundary | The authorized publication or distribution of a product artifact through a named channel after its release gates pass. Verified work alone is not Release. |
| **Public Claim** | Current, Boundary | A statement presented externally as product truth. It requires the exact evidence and authority defined by the applicable promise or release contract. |
| **Settlement** | Current, Boundary | The authoritative financial closeout of an obligation. Usage, results, relay events, or no-spend assignment receipts do not independently create Settlement. |
| **Closeout** | Current | The terminal reconciliation of assignment, execution, evidence, payment mode, settlement posture, and rejection state. |

## 14. Signed interoperability and federated workrooms

| Term | Status | Definition |
| --- | --- | --- |
| **Signed Event** | Current, Interop | A cryptographically signed record with author key, kind, timestamp, tags, content, and identifier. Signature validity proves authorship of bytes, not OpenAgents admission. |
| **Event Kind** | Current, Interop | The protocol number or type that defines a Signed Event's wire semantics. OpenAgents maps kinds into typed records before use. |
| **Addressable Event** | Current, Interop | A replaceable Signed Event addressed by author, kind, and a stable identifier tag. Replacement semantics do not replace OpenAgents revision rules. |
| **Ephemeral Event** | Current, Interop | A Signed Event intended for live delivery rather than relay persistence, such as presence or observer frames. It cannot serve as sole durable proof. |
| **Relay** | Current, Interop | A server that accepts, stores, filters, and distributes Signed Events under relay policy. It is not an OpenAgents command, work, receipt, or settlement authority. |
| **Relay Receipt** | Current, Interop, Boundary | A relay acknowledgement that an event was accepted or rejected by that relay. Acceptance means transport/storage policy passed, not that the requested action ran. |
| **Relay-Qualified Coordinate** | Current, Interop | An identifier that includes relay origin plus the protocol-local object identifier, preventing unrelated relays from collapsing into one apparent object. |
| **Relay Group** | Current, Interop | A relay-managed group with identifier, metadata, membership, roles, moderation, and scoped messages. It can project a Workroom when OpenAgents binds it to canonical scope and policy. |
| **Group Membership State** | Current, Interop | The relay-signed projection of members and roles for a Relay Group. Effective OpenAgents capability still requires grants and policy. |
| **Group Message** | Current, Interop | A Signed Event scoped to a Relay Group. It can appear as a Workroom Message after validation and projection. |
| **Moderation Event** | Current, Interop | A group-scoped Signed Event that changes relay-visible moderation state under the Relay's policy. It does not silently delete canonical OpenAgents records. |
| **Direct Message** | Current, Interop | A private message addressed to one or more recipients through an encrypted interoperable envelope. Metadata and delivery privacy must be stated precisely. |
| **Gift Wrap** | Current, Interop | A privacy-preserving outer event used to deliver an encrypted Direct Message while reducing exposed sender and routing metadata. |
| **Encrypted Payload** | Current, Interop | Ciphertext whose intended recipients, key custody, metadata leakage, and retention rules are known. A `private` label without encryption is not an Encrypted Payload. |
| **Owner-Decryptable** | Current, Boundary | A privacy invariant requiring the owner to be able to decrypt agent-held private memory or records under the stated recovery model. |
| **Agent Persona Record** | Current, Interop | An owner-authored public or scoped Signed Event that describes an agent persona or managed instance without carrying secrets or grants. |
| **Agent Turn Metric** | Current, Interop | An encrypted per-turn usage projection for owner visibility. Canonical usage and any financial ledger remain in their owning OpenAgents service. |
| **Agent Observer Frame** | Current, Interop | An ephemeral encrypted telemetry or control frame for live agent state. Durable results and commands use canonical contracts. |
| **Repository Event** | Current, Interop | A Signed Event that announces or proposes repository, patch, issue, pull request, or status information. Git remains authority for objects and refs; OpenAgents admits mutations separately. |
| **Blob Artifact** | Current, Interop | A content-addressed binary object stored outside ordinary event content and referenced by digest and metadata. Access and retention are separate policies. |
| **Compute Request** | Current, Interop | An optional signed market request for compute or data work. It is not the default OpenAgents work command and requires separate provider and settlement admission. |
| **Compute Result** | Current, Interop | A signed result reference for a Compute Request. It remains unverified until the applicable result and settlement contracts admit it. |
| **Interoperability Outbox** | Target | The durable publisher that maps admitted canonical changes into optional Signed Events, retries delivery, records relay receipts, and never blocks canonical transactions on relay availability. |
| **Interoperability Inbox** | Target | The validator and proposal gateway that maps external Signed Events into typed candidate commands or records before OpenAgents admission. |
| **Event Projection** | Current, Target | A derived OpenAgents view of validated Signed Events. It retains source relay, author, kind, signature, freshness, and admission status. |

## 15. Forge, code collaboration, voice, and spatial views

| Term | Status | Definition |
| --- | --- | --- |
| **Repository** | Current | A Git repository with canonical Git object and ref state plus OpenAgents project, work, policy, and collaboration links. |
| **Forge** | Current | The OpenAgents code-collaboration surface for repositories, patches, proposals, issues, review, membership, and signed interoperability. |
| **Patch** | Current, Interop | A proposed code change represented as a diff or Git object reference. A Patch is not applied merely because it is signed or received. |
| **Pull Request** | Current, Interop | A proposed integration of code changes with source, target, discussion, review, and status. Repository policy owns merge authority. |
| **Code Review** | Current, Target | Structured evaluation of a Patch or Pull Request. It can contribute evidence to a Work Review but does not replace product acceptance. |
| **Merge** | Current, Boundary | The authorized Git ref mutation that integrates changes. Review approval, signed events, or passing tests do not independently grant Merge. |
| **Voice Gateway** | Current | The managed boundary that authenticates and authorizes realtime voice access without exposing provider credentials to clients. |
| **Voice Room** | Current | A realtime media room with participant identity, grants, and lifecycle distinct from the durable Workroom and Thread records it may accompany. |
| **Participant** | Current | A person, agent, or service present in a Voice Room, with media and identity state. Presence does not imply Workroom membership. |
| **Avatar Stage** | Target | A bounded native spatial view that presents agents, people, activity, and voice without becoming the authority for work or identity. |
| **Avatar Manifest** | Target | A signed or admitted durable description of an avatar's assets, rig, presentation, provenance, and safety metadata. |
| **Live Avatar State** | Target | Ephemeral pose, expression, action, and presence data projected into an Avatar Stage. It is not a durable identity record. |

## 16. Avoided or overloaded terms

| Avoid | Use instead | Reason |
| --- | --- | --- |
| **Space** by itself | **Product Space**, **Workspace**, or **Workroom** | The unqualified word collapses organizational, window, and collaboration scopes. |
| **Project** for the editor service graph | **IDE Project Graph** | Product Project is the canonical product-work meaning. |
| **Workspace** for an organization | **Product Space** | Workspace is reserved for the Omega window/workbench container. |
| **Task** for product work | **Work Item** | Task remains an IDE process command and is also overloaded in agent clients. |
| **Item** for product work | **Work Item** | Item is reserved for a pane-hosted UI object. |
| **Chat** as the whole work model | **Thread**, **Workroom**, or **Conversation** | Chat hides structured work, sessions, decisions, activity, and authority. |
| **Agent session** without scope | **Thread**, **Work Session**, **Coding Session**, or **Runtime Session** | The unqualified phrase conflates durable context and bounded execution. |
| **Job** for canonical product work | **Work Item**, **Assignment**, or **Compute Request** | These terms distinguish product work, internal delegation, and optional market work. |
| **Event is truth** | **Signed Event**, **Canonical Event**, or **Projection** | Origin, validation, and authority must stay visible. |
| **Accepted** without subject | **Relay accepted**, **admitted**, **owner accepted**, or **released** | Each word names a different authority boundary. |
| **Verified means shipped** | **Verification Receipt** plus separate **Release** | Verification and release are different decisions. |
| **Role grants access** | **Capability Grant** | A role label alone is not effective authority. |
| **Private group** as an encryption claim | **Relay read-restricted** or **Encrypted Payload** | Relay access control and end-to-end encryption are different properties. |
| **Agent is the model** | **Agent**, **Model**, and **Provider** | Identity, inference capability, and supply service are distinct. |
| **Sync owns state** | **Canonical State** projected through **Khala Sync** | Sync transports and projects authority owned by domain services. |
| **Relay command** | **Signed command proposal** or **Command Intent** | A relay only transports the record; OpenAgents admits and executes the command. |

## 17. Minimum fields for target Product Work records

These are vocabulary-level minimums, not final schemas.

| Record | Minimum fields |
| --- | --- |
| **Product Space** | `spaceRef`, name, owner policy ref, created time, lifecycle state |
| **Team** | `teamRef`, `spaceRef`, name, member refs, workflow policy ref |
| **Initiative** | `initiativeRef`, `spaceRef`, title, owner ref, project refs, outcome metric refs, status |
| **Project** | `projectRef`, `spaceRef`, team refs, title, owner ref, state, start/target dates, milestone refs |
| **Cycle** | `cycleRef`, `teamRef`, name, start time, end time, state |
| **Milestone** | `milestoneRef`, `projectRef`, title, target time, state |
| **Work Item** | `workItemRef`, `spaceRef`, team ref, project/cycle/milestone refs, title, body, state, priority, accountable owner ref, delegate refs, label refs, relation refs, revision |
| **Work Item Relation** | `relationRef`, source Work Item ref, relation kind, target Work Item ref, actor ref, created time |
| **Customer Signal** | `signalRef`, source class/ref, consent class, summary, linked Work Item refs, created time |
| **Decision** | `decisionRef`, scope refs, decision maker ref, question, decision, rationale, alternatives, decided time |
| **Agent Member** | `agentMemberRef`, agent profile ref, owner relation ref, membership scope, declared capabilities, grant refs, lifecycle state |
| **Delegation Grant** | `grantRef`, issuer ref, agent member ref, work scope refs, allowed actions, constraints, valid interval, revocation state |
| **Work Session** | `workSessionRef`, Work Item ref, participant refs, Code Context ref, started/ended times, Run refs, state |
| **Work Activity** | `activityRef`, Work Session ref, Work Item ref, actor ref, activity kind, body or payload ref, related refs, created time |
| **Work Review** | `reviewRef`, target refs, reviewer ref, guide ref/revision, evidence refs, verdict, findings, reviewed time |
| **Product Pulse** | scope ref, as-of revision/time, source cursor refs, metric rows, risk rows, freshness and projection issues |

## 18. Authority summary

| Question | Authority |
| --- | --- |
| What work exists and what state is it in? | OpenAgents Product Work canonical service and store |
| What may an agent do? | Applicable policy plus an explicit Capability Grant or Delegation Grant |
| What happened during execution? | Runtime, command, Run, and Work Activity canonical events and receipts |
| What does the UI show? | Rebuildable projections over canonical and validated interoperable records |
| What did a relay accept? | Relay Receipt only |
| What did Git accept? | Git objects and refs under repository policy |
| What proves a ProductSpec criterion? | Evidence Receipt plus admitted Verification Receipt |
| What did the owner accept? | Owner Disposition |
| What is released or publicly claimable? | The applicable release and public-claim authority after its gates pass |
| What is settled? | The designated settlement ledger and authority, never a chat message or relay event alone |

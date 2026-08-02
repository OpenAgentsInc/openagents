# Linear Agents on OpenAgents: a Nostr-centric first-class adaptation

- Date: 2026-08-02
- Lane: Fast Follow research
- Disposition: recommended target architecture; not implementation authority
- OpenAgents source pin: `961d8d2297f528d3594680d52affef05da039555`
- OpenAgents tree pin: `55b524ae36c599cfe6cf41b689d084fba8bef7b4`

## 1. Decision

OpenAgents should support Linear as a first-class product-development context
and control surface.

The right design is not a Linear clone, a generic MCP wrapper, or a bulk mirror
of Linear data onto a relay. It is a three-authority composition:

1. **Linear owns Linear objects and permissions.** Linear remains authoritative
   for its workspaces, teams, projects, issues, comments, documents, Linear
   agent sessions, and the human accountability visible in Linear.
2. **OpenAgents owns execution and admission.** OpenAgents remains authoritative
   for agent identities, capability grants, workrooms, runs, leases, budgets,
   tool admission, evidence, verification, receipts, and owner disposition.
3. **Nostr is the signed collaboration and interoperability edge.** It carries
   portable identities, causal references, bounded context projections,
   delegations, progress, questions, handoffs, and closeout references. A
   signature or relay acknowledgement never becomes Linear permission or
   OpenAgents execution, verification, acceptance, release, payment, or
   settlement authority.

This is how OpenAgents should adapt Buzz's most important move. Buzz turned a
Slack-shaped collaboration product into a signed Nostr event graph. OpenAgents
should make Linear-shaped product work available to people and agents through
the same signed, portable graph, but retain OpenAgents' stricter authority and
receipt boundaries.

The shortest product statement is:

> Linear supplies the product context and accountable human surface.
> OpenAgents supplies the durable multi-agent execution and proof system.
> Nostr makes the relationship signed, portable, and interoperable.

## 2. What this document does and does not authorize

This is a research-lane artifact under the repository's Fast Follow contract.
It can recommend target contracts and candidate work packets. It does not:

- authorize implementation, deployment, release, spend, or a public parity
  claim;
- change the Sol roadmap or an accepted ProductSpec;
- create a separate Buzz application, relay product, Forum, Forge, or Linear
  replacement;
- make any external repository, product announcement, teardown, signed Nostr
  event, or relay response authoritative for OpenAgents;
- grant access to a Linear workspace, repository, provider account, or owner
  scope; or
- claim that the proposed complete NIP-29 or Omega workroom program has landed.

Implementation requires a separately admitted issue, accepted plan, or bounded
work packet, exact owner scope, target-local verification, and current-roadmap
reconciliation.

## 3. Evidence and freshness

This synthesis is grounded in the repository at the source and tree pins above.
It does not perform a new live audit of Linear. The current Linear teardown
contains the repository's public-source research through 2026-07-30. Its
preview-stage API claims and vendor-reported results must be rechecked before
implementation.

### 3.1 Primary evidence manifest

| Evidence                                                                                                | SHA-256                                                            | Role                                                                                     |
| ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| [`FASTFOLLOW.md`](../../FASTFOLLOW.md)                                                                  | `f337679bb633dea0a299563950901d9c66d022ab2d013d62173b30999f32dae0` | Admitted learning intent and Buzz and Linear lesson selection                            |
| [`INVARIANTS.md`](../../INVARIANTS.md)                                                                  | `77c8bcb6b8549b17c22aaa2a5886cc26687ab8162321deb366f35defc02e443f` | Current target authority, connector, workroom, infrastructure, and product laws          |
| [Buzz teardown](./2026-07-21-buzz-teardown.md)                                                          | `25ae1fe51370297725ce621c10939fa95ccfe5cb1f91c7cb66789e937915b7cd` | Commit-pinned Buzz architecture, NIPs, agents, workflows, Git, UI, and security evidence |
| [Buzz program status](../buzz/README.md)                                                                | `4e830aad20b16bb15e904efc5fe58b0a0ab6a924c260d93ca54ea101bc7daa8d` | Current versus canceled Buzz-derived directions                                          |
| [Buzz parity recommendation](../buzz/2026-07-24-omega-buzz-full-parity-recommendation.md)               | `960a5800ff1a96d7414bfb63eb78799d20691782561eed50106f66830e243627` | Product vocabulary and parity outcome map                                                |
| [Buzz UI harvest](../buzz/2026-07-25-collab-ui-harvest-before-retirement.md)                            | `c14da1874f99b642da12cbd09b26e8525120477a973f0ddcd2219d40be23c3f7` | Workroom tree, attention, membership, and presence patterns                              |
| [Canceled Buzz self-host runbook](../buzz/2026-07-22-buzz-self-host-and-sarah-runbook.md)               | `483fdc8ab05faacbe6e3cf6d7e68c78a98a38891130e47dd4ad7792df68cf68d` | Historical deployment evidence; explicitly not an executable path                        |
| [Linear Agents teardown](./linear-agents.md)                                                            | `a6430c26b9b532310adc8adbfcd4413b73ac7380ba600ac6742c95b779920583` | Linear agent, session, Loop, triage, code, review, pricing, and governance evidence      |
| [Linear Diffs report](../forge/linear.md)                                                               | `edd590ea05ffd25eb2340060e71072ced2d8178ae50a27007fe6c21f575f5353` | GitHub-authoritative review and issue-context composition                                |
| [Sol master roadmap](../sol/MASTER_ROADMAP.md)                                                          | `aecf21c6ceaad43394ee5bb051454341ea6d8cbdf4386147fbf189928f9e3cca` | Current priority, product posture, and retired-path authority                            |
| [Omega accepted plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)                     | `6c0a1b84f1c0f428220b2112b0e2a36585e92ec2c4570ef9cc9234d21af56ce1` | Native workroom and supervised Nostr component direction                                 |
| [Cross-app delegation analysis](../nostr/2026-07-22-full-auto-cross-app-agent-delegation-over-nostr.md) | `9344216c26d1000f20fd5e98e3b2fc185258ebf1c0de26ef4ce760a43f9fc07d` | Ref-only Nostr delegation and authority separation; strategy evidence only               |
| [Omega NIP-29 specification](../nostr/2026-07-27-omega-nip29-relay-groups-integration-spec.md)          | `7094a9880708c787ae43427f204f451e07ca3c889a92be4633787354ddb3f783` | Proposed relay-qualified room behavior and its limits                                    |
| [Omega NIP candidates](../omega/2026-07-24-nip-adoption-candidates.md)                                  | `9a383b2553b7dffeba8fb616de128346b46029f2d9f97260d2b175736901f1f8` | Current NIP adoption and deprecation analysis                                            |
| [Omega Nostr authentication target](../omega/2026-07-30-omega-nostr-authentication-and-onboarding.md)   | `252cb6a72146ff8964c08610c14d99d86d7e3408a04d669e7fee6b8936b27934` | Signer custody, account linking, relay auth, and device enrollment boundaries            |

### 3.2 Current implementation samples

The target is not starting from zero:

| Current surface                                                                                                      | Observed capability                                                                                                                        | Important limit                                                                            |
| -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| [`packages/connector-sidecar`](../../packages/connector-sidecar/src/index.ts)                                        | Verified GitHub webhook envelopes, delivery deduplication, bounded subjects, explicit writeback tools, and fail-closed authority decisions | Provider, object, and tool unions are GitHub-only                                          |
| [`event-ledger.ts`](../../apps/openagents.com/workers/api/src/event-ledger.ts)                                       | Durable owner-scoped event entries, deduplication, ordering, and public-safe gateway projections                                           | Current source vocabulary does not include Linear                                          |
| [`agent-definition-webhook-routes.ts`](../../apps/openagents.com/workers/api/src/agent-definition-webhook-routes.ts) | GitHub, Forum, and Slack signature verification and agent-trigger entry points                                                             | No Linear verification or immediate durable acknowledgement path                           |
| [`packages/khala-sync-client`](../../packages/khala-sync-client/src/live-agent-graph.ts)                             | Cross-client threads, timelines, live agent graphs, conversations, attention, sessions, and control projections                            | Linear concepts and external-source mappings are absent                                    |
| [`packages/khala-sync-server`](../../packages/khala-sync-server/src/live-agent-graph-projection.ts)                  | Cloud SQL-backed canonical projections, mutation fencing, durable runs, and live graph delivery                                            | No Linear installation or object-link authority                                            |
| [`packages/nip90`](../../packages/nip90/src/lbr.ts)                                                                  | Public-safe ref-only labor request/result events and content-addressed closeout composition                                                | It is not a Linear connector or permission system                                          |
| [`packages/public-nostr-chat`](../../packages/public-nostr-chat/src/channel.ts)                                      | Relay-qualified groups, signed event validation, NIP-42, reconnect, cursors, and relay identity checks                                     | Landed product is a narrow public chat, not the proposed complete private workroom system  |
| OpenAgents Desktop workroom contracts                                                                                | Work packets, dependencies, evidence, verification refs, leases, and owner disposition                                                     | Current Electron surface is retained evidence while Omega is the future native destination |
| Forge and forge-protocol                                                                                             | Git coordination, review primitives, NIP-34 interop, and Git-object authority                                                              | Linear Diffs is not a Git authority and must not become one here                           |

### 3.3 Fast Follow disposition axes

| Axis                  | Result                                                                                                       |
| --------------------- | ------------------------------------------------------------------------------------------------------------ |
| Source freshness      | Repository-current at the pinned commit; Linear API details require live revalidation before build           |
| Evidence completeness | Complete for the repository's primary Buzz and Linear analyses and the current target contracts used here    |
| Target fit            | High for accountable delegation, shared sessions, signed identity, causal projections, and integrated review |
| Implementation state  | Foundations landed; Linear-specific connector, mapping, sessions, writeback, Nostr profile, and UI are gaps  |
| Verification state    | Architecture reconciled against current code and contracts; no Linear live acceptance exists                 |
| Disposition           | Recommend an admitted, phased first-class Linear program beginning with one read-mostly end-to-end slice     |

### 3.4 Conflicting Buzz direction

The Buzz corpus contains real document drift. The 2026-07-24 parity
recommendation describes Buzz parity as a floor and Nostr as the primary
workroom record. The later 2026-08-02 Sol and Omega roadmaps narrow that
direction to selected Buzz workroom capabilities and optional standard-Nostr
interoperability. The later canonical roadmaps control where the two disagree.

This document therefore uses the July Buzz records as design evidence, not
sequencing or authority. It does not revive the canceled hosted-relay, separate
Buzz, standalone Forge, or cross-app delegation programs. Already-landed
NIP-34 claim-ledger, Forge conformance, public NIP-29 chat, and selected Sarah
Nostr code remain current implementation evidence under their own contracts.

### 3.5 Repository-wide Buzz corpus coverage

The audit triaged every repository path returned by a bounded case-insensitive
Buzz search outside dependency and Git metadata. The substantive corpus
included:

- the current authority chain in `AGENTS.md`, `INVARIANTS.md`, `FASTFOLLOW.md`,
  the Sol roadmap, the Omega roadmap, the Omega accepted plan, and the Forge
  ProductSpec;
- the Buzz teardown, all four `docs/buzz/` records, and the retained Nostr-
  first and Omega strategy analyses;
- Forge, ngit, Nostr, Sarah, identity, mobile, workroom, LiveKit, Mesh, Armada,
  Goose, TokenRelay, and related teardown comparisons;
- Forge conformance fixtures and collaboration admission/projection code;
- the preserved NIP-34 claim-ledger signer, store, relay, and subscription
  paths;
- the complete public NIP-29 chat package and its web/API surfaces; and
- selected Sarah identity, turn, owner-decryptable memory, persona, and
  community implementations and fixtures.

Incidental literal matches such as FizzBuzz, the BIP-39 word `buzz`, colloquial
prose, and administrative manifest entries supplied no Buzz product evidence
and were excluded after inspection.

## 4. The Buzz lessons that actually transfer

Buzz is useful because its architecture is not “put an AI bot in Slack.” It
makes people, agents, channels, messages, DMs, workflows, memory, telemetry,
Git coordination, search, and device behavior members of one signed event
system.

The transferable part is the protocol shape, not Buzz's product shell.

### 4.1 Adapt directly

| Buzz lesson                                                       | Linear adaptation                                                                                                                               |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| People and agents have separate signed identities                 | Bind each OpenAgents agent profile to its own Nostr identity and its distinct Linear app-user installation identity                             |
| The agent is a participant, not a hidden bot token                | Show the delegate, session, activity, and result on the Linear issue and in the OpenAgents workroom                                             |
| Channels are stable shared work contexts                          | Map a Linear workspace, team, or project to an explicit relay-qualified room projection; map an issue to a stable thread/work link              |
| Signed causal events connect messages, actions, and artifacts     | Project issue changes, delegations, activities, questions, PR refs, and closeouts as bounded causal references                                  |
| ACP workers are pooled and supervised                             | Route Linear sessions through OpenAgents' admitted runtimes, Pylons, and Agent Computers with typed stall and terminal states                   |
| One prompt per channel queue prevents overlapping turns           | Fence each Linear agent session and issue-linked run by generation and idempotency key                                                          |
| Agent memory is owner-decryptable and separately admitted         | Allow Linear-derived memories only through the existing memory admission and NIP-AE policy; never treat issue text as durable memory by default |
| Live metrics and control are projections, not billing truth       | Use NIP-AO and NIP-AM only as convenience projections; retain exact run and token ledgers in OpenAgents                                         |
| Approvals and workflow loop prevention are structural             | Separate suggested, approved, dispatched, verified, and accepted states; exclude a Loop's own output from its trigger set                       |
| Git coordination can be signed while Git objects remain canonical | Link Linear issues and reviews to NIP-34 or Forge coordination, while Git refs, commits, PRs, checks, and merges remain Git-host authoritative  |
| A shared skill source avoids harness drift                        | Compile OpenAgents Skills and Linear-facing instructions from one versioned source with explicit provider projections                           |
| Search is a typed operator compiler                               | Compile authorized Linear, workroom, event, and code queries; do not route hidden intent through keyword heuristics                             |
| Attention is a stable projection                                  | Combine Linear notifications, issue priority, mentions, agent questions, review requests, failures, and receipts in one bounded Inbox model     |
| Device pairing uses explicit ephemeral exchange                   | Enroll revocable device keys or remote signers; never copy a root `nsec` as the normal mobile path                                              |

### 4.2 Adapt with a stricter boundary

Buzz can treat its relay as the workspace. OpenAgents must not treat a Linear
projection relay as the authority for Linear or OpenAgents state.

The relay can be authoritative for one NIP-29 room's membership, moderation,
and event acceptance. That authority is confined to that room. It does not
grant:

- Linear workspace or team membership;
- issue read or write permission;
- repository access;
- an OpenAgents capability grant;
- an agent run or tool call;
- verification, acceptance, merge, release, payout, or settlement; or
- a hosted OpenAgents session.

The safe adaptation is therefore **Nostr-centric but not relay-sovereign over
product state**. Every important Linear or OpenAgents object has a signed Nostr
projection or causal reference when portability helps, but the owning service
still admits and records the canonical mutation.

### 4.3 Reject

Do not copy:

- Buzz's React/Tauri/Flutter application stack;
- a second standalone collaboration application;
- a Buzz deployment or Buzz agent home;
- relay membership as application authorization;
- broad credentials in an agent process;
- root-key copying as device enrollment;
- custom-kind proliferation before standard NIP composition is exhausted;
- a valid event signature as proof that an action was authorized or completed;
- a relay acknowledgement as receipt of a Linear or OpenAgents mutation;
- session counters as exact usage or billing evidence;
- NIP-29 `private` as an end-to-end-encryption claim; or
- a Nostr Git proposal as a merged Git ref.

## 5. Product model

### 5.1 First-class means more than a connector

Linear support is first-class only when all of these are true:

1. A Linear installation is a durable, owner-scoped OpenAgents object with
   visible health, scopes, revocation, and last-reconciled state.
2. Linear workspace, team, project, issue, document, comment, and agent-session
   references, including initiatives, cycles, milestones, dependencies,
   subissues, labels, customer requests, and SLA state where available, are
   typed objects, not URLs embedded in prompts.
3. Human assignee and agent delegate are distinct fields with distinct rights.
4. A Linear `AgentSession` maps to a durable OpenAgents work context, thread,
   and bounded run rather than a transient webhook handler.
5. Plans, progress, questions, actions, errors, artifacts, and results move in
   both directions through typed projections.
6. OpenAgents agents can receive explicit Linear-bound grants and can never use
   a generic provider tool to widen the subject.
7. The same issue-linked run can be viewed and controlled from Linear,
   OpenAgents Desktop, Omega, mobile, Pylon, and authorized Nostr clients
   without creating multiple authorities.
8. Git review and merge stay tied to the issue context while Git remains the
   source of truth.
9. Every delivery, trigger, run, action, writeback, verification, and owner
   decision has an idempotent receipt chain.
10. Relay loss, missed webhooks, external API lag, agent crash, and client
    disconnect produce honest degraded states rather than optimistic success.

### 5.2 Canonical work link

Introduce a stable owner-scoped `LinearWorkLink` contract:

```text
LinearWorkLink
  ref
  ownerScopeRef
  installationRef
  linearWorkspaceRef
  linearTeamRef
  linearInitiativeRef?
  linearProjectRef?
  linearCycleRef?
  linearMilestoneRef?
  linearIssueRef
  linearParentIssueRef?
  linearSubissueRefs[]
  linearDependencyRefs[]
  linearLabelRefs[]
  linearCustomerRequestRefs[]
  linearDocumentRefs[]
  linearAgentSessionRef?
  humanAssigneeRef?
  agentDelegateRef?
  workContextRef
  threadRef
  runRef?
  nostrRoomCoordinate?
  nostrRootEventRef?
  repositoryRefs[]
  pullRequestRefs[]
  authorityMode
  sourceVersion
  projectionGeneration
  lastReconciledAt
  receiptRefs[]
  blockerRefs[]
```

Identifiers are opaque, stable refs. Display URLs are optional projections and
must not become identity. A mapping cannot be inferred from title text, issue
number alone, project name, Nostr tag, or repository branch.

### 5.3 Authority modes

Every link declares one mode. There is no generic bidirectional-sync mode.

| Mode                              | Canonical owner                                            | Permitted projection                                                             |
| --------------------------------- | ---------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `linear_authoritative`            | Linear owns the issue/project fields and human assignment  | OpenAgents mirrors bounded context and writes only through exact Linear commands |
| `openagents_authoritative_linked` | OpenAgents owns the workroom/run/packet/verification state | Linear receives issue, session, activity, and result projections                 |
| `git_authoritative_review`        | Git host owns commit, PR, checks, review, and merge        | Linear and OpenAgents render context and issue subject-bound Git commands        |
| `imported_snapshot`               | The pinned source snapshot                                 | Read-only research or migration view; no writeback                               |

A user journey can compose multiple modes by field. One field or operation must
never have two canonical writers.

## 6. Linear capability map

### 6.1 Agent Platform and app users

Linear's app-user model should map to three distinct identities:

1. **Linear installation identity** — the OAuth application and Linear app user
   that receives sessions and performs allowed Linear actions.
2. **OpenAgents agent identity** — the durable agent profile, provider/runtime
   bindings, capabilities, policy, and activity history.
3. **Nostr agent identity** — the optional signed interoperability identity,
   with owner attestation and explicit signer custody.

The binding is durable and auditable. None of the three identities implies the
others. A Linear installation cannot mint an OpenAgents agent. A Nostr public
key cannot install an app or acquire a Linear team scope. An OpenAgents agent
cannot sign with a person's key.

### 6.2 Human assignee and agent delegate

This is the most important product primitive to preserve.

```text
Linear issue
  accountable human assignee
  bounded current agent delegate
  one or more agent sessions
  exact session state and result
```

OpenAgents should model the same split:

- `humanAccountableOwnerRef` remains the responsible person or admitted team
  role;
- `agentDelegateRef` identifies the current executor;
- `delegationGrantRef` defines the allowed subject, operations, budget, time,
  and termination conditions;
- `runRef` identifies execution;
- `verificationRef` identifies independent host verification; and
- `ownerDispositionRef` records acceptance, rejection, or a request for more
  work.

An agent never becomes a legal, financial, employment, release, or product
accountability subject merely because Linear shows it as the delegate.

### 6.3 AgentSession and AgentActivity

Map Linear's session and activity surface onto existing OpenAgents semantics:

| Linear object  | OpenAgents meaning                                | Projection rule                                                                              |
| -------------- | ------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `AgentSession` | `workContextRef` + `threadRef` + bounded `runRef` | One durable mapping, generation-fenced                                                       |
| `thought`      | Public-safe progress summary                      | Never expose hidden chain of thought, private prompts, or raw model traces                   |
| `elicitation`  | Durable pending human interaction                 | Preserve choices, deadline, requesting actor, and resolution                                 |
| `action`       | Typed tool/action attempt and outcome             | Include exact subject, grant, idempotency, and receipt refs                                  |
| `response`     | Provider or agent result                          | Never label it verified or accepted without separate records                                 |
| `error`        | Typed failure or blocker                          | Distinguish retryable, terminal, policy-denied, stale, interrupted, and external-unavailable |
| plan           | OpenAgents plan or ProductSpec packets            | Preserve step identity and state; do not flatten dependency or verification gates            |
| session URL    | Deep link to the owning surface                   | Convenience only; never identity or authority                                                |

The ingress handler must acknowledge a valid Linear session webhook quickly,
durably enqueue the work, and publish a first safe activity within Linear's
expected responsiveness window. It must not perform the entire agent run before
responding to the webhook.

### 6.4 Skills

Linear Skills should be projections of versioned OpenAgents skill or procedure
packages, not a second drifting instruction store.

Each projected skill needs:

- stable skill ref, revision, content digest, source owner, and publication
  state;
- intended teams, agents, tools, and subject types;
- required context and expected output;
- explicit exclusions and stop conditions;
- budget and verification policy;
- compatible Linear activity representation; and
- a record of the exact revision used by each run.

Provider-specific prompt text is a compiled projection. It is not the skill's
authority or proof that the procedure was followed.

### 6.5 Guidance

Workspace and team Guidance should become a typed policy and knowledge package
with explicit precedence:

```text
repository invariant and ProductSpec
  > owner-scoped policy
  > workspace guidance
  > team guidance
  > project guidance
  > issue-specific instruction
  > agent default
```

The actual precedence must be selected by the owning product contract. The
important law is that conflict is visible, deny is fail-closed, and the run
records the revisions it used. Natural-language Guidance cannot grant a tool,
widen a repository, change a budget, bypass verification, or override a higher
authority.

### 6.6 Loops

Linear Loops map well to OpenAgents background agents, but OpenAgents should
make their runtime contract stronger.

Each Loop needs:

- immutable draft and published revisions;
- trigger kind, source, filters, owner scope, and exact subject mapping;
- normalized source-event digest and dedupe key;
- explicit agent, skill, tool policy, budget, concurrency, and schedule;
- confidence or evidence gate before mutation;
- human ask/approve checkpoints;
- circuit breakers for repeated failure, cost, time, and external outages;
- a structural rule excluding the Loop's own writebacks from retriggering it;
- visible run history, current state, and next eligible time; and
- pause, drain, resume, revoke, and archive behavior.

The first implementation should be “suggest, ask, then write,” not “mutate all
matching issues automatically.”

### 6.7 Triage Intelligence

Triage is an advisory pipeline before it is an autonomous mutation path.

```text
verified Linear issue event
  -> bounded context fetch under the triggering principal
  -> classification proposal
  -> confidence + evidence refs
  -> human disposition or admitted auto-apply policy
  -> exact Linear mutation
  -> canonical response + signed projection + receipt
```

Suggestions should cover duplicate candidates, team, label, priority, project,
and routing. Each suggestion must expose source refs and confidence. A low-
confidence result asks or stops. An explanation is useful context, not proof of
correctness.

### 6.8 Code Intelligence

Linear-linked code intelligence should use the same repository permission
intersection as execution:

```text
effective code access
  = OpenAgents owner grant
  ∩ Linear triggering-principal permission
  ∩ connector installation scope
  ∩ repository-host permission
  ∩ task-local path and tool policy
```

Answers should return file, symbol, commit, PR, and evidence refs. They must not
copy private source into Linear or Nostr unless the explicit audience and data
policy permit it. Code search does not grant code mutation.

### 6.9 Coding Sessions

A Linear coding session should become a normal OpenAgents run placement, not a
special unreceipted cloud path.

The session can run on:

- an owner-local Pylon;
- an admitted Agent Computer;
- an admitted managed environment;
- an attached external agent runtime; or
- a future separately admitted provider lane.

The run records requested and effective runtime, model, account, workspace,
commit, containment, tools, grants, budget, and verification. Linear receives
safe activity and artifact refs. Raw provider events, local paths, credentials,
private prompts, and private repository material remain on the owning host.

### 6.10 Diffs and Guided Reviews

Linear Diffs proves the value of reviewing code beside product intent. The
OpenAgents design should preserve that outcome without treating Linear as a Git
forge.

- Git host or OpenAgents Forge owns Git objects, refs, PR state, checks,
  comments, review state, and merge outcome.
- Linear owns its issue association and Linear-side review projection.
- OpenAgents owns the run, evidence, verification, AssuranceSpec evaluation,
  and owner disposition.
- Nostr can carry signed NIP-34 coordination, guide refs, review requests, and
  closeout refs.

Guided Reviews should be evidence-linked, regenerable projections. They can
order a change into conceptual chapters and point to exact diff spans, tests,
requirements, and risks. They cannot approve, verify, or merge a change.

### 6.11 MCP

Linear MCP is useful for discovery and bounded operator flows. It must not be
the only integration boundary.

First-class support needs a typed connector because the product requires:

- durable app installation and revocation;
- verified event ingress;
- stable subject mapping;
- exact activity timing;
- idempotent outbound actions;
- reconciliation after missed webhooks;
- agent app-user identity;
- explicit data redaction; and
- receipts independent of a model's tool transcript.

An MCP server can remain an optional interactive adapter over those contracts.
Generic “call Linear” tools are forbidden. Each tool binds provider, actor,
subject kind, subject ref, operation, and idempotency key.

### 6.12 Inbox, notifications, and mobile

Adapt Buzz's attention model and Linear's review queue into one stable Inbox
projection. Candidate attention classes are:

- issue assigned to the human;
- issue delegated to an agent;
- session waiting for input;
- agent plan or scope changed;
- action denied or run stalled;
- review requested;
- check failed;
- verification complete;
- writeback conflict;
- owner disposition required; and
- connector, relay, signer, or runtime degraded.

Mobile should support read, answer, steer, approve or reject where authorized,
pause, stop, and review. It must use the same commands and receipts as desktop;
it does not create a mobile authority plane.

### 6.13 Credits and budgets

Do not copy a pooled credit balance as the only guard.

Each run must bind:

- owner and funding scope;
- per-run and rolling time/token/cost ceilings;
- allowed provider accounts and fallback order;
- retry and failure charging policy;
- approval threshold;
- exact usage rows; and
- terminal budget disposition.

A NIP-AM event, Linear activity, provider counter, or agent claim is a
projection. It is not billing or settlement truth.

### 6.14 Product planning and customer context

Linear's advantage is not only issue execution. Its initiatives, roadmaps,
projects, cycles, milestones, dependencies, subissues, documents, customer
requests, and SLA signals explain why work matters and what it can block.

OpenAgents should absorb that graph as typed, permissioned context:

- preserve external object type, stable ref, source version, state, parent,
  ordered relation, and audience;
- retain dependency direction and distinguish blocked-by from blocking;
- bind each summary or context package to the source revisions it used;
- let planning context rank attention but never silently widen execution;
- keep customer identities and request bodies out of public receipts and Nostr
  projections;
- use product milestones and cycles as scheduling inputs, not proof of urgency
  or permission; and
- return accepted outcomes and verified artifact refs to the exact originating
  issue, project, initiative, and customer-request links.

The first slice does not need to mirror the entire graph. Its schemas must avoid
an issue-only dead end so later planning and customer context can attach without
changing identity or authority semantics.

## 7. Nostr profile for Linear-linked work

### 7.1 Design law

Use standard NIPs first, the already selected Buzz companion profiles second,
and new custom kinds only after an explicit collision and interoperability
review.

Do not depend on NIP-31 as the fallback strategy. The repository's current NIP
candidate audit records it as unrecommended upstream. Unknown events must have
a current, explicit application-handler and compatibility story.

### 7.2 Initial composition

| Need                                        | Protocol candidate                                     | OpenAgents boundary                                                    |
| ------------------------------------------- | ------------------------------------------------------ | ---------------------------------------------------------------------- |
| Signed event envelope and causal refs       | NIP-01 and NIP-10                                      | Signature proves key control only                                      |
| Relay-qualified team/project room           | NIP-29                                                 | Room authority remains relay-scoped                                    |
| Private person/agent exchange               | NIP-17 over NIP-44 and NIP-59                          | Audience and signer grants remain host-admitted                        |
| Addressable app-specific mapping projection | NIP-78 or another current standard addressable profile | Canonical `LinearWorkLink` remains in OpenAgents                       |
| Handler discovery                           | NIP-89                                                 | Advertisement is not capability admission                              |
| HTTP proof                                  | NIP-98                                                 | Proof binds one request; hosted session and Linear OAuth stay separate |
| Remote signer                               | NIP-46                                                 | Signer consent does not authorize the product action                   |
| Relay preferences                           | NIP-65                                                 | Relay choice does not change product authority                         |
| Event expiration                            | NIP-40                                                 | Best-effort relay deletion is not guaranteed erasure                   |
| Code coordination                           | NIP-34                                                 | Git objects and refs remain authoritative in Git                       |
| Files and large evidence                    | Blossom with NIP-94 and NIP-92                         | Signed manifest and digest; private artifacts remain access-controlled |
| Agent owner proof and auth                  | Buzz NIP-OA and NIP-AA profiles                        | Attestation and session auth remain bounded                            |
| Agent persona                               | Buzz NIP-AP                                            | Persona is not a capability grant                                      |
| Owner-decryptable memory                    | Buzz NIP-AE                                            | Separate memory admission; Linear content is not memory by default     |
| Live status and usage projection            | Buzz NIP-AO and NIP-AM                                 | Never accounting, acceptance, or health authority                      |
| Ref-only task request/result                | NIP-90 or OpenAgents NIP-LBR                           | Host admits execution; result is not verification                      |
| Read/attention state                        | Buzz NIP-RS pattern                                    | User-private projection, not canonical issue state                     |
| Reminder and push hints                     | Buzz NIP-ER and NIP-PL patterns                        | Scheduler and delivery ledgers remain canonical                        |
| Git signatures                              | Buzz NIP-GS pattern                                    | Complements, never replaces, Git verification                          |

### 7.3 The signed projection graph

The Nostr graph should carry only the minimum useful material:

```text
relay-qualified project room
  └── Linear issue root projection
      ├── accountable owner + bounded delegate refs
      ├── context digest + authorized source refs
      ├── delegation request
      ├── session acknowledgement
      ├── safe plan/progress events
      ├── elicitation and answer refs
      ├── action and artifact receipt refs
      ├── PR/review/check refs
      ├── provider result
      ├── host verification
      └── owner disposition / closeout
```

The graph is useful because any authorized client can rebuild chronology and
provenance from signed events. It is not sufficient to rebuild private Linear
state or OpenAgents authority from the relay alone.

### 7.4 Content policy

Default Nostr projection is **ref-only and public-safe**:

- opaque owner, workspace, team, project, issue, thread, run, and receipt refs;
- state and activity kinds;
- content and artifact digests;
- bounded summaries approved for the audience;
- signer, schema, revision, idempotency, and causal refs; and
- public repository or PR refs only when already public.

Never publish these to public or third-party relays:

- Linear OAuth tokens or webhook secrets;
- raw webhook bodies;
- private issue, comment, document, customer, or attachment content;
- workspace or channel names that disclose private organization data;
- raw prompts, model traces, chain of thought, provider payloads, or session
  histories;
- private repository source, paths, patches, logs, or artifact URLs;
- emails, phone numbers, customer identifiers, or private profile data;
- provider credentials or account metadata;
- wallet, payment, invoice, or settlement material; or
- unrestricted bearer links.

NIP-44 encryption protects content from a relay operator under the intended
key model, but ciphertext metadata, retention, recipient mistakes, key loss,
and copied relay data remain risks. A NIP-29 `private` tag is access policy, not
end-to-end encryption.

### 7.5 Identity and permission intersection

For an action originating from a Nostr-linked Linear context:

```text
effective authority
  = verified Nostr signer binding
  ∩ hosted OpenAgents owner/session scope
  ∩ active OpenAgents capability grant
  ∩ active Linear installation scope
  ∩ triggering Linear principal permission
  ∩ exact mapped subject
  ∩ operation policy
  ∩ remaining budget and time
```

For a Git action, add the effective Git-host principal and repository policy.
If any term is missing, stale, or ambiguous, deny or ask.

## 8. Connector and synchronization architecture

```text
Linear API / webhooks / agent sessions
                 |
                 v
Google Cloud Linear connector
  - OAuth token broker
  - signature and replay verification
  - delivery acknowledgement
  - object normalizer
  - reconciliation reader
                 |
                 v
Cloud SQL connector ledger + LinearWorkLink authority
  - exact refs and source versions
  - dedupe and generation fencing
  - trigger decisions
  - command inbox/outbox
                 |
       +---------+----------+
       |                    |
       v                    v
OpenAgents workroom/run     signed Nostr projection outbox
  - grants and budgets        - room/thread refs
  - execution and tools       - identity and causal events
  - evidence/verification     - safe progress/closeout refs
  - owner disposition         - relay acknowledgements
       |                    |
       +---------+----------+
                 |
                 v
subject-bound Linear writeback
  - app/user actor selection
  - idempotency and source-version check
  - canonical Linear response
  - writeback receipt and reconciliation
```

### 8.1 Linear to OpenAgents

1. Receive raw bytes and headers on the host-side connector.
2. Verify the exact Linear signature scheme, timestamp window, installation,
   and delivery identity before parsing into trusted fields.
3. Persist a minimal delivery envelope and dedupe key. Do not persist or expose
   the raw body beyond the bounded verification need.
4. Return a durable acknowledgement before long-running work.
5. Normalize the exact object, actor, subject, source version, audience, and
   source refs.
6. Resolve the owner-scoped installation and `LinearWorkLink`.
7. Record stale, duplicate, conflicting, unmapped, or revoked deliveries as
   typed dispositions.
8. Evaluate trigger policy and capability intersection.
9. Start or update an OpenAgents workroom/run only after admission.
10. Append safe Linear and Nostr projections through their own durable outboxes.

### 8.2 OpenAgents to Linear

1. Accept a typed intent, not a generic provider request.
2. Bind installation, actor, subject kind, subject ref, operation, expected
   source version, grant, and idempotency key.
3. Recheck installation health, current permission, policy, budget, and
   generation.
4. Perform the exact external request through the host-side token broker.
5. Record response status, external operation ref, returned source version,
   public-safe failure, and reconciliation deadline.
6. Apply canonical Linear state only from Linear's response or a later verified
   read, never from the outgoing request.
7. Emit the OpenAgents receipt and Nostr projection after canonical recording.
8. Retry only when the operation and provider contract are idempotent. Otherwise
   reconcile before another attempt.

### 8.3 Missed webhooks and drift

Webhook delivery is acceleration, not the only truth source. The connector
needs bounded reconciliation by installation and object cursor:

- compare source versions and last-reconciled time;
- recover missed issues, comments, activities, and session transitions;
- detect objects deleted or made inaccessible;
- stop projecting content after scope revocation;
- preserve tombstones and public-safe loss receipts where allowed; and
- never ask a user to perturb an external object merely to provoke a webhook.

### 8.4 Deletion and retention

Linear deletion or revoked access must:

1. mark the link inaccessible or tombstoned;
2. stop new context fetches and writes;
3. purge derived plaintext caches according to policy;
4. revoke affected agent context grants;
5. rebuild search, memory, and attention projections without the source;
6. publish only allowed tombstone or revocation refs; and
7. state honestly that Nostr events copied by independent relays or peers may
   not be erasable.

This is why private Linear bodies should not be placed on public relays, even
when encrypted.

## 9. First-class user journeys

### 9.1 Install and bind

1. An owner installs the OpenAgents Linear app.
2. The UI previews requested workspace, team, object, webhook, and write scopes.
3. The host stores credentials in the credential broker, never the renderer.
4. The owner binds selected Linear teams/projects to OpenAgents work contexts
   and optional NIP-29 room coordinates.
5. A dry-run reconciliation reports accessible objects, missing scopes,
   ambiguous mappings, and privacy posture.
6. The owner explicitly publishes the binding revision.

### 9.2 Delegate a Linear issue to an OpenAgents agent

1. The human remains the accountable assignee.
2. The human selects an OpenAgents app user or invokes an admitted Loop.
3. Linear creates or updates an `AgentSession` and sends a verified event.
4. OpenAgents durably acknowledges, resolves the work link, checks the grant,
   and publishes an initial activity.
5. The run receives only the authorized context package and repository scope.
6. OpenAgents streams safe plan, progress, questions, and artifact refs to
   Linear and the signed Nostr thread.
7. A result remains `provider_completed` until host verification finishes.
8. Linear shows the PR or artifact, verification state, and required human
   disposition.
9. Acceptance, merge, and release remain separate authorized actions.

### 9.3 Agent asks a question

1. The run creates a durable elicitation with choices, deadline, and blocker.
2. Linear, OpenAgents clients, and the authorized Nostr thread receive the same
   projection.
3. The first valid answer wins under generation fencing.
4. Other clients converge on the answer receipt.
5. Timeout follows the declared stop, default, or escalation policy; it never
   silently invents permission.

### 9.4 Triage a new issue

1. Verified issue delivery enters the owner-private event ledger.
2. A published Loop revision matches the exact team and filter.
3. The agent gathers only authorized issue, project, code, and history context.
4. It publishes a classification and duplicate proposal with confidence and
   source refs.
5. A human accepts, edits, or rejects during the first slice.
6. The connector performs exact subject-bound updates and records their Linear
   versions.

### 9.5 Review agent-written code

1. The issue links to a Git-host PR and OpenAgents run.
2. The review surface composes product intent, plan, changed files, tests,
   checks, evidence, and risk.
3. A Guide groups the diff by implementation purpose with exact source spans.
4. Review comments are written under the effective Git user, not an ambient
   service identity.
5. Agent follow-up creates a new generation of the same bounded run or a linked
   run.
6. Host verification reruns against the resulting commit.
7. Human approval and merge are distinct receipts.

### 9.6 Relay, connector, or runtime failure

- Relay down: retain canonical Linear/OpenAgents operation, queue or mark the
  signed projection, and show a Nostr-degraded state.
- Linear API down: do not claim writeback; retain the intent and reconcile.
- Webhook delayed: reconciliation catches up by source version.
- Agent crashed: show interrupted or stalled, preserve activities and
  artifacts, and require an admitted retry or replacement.
- Signer unavailable: preserve unsigned draft state; never silently switch
  identity.
- Permission revoked: stop context fetch and mutation immediately, revoke
  dependent grants, and surface the exact blocked subject.

## 10. Surfaces

### 10.1 OpenAgents Desktop now

The retained Desktop can validate the contracts before Omega cutover:

- Linear installation and mapping health;
- issue-linked workroom header and context rail;
- human-owner and agent-delegate badges;
- session plan, activities, elicitation, artifacts, and errors;
- pause, stop, retry, and owner disposition controls;
- PR, checks, diff, evidence, and verification refs; and
- connector, runtime, signer, and relay degradation.

### 10.2 Omega target

Omega is the eventual native first-class surface. It should render the same
monorepo-owned TypeScript/Effect contracts through its generated local protocol
and supervised Rust components.

Candidate native structure:

```text
Project panel
  Linear
    My issues
    Delegated
    Waiting for me
    Reviews
    Projects / teams

Editor center
  issue / document / diff / guide / artifact

Agent panel
  active Linear-linked session
  plan + activity + elicitation + controls

Inspector
  authority + scopes + source versions + receipts + Nostr refs
```

This does not authorize an Omega implementation packet. The separate Omega
repository owns Rust/GPUI implementation and must consume admitted monorepo
contracts.

### 10.3 Web

Web should supply authenticated installation, scope, mapping, health, Loop,
Guidance, history, and receipt administration through an admitted existing
surface. Do not create a new public `/linear` product route from this document;
the root web-surface contract and ProductSpec must admit any route change.

### 10.4 Pylon and API

Pylon should expose bounded operator commands over the same contracts:

```text
pylon linear installations list
pylon linear mappings list
pylon linear issue inspect <ref>
pylon linear session status <ref>
pylon linear session run <ref>
pylon linear session closeout <ref>
pylon linear reconcile --installation <ref>
```

Names are illustrative, not a CLI contract. Commands must return public-safe
typed JSON, never tokens or raw external payloads.

## 11. Security and abuse model

### 11.1 Untrusted Linear content

Issue bodies, comments, documents, attachments, customer messages, MCP output,
and pasted logs are untrusted data. They can contain prompt injection or
malicious instructions.

The context compiler must:

- label source, actor, object, audience, retrieval time, and source version;
- separate data from authority-bearing instructions;
- reject requests to widen tools, repositories, identities, budgets, or
  disclosure;
- sandbox or exclude attachments by type and policy;
- retain provenance through summaries; and
- require an explicit admitted instruction source for mutations.

### 11.2 Credential custody

- OAuth tokens and webhook secrets remain host-side.
- Desktop, Omega GPUI, web renderer, mobile, agent prompt, Nostr event, logs,
  receipts, and crash reports receive only public-safe refs and status.
- Use per-installation rotation and revocation.
- Distinguish app identity from a connected human Linear identity.
- Use the least Linear scopes compatible with each operation.

### 11.3 Confused-deputy prevention

Every command binds the exact installation, actor, owner scope, team, object,
operation, source version, grant, and idempotency key. A model cannot supply an
arbitrary workspace id or tool name and thereby redirect a valid token.

### 11.4 Cross-tenant and cross-team isolation

The 2026 Linear access-control incident documented in the Linear teardown is a
direct product warning. Tests must cover:

- private team versus workspace-wide objects;
- guests and restricted members;
- installation scopes changed after mapping;
- actor-specific versus app-level reads and writes;
- same issue identifiers in different workspaces;
- cached context after access revocation;
- search and memory results crossing team boundaries; and
- Nostr room membership that does not match Linear membership.

### 11.5 Replay and duplication

Webhook delivery id plus installation and payload digest form the initial
dedupe identity. A repeated delivery cannot create another run. A changed
payload under a reused delivery id is a conflict and security signal. All
outbound actions use app-owned idempotency keys and source-version checks.

### 11.6 Autonomy and runaway Loops

Use per-Loop concurrency, daily run/time/token/cost ceilings, repeated-failure
circuit breakers, tool deny defaults, confidence gates, and self-trigger
exclusion. A human can pause or revoke the Loop without waiting for the agent.

## 12. Ordered candidate work packets

These are research candidates, not admitted implementation issues.

### LNOA-00 — authority and contract freeze

Define Linear refs, installation, actor, object, work-link, session, activity,
trigger, writeback, reconciliation, privacy, and receipt schemas. Name the
authority owner for every field and operation. Revalidate current Linear API,
webhook, OAuth, AgentSession, AgentActivity, and rate-limit contracts.

Exit: conformance fixtures and an authority matrix fail closed on unknown
providers, objects, actors, scopes, versions, and operations.

### LNOA-01 — installation and read-only ingress

Implement host-side OAuth custody, installation health, exact webhook
verification, durable early acknowledgement, delivery dedupe, event-ledger
normalization, read-only object fetch, and reconciliation.

Exit: one private test workspace can deliver and reconcile issue/comment events
without a duplicate run or raw-body/token leak.

### LNOA-02 — work links and human/delegate identity

Land `LinearWorkLink`, subject mapping, source-version fencing, human assignee,
agent delegate, app-user binding, and revocation behavior.

Exit: changing assignee, delegate, team access, or installation scope converges
without collapsing identities or retaining unauthorized context.

### LNOA-03 — AgentSession and activity bridge

Map one Linear agent session to a durable OpenAgents workroom/thread/run. Return
fast acknowledgement, safe progress, elicitation, action, response, and error
activities. Keep provider completion separate from host verification and owner
disposition.

Exit: a bounded no-spend fixture completes end to end with typed failure,
interrupt, resume, and stale-generation coverage.

### LNOA-04 — subject-bound writeback

Add exact tools for comment, issue property suggestion/application, delegate
state, session activity, and result refs. Use actor-aware scopes, app-owned
idempotency, outbox, canonical response recording, and reconciliation.

Exit: retries cannot duplicate comments or mutate the wrong issue; stale and
revoked writes fail closed.

### LNOA-05 — signed Nostr projection

Define and implement the standard-NIP-first Linear projection profile, signer
binding, safe content policy, durable outbox, relay receipts, reconnect, and
rebuild. Reuse NIP-LBR for ref-only delegation where appropriate.

Exit: two authorized clients rebuild the same causal Linear-linked session;
relay loss does not change canonical Linear/OpenAgents state; private data does
not enter public fixtures.

### LNOA-06 — Skills, Guidance, and Triage

Compile versioned Skills and hierarchical Guidance into run context. Add
suggest-only triage with confidence, source refs, disposition, and exact
writeback.

Exit: conflicting Guidance fails closed; suggestion acceptance and rejection
are measurable; no unapproved low-confidence mutation occurs.

### LNOA-07 — Loops

Add immutable draft/published revisions, Linear triggers, schedules, filters,
budgets, circuit breakers, run history, self-trigger prevention, and pause/
drain/revoke.

Exit: duplicate delivery, writeback echo, retry storm, and external outage do
not cause duplicate or unbounded runs.

### LNOA-08 — coding, diffs, and verification

Bind issue-linked coding sessions to exact repo/commit/verification, project
safe activity to Linear, associate PRs, render evidence-linked Guided Reviews,
and preserve Git authority.

Exit: the first end-to-end real repository task reaches a PR, host
verification, human review, merge disposition, and exact closeout without
claiming that Linear or Nostr performed the merge.

### LNOA-09 — first-class clients and live acceptance

Ship the admitted Desktop/web contract surface, then Omega and mobile packets.
Exercise multi-client control, reconnect, missed webhooks, relay failure,
permission revocation, token rotation, deletion, and accessibility.

Exit: one owner can start in Linear, observe and steer in OpenAgents, answer on
mobile, review the PR, and close out from any authorized surface with one
canonical history.

## 13. First proof to build

The first proof should be deliberately narrow:

> A human delegates one Linear issue to an OpenAgents app user while remaining
> the assignee. A verified Linear webhook creates one durable OpenAgents
> workroom/run. An owner-local Pylon performs a no-spend fixture against a
> pinned repository and verification command. Safe plans, progress, a question,
> result, host verification, and closeout refs return to Linear and a signed
> Nostr thread. Replay, relay outage, and permission revocation do not duplicate
> work or leak private material.

This slice proves the essential architecture before Loops, automatic triage,
Guided Reviews, broad code intelligence, or spendful execution.

## 14. Acceptance and falsification matrix

| Claim                                   | Required proof                                                                     | Falsifier                                                            |
| --------------------------------------- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| Human accountability is preserved       | Linear and OpenAgents show human owner plus bounded delegate through the full run  | Agent replaces or obscures the accountable human                     |
| Ingress is authentic and idempotent     | Signed fixture, timestamp/replay tests, one delivery/one run, conflict detection   | Duplicate or forged delivery starts work                             |
| Permissions do not widen                | Cross-team, guest, revoked-scope, actor, and repository tests                      | Any cached or projected data survives beyond permission              |
| Session state is honest                 | Provider completion, verification, and owner disposition are separate              | Linear `response` or Nostr result appears as verified/accepted       |
| Writeback is subject-bound              | Wrong workspace/team/issue/version/actor tests fail closed                         | Valid token mutates an unbound object                                |
| Nostr is portable but not authoritative | Two-client rebuild plus relay-down operation                                       | Relay event creates a canonical Linear/OpenAgents mutation by itself |
| Private content stays private           | Redaction tests across event, prompt, log, activity, receipt, relay, and export    | Raw private body, token, path, prompt, or provider data appears      |
| Git authority remains intact            | PR/ref/check/review/merge reconciled from Git host                                 | Linear or a Guide invents canonical Git state                        |
| Loops are bounded                       | Budget, concurrency, self-trigger, circuit-breaker, pause, and outage tests        | One event creates an unbounded or recursive run chain                |
| Recovery is loss-accounted              | Missed webhook, API outage, relay gap, crash, stale generation, and resume tests   | UI shows success or current state without canonical evidence         |
| Deletion is honest                      | Cache purge, grant revoke, tombstone, and residual-relay disclosure                | Product promises global erasure it cannot prove                      |
| First-class clients share one model     | Desktop, Omega/mobile target, web, Pylon, and Linear converge on refs and receipts | A client invents its own state or command authority                  |

## 15. Current gaps

### Landed foundations

- verified GitHub and Slack webhook patterns;
- connector event normalization, subject binding, and idempotency vocabulary;
- owner-private event ledger and public-safe gateway projection;
- background-agent budgets, tool policy, triggers, and circuit-breaker
  foundations;
- stable agent profiles and safe agent cards;
- durable workrooms, packets, leases, evidence, verification refs, and owner
  disposition;
- Khala Sync conversations, timelines, attention, coding sessions, live agent
  graphs, and cross-client projections;
- Pylon, owner-local execution, managed environment, and agent-computer paths;
- NIP-29 public chat mechanics, NIP-42 auth, signer boundaries, and relay
  health behavior;
- NIP-LBR ref-only labor requests, results, and closeout receipts;
- NIP-34/Forge coordination with Git-object authority; and
- Omega Nostr account-link and signer architecture evidence.

### Missing Linear-specific product

- Linear OAuth installation, credential custody, refresh, revoke, and health;
- Linear webhook signature/replay validation and durable early acknowledgement;
- Linear workspace/team/project/issue/comment/document/session/activity schemas;
- Linear initiative/cycle/milestone/dependency/subissue/label/customer-request
  and SLA projections;
- event-ledger source, migration, reconciliation, and retention behavior;
- `LinearWorkLink` and source-version authority;
- human-assignee/agent-delegate structural identity;
- Linear AgentSession and AgentActivity adapter;
- Linear-scoped grants and subject-bound writeback tools;
- Skills and Guidance projection with revision and precedence;
- Linear Loop trigger/action support and self-trigger prevention;
- triage confidence/evidence/disposition contracts;
- code-context permission intersection;
- issue-linked coding and review composition;
- Nostr Linear projection profile and content policy;
- first-class Desktop, Omega, web, mobile, and Pylon surfaces;
- live external acceptance, outage, drift, deletion, and cross-tenant evidence;
  and
- an admitted ProductSpec, AssuranceSpec, roadmap packet, and issue set.

## 16. Non-goals and hard rejections

- Do not reproduce Linear's closed implementation or branding.
- Do not replace Linear issue/project authority with Cloud SQL or Nostr.
- Do not make a Nostr relay the OpenAgents workroom, permission, or execution
  authority.
- Do not create a second Forum or Forge.
- Do not restore the canceled Buzz deployment path.
- Do not put Linear OAuth or private content in Omega, Desktop renderers, agent
  prompts beyond admitted context, Nostr events, or receipts.
- Do not make Linear team membership and NIP-29 room membership equivalent.
- Do not use an agent as the accountable human owner.
- Do not let an AgentSession or AgentActivity become proof of completion.
- Do not let a Guide become review approval or verification.
- Do not implement generic provider tools or ambient organization-wide access.
- Do not start with autonomous broad triage, issue mutation, or spendful Loops.
- Do not claim NIP-29 privacy is encryption.
- Do not claim Nostr deletion is global erasure.
- Do not treat NIP-90 advertisement or result as provider capability,
  completion, payment, or settlement authority.
- Do not add a new public web product route without separate authority.
- Do not treat this document as an implementation or release receipt.

## 17. Final recommendation

Admit a first-class Linear program after the current roadmap owner selects its
position. Begin with the single end-to-end delegated-issue proof in section 13.

The enduring opportunity is larger than a connector. Linear has assembled the
best current vertical model for product context, accountable delegation, shared
agent sessions, recurring workflows, coding, and integrated review. Buzz shows
how a collaboration product can become a portable signed event graph.
OpenAgents already has the pieces Linear and Buzz do not make authoritative:
durable multi-agent execution, typed grants, budgets, placement, verification,
receipts, owner disposition, and cross-client workrooms.

Composed correctly, Linear becomes a first-class doorway into OpenAgents and
OpenAgents becomes a first-class execution substrate under Linear. Nostr makes
that relationship portable and inspectable without pretending that a relay,
signature, activity, or vendor session is the final authority.

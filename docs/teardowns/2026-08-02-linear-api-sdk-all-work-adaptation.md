# Linear API and TypeScript SDK: All Work adaptation and code-reuse plan

- Date: 2026-08-02
- Lane: Fast Follow research
- Disposition: adopt the API vocabulary broadly; fork selected MIT codegen;
  generate OpenAgents-owned clients from OpenAgents-owned contracts
- Implementation authority: none; each code import or product change still needs
  a separately admitted packet

## Executive decision

The public Linear repository changes the answer from the earlier product-only
teardown.

Linear's application implementation is not in this repository, but its API
schema, TypeScript SDK, GraphQL document generator, SDK generator, generated
test generator, webhook client, error model, and import tooling are public
under the MIT License. This is reusable code, not merely behavioral evidence.

OpenAgents should do three things:

1. **Adopt most of the public API vocabulary.** Use `Organization`, `Team`,
   `Initiative`, `Roadmap`, `Project`, `ProjectStatus`, `ProjectMilestone`,
   `Cycle`, `Issue`, `IssueRelation`, `WorkflowState`, `IssueLabel`, `Comment`,
   `Document`, `Attachment`, `Customer`, `CustomerNeed`, `ProjectUpdate`,
   `InitiativeUpdate`, `Favorite`, `Notification`, `AgentSession`,
   `AgentActivity`, `AgentSkill`, `Webhook`, `Integration`, and `AuditEntry`
   where those nouns have the same meaning in OpenAgents.
2. **Fork the generator architecture, not the generated Linear client.** The
   valuable code is the schema visitor and printer pipeline that derives
   documents, model classes, root and nested operations, Relay connections,
   and tests. The generated client is a service-specific output and should be
   regenerated from an OpenAgents schema.
3. **Keep the All Work ontology above that API.** `Work` remains the universal
   lifecycle object. `Issue` is the concrete tracking and planning projection
   used by Linear-class lists, boards, cycles, triage, and API ergonomics. It
   shares Work identity and cannot become a second authority. Projects,
   initiatives, cycles, milestones, customers, and updates are optional
   organization layers for every applicable Work Domain, not a declaration
   that OpenAgents is only a product-development tool.

The concise target is:

```text
OpenAgents Contract Profile
  -> All Work domain and authority contracts
  -> GraphQL API projection with familiar resource names
  -> adapted MIT document/SDK/test generators
  -> Effect-native TypeScript SDK plus generated Rust boundary bindings
  -> Omega, web, mobile, Pylon, agents, and third-party clients
```

This keeps the excellent API shape while removing the product-development
ceiling and the Linear service dependency.

## 1. Pinned source and evidence boundary

The source inspected for this study is the clean local checkout at
`~/work/projects/repos/linear`.

| Field | Value |
| --- | --- |
| Repository | `https://github.com/linear/linear.git` |
| Commit | `7ef4c5024f88667b2c85057ff4c905676c4a93c2` |
| Tree | `8b0d767165bbac53ef3e1beed2121757077be3f7` |
| Branch | `master` tracking `origin/master` |
| SDK package | `@linear/sdk` `89.0.0` |
| License | MIT, Copyright (c) 2019 Linear |
| License digest | `6d01159ad48e1c4a2c053ec32a97afc658786aa69362ead7d3adc912f9a10ce2` |
| GraphQL schema digest | `11c549e81063b746b046fa272b4fd018e289768a3a1633e29bfabff77bdc16ec` |

The repository contains:

| Artifact | Observed size | What it proves |
| --- | ---: | --- |
| `packages/sdk/src/schema.graphql` | 50,261 lines | Public production GraphQL vocabulary and structural surface |
| generated GraphQL documents | 24,744 lines | Schema-wide fragment and operation generation |
| generated typed documents | 151,143 lines | GraphQL Code Generator TypeScript operation types |
| generated SDK | 53,458 lines | Model, query, mutation, and connection ergonomics |
| generated SDK test | 6,844 lines | Broad generated compile/execution coverage |
| object types | 667 | Entity, connection, edge, payload, webhook, and support objects |
| input types | 389 | Create, update, filter, relation, and operation inputs |
| enums | 113 | Workflow, agent, notification, integration, and product states |
| root queries | 164 | Query coverage at the pinned schema |
| root mutations | 372 | Mutation coverage at the pinned schema |

Core donor file digests at the same pin:

| File | SHA-256 |
| --- | --- |
| `packages/sdk/src/client.ts` | `a813848308f592817630d58098e5b746ee2ca9e7bf0d24bb2287dda5b908397a` |
| `packages/sdk/src/graphql-client.ts` | `513b83ee7a47acfb5ed8b3d946699cb0e83a15a4013af523773d5ffc13b26627` |
| `packages/sdk/src/error.ts` | `72ce8f7f0b3942ec4abbfae63df55b037a9d6c4de714e31f36a1d40c9ca452b9` |
| `packages/sdk/src/webhooks/client.ts` | `d97fa587e308055f6ebc76badcef514c4c3627326ae43905a9d991c648984fbf` |
| `packages/sdk/src/webhooks/types.ts` | `7436c75431b85a4a19439137971abe7eaeb3ed4b8f8b8c98bdcf242e676e4712` |
| `packages/codegen-doc/src/context-visitor.ts` | `b0b945d5b391773dd7908dfcf2ae7046302fa73eee65917769abc8e8011bc620` |
| `packages/codegen-doc/src/fragment-visitor.ts` | `c449043cec1394120318144bee8523b51c132847ede7d24e11ac16903107efe8` |
| `packages/codegen-doc/src/operation-visitor.ts` | `9b4dd66fa29d6573eae236f10f94490af161dc67cd9bd9a6ebfe9266a613df00` |
| `packages/codegen-sdk/src/model-visitor.ts` | `2c1967999d52af1e21b5b31af9574b57947821a5eb976e2afbc93c3cebbd1e1` |
| `packages/codegen-sdk/src/parse-operation.ts` | `b40f2318cd3740d648161782621ff42b4f29559b490e90896a433e07cce1ec13` |
| `packages/codegen-sdk/src/print-model.ts` | `58d4395076b681a0b44aba26ea21be5028f9f4f33ae4621a7eba4b31222c562d` |
| `packages/codegen-sdk/src/print-operation.ts` | `c64abd69c272709974cc9480aa78d1534d776fe88077376a33354a5ae03e7a44` |
| `packages/codegen-sdk/src/print-connection.ts` | `604d7ece5832b1edaed28c437bf6a0549550307ff47e299801bc823df75c993b` |
| `packages/codegen-test/src/print-test.ts` | `201444bacbaa1af41d54e0c43c3415c3f001f0eeb53be6ffff8c53f80940518a` |

These counts are source observations, not a recommendation to recreate every
private or product-specific type. The schema includes `[Internal]`,
`[INTERNAL]`, and `[ALPHA]` declarations. The public SDK generator explicitly
skips configured comments and fields. The schema proves API shape; it does not
reveal the server implementation, database design, authorization logic, UI
implementation, prompts, or operational behavior.

## 2. What the public API actually models

The API is much broader than an issue CRUD wrapper. It supplies a coherent
resource grammar across organization, planning, collaboration, customers,
agents, attention, code delivery, and integrations.

### 2.1 Organization and identity

| API term | OpenAgents use |
| --- | --- |
| `Organization` | Administrative scope for people, agents, teams, policy, and Work; replaces the earlier `ProductSpace` placeholder |
| `Team` | Stable group with membership, workflow defaults, planning settings, integrations, templates, and Work scope |
| `User` | Human, app, bot, or system-facing actor projection; OpenAgents must preserve exact principal class |
| `TeamMembership` | Scoped relation between a principal and Team; never sufficient authority by itself |
| `Node` | Globally addressable API resource with `id` |
| `Entity` | Node with `createdAt`, `updatedAt`, and nullable `archivedAt` |

Linear's UI calls the organization a workspace in some descriptions while the
API names it `Organization`. OpenAgents should use the API name. `Workspace`
is already reserved for an Omega window and workbench container.

### 2.2 Planning and tracked work

| API term | Important relations | All Work interpretation |
| --- | --- | --- |
| `Initiative` | projects, sub-initiatives, owner, lead team, status, health, updates, labels, documents | Strategic portfolio grouping across any Work Domain |
| `Roadmap` | projects, owner, sort order | Ordered portfolio projection, not implementation authority |
| `Project` | initiatives, teams, issues, milestones, members, lead, status, updates, documents, needs | Bounded multi-Work outcome across coding, operations, research, incidents, service, data, or another domain |
| `ProjectStatus` | type, position, color, organization | Configured Project lifecycle state |
| `ProjectMilestone` | project, issues, target date, progress, status | Named checkpoint within a Project |
| `Cycle` | team, issues, dates, progress, carryover | Optional timebox for a Team |
| `Issue` | team, state, assignee, delegate, parent, children, project, cycle, milestone, labels, relations, comments, documents, needs, sessions, history | Concrete tracking and planning projection of Work |
| `IssueRelation` | issue, related issue, relation type | Typed Work edge such as blocks, duplicate, or related |
| `WorkflowState` | team, type, position, issues | Configured Issue lifecycle state; policy data, not authority |
| `IssueLabel` | organization/team, parent/children, issues | Scoped classification and optional label hierarchy |
| `Template` | template data and target type | Reusable defaults for creating compatible resources |
| `CustomView` / `ViewPreferences` | filters, grouping, ordering, owner, team | Saved projection and presentation preferences, never canonical Work state |

The `Issue` object is structurally rich. At the source pin it includes an
assignee, agent delegate, workflow state and history, priority, estimate, due
date, project, milestone, cycle, parent and child Issues, relations and inverse
relations, labels, subscribers, comments, attachments, documents, customer
needs, agent sessions, source links, SLA timestamps, and exact history.

OpenAgents should not create a separate `WorkItem` aggregate beside `Work` and
`Issue`. The intended relation is:

```text
Work                         universal identity and lifecycle
└── Issue projection         planning/tracking fields and Linear-class UI
    ├── assignee             accountable human or admitted role
    ├── delegate             bounded Agent Member
    ├── WorkflowState
    ├── parent / children
    ├── IssueRelation
    ├── Project / Cycle / ProjectMilestone
    └── comments / documents / needs / AgentSessions
```

An Issue uses the same `workRef`. Creating, updating, archiving, or restoring
the Issue projection must submit a typed Work Intent and produce the same Work
Event history. There is no synchronization job between two writable records.

### 2.3 Collaboration and knowledge

| API term | OpenAgents adaptation |
| --- | --- |
| `Comment` | Authored discussion attached to an Issue, Project, update, document, or thread; not a structured action or receipt |
| `Document` | Revisioned authored context attachable to a Team, Initiative, Project, Cycle, Issue, or release |
| `Attachment` | External or uploaded artifact reference with provenance, audience, digest, and retention requirements added by OpenAgents |
| `Reaction` | Lightweight response; never Approval or Owner Disposition |
| `Favorite` | Principal-owned ordered sidebar reference to heterogeneous resources or folders |

The heterogeneous `Favorite` object is especially useful UI evidence. One
ordered sidebar can contain folders, Issues, Projects, Initiatives, Cycles,
Documents, Customers, Teams, views, releases, and agent conversations without
making the sidebar the authority for any of them.

### 2.4 Customers and needs

| API term | OpenAgents adaptation |
| --- | --- |
| `Customer` | External or internal recipient/stakeholder record with source, owner, status, tier, and access policy |
| `CustomerStatus` | Configured relationship state |
| `CustomerTier` | Configured service or importance classification |
| `CustomerNeed` | Typed link between a Customer and an Issue, Project, Comment, Attachment, or other source context |

`CustomerNeed` is more precise than the earlier catch-all `CustomerSignal`.
OpenAgents can retain Customer Signal as the normalized evidence class while
using Customer and Customer Need as the first-class API resources. Private
identity, consent, retention, and source provenance stay explicit.

### 2.5 Status and history

`ProjectUpdate` and `InitiativeUpdate` are first-class authored resources with
health, body, comments, reactions, snapshots, diffs, stale state, author, and
timestamps. `IssueHistory`, `ProjectHistory`, and `InitiativeHistory` separate
field-change history from comments.

OpenAgents should adopt the distinction:

- `ProjectStatus` is configured lifecycle state;
- `ProjectUpdate` is a dated report about health, progress, and risk;
- `Work Activity` is a typed execution or collaboration event;
- `Work Event` is canonical admitted history; and
- `Receipt` binds a claimed effect or observation to evidence.

Those records can be presented in one timeline, but they are not
interchangeable.

### 2.6 Agent resources

The source schema now exposes a substantial agent model.

#### `AgentSession`

The object includes:

- `appUser`, responsible human `creator`, Issue, Comment, source Comment, and
  optional Pull Request anchors;
- `context`, dynamically updated `plan`, status, start/end/dismissal state,
  summary, external links, and source metadata;
- an `activities` connection;
- associated Pull Requests and a live workspace-diff summary; and
- statuses such as pending, active, awaiting input, complete, error, or stale.

OpenAgents should use `AgentSession` as the API and product name for the
agent-specialized `Session`. It stays bounded to one Work identity and one
generation. It does not replace Work, the durable Thread, or the lane-specific
Run.

#### `AgentActivity`

The object binds one AgentSession, author, typed content, ephemeral and queued
flags, sent time, signal and signal metadata, source Comment, source metadata,
and optional push summary. Its content union covers thought, action, response,
prompt, error, and elicitation shapes.

OpenAgents should use `AgentActivity` as the agent-specialized `Work Activity`
projection. It must expose safe progress rather than hidden chain of thought,
and must keep action attempts, effects, evidence, verification, and human
disposition separate.

#### `AgentSkill`

The object has title, body, description, owner, creator, last updater,
inheritance, sharing and Team scope, recent use, last-used time, icon, color,
and lifecycle fields. OpenAgents should preserve the general `Skill` contract
and use `AgentSkill` for its agent-facing API resource.

#### Assignment grammar

The public Issue inputs contain both:

- `assigneeId`: the human assignee; and
- `delegateId`: the agent user to which the Issue is delegated.

This exact grammar is better than hiding the distinction behind one `owner`
field. OpenAgents should expose Assignee and Delegate while retaining
`Accountable Owner` and `Delegation Grant` as the stronger authority terms.

### 2.7 Attention, integration, and audit

| API term | OpenAgents use |
| --- | --- |
| `Notification` | Principal-scoped attention event with actor, category, read and snooze state, grouping, and target URL |
| `NotificationSubscription` | Active subscription scoped to an Initiative, Project, Customer, Cycle, Label, Team, User, or custom view |
| `Webhook` | Configured HTTP event delivery with resource types, Team scope, enabled state, HMAC secret, and failure history |
| `Integration` | Typed connection to an external service under Organization and optional Team scope |
| `OAuthApplication` | External application identity and grant surface; not an Agent identity by implication |
| `AuditEntry` | Actor, action type, request information, metadata, time, and administrative scope |

The API also includes Pull Requests, Releases, Release Pipelines, Release
Stages, Release Notes, Issue-to-Release relations, SLA configurations, search,
semantic search, imports, and authentication sessions. These are relevant
profiles, not reasons to make software delivery the root ontology.

## 3. The SDK generation architecture

The SDK's small handwritten runtime is not its main value. Its main value is a
deterministic schema-wide compiler pipeline.

```text
production GraphQL introspection + webhook schema
                  |
                  v
        normalized schema.graphql
                  |
       @linear/codegen-doc
       - ContextVisitor
       - FragmentVisitor
       - OperationVisitor
                  |
                  v
      generated fragments + operations
             /                    \
            v                      v
GraphQL typed-document-node   @linear/codegen-sdk
generated operation types     models / operations / SDK
            \                      /
             v                    v
               @linear/sdk
                  |
           @linear/codegen-test
        generated API coverage
```

### 3.1 Document generator

`@linear/codegen-doc` parses the schema once into a reusable context of
scalars, objects, interfaces, queries, mutations, enums, unions, and interface
implementations. It then generates:

- one fragment per valid model;
- scalar and enum fields directly;
- required identifiers for relationships that have root queries;
- nested fragment spreads for embedded resources;
- interface and union selections;
- every root query and mutation; and
- nested operations such as `user_assignedIssues`.

Configured skip comments and fields prevent internal or alpha schema elements
from leaking into the supported SDK.

### 3.2 SDK generator

`@linear/codegen-sdk` revisits the same schema and generated operations to
produce:

- a model class for each supported model;
- scalar, enum, object, list, union, and relationship fields;
- lazy getters for queryable relationships;
- model-scoped mutations such as `issue.update()`;
- root query and mutation methods;
- required positional arguments followed by one optional variables object;
- interface implementation switching on `__typename`; and
- Relay `Connection<T>` objects with `nodes`, `pageInfo`, `fetchNext()`, and
  `fetchPrevious()`.

The schema names mutations as `issueCreate`, `issueUpdate`, and
`issueArchive`; the SDK presents `createIssue`, `updateIssue`, and
`archiveIssue`. OpenAgents can preserve both: GraphQL resource-first names and
idiomatic verb-first TypeScript helpers.

### 3.3 Generated tests

`@linear/codegen-test` derives a typed test tree for root queries and nested
relationships. It generates required primitive, enum, list, and nested input
values, starts a supplied test client, and checks that returned objects have
the generated model type.

This is not sufficient behavioral assurance, but it is a strong drift and
coverage baseline. OpenAgents should add negative decoding, authorization,
idempotency, revision conflict, cursor, loss, and authority tests rather than
discarding the generated coverage.

### 3.4 Handwritten runtime

The runtime supplies:

- `LinearClient`, which injects a request function into the generated SDK;
- a trimmed isomorphic GraphQL client using `globalThis.fetch`;
- a typed error hierarchy with GraphQL path, request, variables, response,
  user-presentable messages, status, and raw error;
- request and complexity rate-limit metadata; and
- a Fetch/Node webhook adapter with discriminated payloads, wildcard and
  resource-specific handlers, HMAC-SHA256 verification, timestamp freshness,
  and constant-time comparison.

The published error enum distinguishes feature access, invalid input, rate
limit, network, authentication, forbidden, bootstrap, internal, user,
GraphQL, lock-timeout, usage-limit, unknown, and other failures. The taxonomy
is worth preserving as an input, then tightening into OpenAgents tagged errors
with explicit retry, authority, conflict, and evidence semantics.

These are useful donors, but they need Effect-native replacement or hardening.

## 4. Code-reuse disposition

### 4.1 Fork with attribution

The strongest direct code candidates are:

| Source area | Why reuse it | Required adaptation |
| --- | --- | --- |
| `codegen-doc` context and AST visitors | Already turns a large schema into consistent fragments and operations | Rename package and constants; add OpenAgents schema metadata, authority classes, and deterministic diagnostics |
| `codegen-sdk` operation parser and printers | Generates ergonomic models, nested queries, mutations, unions, and relationships | Emit Effect services/codecs, avoid mutable authority-bearing models, support OpenAgents command envelopes |
| Relay connection generator | Familiar cursor pagination and lazy next/previous helpers | Carry source cursor, freshness, truncation, loss, and `must_refetch` semantics |
| `codegen-test` | Schema-wide test generation and mock input construction | Generate positive, negative, compatibility, authorization, revision, and cursor fixtures |
| error taxonomy and rate-limit fields | Good client ergonomics and actionable capacity state | Use tagged Effect errors; preserve causes and distinguish retry, denial, conflict, stale generation, budget, and incomplete evidence |
| webhook discriminated union and HTTP adapters | Useful Fetch/Node compatibility and exact event dispatch | Decode with Effect Schema after signature verification; add replay identity, audience, idempotency, and durable delivery receipts |

A practical fork should keep copied files together initially. Preserve the
upstream copyright and MIT license, record the exact source commit and file
digests, and keep an import/diff ledger. Do not paste fragments anonymously
throughout unrelated packages.

### 4.2 Study, then regenerate

Use the 50,261-line schema as a terminology and relationship corpus. Do not
make it the OpenAgents source of truth and do not publish a renamed copy of the
Linear production schema as though it were an OpenAgents contract.

Do not vendor these generated files into OpenAgents product code:

- `_generated_documents.graphql`;
- `_generated_documents.ts`;
- `_generated_sdk.ts`; or
- `_generated.test.ts`.

They encode Linear's service, supported fields, skip policy, nullability,
operation set, and compatibility history. Regenerate equivalent artifacts from
the OpenAgents-owned schema.

### 4.3 Reject or harden

Do not carry these behaviors unchanged:

- invalid non-null dates silently falling back to `new Date()`;
- invalid non-null JSON silently falling back to `{}`;
- webhook JSON being cast to a union without runtime schema validation;
- open `JSON` and `JSONObject` fields for authority-bearing context, plans,
  activities, signals, or metadata;
- mutation `success` as the complete result contract;
- mutable shared request headers and a bare global `fetch` as the production
  service abstraction;
- model completion or webhook receipt as proof of an admitted Work effect;
- an API token, OAuth actor, Team membership, or Nostr signature as sufficient
  action authority; or
- a generated model object as the authoritative domain aggregate.

OpenAgents mutation payloads need, as applicable:

```text
success
subjectRef
acceptedRevision
eventCursor
idempotencyKey
actorRef
authorityDecisionRef
receiptRefs[]
projectionFreshness
typed failure or conflict
```

## 5. One schema across Effect, Rust, and GraphQL

The public SDK is evidence for schema-first generation, not evidence that
GraphQL must own every OpenAgents contract.

The current
[Effect/Rust unified-contract analysis](../sol/2026-08-02-effect-rust-unified-contract-models-analysis.md)
requires one language-neutral definition for data that crosses the Effect and
Rust boundary. Apply that rule here:

```text
OpenAgents Contract Profile
  - structural records, tags, refs, presence, bounds, sensitivity
  - command / event / receipt / projection relationship class
  - compatibility and canonical encoding
            |
            +--> Effect Schema + TypeScript types
            +--> Rust serde types + validators
            +--> canonical JSON Schema + fixtures
            +--> GraphQL schema and resolver DTO projection
                         |
                         +--> adapted document generator
                         +--> adapted TypeScript SDK generator
                         +--> generated SDK tests
```

GraphQL should own public query shape, selection, filtering, Relay pagination,
and client ergonomics. The Contract Profile and authoritative domain services
should own encoded cross-runtime identity, command admission, Events,
Receipts, revisions, and compatibility.

This prevents three forms of drift:

1. Effect and Rust do not hand-maintain the same record.
2. the GraphQL API does not become a second business-policy implementation;
3. Nostr projections do not become a third writable authority.

The SDK should be Effect-native at the boundary:

- generated response schemas decode untrusted GraphQL and webhook values;
- client operations return typed `Effect` values with tagged errors;
- Layers supply transport, authentication, tracing, retry, scheduling, and
  configuration;
- redaction and sensitivity metadata survive generation;
- streaming or subscription clients retain cursor and loss semantics; and
- Promise conveniences can be a thin outer adapter, not the core contract.

## 6. API naming contract for OpenAgents

The target naming rule is “adopt unless it narrows or conflicts,” not “rename
everything to sound different.”

| Public API/UI name | Canonical OpenAgents meaning |
| --- | --- |
| Organization | Administrative scope; never Omega Workspace |
| Team | Stable membership and policy group |
| Initiative | Strategic grouping of Projects across applicable Work Domains |
| Roadmap | Ordered portfolio projection |
| Project | Bounded collection of Work toward an outcome; not limited to software/product development and not an IDE Project Graph |
| ProjectStatus | Configured Project lifecycle state |
| ProjectMilestone | Named Project checkpoint |
| Cycle | Optional Team timebox |
| Work | Universal objective and lifecycle root |
| Issue | Concrete planning/tracking projection of Work with the same identity |
| WorkflowState | Configured Issue state |
| IssueRelation | Typed Work-to-Work edge shown through Issue surfaces |
| IssueLabel | Scoped classification on the Issue projection |
| Assignee | Human or admitted organizational role accountable for the Issue/Work |
| Delegate | Agent Member performing bounded work under a Delegation Grant |
| AgentSession | Agent-specialized Session; related Run and Thread remain distinct |
| AgentActivity | Agent-specialized Work Activity; not hidden reasoning or a Receipt |
| AgentSkill | Agent-facing representation of a versioned Skill |
| Customer / CustomerNeed | Typed stakeholder context and its link to Work |
| ProjectUpdate / InitiativeUpdate | Dated status report, distinct from lifecycle state and canonical Event |
| Notification / NotificationSubscription | Principal-scoped attention projection and its source preference |
| Favorite | Principal-owned navigation reference |
| Integration / Webhook / OAuthApplication | External-system boundaries under explicit grants and verification |
| AuditEntry | Administrative activity projection; not automatically an effect Receipt |

Suffixes should also remain consistent:

| Suffix | Meaning |
| --- | --- |
| `CreateInput` / `UpdateInput` | Client-supplied mutation shape; never trusted before decoding and admission |
| `Payload` | Typed mutation result containing subject plus admission and cursor metadata |
| `Connection` / `Edge` / `PageInfo` | Relay-style cursor pagination |
| `Filter` | Declarative query selection; not action authority |
| `History` | Field/relation change projection with exact source cursor |
| `Notification` | Attention projection for one recipient |
| `WebhookPayload` | Signed and decoded external delivery projection |

## 7. Nostr-centric composition

The API vocabulary and codegen do not replace the Buzz-derived Nostr design.
They clarify what the signed projections describe.

```text
OpenAgents Organization / Team / Work / Issue / AgentSession
       |
       +--> canonical admitted command, Event, projection, and Receipt
       |
       +--> safe signed Nostr projection
              identity / workroom / thread / delegation / activity refs
              relay-qualified cursor and delivery evidence
```

Nostr events can carry stable references, causal parents, signed actor
identity, collaboration messages, delegation proposals, public-safe progress,
and evidence links. Cloud SQL and owning services remain canonical under the
current architecture. A relay acknowledgement cannot create an Issue, change a
WorkflowState, establish an Assignee, admit a Delegate, complete an
AgentSession, verify an outcome, or settle payment.

## 8. Candidate implementation packets

These are research dispositions, not admitted issues.

### LASDK-00 — vocabulary and schema corpus

Freeze the glossary mapping in section 6 and classify each relevant upstream
resource as `adopt`, `adapt`, `profile-only`, or `reject`. Remove
`ProductSpace`, `WorkItem`, `WorkSession`, and `WorkActivity` placeholders
where the adopted API name is now explicit, without weakening the universal
Work/Session/Activity model.

### LASDK-01 — provenance-preserving code import

Select the smallest `codegen-doc`, `codegen-sdk`, and `codegen-test` file set.
Import it in one reviewable commit with the upstream MIT license, copyright,
commit, digests, dependency license inventory, and a source-to-target path
map.

### LASDK-02 — OpenAgents generator profile

Add deterministic support for Effect Schema decoders, tagged errors,
sensitivity metadata, command/result envelopes, revision fields, and
compatibility fixtures. Generated files must fail CI on drift and must never be
manually edited.

### LASDK-03 — All Work API schema

Generate or project Organization, Team, portfolio, Work/Issue, collaboration,
customer, agent, attention, integration, and audit resources from the owned
contracts. Preserve exact field semantics and mark every projection's
authority class.

### LASDK-04 — Effect TypeScript client

Generate typed operations that return Effect values, decode every untrusted
response, expose Relay pagination with loss/freshness state, and use Layers for
transport, auth, retry, observability, and configuration.

### LASDK-05 — Rust binding parity

Generate the cross-runtime records from the same Contract Profile. Run the
positive, negative, canonical-encoding, and compatibility corpus against
Effect and Rust. Do not mirror domain services or UI entities.

### LASDK-06 — event delivery

Adapt the webhook client only after runtime decoding, replay identity,
idempotency, audience, durable delivery, and receipt semantics are specified.
Generate Nostr safe-projection types from the same semantic source where the
wire profile permits it.

### LASDK-07 — import adapters

Study the MIT import package for label, state, priority, assignee, comment,
attachment, and source-link normalization. Imports create proposed OpenAgents
Intents with provenance and dry-run reports; they do not write around normal
authority or claim that source history was preserved when it was not.

## 9. Acceptance and falsification

| Claim | Required proof | Falsifier |
| --- | --- | --- |
| Vocabulary is substantially compatible | Glossary and generated API use the adopted names with documented exceptions | Parallel synonyms remain without a semantic reason |
| All Work remains universal | Non-development Project and Work fixtures pass without fake product fields | Incident, research, service, or operations Work must pretend to be software-product work |
| Issue does not create dual authority | Work and Issue share identity, revision, Event sequence, and command admission | A sync process reconciles two writable lifecycles |
| Code reuse is lawful and traceable | MIT notice, upstream commit, file map, digests, and dependency inventory are present | Copied implementation has no provenance or required notice |
| Generated code is owned by the schema | Clean regeneration is byte-stable and CI rejects drift | Generated files need hand edits |
| Effect and Rust agree | Shared positive, negative, and compatibility corpus passes | Either runtime accepts a value rejected by the other without an explicit projection rule |
| API clients do not gain authority | The same commands, grants, revisions, and receipts apply across Omega, web, SDK, Pylon, and agents | One client bypasses admission |
| Events remain honest | Webhook, Nostr, Sync, Work Event, and Receipt types stay distinct | Delivery acknowledgement is presented as effect, verification, or acceptance |

## 10. Final recommendation

Use the public repository aggressively but selectively.

- **Use its nouns.** They are coherent, familiar, and now encoded in a large
  public schema. Generalize Project, Initiative, Cycle, Customer, and related
  views across All Work rather than inventing product-only replacements.
- **Use its generator code.** The MIT document, SDK, and test generators are a
  much better starting point than writing schema-wide GraphQL client code from
  scratch.
- **Do not use its service.** OpenAgents owns the schema, data, authority,
  execution, Nostr projections, evidence, verification, and outcomes.
- **Do not use its generated SDK as the target.** Generate a new SDK from the
  OpenAgents contract and retain familiar ergonomics.
- **Make Effect and Rust derive from one boundary definition.** GraphQL is one
  projection of that definition, not a reason to hand-maintain another model.

That is the legitimate version of “yoink the SDK”: preserve attribution, fork
the high-leverage compiler machinery, adopt the useful language, and make the
output natively OpenAgents and natively All Work.

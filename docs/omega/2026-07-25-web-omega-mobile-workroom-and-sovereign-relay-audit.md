# Web, Omega, mobile, and sovereign workroom audit

- Date: 2026-07-25
- Class: product and architecture audit
- Status: recommendation, not implementation authority
- Audience: product, web, Omega, mobile, cloud, security, and support teams
- Audited OpenAgents commit: `40e704fbcde58af734f9f3e3244cddba574aad6b`
- Scope: web workrooms, Omega, OpenAgents Mobile, and relay service options

## 1. Executive conclusion

OpenAgents should build one workroom product with three clients.
The web client should coordinate teams and projects.
Omega should own deep work and local execution.
OpenAgents Mobile should own attention, review, and safe remote control.

The product needs one shared domain contract and one event identity model.
It must not copy durable state between three application-specific databases.
Each client can keep a local projection and an offline queue.
Only the domain owner can confirm a mutation.

Substantial prior web work exists.
The current repository still contains team, project, chat, invite, workroom,
evidence, approval, lifecycle, and projection code.
However, the prior logged-in Foldkit workroom client is gone.
The current TanStack Start app does not yet replace that complete client.
It has a workspace invite route, a real personal Sync chat route, and several
bounded supervision routes.

The recommended first commercial wedge is a managed private workroom relay for
small technical teams.
OpenAgents should operate it on Google Cloud.
The customer should control member keys and receive a complete signed export.
The first offer should include guided Omega and mobile setup.
It should not promise a complete Buzz replacement.

The customer-operated option should follow as a supported deployment kit and
an advisory service.
A hybrid option should then add an OpenAgents-managed service plane without
moving the customer record.
All three options must use the same event and client contracts.

This direction needs a new admitted web workroom packet.
The current master roadmap makes web the public trust and API surface.
It says that new web routes need bounded admission.
This audit does not change that authority.

## 2. Method and evidence labels

This audit uses four labels.

- **Verified** means current code, a current contract, or a retained receipt
  supports the statement.
- **Historical** means a prior implementation or design supports the statement,
  but current product direction does not.
- **Inference** means current parts make the result credible, but no joined
  proof exists.
- **Proposal** means this audit recommends new product or service work.

Current code and tests own implementation truth.
The [master roadmap](../sol/MASTER_ROADMAP.md) owns program direction.
The [Omega accepted plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)
owns the Omega repository and client boundary.
The [Omega roadmap](./ROADMAP.md) owns Omega implementation order.

## 3. Product thesis and user jobs

### 3.1 Product thesis

A workroom is the durable place where people and agents complete one body of
work.
It joins conversation, project context, agent activity, decisions, evidence,
and outcomes.
It is not one chat thread and it is not one desktop window.

The three clients serve different work conditions.

| Client | Primary condition | Primary promise |
| --- | --- | --- |
| Web | A team needs a shared and reachable coordination view | Organize people, projects, rooms, policies, and outcomes |
| Omega | A person or agent needs deep repository work | Execute, inspect, edit, test, and verify with local authority |
| Mobile | A person is away from the desk | Notice, inspect, approve, steer, and stop without raw host access |

The relay choice is a deployment property.
It must not create a different workroom product.
A managed customer and a customer-operated customer must open the same room in
web, Omega, and mobile.

### 3.2 Primary user jobs

#### Team lead

- Create a team and project.
- Invite members and agents.
- Set roles and project policy.
- Attach repositories and execution hosts.
- See work, blockers, decisions, and evidence.
- Export the complete signed record.

#### Contributor

- Read shared project history.
- Start or join an admitted work unit.
- Attach work from Omega.
- Ask for review.
- Respond to comments and decisions.

#### Reviewer

- Inspect the intent, changes, tests, and receipts.
- Record an independent decision.
- Keep review identity separate from producer identity.

#### Operator

- Select a managed or customer-operated relay.
- Monitor service health and storage.
- Apply upgrades and backups.
- Restore or move the workroom.
- Produce incident and recovery receipts.

#### Mobile user

- Receive a private notification with an opaque reference.
- Open the exact room, run, decision, or receipt.
- Approve only an allowed action.
- Pause, resume, stop, or comment.
- Confirm the durable outcome after reconnect.

## 4. Verified state

### 4.1 Omega

**Verified.** Omega is the selected primary Desktop, IDE, and company workroom
destination.
The separate Omega repository owns Rust, GPUI, editor, project, buffer, Git,
terminal, task, and native application truth.
The OpenAgents monorepo owns reusable Effect services, schemas, fixtures, and
generated clients.

**Verified.** The Omega workroom direction includes channels, threads, agent
membership, work history, reviews, approvals, receipts, and signed
interoperability.
The Sarah workroom plan also includes a private room and a later community
room.

**Verified.** Omega Full Auto can bind a run to native project and worktree
references.
The join stores path digests, not raw absolute paths.
Omega still owns the native project state.
OpenAgents still owns run admission and completion.

**Verified.** The Full Auto service defines report, receipt, Sync status,
projection publication, and mobile control methods.
The report note says that the Omega Rust host did not implement the Sync
session request at that packet date.

**Verified.** The mobile adaptation audit found a Sarah Nostr client, NIP-42
tests, signed event construction, and workroom UI sources in Omega.
It also found a default mock relay adapter, local-only read state, and an
in-memory community projection.
Those gaps prevent a complete relay-backed workroom claim.

### 4.2 Current web application

**Verified.** `apps/openagents.com/apps/start` is the current TanStack Start
application.
It uses TanStack Router, TanStack DB, React Query, React 19, Tailwind 4,
Effect, and Vite Plus.
The Node, pnpm, and Vite Plus conversion is complete.

**Verified.** The current Start application has these relevant surfaces:

- `/workspaces/{workspaceId}` shows a project workspace invite.
- `/khala/chat-sync` is a real personal Khala Sync chat client.
- `/managed-sandboxes` is a bounded supervision route.
- `/share/{shareId}` is a read-only share surface.
- the web agent surface decodes shared safe agent facts.

**Verified.** The Start app does not have the prior complete team and project
workroom client.
The `/app` route redirects to `/splash`.
The `/autopilot` route is a funnel page, not a team workroom.

**Verified.** The web Sync client has a persistent SQLite-WASM store design.
It uses a SharedWorker, OPFS, and Web Locks.
It shares the same store semantics as the Node and Expo adapters.
This is a strong base for a local-first web workroom.

### 4.3 Retained web workroom and team code

**Verified.** The API still has team and project storage.
Migrations define `team_projects`, project-scoped team messages, project-scoped
agent runs, team files, invites, and related metadata.
The Worker still registers team chat routes for team and project rooms.

**Verified.** The API still has private project workspace and invite services.
The current Start client only exposes the invite landing page.
It does not expose the complete project administration flow.

**Verified.** The Omni workroom implementation remains substantial.
It includes workroom records, evidence bundles, lifecycle decisions,
classification, surface projections, approval gates, mobile approval cards,
templates, and source-authorized business objects.
The Worker registers the workroom and lifecycle routes.

**Historical.** The removed Foldkit client had team chat, project chat,
project agents, files, inline Autopilot run cards, answer-back messages, and
shared room history.
Its records show real production smokes at the time.
The removed UI and old provider details are not current product proof.

**Historical.** The old Autopilot web audit designed team membership, project
dashboards, repository attachment, and live Pylon session attachment.
It correctly required team projections and read-only member defaults.
Its named `apps/web` paths and loopback bridge plan are no longer current
implementation authority.

### 4.4 OpenAgents Mobile

**Verified.** `apps/openagents-mobile` is the only supported mobile product.
It uses Expo, React Native, Effect Native, SecureStore, SQLite, notifications,
deep links, and owned OTA updates.

**Verified.** Mobile already has an adaptive workbench, conversations, work
logs, approvals, files, changes, and Git.
It also has a bounded terminal view, a controller directory, settings, Full
Auto projections, and managed sandbox controls.

**Verified.** Mobile is a controller.
It must not become an execution host.
Its managed sandbox contract already excludes provider credentials, raw paths,
PTY authority, and generic cloud control.

**Verified.** The mobile Omega audit found that several repository and
environment routes have client contracts without a current production host.
It calls this surface and contract parity, not host parity.

**Verified.** The recommended mobile product remains OpenAgents Mobile.
Omega is a host and destination inside that app.
There is no current Omega iOS or Android binary target.

### 4.5 Shared contracts and local-first assets

**Verified.** `@openagentsinc/khala-sync-client` provides one local-store core
for Node SQLite, Expo SQLite, and web SQLite-WASM.
It supports confirmed state, cursors, tombstones, an offline queue, optimistic
overlays, rebase, and typed transport failures.

**Verified.** `@openagentsinc/agent-surface` projects canonical turn facts into
safe cards for desktop, web, and mobile.
It does not own providers, renderers, or a second wire schema.

**Verified.** Effect Native provides a shared component, token, intent, and
portable view contract.
TanStack Start remains the web route, server-rendering, and hydration host.
React and React Native remain renderer technologies.

### 4.6 Buzz and the OpenAgents relay

**Historical.** Buzz proves that one Nostr relay can serve as a workspace for
chat, groups, agents, reviews, workflows, Git events, and signed history.
Buzz also has self-host and Kubernetes deployment paths.
OpenAgents canceled the separate Buzz product and deployment.
The retained Buzz runbook is not active authority.

**Verified.** OpenAgents chose Nostr-primary Omega workrooms as the current
Buzz compatibility direction.
Buzz is a behavior and protocol reference.
It is not an application dependency.

**Verified.** The OpenAgents-owned `nostr-effect` relay has a Node 24 host and
a Postgres event store.
The deploy runbook maps it to Cloud Run, Cloud SQL, Secret Manager, and
`relay.openagents.com`.

**Verified.** The runbook records a live 2026-07-25 load test.
It records 3,710 accepted events with no rejection.
It also records durable read-back after a forced revision restart.

**Verified.** That proof does not establish a multi-tenant customer service.
The runbook identifies connection admission as the first measured limit.
It also records a NIP-11 and NIP-42 advertisement mismatch.

**Verified.** Sarah production still defaults to the Khala record.
The Nostr migration has explicit `khala`, `shadow`, `cutover`, and
`retirement` stages.
Production Nostr cutover requires an operator action and joined proof.

## 5. What exists and what remains aspirational

| Capability | Existing state | Honest status |
| --- | --- | --- |
| Team and project records | API tables, routes, tests, and Sync projections | Reusable server substrate |
| Team and project web UI | Historical Foldkit implementation | Needs a new TanStack client |
| General workroom records | Omni records, evidence, lifecycle, projections | Rich but not one current product model |
| Personal web chat | Current TanStack and Khala Sync client | Real narrow slice |
| Omega project work | Native Omega owners and Full Auto join | Real local owner |
| Omega relay workroom | Contracts and partial clients | Not joined end to end |
| Mobile workbench | Broad source and tests | Strong client, incomplete host binding |
| Cross-client local store | Node, Expo, and web adapters | Real shared base |
| Owned relay | Live host and measured load record | Real single-service base |
| Customer relay product | No tenant, billing, support, or portability proof | Proposal |
| Customer-operated kit | Buzz evidence and OpenAgents runbook parts | Proposal |
| One room on all clients | No joined team/project/relay proof | Aspirational |

The key finding is not “start again.”
The key finding is “select one domain model and join the existing parts.”

## 6. Target ownership boundaries

### 6.1 Domain owners

| Domain | Durable owner | Client responsibility |
| --- | --- | --- |
| Team, membership, role, invite | Workroom authority service and signed membership record | Show and request changes |
| Project identity and repository binding | Workroom authority service | Select and display project context |
| Signed room history | Selected customer relay set | Verify, cache, and project events |
| Local repository, buffer, Git, terminal | Omega project owners | Project bounded state and accept intents |
| Agent run lifecycle | Owning run service or Omega host | Show projections and submit controls |
| Evidence and review decision | Evidence service and named reviewer identity | Inspect and sign a bounded decision |
| Billing and exact usage | OpenAgents service ledger | Show statements and receipt references |
| Notification delivery | Platform push service | Deliver opaque references only |
| Client cache and pending queue | Each client | Never promote cache state to authority |

### 6.2 Surface boundaries

#### Web owns

- team and project administration
- member invitations and role policy
- room and project discovery
- portfolio and workroom overview
- shareable, bounded review surfaces
- billing, service state, exports, and support requests

Web can also offer a full workroom view.
It should not become a browser shell for unrestricted Omega authority.

#### Omega owns

- repository and worktree attachment
- editor, file, buffer, Git, terminal, and task truth
- agent execution and local provider custody
- detailed evidence production
- deep review and change inspection
- host pairing and local capability grants

#### Mobile owns

- attention and notification inbox
- compact room and run state
- review, approval, comments, and safe control
- host and relay connection health
- offline intent storage and later reconciliation

Mobile must not receive raw paths, credentials, unrestricted shell access, or
hidden repository content.

## 7. Canonical workroom model

The first contract should use stable references for these objects.

| Object | Required identity | Important relations |
| --- | --- | --- |
| Tenant | `tenantRef` | owns teams, policy, service plan, and relay binding |
| Team | `teamRef` | contains members and projects |
| Member | `memberRef` plus actor public key | has role, status, and grants |
| Agent | `agentRef` plus agent public key | binds to one accountable operator |
| Project | `projectRef` | belongs to team and binds repositories |
| Workroom | `workroomRef` | belongs to project or team |
| Thread | `threadRef` | belongs to workroom and can bind a run |
| Run | `runRef` | binds host, project, worktree, and evidence root |
| Decision | `decisionRef` | binds subject digest, actor, role, and outcome |
| Evidence item | content digest plus `evidenceRef` | binds source and visibility |
| Permission grant | `grantRef` | binds actor, actions, scope, expiry, generation |
| Notification | `notificationRef` | points to one subject and carries no content |
| Relay binding | `relayBindingRef` | lists the admitted relay set and policy |

Do not merge “project” and “workroom.”
A project is a durable team and repository context.
A workroom is one coordinated body of work inside that context.
A project can have many workrooms.

Do not merge “agent” and “run.”
An agent is an identity and capability holder.
A run is one bounded execution attempt.

## 8. Event, identity, and synchronization requirements

### 8.1 Event envelope

Every shared event needs these facts.

- schema and schema revision
- event id and content digest
- tenant, team, project, workroom, and thread references
- actor public key and actor role
- event kind and causal parent references
- created time and logical sequence
- visibility and encryption policy
- idempotency reference
- optional command, outcome, evidence, and receipt references

Signed relay events should carry shared history.
They must not carry raw credentials, private local paths, or exact metering
rows.

### 8.2 Identity

Each person, agent, device, and relay needs a distinct identity.
A device key must not become the owner key.
An agent key must bind to an accountable operator.
A relay key must identify the service, not a customer member.

OpenAuth can bind billing and managed service access.
It must not replace the customer Nostr identity.
Customers must still read and verify exported events without OpenAuth.

### 8.3 Sync

The selected relay set carries signed workroom events.
Khala Sync can carry service projections, durable command outcomes, billing
facts, and bounded host state.
Omega can also use a direct paired host channel for local control.

The clients must join these sources by stable reference.
They must show source, freshness, and gap state.
They must not hide a missing source behind one optimistic status.

### 8.4 Offline and conflict rules

Reads can continue from a verified local cache.
New text, comments, and permitted intents can enter a bounded offline queue.
The queue must keep the exact signed bytes and idempotency identity.

The server or host decides command admission.
A pending mobile or web command is not a completed action.

Use these conflict rules:

1. Append-only history orders by causal links and event identity.
2. Replaceable preferences use an expected version.
3. Membership and permission changes use a generation fence.
4. Project bindings use compare-and-set semantics.
5. Evidence items are immutable and content-addressed.
6. Decisions cannot overwrite a prior signed decision.
7. A retraction adds a new event and does not erase the audit record.

## 9. Cross-surface mapping

| Concern | Web | Omega | Mobile |
| --- | --- | --- | --- |
| Workroom | full coordination and overview | deep work pane | compact room and attention view |
| Project | create, invite, policy, repository refs | open repository and worktree | select and inspect |
| Team | roster, roles, billing, support | current member context | roster and role summary |
| Agent | roster, assignment, readiness | configure and execute local agent | inspect, steer, or stop |
| Evidence | review bundle and share view | produce and inspect full detail | compact chain and decision |
| Permissions | role and policy administration | host capability enforcement | derived allowed actions only |
| Notifications | in-app inbox | native desktop attention | APNs or FCM deep link |
| Relay | plan, health, export, support | connect, verify, cache, publish | connect, verify, cache, publish |

All clients should use the same labels for pending, confirmed, refused, stale,
unavailable, and failed states.

## 10. Web reuse and TanStack migration

### 10.1 Reuse

Reuse these current assets:

- team and project tables and route behavior
- invite and private workspace services
- Omni workroom, evidence, lifecycle, and projection rules
- Khala Sync schemas, server projection, and local stores
- TanStack DB collection adapters
- shared agent surface projectors
- shared UI tokens and Effect Native components
- historical behavior tests for shared history and compact run cards

Reuse behavior, schemas, and tests before UI source.
The removed Foldkit component tree is a design reference.
It is not the current client architecture.

### 10.2 Migration target

Build the admitted web workroom in `apps/openagents.com/apps/start`.
Use TanStack Start for routes, server rendering, loaders, and hydration.
Use Effect services and Effect Schema for domain work.
Use TanStack DB over the existing Khala Sync collection adapter.
Use the shared web SQLite-WASM store for restart and offline behavior.

Use Effect Native for portable workroom components where its catalog is ready.
Add bounded React renderer hosts for complex web-only mechanics.
Do not create a second React-owned domain store, schema set, or permission
model.

### 10.3 Migration order

1. Add a read-only team and project list.
2. Add one read-only workroom with confirmed event history.
3. Add member invite and role administration.
4. Add a compact linked run and evidence card.
5. Add comments and bounded review decisions.
6. Add Omega host pairing and safe run controls.
7. Add export, service health, billing, and support.

Do not port every Omni workroom kind into the first client.
Start with coding and product workrooms.

## 11. Permissions, security, and notifications

### 11.1 Permission model

Use roles only as policy inputs.
Each effectful action still needs an exact grant.

The first closed action set should include:

- observe room
- observe evidence
- comment
- request work
- start admitted run
- steer run
- pause, resume, or stop run
- request review
- decide review
- manage members
- manage relay binding
- export tenant data

Every grant binds tenant, actor, role, action, target, expiry, and generation.
High-risk actions also bind an expected subject digest.

### 11.2 Security boundaries

- Relay acceptance is not work admission.
- A signed event proves who signed exact bytes.
- It does not prove that the event is true or authorized.
- The relay must not hold customer member private keys.
- OpenAgents must not hold customer owner keys by default.
- Managed service keys must stay in Secret Manager or an admitted signer.
- Exact usage, payment, release, and public claim authority stay outside the
  relay.
- Browser and mobile clients receive bounded projections, not raw service
  credentials.

### 11.3 Notifications

The workroom event or service outcome creates an attention target.
The notification service sends only an opaque target reference and category.
The client fetches and verifies the permitted content after it opens.

Notification preference conflicts use expected versions.
Read state is a signed or server-confirmed event.
A delivered push is not a read receipt.

## 12. Commercial and sovereignty options

### 12.1 Managed private relay

**Proposal.** OpenAgents operates a dedicated logical tenant on the owned
Google Cloud relay service.
The customer uses OpenAgents web, Omega, and mobile.
OpenAgents operates compute, storage, backups, monitoring, and upgrades.

Advantages:

- fastest setup
- one support boundary
- immediate web access
- simple mobile push
- simplest trial and billing flow

Risks:

- OpenAgents carries the largest operational burden
- customers can confuse managed storage with OpenAgents data ownership
- a shared service needs strong tenant isolation
- service outage can affect many customers

The first version should use one database and cryptographic tenant fences only
after an isolation proof.
A dedicated database per early design partner is safer and easier to explain.

### 12.2 Customer-operated relay

**Proposal.** The customer runs the OpenAgents relay package on its own Google
Cloud, Kubernetes, or supported server environment.
The customer controls storage, backup policy, network policy, and relay
operations.

OpenAgents supplies:

- pinned images and digests
- deployment templates
- configuration validation
- migration and rollback tools
- health and conformance tests
- support and advisory services

Advantages:

- strongest infrastructure control
- clear data custody
- fits regulated or security-sensitive teams
- creates a credible consulting offer

Risks:

- higher setup cost
- more support variance
- slower upgrades
- customer errors can damage availability
- OpenAgents cannot promise recovery without access and current backups

### 12.3 Hybrid relay

**Proposal.** The customer operates the record relay.
OpenAgents provides a separate service plane for web access, push, update
advice, optional encrypted blobs, billing, and support.

The service plane reads only admitted projections.
It must not become a hidden record authority.

Advantages:

- customer keeps the signed record and storage
- OpenAgents can still deliver a polished web and mobile service
- support and billing remain simple
- migration between operators remains possible

Risks:

- the split is harder to explain
- outage diagnosis crosses two operators
- projection lag can confuse users
- support needs exact boundary receipts

### 12.4 Recommended sequence

Start with a managed private relay for three to five design partners.
Use a dedicated database for each partner during this stage.
Include a signed export and restore exercise in onboarding.

Next, publish a customer-operated deployment kit.
Offer paid setup, migration, policy design, and incident review.

Then add the hybrid service.
Do this only after web and mobile can bind to a customer relay without a hidden
OpenAgents record copy.

## 13. Data ownership, export, and portability

The commercial message should be precise:

> You control your member keys and your signed workroom record. OpenAgents
> operates the service only in the managed plan.

Do not say that the customer owns every system datum.
OpenAgents can retain its own billing, security, abuse, and support records
under the service agreement.

Each tenant export must include:

- ordered signed events
- relay and schema manifests
- member and agent public identities
- encrypted blob objects or portable blob references
- project and workroom reference maps
- deletion and replacement events
- event and blob digests
- export time and tool version
- verification instructions

The export must not include OpenAgents service secrets.
It must not claim that relay deletion removed external copies.

Portability proof needs these tests:

1. Export a managed tenant.
2. Verify all signatures and digests offline.
3. Import into a clean customer-operated relay.
4. Open the same workroom in Omega and mobile.
5. Confirm stable references and causal history.
6. Publish a new event on the new relay.
7. Remove the old relay from the admitted relay set.

## 14. Operational responsibility

| Responsibility | Managed | Customer-operated | Hybrid |
| --- | --- | --- | --- |
| Relay compute | OpenAgents | Customer | Customer |
| Record database | OpenAgents | Customer | Customer |
| Backups and restore | OpenAgents | Customer | Customer |
| Relay identity key | OpenAgents service key | Customer | Customer |
| Member and agent keys | Customer actors | Customer actors | Customer actors |
| Relay upgrades | OpenAgents | Customer with OpenAgents guidance | Customer |
| Web application | OpenAgents | OpenAgents or customer package | OpenAgents |
| Omega and mobile updates | OpenAgents release channels | OpenAgents release channels | OpenAgents release channels |
| Incident lead | OpenAgents | Customer | Shared by fault domain |
| Export support | OpenAgents | Customer tools | Shared |
| Tenant policy | Customer, enforced by service | Customer | Customer |

Every incident needs one fault-domain label.
Examples are relay, database, signer, projection, client, push, host, or
OpenAgents service.

The managed service needs published recovery objectives.
The customer-operated offer needs a support matrix.
OpenAgents must not promise an objective that customer backup policy cannot
support.

## 15. Billing, service, support, and adoption

### 15.1 Initial billing model

Use a simple subscription.
Price by active member band and service tier.
Include a bounded event and storage allowance.
Charge separately for managed execution, large blobs, and advisory work.

Do not price the first offer by raw event count.
Customers buy a reliable private workroom, not a relay packet meter.

Candidate tiers:

- managed private workroom
- managed workroom with dedicated database
- customer-operated support
- migration and sovereignty advisory
- incident and recovery review

Exact prices need an owner decision and measured service cost.
This audit does not set prices.

### 15.2 Support promise

The managed plan can cover relay, storage, backups, upgrades, and client
binding.
It cannot cover customer-controlled keys or unavailable customer hosts.

The customer-operated plan can cover supported images, deployment templates,
conformance, and advisory response.
It cannot guarantee customer network, storage, or backup operations.

### 15.3 Adoption motion

The first buyer should be a small software team with these needs:

- agents and people work in the same project
- the team wants a durable audit record
- members use desktop and phone
- the team values data portability
- the team can join a guided design partnership

The first demonstration should show this journey:

1. Create a team and project on web.
2. Invite a member.
3. Open the project in Omega.
4. Start one bounded agent run.
5. Review evidence on mobile.
6. Record a decision.
7. Export the signed room.
8. Verify the export without the OpenAgents service.

This journey sells workroom value and sovereignty together.

## 16. Shared libraries and platform-specific UI

### 16.1 Shared packages

Create or extend shared packages for:

- workroom identity and event schemas
- team, project, membership, and grant schemas
- relay binding and export manifests
- workroom read-model projectors
- command and durable outcome schemas
- evidence and review projections
- notification target schemas
- role-to-capability policy inputs
- canonical and negative fixtures
- TypeScript and Rust generated clients

Extend `@openagentsinc/khala-sync-client` for service projections and command
outcomes.
Do not force signed relay history into a second Khala entity log.

Extend `@openagentsinc/agent-surface` for safe workroom agent cards.
Keep provider and renderer code out of that package.

### 16.2 Platform-specific code

Keep these items platform-specific:

- TanStack routes, server loaders, and browser storage worker
- Omega GPUI panes and native project adapters
- mobile navigation, push registration, SecureStore, and native feedback
- relay deployment composition and operator controls
- platform accessibility and high-density editor or terminal hosts

Share fixtures and behavior.
Do not share view code when the interaction or security boundary is different.

## 17. Phased roadmap

### Phase 0: admission and contract freeze

Duration target: one to two weeks after owner admission.

Deliver:

- one accepted product packet
- canonical object and event schemas
- role and grant matrix
- relay binding manifest
- managed and customer-operated responsibility matrix
- export format and portability test plan
- negative fixtures for tenant and permission leaks

Exit:

- web, Omega, mobile, relay, and service teams decode the same fixtures
- no object has two durable owners
- the master roadmap admits the first web route

### Phase 1: read-only joined workroom

Duration target: two to four weeks.

Deliver:

- TanStack team and project list
- one read-only coding workroom route
- signed relay history
- linked Omega run and evidence projections
- mobile compact workroom view
- honest source, freshness, and gap labels

Exit:

- all three clients show the same stable references
- restart and offline cache tests converge
- no client can mutate work

### Phase 2: managed design-partner service

Duration target: four to six weeks.

Deliver:

- three to five isolated design-partner tenants
- dedicated database per tenant
- invite and role administration
- comments and bounded review decisions
- managed backup, restore, monitoring, and incident runbooks
- signed export and offline verifier
- simple subscription and support process

Exit:

- one customer completes the full adoption demonstration
- restore and export-import exercises pass
- no cross-tenant read or write succeeds

### Phase 3: Omega and mobile control

Duration target: four to eight weeks.

Deliver:

- Omega host pairing and grant projection
- start, steer, pause, resume, and stop through typed intents
- mobile attention and safe control
- durable outcomes after lost acknowledgement
- evidence review with producer and reviewer separation

Exit:

- physical mobile and installed Omega complete one real project journey
- every action has one durable outcome
- no raw host credential or path reaches web or mobile

### Phase 4: customer-operated kit

Duration target: after managed operational proof.

Deliver:

- pinned container and deployment templates
- Google Cloud and Kubernetes profiles
- upgrade, rollback, backup, restore, and migration tools
- conformance and security checks
- paid setup and advisory package

Exit:

- a clean customer environment imports a managed export
- the same clients open the moved workroom
- the customer completes an upgrade and rollback exercise

### Phase 5: hybrid service

Deliver:

- customer relay binding in the hosted web application
- bounded service projections
- push and support without record duplication
- exact incident boundary reports

Exit:

- OpenAgents service outage does not erase customer relay access
- relay outage appears as a relay fault, not generic workroom failure
- removal of the hosted service leaves a usable customer record

## 18. Risks and prerequisites

### 18.1 Highest risks

1. The team can create a new web UI before it freezes one shared model.
2. Omni, team chat, Sarah workroom, and Full Auto can become four competing
   workroom contracts.
3. A managed relay can become an undeclared identity or action authority.
4. “Customer owns the data” can become a claim without tested export and move.
5. Multi-tenant relay isolation can fail at query, subscription, search, blob,
   backup, or support boundaries.
6. Mobile can show an optimistic command as complete.
7. Omega can expose local authority through a convenient remote control.
8. A customer-operated matrix can exceed the support team.
9. The product can revive canceled Buzz application code by accident.
10. Broad web expansion can conflict with the current roadmap.

### 18.2 Required prerequisites

- owner admission for the web workroom product slice
- one canonical workroom contract owner
- tenant threat model and isolation proof
- relay signer and customer key custody policy
- export and deletion policy
- backup and recovery objectives
- service terms and privacy review
- metering and billing boundary
- support matrix
- physical mobile and installed Omega proof plan
- independent review for security and customer ownership claims

## 19. Decision points

The owner must decide these points before Phase 2.

1. Is the first managed tenant database dedicated or shared?
   This audit recommends dedicated.
2. Does the customer choose one primary relay or an admitted relay set?
   This audit recommends an admitted set with one write primary in v1.
3. Does OpenAgents custody any customer actor key?
   This audit recommends no by default.
4. Which workroom types enter v1?
   This audit recommends coding and product workrooms only.
5. Which web route family will the owner admit?
   This audit recommends `/workrooms` and `/projects`, behind authentication.
6. Which billing identity binds to a Nostr tenant?
   This audit recommends an explicit reversible OpenAuth binding.
7. What service data remains after tenant export or termination?
   The service terms must state this exactly.

## 20. Strategic recommendations

1. Treat workrooms as a cross-platform product, not an Omega feature.
2. Keep Omega as the deep-work and execution owner.
3. Make web the team and project control surface after explicit admission.
4. Keep mobile as a safe controller and attention surface.
5. Reuse server records, contracts, Sync stores, and behavior tests.
6. Rebuild the current web client in TanStack Start.
7. Select one canonical workroom model before UI expansion.
8. Use Nostr for signed shared history and portable identity.
9. Keep admission, metering, execution, billing, and release outside the relay.
10. Sell the managed private relay first.
11. Prove export and restore during every design-partner onboarding.
12. Offer customer-operated deployment as a paid support and advisory product.
13. Add hybrid service only after the customer relay can stand alone.
14. Do not market Buzz parity.
15. Market a portable OpenAgents workroom with honest operational choices.

## 21. Reference set

### Current direction

- [Master roadmap](../sol/MASTER_ROADMAP.md)
- [Omega accepted plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)
- [Omega roadmap](./ROADMAP.md)
- [Omega mobile adaptation audit](./2026-07-24-openagents-mobile-omega-adaptation-audit.md)
- [Sarah workroom specification](./2026-07-24-sarah-workroom-mvp-spec.md)
- [Community workroom contract](./2026-07-24-community-workroom-contract.md)
- [Sarah Nostr cutover](./2026-07-24-sarah-nostr-cutover.md)
- [Owned relay deploy runbook](../ops/2026-07-24-owned-nostr-relay-deploy.md)

### Current code and architecture

- `apps/openagents.com/apps/start/package.json`
- `apps/openagents.com/apps/start/src/routes/khala/chat-sync.tsx`
- `apps/openagents.com/apps/start/src/routes/workspaces/$workspaceId.tsx`
- `apps/openagents.com/workers/api/src/team-chat-routes.ts`
- `apps/openagents.com/workers/api/src/private-project-workspace-routes.ts`
- `apps/openagents.com/workers/api/src/omni-workroom-routes.ts`
- `apps/openagents-mobile/README.md`
- `packages/khala-sync-client/README.md`
- `packages/agent-surface/README.md`

### Historical evidence

- `apps/openagents.com/docs/2026-06-03-team-project-rooms.md`
- `apps/openagents.com/docs/2026-06-03-team-room-shared-history-autopilot-audit.md`
- `apps/openagents.com/docs/omni/README.md`
- [Historical web team Sync audit](../autopilot-coder/2026-06-13-autopilot-web-team-sync-audit.md)
- [Buzz documentation boundary](../buzz/README.md)
- [Canceled Buzz self-host runbook](../buzz/2026-07-22-buzz-self-host-and-sarah-runbook.md)

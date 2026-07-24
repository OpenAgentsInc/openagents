# Omega implementation roadmap

- Class: owner-accepted work-packet ledger
- Date: 2026-07-23
- Revised: 2026-07-24
- Revision: 3
- Status: active initial roadmap
- Product: Omega, the Zed-based OpenAgents Desktop application
- Client repository: `OpenAgentsInc/omega`
- Shared services repository: `OpenAgentsInc/openagents`
- Immediate version: `v0.2.0-rc1`

## 1. Outcome

Omega will become the primary OpenAgents Desktop surface.
It starts from the OpenAgents Zed fork.
Rust and GPUI own the native application.
Released TypeScript and Effect artifacts supply shared OpenAgents semantics.

The implementation order is:

1. Build the identity-first Omega onboarding journey.
2. Release a fully branded Omega bootstrap candidate.
3. Port the current OpenAgents Desktop product.
4. Add selected Buzz workroom and Nostr capabilities.
5. Make Omega the primary desktop application.
6. Retire the Electron application after a safe migration window.

The first target is `v0.2.0-rc1` during the 2026-07-23 owner session.
The target name is Omega.
The release description is OpenAgents Desktop bootstrap candidate.

## 2. Authority and status

The owner accepted this sequence on 2026-07-23.
This document is the initial work-packet ledger for that sequence.
It supersedes the prior Omega packet order where the two orders conflict.

The [accepted Omega plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)
still owns architecture and repository boundaries.
The [master roadmap](../sol/MASTER_ROADMAP.md) still owns program priority.
The [claim protocol](../sol/CLAIM_PROTOCOL.md) controls each mutation.

This roadmap does not prove a release.
It does not admit an unsigned artifact.
It does not make Omega the primary application.
Each release and cutover needs its named evidence.

The current Electron application remains supported.
It remains the rollback source until the final cutover.

The [identity-first onboarding roadmap](./2026-07-23-identity-first-onboarding-roadmap.md)
owns the first native product journey.
It specializes the brand and identity lane in this roadmap.

## 3. Current source truth

The audited Omega commit is `30c80504403b7dcb10c7d0a476577014ebc871f6`.
Its audited Zed fork base is `137c981cb03b38034ce727f4055f7423c051627f`.
At the roadmap date, the fork had no Omega release.

The inherited Zed workflow cannot publish an Omega release.
It uses Zed repositories, private runners, and Zed credentials.
It also needs Zed-specific release services.

The fork still contains important Zed identities.
These identities include names, application IDs, icons, and data paths.
They also include server, telemetry, update, and documentation URLs.

The inherited macOS package script uses Zed signing assumptions.
The inherited disk image also includes Zed commercial terms.
Omega must remove those terms from its package.

Some internal identifiers can remain for compatibility.
Examples include Rust crate paths and extension protocol values.
The release record must list each permitted compatibility identifier.

The audited Buzz commit is `76aeae703664a6a6741b82771df67c546886aafd`.
Its audited release is `v0.4.24`.
These values update the study pin only.
They do not change the accepted product boundary.

## 4. Product boundary

Omega is a separate repository.
Omega owns all GPUI client code.
Omega also owns native packaging, updates, icons, and application identity.

The OpenAgents monorepo owns reusable product semantics.
These semantics include Effect services, schemas, fixtures, and generated clients.
The monorepo also owns ProductSpec and AssuranceSpec packages.

Omega consumes released artifacts from the monorepo.
It must not use a relative workspace path into the monorepo.
Each consumed artifact needs a version and digest.

The first shared runtime can use a packaged Node 24 service.
Omega supervises that service from Rust.
The service uses a private, versioned protocol.
It must not expose a general local network API.

The team can move suitable services from TypeScript to Rust.
Each move needs differential behavior proof.
Each move also needs one clear authority cutover.
The old authority path must stop after that cutover.

## 5. Permanent design laws

1. Zed owns editor, project, buffer, language, terminal, and worktree truth.
2. OpenAgents owns work, agent, policy, receipt, and run truth.
3. Khala Sync owns shared conversation and timeline truth.
4. Nostr is an interoperability boundary, not command authority.
5. GitHub remains repository and claim authority until a separate cutover.
6. External agents keep their own configuration and credential custody.
7. Omega must not create a second home for an external agent.
8. A UI projection must not become a second durable authority.
9. A fixture pass must not become a packaged-release claim.
10. Rust does not gain authority only because it is native code.

## 6. Phase 0: release `v0.2.0-rc1`

### 6.1 Exact release claim

`v0.2.0-rc1` is a branded bootstrap candidate from the Omega fork.
It proves the new product identity and a clean native build.
It does not claim current Desktop feature parity.
It does not claim Buzz parity.
It does not trigger the primary cutover.

The first public text must state these limits.
The release must remain a GitHub prerelease.
It must not receive a stable or `latest` label.
It must not enter the current OpenAgents update feed.

The desired first artifact is macOS arm64.
That artifact is a discovery build, not cross-platform support.
The four-cell Desktop ReleaseSet remains a later support gate.

### 6.2 Release identity

The visible product name is `Omega RC`.
The publisher name is OpenAgents.
The macOS bundle ID is `com.openagents.omega.rc`.
The command name is `omega`.
The RC protocol name is `omega-rc`.
The RC data root is `Omega RC`.

The RC cache and log root uses `omega-rc`.
The artifact name is `Omega-v0.2.0-rc1-macos-arm64.dmg`.
The release tag is `v0.2.0-rc1`.

Stable and RC identities must stay separate.
Omega must not read or write Zed user data.
Omega must not read Electron Desktop secrets.
The first release must not migrate existing state.

### 6.3 Identity-first implementation slice

The first native product slice is Omega identity onboarding.
Use the
[identity-first onboarding roadmap](./2026-07-23-identity-first-onboarding-roadmap.md)
for its contract, architecture, issue order, and proof gates.

This slice uses the inherited Zed onboarding structure as its base.
It adds a new native identity section before editor setup.
It preserves the current Theme section exactly.
It also preserves the current registry-agent setup for later expansion.
The shared screen gets FirstRun and EditorSetup modes.

Start the rebrand at application identity and the new first-run surface.
Do not use a repository-wide string replacement as the implementation method.
The RC scan remains the release falsifier for exposed Zed product text.

Do not write a real person key before these gates pass:

- Omega data and credential isolation
- ProductSpec identity-profile admission
- AssuranceSpec proof design
- a released and digested shared identity contract
- secure-only custody

The suggested issue order is OMEGA-OID-00 through OMEGA-OID-09.
OMEGA-OID-09 joins the RC installed-journey gate.

### 6.4 Packet OMEGA-RC1-01: freeze source and legal inputs

Inputs:

- the exact Omega commit
- the exact upstream Zed commit
- all applicable licenses and notices
- the complete patch series
- the planned artifact identity

Work:

- Record the fork and upstream commits.
- Record the source and patch provenance.
- Inventory all third-party license duties.
- Define the source-delivery package.
- Remove Zed commercial terms from Omega packages.
- Keep required open-source notices.

Exit:

- A reviewer can reconstruct the exact source.
- The package contains the correct legal material.
- No Zed commercial terms appear as Omega terms.

### 6.5 Packet OMEGA-RC1-02: replace public branding

Replace all user-visible Zed branding.
Build new Omega surfaces in vertical slices.
Do not use an automatic global text replacement.
Use the final scan to find release blockers.
The scan must cover:

- application and executable names
- window titles and menu labels
- About, Welcome, Help, and error text
- icons, logos, disk image art, and installer art
- bundle IDs, protocol handlers, and file associations
- documentation, support, privacy, and terms links
- telemetry, crash, server, and update labels
- package names, artifact names, and release notes
- command names and shell integration
- default data, cache, log, and state directories

Do not replace technical identifiers without review.
Create a compatibility allow-list for each retained Zed identifier.
The allow-list must give a reason and an owner.

Exit:

- A user does not see Zed as the product or publisher.
- Omega does not collide with a Zed installation.
- The release record lists each retained technical identifier.

### 6.6 Packet OMEGA-RC1-03: isolate services and data

Disable inherited Zed production endpoints by default.
This work includes:

- account and collaboration services
- telemetry and crash submission
- update discovery and update download
- documentation and support links
- feature flags and remote configuration
- extension publication controls

A disabled service must show an honest state.
It must not silently call a Zed endpoint.
Omega can retain public Zed extension compatibility where licenses permit it.
That compatibility must use a documented boundary.

Use separate paths for:

- application data
- cache
- logs
- credentials
- browser state
- update state
- protocol handlers

Exit:

- A network audit finds no unexpected Zed service calls.
- A filesystem audit finds no Zed data mutation.
- A normal Zed installation still works after the Omega test.

### 6.7 Packet OMEGA-RC1-04: create an owned build path

Do not use the inherited Zed release workflow.
Create an Omega-owned local release command.
The command must start from a clean checkout.

The build record must include:

- Omega source commit
- upstream Zed commit
- Rust toolchain
- macOS and Xcode versions
- dependency lock digest
- target architecture
- application version
- bundle ID
- artifact digest and length
- signing and notarization identity
- source archive digest

The first path targets `darwin-arm64`.
Later packets add the complete release matrix.

Exit:

- One command creates repeatable candidate bytes.
- The command fails on a dirty source tree.
- The command fails when required identity data is absent.

### 6.8 Packet OMEGA-RC1-05: package and sign

Use the OpenAgents Developer ID convention.
Sign the application and all nested native code.
Notarize and staple the public macOS package.

Do not print credential values.
Do not inspect or export unrelated credentials.
Use only the existing release authority and operator path.

The package must include:

- the Omega application
- required notices
- exact version information
- source and provenance links
- a bootstrap limitation notice

Exit:

- `codesign` accepts the complete application.
- Gatekeeper accepts the downloaded package.
- Notarization and staple checks pass.
- The Team ID is `HQWSG26L43`.

### 6.9 Packet OMEGA-RC1-06: verify the installed journey

Test from a clean user profile.
Keep any installed Zed application in place.

The journey is:

1. Download the immutable candidate.
2. Check its digest.
3. Install Omega beside Zed.
4. Start Omega from Finder.
5. Open one local project.
6. Edit and save one file.
7. Open Git status.
8. Open a terminal.
9. Close and restart Omega.
10. Confirm project and layout restoration.
11. Remove Omega.
12. Confirm that Zed data did not change.

Also test:

- identity section before editor setup sections
- identity continuity after restart
- locked, lost, and unavailable custody
- no secret in logs, telemetry, or crash output
- first start without network access
- disabled service states
- invalid or absent update metadata
- crash and restart
- application shutdown
- data and process cleanup
- keyboard navigation
- visible focus
- minimum window size
- screen-reader labels for changed branding

Exit:

- The installed journey has a public-safe receipt.
- An independent reviewer accepts the receipt.
- No release-blocking defect remains open.

### 6.10 Packet OMEGA-RC1-07: publish the prerelease

Publish only after OMEGA-RC1-01 through OMEGA-RC1-06 pass.
Use tag `v0.2.0-rc1`.
Mark the GitHub release as a prerelease.

Attach:

- the signed and notarized package
- SHA-256 checksums
- the source archive
- required notices
- concise release notes
- known limitations
- the installation receipt
- the provenance receipt

The release notes must say:

- Omega is the new OpenAgents Desktop foundation.
- This build is a Zed-based bootstrap candidate.
- Current OpenAgents Desktop features are not present yet.
- This build is for evaluation.
- It does not replace the supported Electron application.
- It does not claim cross-platform support.

If a required gate fails, do not publish an ad hoc RC.
Record the failed gate and the next exact action.

## 7. Phase 1: port OpenAgents Desktop

Phase 1 starts after the bootstrap release.
Current code and behavior contracts define the parity baseline.
Dated roadmaps can supply acceptance evidence.
They do not override current source truth.

Do not port Electron, React, Monaco, Pierre, or xterm.
Do not port Electron IPC or Electron packaging.
Use Zed and GPUI for their native responsibilities.

### 7.1 Packet OMEGA-OA-01: shared runtime seam

Package the current headless OpenAgents runtime.
Start it under an Omega Rust supervisor.
Use a private framed protocol over standard input and output.

The protocol must include:

- schema and service versions
- capability negotiation
- request and event IDs
- stable work and thread refs
- generation fencing
- cancellation
- bounded queues
- backpressure
- gap and overload results
- health and restart state
- redacted diagnostics

Prove deterministic shutdown and zero surviving processes.
Do not import the Electron main process.

### 7.2 Packet OMEGA-OA-02: one agent front door

Add one native OpenAgents agent surface.
Complete one real Codex turn first.
Then add Claude and declared ACP agents.

Preserve each agent's:

- home directory
- configuration
- provider accounts
- credentials
- memory
- skills
- tools
- MCP configuration
- instruction files
- existing sessions where the adapter permits it

The first Hermes path uses its declared ACP or terminal adapter.
Omega must not create a second Hermes profile.

### 7.3 Packet OMEGA-OA-03: conversations and controls

Port:

- new, resume, rename, and search
- durable drafts and queued messages
- streaming text, reasoning, tool, and plan rows
- queue, steer, stop, and retry
- questions and approvals
- images and structured payloads
- child-agent topology
- history loss accounting
- startup and focus restoration
- cited deterministic and semantic recall

Use one neutral runtime event stream.
Do not reproduce the duplicate Electron turn engines.

### 7.4 Packet OMEGA-OA-04: identity, Sync, and mobile continuity

Bind Omega to canonical OpenAgents identity.
Use Khala Sync for shared conversations and timelines.
Do not create an Omega-only cloud thread store.

Prove:

- stable refs and versions
- cursor and gap behavior
- durable outcomes
- revocation
- lost acknowledgement
- restart
- one mobile follow-up or interrupt

### 7.5 Packet OMEGA-OA-05: Full Auto

Port the flagship Full Auto run.
Preserve the current eight-run limit.
Preserve one active lease for each thread.

Port:

- the default objective launcher
- advanced mission controls
- the concurrent run monitor
- pause, resume, stop, and retry
- routing policy and capacity truth
- guardrails and liveness
- restart reconciliation
- reports and receipts
- attention and RLM recall

Provider text must not determine success.
Only typed outcomes can close a run.

### 7.6 Packet OMEGA-OA-06: native agent-to-code loop

Connect work records to Zed projects and worktrees.
Use native buffers, diagnostics, Git state, and selections.

Add:

- exact-version proposal review
- partial accept and reject
- stale-edit and rebase refusal
- undo
- evidence backlinks
- hidden Git turn checkpoints
- reviewed revert and redo

Zed owns document and worktree truth.
The service receives stable capability references.

### 7.7 Packet OMEGA-OA-07: settings and agent ecosystem

Add:

- provider and model selection
- no-substitution truth
- harness health and recovery
- ACP diagnostics
- local usage consent
- maintenance controls
- extension provenance
- external-agent setup

Show capabilities from observed adapter truth.
Do not infer a capability from a provider name.

### 7.8 Packet OMEGA-OA-08: portable execution

Add native supervision for:

- managed sandboxes
- portable sessions
- Pylon
- admitted Fleet surfaces
- mobile run control

Use exact-version commands and durable outcomes.
Do not create a client-local run universe.
Do not give a phone local execution authority.

### 7.9 Packet OMEGA-OA-09: secondary capabilities

Add these capabilities after the main work loop:

- graph memory controls
- browser preview and annotations
- typed voice inputs
- ProductSpec inspection
- AssuranceSpec inspection
- redacted diagnostics
- evidence export
- public trace links

Keep graph memory off by default.
A recall result is a candidate, not authority.

### 7.10 Packet OMEGA-OA-10: parity closure

Create an exact parity ledger.
Give every current behavior one disposition:

- equivalent
- replaced with a stronger native behavior
- intentionally retired
- blocked with a named owner

Internal substrates do not count as visible parity.
These include unmounted workroom and Fleet components.
Each surface needs an admitted visibility contract.

The parity gate requires:

- current behavior-contract coverage
- packaged Omega journeys
- native accessibility proof
- restart and recovery proof
- owner-real provider proof
- Sync and mobile seam proof
- security and privacy review
- independent assurance

## 8. Phase 2: add selected Buzz capabilities

Omega will reproduce useful Buzz outcomes.
It will not deploy Buzz as a second product.
It will not claim complete Buzz parity.

### 8.1 Buzz outcome

A workroom joins people, agents, code, decisions, and receipts.
Each feature can have one durable work channel.
An authorized agent can read the channel history.
Each important action has an actor and signature.

Omega extends this model across the OpenAgents ecosystem.
It is not limited to one company workspace.
It can connect workrooms, Forum, Git, mobile, and market activity.

### 8.2 Packet OMEGA-BZ-00: freeze the workroom contract

Freeze the identities for:

- workrooms
- threads
- items
- people
- agents
- decisions
- blockers
- approvals
- evidence
- receipts

Define paging, audience, retention, deletion, and gap fields.
Define one writable owner for each domain.
Define one generated Rust and TypeScript protocol.
Do not import Buzz code in this packet.

### 8.3 Packet OMEGA-BZ-01: native workroom panes

Add GPUI pane types in this order:

1. Workrooms rail and list.
2. Home and Attention pane.
3. Thread and timeline pane.
4. Agents and People roster.
5. Work, run, and receipt inspector.

Start with read-only projections of real durable records.
Use stable refs across restarts.
Show source, freshness, and gap labels.

### 8.4 Packet OMEGA-BZ-02: bring your existing agent

Add one adapter registry for:

- ACP standard input and output
- authenticated ACP endpoints
- native OpenAgents harnesses
- bounded terminal adapters

MCP can supply tools.
It must not appear as a complete agent.

The first journey uses `hermes acp`.
It uses the existing `HERMES_HOME`.
Then add Codex, Claude, Goose, Grok, and Pylon.

Use read-only access for the default attachment.
Omega must not change agent setup or provider state.
It must not change model, skill, memory, tool, or MCP state.

An adapter declares its version and capabilities.
It also declares gaps, process generation, health, and cancellation.
Removing an agent revokes only the Omega grant.
It never deletes or rewrites the external home.

### 8.5 Packet OMEGA-BZ-03: interaction and attention

Add:

- replies and reactions
- mentions and pins
- bookmarks and read state
- reminders
- presence and typing
- notifications
- direct and group threads
- authorized search

Each search result needs a new access check.
Show source, scope, freshness, and gap state.
Do not put private records in a public index.

Use typed lifecycle state.
Show silence, stalls, and timeouts.
Do not infer completion from prose.

### 8.6 Packet OMEGA-BZ-04: code work as one room

Join each workroom to:

- project and worktree
- editor and language tools
- Git and terminal
- task and test
- diff and review
- commit and delivery

A branch or feature room is a projection.
Git refs remain repository authority.
OpenAgents admission remains action authority.
GitHub remains source and claim authority.

### 8.7 Packet OMEGA-BZ-05: decisions and workflows

Add:

- typed decisions
- typed blockers
- workflow steps
- human approval
- approve, deny, expire, and resume
- structural loop prevention
- delivery receipts

Use current OpenAgents intent and Full Auto paths.
Do not port the incomplete Buzz workflow runtime.
Each restart path must be durable and idempotent.

### 8.8 Packet OMEGA-BZ-06: multi-user governance

Add:

- company and team membership
- workroom roles
- guests and revocation
- private report queues
- moderator decisions
- tombstones
- audit
- export, deletion, and recovery

Reports never enforce an action by themselves.
Enforcement occurs at identity and command seams.
Public tombstones must not contain private report data.

Membership does not grant file or process authority.
It does not grant provider, release, spend, or publication authority.

### 8.9 Packet OMEGA-BZ-07: optional Nostr interoperability

Use an isolated signer and Nostr process.
It owns validation, IDs, signatures, relay sessions, and replay.
It can also own outbox recovery and selected event builders.

Start with:

- NIP-01 event mechanics
- NIP-42 relay authentication
- NIP-29 group projections
- NIP-34 Git fact projections
- selected OpenAgents event types

Private NIP-17 and NIP-44 data comes later.
It needs separate custody and retention admission.

A signature proves that a key signed exact bytes.
It does not prove authorization or truth.
Relay acceptance does not settle an OpenAgents action.
A valid signed command still needs OpenAgents admission.

### 8.10 Packet OMEGA-BZ-08: secondary collaboration

Add files and previews first.
Then add canvases and frame comments.
Add existing Forum projections and public-safe social views.

Voice and recording need a separate audio decision.
Do not clone the Buzz media stack.

### 8.11 Packet OMEGA-BZ-09: internal dogfood

Move one complete feature through Omega.
The journey starts with a decision.
It ends with a delivery receipt.

The journey includes:

1. Open the feature workroom.
2. Attach an existing configured agent.
3. Complete code and tests.
4. Review the exact change.
5. Record approval.
6. Record the delivery receipt.

Keep GitHub as source and claim authority.
Keep Electron as rollback until cutover.
Repeat this journey before a broader company-home claim.

## 9. Buzz capabilities that Omega will not port

Omega will not port:

- the Buzz server stack
- the Buzz relay as an authority
- the Buzz forge product
- the Tauri application
- the Flutter application
- the Buzz administrative client
- Postgres, Redis, or MinIO only for Buzz parity
- the Buzz CLI or Buzz Agent
- broad Buzz ACP code
- a non-streaming agent turn model
- a vendor-specific extension registry
- all custom Buzz NIPs
- the complete Buzz screen or event set

Omega will not create a second Forum or Git host.
It will not create a second conversation or receipt store.
It will not create a second claim or provider authority.

An inbound event must not start work automatically.
A membership must not expand authority.
Omega must not copy a home, key, credential, or secret.
It must not put confidential plain text on Nostr.

Voice, recording, and culture features are outside the first slice.
The first slice must not use a “Slack killer” claim.

## 10. Phase 3: migration and primary cutover

### 10.1 Data migration

Import supported:

- conversations
- run records
- preferences
- receipts
- project refs
- Sync state

The import must be idempotent.
It must preserve stable identities.
It must support export and rollback.

Do not extract Electron secure-storage credentials.
Require authentication again when safe migration is not possible.

### 10.2 Release completion

Before primary cutover, build the complete supported matrix.
The required signed cells are:

- `darwin-arm64`
- `darwin-x64`
- `linux-arm64`
- `linux-x64`

Windows x64 remains experimental under the current Desktop contract.
Omega needs its own accepted release contract before support claims.

The final release set must bind:

- both repository commits
- Rust and Node versions
- protocol and schema versions
- every artifact digest
- source and license material
- update and rollback data
- component ledgers
- release notes and known limitations

### 10.3 Cutover gates

Omega becomes primary only when:

1. The exact Omega ProductSpec has an admission receipt.
2. The exact Omega AssuranceSpec has an admission receipt.
3. Current Desktop parity dispositions are complete.
4. The signed release matrix passes.
5. Update, rollback, and recovery pass.
6. Accessibility and performance gates pass.
7. Mobile continuity passes.
8. External-agent configuration stays intact.
9. Two consecutive Omega candidates pass migration and rollback.
10. The owner accepts the installed journey.

### 10.4 Electron retirement

First make Electron read-only for shared state.
Keep it for one migration and rollback window.
Then stop Electron writes.

Delete the old application only after:

- import and export proof
- repeat migration proof
- rollback proof
- credential reauthorization proof
- mobile continuity proof
- owner disposition

Keep historical artifacts and receipts.
Do not rewrite old evidence as Omega evidence.

## 11. Proof contract for every packet

Each implementation packet must name:

- exact repository and commit
- owned paths
- hot contracts
- source authority
- acceptance criteria
- falsifier
- verification commands
- packaged journey
- evidence paths
- rollback
- owner and independent reviewer

Each runtime packet must test:

- current and previous protocol versions
- stale generations
- cancellation
- crash and restart
- bounded buffers
- private-data redaction
- deterministic shutdown
- no surviving child process

Each GPUI packet must test:

- keyboard use
- visible focus
- screen-reader labels
- minimum window size
- state restoration
- light and dark appearance
- reduced motion where applicable

Each stateful packet must test:

- wrong thread
- wrong project
- revoked grant
- dirty worktree
- stale file version
- late event
- duplicate event
- missing event
- output truncation

## 12. Parallel work model

One coordinator owns sequence, integration, and release truth.
Implementation uses separate clean worktrees.
Read-only audits can run in parallel.

The first release can use these parallel lanes:

1. Brand and identity lane.
2. Service and data-isolation lane.
3. Packaging and signing lane.
4. Legal, provenance, and release-notes lane.
5. Independent installed-journey lane.

Serialize changes to:

- application identity
- bundle metadata
- package scripts
- lockfiles
- release manifests
- protocol schemas
- generated catalogs

After RC1, parallelize separate capability packets.
Do not parallelize two changes to one authority contract.

## 13. Immediate execution queue

The queue starts now:

1. OMEGA-OID-00: freeze the identity-first contract.
2. OMEGA-OID-01: isolate Omega application identity and data.
3. OMEGA-OID-02: publish the shared identity contract.
4. OMEGA-OID-03: add fixture identity states to the current GPUI screen.
5. OMEGA-OID-04: add isolated signing and secure custody.
6. OMEGA-OID-05: add recovery and explicit creation.
7. OMEGA-OID-06: connect live identity state to GPUI.
8. OMEGA-OID-07: route every first launch through identity setup.
9. OMEGA-OID-08: add the Editor Onboarding replay mode.
10. OMEGA-OID-09: prove the packaged identity-first journey.
11. OMEGA-RC1-01 through OMEGA-RC1-07: complete the bootstrap release.
12. OMEGA-OA-01 through OMEGA-OA-10: close Desktop parity.
13. OMEGA-BZ-00 through OMEGA-BZ-09: add selected Buzz outcomes.
14. Complete migration, release, cutover, and retirement.

Only the current ready packet can mutate its owned paths.
A failed packet stays open with typed evidence.
The coordinator then continues independent ready work.

## 14. Milestone definitions

### Milestone A: branded bootstrap

Omega `v0.2.0-rc1` is available as a signed prerelease.
It has no Zed product identity or state collision.
Its limitations are explicit.

### Milestone B: useful daily agent surface

Omega can run one real agent turn.
It can resume work and review a native change.
It preserves the external agent home.

### Milestone C: current Desktop parity

All current Desktop behavior has a reviewed disposition.
Full Auto, Sync, mobile, review, and recovery pass.
The Electron application remains available for rollback.

### Milestone D: native workroom

People and external agents can use one durable work channel.
The channel connects conversation, code, decisions, and receipts.
Permissions and agent configuration remain correct.

### Milestone E: ecosystem work home

Omega connects workrooms with Forum, mobile, market, and release records.
Each connected system keeps its own authority.

### Milestone F: primary cutover

The full release and migration gates pass.
The owner accepts Omega as the primary desktop application.
Electron enters its final read-only window.

## 15. Completion rule

Milestone F acceptance completes this roadmap.
Completion requires code, tests, packaged journeys, and receipts.
A document update or fixture pass is not completion.

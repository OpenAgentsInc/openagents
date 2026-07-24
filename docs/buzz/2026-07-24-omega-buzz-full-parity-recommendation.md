# Omega Buzz full-parity recommendation

- Class: current product recommendation
- Status: proposed weekly execution target
- Date: 2026-07-24
- Decision owner: owner
- Buzz baseline: `v0.4.24` at `710ed9fff57878a1d69f809b80a6ee0416c53fc4`
- Buzz main observed: `b78a684cfa997bbffbc86ac9c311f4f7af25d11a`
- Omega main observed: `676392ed32e5ab33c14c651974c6682f2e32920f`
- OpenAgents main observed: `ee1f919ab70772e14ff1aafb9437c92a2cb4fc54`
- Zed main observed: `2d692b4f2a0599d22d5c9c123e2ce6cad4d967fc`

## Recommendation

Aim for full Buzz product-outcome parity.

Accept the full useful product surface now.
Do not make a smaller product the goal.
We can remove weak features after real use.

This does not mean a full Buzz source port.
It does not mean exact screen, event-kind, or server parity.
It means that an Omega user can complete each useful Buzz job with an equal or
better native path.

The correct target for this week is **full-core parity in owner dogfood**.
The full parity ledger should be accepted on the first day.
The daily workroom, agent, code, decision, and receipt paths should work by the
end of the week.
Device breadth, voice, rich media, and optional Nostr projection can close in
later proof milestones.

The practical import ratio is:

| Surface                         |          Amount to accept | Method                                                            |
| ------------------------------- | ------------------------: | ----------------------------------------------------------------- |
| Useful user outcomes            |                      100% | Rebuild as native Omega and OpenAgents behavior                   |
| Reliability lessons             |                      100% | Turn known failures into tests and contract rules                 |
| Product names and screen layout |               0% required | Use the Omega pane grammar                                        |
| Buzz application source         |                        0% | Do not copy the Tauri, React, Flutter, relay, or ACP applications |
| Buzz deployment stack           |                        0% | Do not restore Postgres, Redis, S3, or relay services for parity  |
| Buzz custom NIP surface         | Only proven interop needs | Keep it behind an optional projection boundary                    |

## Why this is the right time

Omega already owns the expensive editor and agent substrate.
It does not need to rebuild a code editor, terminal, Git client, task runner,
debugger, or ACP thread system.

At the observed revisions, Omega has 48 commits that are not in current Zed.
Current Zed has 15 commits that are not in Omega.
That is a manageable first rebase budget.
It is also a warning to keep product work outside deep Zed seams where
possible.

Omega already has these owned pieces:

- native Nostr identity and isolated signing
- platform key storage and recovery
- the versioned `openagents.omega.effectd.v1` protocol
- a supervised and bounded `omega-effectd` process
- native Full Auto launch, control, handoff, and receipt surfaces
- native Agent Computer dispatch
- real Codex and Claude ACP threads
- native project, editor, terminal, diff, Git, task, and worktree truth

The missing center is the workroom.
Omega needs one place that joins people, agents, code, decisions, runs, and
receipts.
Buzz is strong evidence for that product shape.
Zed is the stronger native implementation base.
OpenAgents is the stronger policy, protocol, and durable record base.

## What full parity means

Full parity is measured by jobs and failure behavior.
It is not measured by file count or route count.

An outcome is at parity only when all these statements are true:

1. The normal user job works in a packaged Omega build.
2. The job uses the declared OpenAgents authority.
3. Restart, replay, paging, and duplicate delivery do not corrupt it.
4. The user can see pending, running, stalled, failed, and complete states.
5. The user can recover or retry without silent loss.
6. Authorization and audience checks happen before an effect.
7. The path has an executable contract test or a recorded human proof.

### Accepted parity ledger

| Domain                  | Full-parity outcome                                                                                       | Omega implementation direction                                       | This week                           |
| ----------------------- | --------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- | ----------------------------------- |
| Home and attention      | Inbox, mentions, unread work, reminders, active runs, and failures                                        | Native attention projection and rail                                 | Required                            |
| Workrooms               | Rooms, channels, nested threads, replies, reactions, pins, bookmarks, and read state                      | Native GPUI workroom panes                                           | Required                            |
| Direct messages         | Owner-private and group conversations with the same history rules                                         | OpenAgents thread authority with native Omega projection             | Required for owner-private use      |
| People and agents       | Roster, roles, presence, status, profile, mention, and team membership                                    | One actor and membership model for humans and agents                 | Required                            |
| Agent operation         | Existing agents, managed agents, streaming turns, interrupt, retry, handoff, queues, stalls, and receipts | Zed ACP mechanics plus the Omega supervisor and OpenAgents contracts | Required                            |
| Code work               | Project, branch, worktree, editor, terminal, diff, review, test, approval, and delivery context           | Existing Zed substrate inside the workroom                           | Required                            |
| Decisions and workflows | Typed decisions, blockers, approvals, schedules, reminders, and loop prevention                           | OpenAgents intents and Full Auto policy                              | Required                            |
| History and search      | Authorized full-text search across messages, decisions, runs, files, and code references                  | Server query plus bounded local projection                           | Required                            |
| Files and canvas        | Attachments, previews, annotations, and spatial work artifacts                                            | Native preview first, richer canvas after the core path              | Stretch                             |
| Forum and stream        | Long-form topics, activity stream, and deliberate public projection                                       | OpenAgents Forum and timeline projections                            | Accepted, not a Week 1 release gate |
| Governance              | Membership, role changes, moderation, tombstones, audit, and revocation                                   | OpenAgents authority and signed receipts                             | Stretch                             |
| Cross-device use        | Web and mobile control with consistent history and read state                                             | OpenAgents web and mobile clients over the same record               | Accepted, later proof milestone     |
| Voice                   | Dictation, huddles, transcripts, and recording controls                                                   | Governed OpenAgents audio lane with workroom records                 | Accepted, later proof milestone     |
| Signed interop          | Pairing, remote signing, and optional Nostr ingress and egress                                            | Isolated signer and admitted projection process                      | Accepted, later proof milestone     |

This ledger accepts the whole useful surface.
The last column sets delivery order.
It does not remove later domains from the product.

## Required architecture substitutions

Full parity should use Omega authorities.
It should not import Buzz authorities.

| Buzz concept                    | Omega equivalent                                          | Reason                                                       |
| ------------------------------- | --------------------------------------------------------- | ------------------------------------------------------------ |
| Nostr relay as workspace log    | Khala Sync and Cloud SQL work record                      | Keeps one writable conversation authority                    |
| Relay materialized views        | Bounded local and server projections                      | Preserves replay without making the projection authoritative |
| Buzz Tauri desktop              | Omega Rust and GPUI                                       | Uses the accepted native product surface                     |
| Buzz Flutter mobile             | OpenAgents mobile                                         | Keeps the accepted three-app product shape                   |
| Buzz Git forge                  | Zed Git UI plus GitHub repository authority               | Avoids a second object and ref authority                     |
| Buzz ACP pool                   | Zed ACP sessions plus Omega supervision                   | Preserves configured agents and native thread behavior       |
| Buzz agent runtime              | OpenAgents harnesses and user-owned external agents       | Avoids a second compute control plane                        |
| Buzz workflow engine            | OpenAgents typed intents, Full Auto, and approval brokers | Keeps policy and effects in one system                       |
| Buzz CLI                        | Versioned OpenAgents protocol and Pylon tools             | Avoids another user-facing command surface                   |
| Relay search                    | Authorized OpenAgents search and Zed project search       | Keeps audience checks at the query boundary                  |
| Nostr identity in every service | Omega identity service and isolated signer                | Reduces key custody and signing scope                        |

The workroom record should have one stable vocabulary.
The first contract should cover these values:

- workroom
- thread
- entry
- actor
- membership
- attention item
- decision
- approval
- agent run
- receipt
- attachment
- external projection reference

Each value needs a stable identifier, audience, author, creation time, and
revision rule.
Each paged collection needs a cursor and an explicit gap state.
Each effect needs an idempotency key and a result receipt.

## Plan for this week

### Day 1: freeze the parity contract

Complete `OMEGA-BZ-00` before broad UI work.

- Pin parity evaluation to Buzz `v0.4.24`.
- Track later Buzz changes in a separate delta list.
- Accept the complete outcome ledger in this report.
- Define the workroom record and actor identity rules.
- Name the one writable authority for each field.
- Generate Rust and Effect protocol types from one schema.
- Define paging, gaps, replay, stale writes, duplicates, and revocation.
- Add the known Buzz failure cases to the test plan.

Do not chase Buzz main during the week.
A weekly delta review is enough.

### Days 1 and 2: build the native workroom frame

Complete the first usable part of `OMEGA-BZ-01`.

Build these GPUI panes:

- workroom rail and list
- Attention
- Thread and timeline
- People and Agents
- details and receipt inspector

Start with real read-only records.
Then add writes through the same service boundary.
Do not connect the inherited Zed collaboration client to Zed production
services.
Its channel and collaboration crates are behavior references only.

### Days 2 and 3: make conversation reliable

Complete `OMEGA-BZ-03` for the core text path.

- send and stream a message
- reply in a thread
- mention a human or agent
- react, pin, bookmark, and mark read
- show unread and attention counts
- search authorized history
- page backward and recover an explicit gap
- restart without duplication or silent loss

The sender must see the same accepted entry that every authorized reader sees.
An agent reply must not depend on a best-effort CLI write.

### Days 2 and 3: attach existing agents

Complete `OMEGA-BZ-02` beside the text path.

Use existing Codex and Claude configurations first.
Add a Hermes journey when a supported local installation is present.

The default mode is `use_existing_read_only`.
Omega must not replace a user configuration, login, model choice, tool set, or
credential file.
It must not install a runtime without a clear user action.

Show these states in the roster and thread:

- unavailable
- ready
- queued
- starting
- running
- stalled
- interrupted
- failed
- complete

Use bounded lazy capacity.
Do not copy Buzz's eager default pool behavior.

### Days 3 and 4: make code work part of the room

Complete the core of `OMEGA-BZ-04`.

- bind a workroom to a real Zed project
- create or select a branch and worktree
- open referenced files and ranges
- run terminal commands, tasks, and tests
- show diffs and review comments
- link agent turns to their project generation
- attach result receipts to the room
- preserve GitHub as the hosted review and merge authority

The workroom should point into the existing project graph.
It must not create a parallel file or Git model.

### Days 4 and 5: add decisions and governed action

Complete the core of `OMEGA-BZ-05`.

- create a typed blocker or decision
- request an approval
- show the independent evidence for the request
- approve, reject, or ask for a change
- apply the admitted effect once
- write a target receipt before a completion claim
- prevent self-trigger loops and duplicate effects

Silent workflow invocation should not be a default.
The user needs an attention item or an explicit policy that permits it.

### Days 5 and 6: close the daily-use surface

- add files and native previews
- add a simple annotated work artifact
- add roster and thread filters
- add saved search or a stable Attention view
- add basic membership and role changes
- add tombstone and revocation behavior
- expose one web or mobile read path if the common record makes it cheap

Do not build a second forum or a second forge.
Project the room into the existing Forum and GitHub surfaces.

### Days 6 and 7: dogfood and prove

Use one real Omega feature as the proof task.
Run the whole task through one workroom.

The proof must include:

1. The owner opens the packaged Omega build.
2. The owner opens a real project workroom.
3. The owner attaches existing Codex and Claude agents.
4. An agent receives a mention and streams a reply.
5. One agent hands work to another agent.
6. The owner interrupts and retries one turn.
7. The room records a code change, test, diff, decision, approval, and receipt.
8. Omega restarts and restores the same state.
9. A stale, duplicate, and unauthorized input is rejected visibly.
10. An independent reviewer checks the evidence.

The week succeeds when this path is in daily owner use.
Call it **full-core parity beta**.
Do not call it full parity complete until the accepted ledger closes.

## Parallel work lanes

The contract work must be serialized first.
After that freeze, four lanes can move in parallel.

| Lane           | Owns                                                            | Must not own                        |
| -------------- | --------------------------------------------------------------- | ----------------------------------- |
| Native UI      | GPUI panes, focus, keyboard, accessibility, rendering           | Durable policy or a second store    |
| Effect service | Queries, commands, policy, sync, and projections                | Zed project truth                   |
| Agent bridge   | ACP lifecycle, streams, queues, interrupt, retry, and handoff   | User home or provider configuration |
| Assurance      | replay, failure injection, packaging, human proof, and receipts | Product authority                   |

Keep protocol changes small and generated.
Do not let each lane add its own message type or status vocabulary.

## Current issue state and its effect on the plan

Omega has four open issues at this snapshot:

- [#8, packaged identity journey](https://github.com/OpenAgentsInc/omega/issues/8)
- [#9, identity-first onboarding](https://github.com/OpenAgentsInc/omega/issues/9)
- [#16, installed RC brand and package](https://github.com/OpenAgentsInc/omega/issues/16)
- [#26, Full Auto proof](https://github.com/OpenAgentsInc/omega/issues/26)

The code pace is high.
The recent closed set includes the Full Auto implementation from
[#19](https://github.com/OpenAgentsInc/omega/issues/19) through
[#25](https://github.com/OpenAgentsInc/omega/issues/25).
It also includes the Agent Computer implementation from
[#27](https://github.com/OpenAgentsInc/omega/issues/27) through
[#30](https://github.com/OpenAgentsInc/omega/issues/30).

The remaining Omega work is proof-heavy.
That supports starting `OMEGA-BZ-00`, `OMEGA-BZ-01`, and `OMEGA-BZ-02` now.
The proof lanes for identity, packaging, and Full Auto should continue beside
the workroom code.
They still block installed-product claims.

OpenAgents [#9216](https://github.com/OpenAgentsInc/openagents/issues/9216)
is also open.
It needs an independent reviewer for the identity-first AssuranceSpec.
The same independent role should review the Week 1 workroom proof.

The canceled Buzz deployment issue set must remain closed.
Do not reopen it as a way to get faster parity.
The retained self-host runbook is evidence, not an operations plan.

## Lessons from current Buzz issues

Buzz is moving quickly.
Its open issues show which parity claims are easy to fake.
Omega should make these cases acceptance tests before they become defects.

| Risk                                                     | Current Buzz evidence                                                                                     | Required Omega rule                                                                   |
| -------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| Startup can wait forever                                 | [#2723](https://github.com/block/buzz/issues/2723)                                                        | Every probe has a deadline and a visible degraded state                               |
| Agent settings can target the wrong instance             | [#2717](https://github.com/block/buzz/issues/2717) and [#2692](https://github.com/block/buzz/issues/2692) | Bind settings to stable workspace and agent identities, then verify the applied value |
| A requester can lose activity visibility                 | [#2716](https://github.com/block/buzz/issues/2716)                                                        | Separate authorization to observe a requested turn from runtime ownership             |
| A valid agent answer can disappear                       | [#2698](https://github.com/block/buzz/issues/2698) and [#2459](https://github.com/block/buzz/issues/2459) | Accept replies through one typed and acknowledged result channel                      |
| An agent can appear healthy while doing no work          | [#2641](https://github.com/block/buzz/issues/2641) and [#2453](https://github.com/block/buzz/issues/2453) | Health needs readiness, subscription, heartbeat, and restart controls                 |
| Eager agent pools can exhaust the machine                | [#2631](https://github.com/block/buzz/issues/2631)                                                        | Start lazily with a small visible capacity limit                                      |
| Duplicate installations can duplicate actors             | [#2648](https://github.com/block/buzz/issues/2648) and [#2515](https://github.com/block/buzz/issues/2515) | Use composite actor identity and idempotent membership                                |
| Audit verification can be structurally present but false | [#2637](https://github.com/block/buzz/issues/2637) and [#2620](https://github.com/block/buzz/issues/2620) | Verify complete replay with canonical time precision and truncation tests             |
| Handoffs can be best effort                              | [#2442](https://github.com/block/buzz/issues/2442)                                                        | Model claim, accept, reject, timeout, return, and receipt as explicit states          |
| Cross-device use is still a gap                          | [#2682](https://github.com/block/buzz/issues/2682)                                                        | Keep the work record client-neutral from the first schema                             |
| Remote signing changes custody                           | [#2700](https://github.com/block/buzz/issues/2700)                                                        | Add it only through the sovereign signer and an explicit capability grant             |
| Approval text can lack checkable evidence                | [#2509](https://github.com/block/buzz/issues/2509)                                                        | Bind approval requests to an independently checkable verdict reference                |

This issue set argues for full parity with stronger contracts.
It does not argue for a direct port.

## Sarah in the first workroom

`principal.sarah` should be a first-class actor in the parity model.
She should use the same thread, decision, run, approval, and receipt records as
every other agent.

Her first path should stay owner-private.
Public posts must remain a separate brokered effect with their own gate.
Omega must not add a Sarah-only store, API, or application.

Sarah should be part of the Week 1 dogfood task:

- read the bounded owner-private project context
- choose one next action or name one blocker
- dispatch work through an admitted broker
- observe agent progress in the room
- ask for an approval when authority requires it
- cite the result receipt in her owner update

The current Sarah video work adds a useful proof lesson.
Episode 262 uses live Omega screenshare, honest state labels, and cut-point
inspection.
Use that discipline for the workroom demo.
Show a current packaged build and visible live behavior.
Do not use a still image as evidence of an interactive path.

The talking-head and music pipeline is a communications tool.
It is not workroom authority.
Do not revive the removed Sarah application or the old avatar stack as part of
this port.

## Things we should not do

1. Do not deploy or operate the canceled Buzz server.
2. Do not use a relay as the workspace authority.
3. Do not copy the Buzz Rust, Tauri, React, Flutter, CLI, or ACP applications.
4. Do not reconnect Omega to Zed production collaboration services.
5. Do not create a second Git forge, Forum, identity service, or workflow store.
6. Do not implement all 15 custom NIPs merely because `nostr-effect` supports
   them.
7. Do not let inbound Nostr events start agents or effects without admission.
8. Do not place raw secret keys in events, environment files, logs, or normal
   UI state.
9. Do not mutate a user's agent home, login, provider account, model, or tool
   configuration by default.
10. Do not start a large managed agent pool at application launch.
11. Do not treat `running` as proof that an agent can receive or answer work.
12. Do not accept best-effort message delivery, handoff, approval, or receipt
    writes.
13. Do not widen authority because an actor joined a room.
14. Do not let projections or local caches accept writes as if they are the
    source record.
15. Do not claim parity from unit tests or source presence alone.
16. Do not chase daily Buzz main changes during the sprint.
17. Do not let parity work delay the first Zed upstream rebase rehearsal.
18. Do not add rich canvas, voice, or public projection before their audience
    and retention rules exist.

## Main considerations

### Baseline drift

Buzz releases quickly.
Pinning `v0.4.24` makes parity testable.
Review upstream once each week.
Classify each delta as accepted, substituted, rejected, or already covered.

### Zed rebase cost

Omega is a tracked fork.
The current divergence is small enough to rehearse a rebase now.
Keep workroom policy in owned crates and `omega-effectd`.
Keep GPUI integration narrow.
Avoid broad edits in inherited collaboration and project internals.

### Authority clarity

A polished pane can hide split authority.
Every writable field needs one owner before UI work lands.
The native client may cache and project.
It must not become a second conversation server.

### Agent identity

Agent identity needs more than a display name.
Use workspace, owner, installation, adapter, and agent identifiers where the
distinction matters.
Membership and mention resolution must be idempotent.

### Agent lifecycle

The room must expose real readiness and delivery state.
Timeouts, queue limits, cancellation, crash recovery, and retry are product
features.
They are not hidden runtime details.

### Proof debt

Omega's open issue set is mostly packaged and human proof.
High source velocity does not remove that gate.
Run an Assurance lane from the first workroom commit.
Do not leave proof for the last day.

### Accessibility and performance

The custom GPUI editor and panes need task-based accessibility proof.
The workroom also needs large-history and high-frequency stream tests.
Virtualize long lists.
Bound retained render state and background workers.

## Milestones after this week

The full parity goal should close through three proof milestones.

| Milestone                | Result                                                                                                                 |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------- |
| 1. Full-core parity beta | One owner can use the workroom daily with people, agents, code, decisions, search, and receipts                        |
| 2. Collaboration parity  | Membership, moderation, cross-device use, files, canvas, Forum projection, and multi-user failure proof                |
| 3. Full ledger closure   | Voice, recording controls, signed interop, optional Nostr projection, accessibility, scale, and packaged release proof |

Pruning should happen only after Milestone 1 dogfood.
Remove a feature when observed use or cost argues against it.
Do not remove it because the first implementation is inconvenient.

## Research basis

This recommendation used the complete current `docs/buzz/` and `docs/sarah/`
corpora.
Historical Sarah avatar, operations, receipts, scoreboards, and removal records
were treated as historical evidence.
The current `principal.sarah` README, authority, runbook, and Episode 262
lessons were treated as current direction.

It also used all current Zed and Omega narrative architecture material and the
generated evidence inventories referenced by those documents.
The main sources were:

- [Buzz teardown](../teardowns/2026-07-21-buzz-teardown.md)
- [canceled Buzz self-host runbook](2026-07-22-buzz-self-host-and-sarah-runbook.md)
- [accepted Omega plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)
- [Omega roadmap](../omega/ROADMAP.md)
- [Zed adaptation analysis](../ide/2026-07-18-zed-agent-ide-adaptation-analysis.md)
- [Zed and Effect Rust architecture](../ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md)
- [Zed port status](../ide/2026-07-19-porting-zed-to-effect-status.md)
- [Omega and Zed UI gap analysis](../sol/2026-07-19-openagents-ide-zed-programmatic-ui-gap-analysis.md)
- [Zed teardown](../teardowns/2026-07-18-zed-teardown.md)
- [Zed ACP server demo](../../apps/openagents-desktop/docs/meta-agent-acp-server-zed-demo.md)
- [Sarah current index](../sarah/README.md)
- [Sarah authority](../authority/SARAH_AUTHORITY.md)

The repository source inspection used current Omega `origin/main`.
It covered the owned identity, effect service, Full Auto, Agent Computer, ACP
bridge, release packaging, and inherited Zed collaboration surfaces.
The issue review used live GitHub state on 2026-07-24.

## Final call

Accept full product-outcome parity as the goal.
Freeze the target to Buzz `v0.4.24`.
Build the result as an Omega-native workroom over OpenAgents authority.

For this week, require the full-core owner journey.
Start with the contract, workroom frame, reliable text, existing agents, code
context, decisions, and receipts.
Run identity, package, and independent proof work beside it.

This is ambitious and credible at the current pace.
A direct Buzz port would be faster only for the first screenshots.
It would be slower for the product because it would add duplicate authorities,
operations, and clients that Omega already chose to replace.

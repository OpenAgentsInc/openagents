# Sarah workroom MVP specification (Omega)

- Class: proposed implementation specification
- Date: 2026-07-24
- Revision: 2
- Status: proposed, not admitted
- Product: Omega, the Zed-based OpenAgents Desktop application
- Packets: `OMEGA-SW-00` through `OMEGA-SW-07`, `SARAH-NR-00` through
  `SARAH-NR-09`
- Client repository: `OpenAgentsInc/omega`
- Service repository: `OpenAgentsInc/openagents`
- Protocol repository: `OpenAgentsInc/nostr-effect`
- OpenAgents pin: `93bbdd70b1` (`origin/main`, 2026-07-24)
- Omega pin: `87703b753a` (`origin/main`, 2026-07-24)
- `nostr-effect` pin: `c160378` (`main`, 2026-07-24)
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`

Revision 2 adds Part 2. The owner directed on 2026-07-24 that the Sarah runtime
moves entirely to Nostr on a relay that OpenAgents controls. Part 1 keeps the
pane, the service seam, and the account session. Part 2 replaces the record and
the transport under that pane. Read both parts together.

## 1. Outcome

An Omega user opens one workroom pane and talks to Sarah.

The pane shows one durable conversation, ordered agent activity, exact
authority receipts, and honest run state. Buzz proves the product shape: a
room where a human and an agent are co-equal members of one durable record.
Omega copies that outcome. Omega does not copy the Buzz substrate.

Sarah stays on OpenAgents API infrastructure. Her record is the existing
owner-private Khala Sync thread on Cloud SQL. Her turn engine is the existing
hosted Khala runtime. Her authority is the existing admitted profile. Omega
becomes a new client of that runtime. Omega does not become a second Sarah.

This document specifies the smallest complete slice that reaches daily owner
use. It is a specification, not an admission and not a release claim.

Part 2 changes what sits under the pane. The owner directed that the Sarah
runtime moves entirely to Nostr on an OpenAgents-controlled relay. Part 1 then
describes the client and the pane. Part 2 describes the record, the transport,
the relay, and the migration.

## 2. Authority chain

| Role | Artifact | Note |
| --- | --- | --- |
| Product intent | `specs/openagents/sarah-owner-orchestrator.product-spec.md` | revision 5 |
| Sarah authority | `docs/authority/SARAH_AUTHORITY.md` | revision 6 |
| Root authority | `AUTHORITY.md` | revision 8 |
| Omega packet order | `docs/omega/ROADMAP.md` | §8, Phase 2 |
| Workroom direction | `docs/buzz/2026-07-24-omega-buzz-full-parity-recommendation.md` | current owner direction |
| Product boundary | `docs/sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md` | accepted plan |
| Buzz evidence | `docs/teardowns/2026-07-21-buzz-teardown.md` | source audit |
| Runtime seam | `docs/omega/2026-07-24-omega-effectd-supervisor.md` | `OMEGA-FA-02` |
| Cloud seam | `docs/omega/2026-07-24-omega-agent-computer-contract-freeze.md` | `OMEGA-AC-00` |

This specification specializes roadmap §8. It does not replace ProductSpec or
AssuranceSpec authority. It does not change the Sarah authority profile. An
Omega issue and an owner acceptance remain the admission gate.

## 3. Current truth

Every statement in this section comes from source at the pinned commits.

### 3.1 The Sarah record already exists

`apps/openagents.com/workers/api/src/sarah-owner-routes.ts` owns the principal
route `POST /api/mobile/sarah` (`route.mobile.sarah.principal.v1`).

The route resolves one opaque thread reference for one authenticated owner.
The reference is `thread.sarah.<first 24 hex of SHA-256 of the owner id>`.
The raw owner id never enters the reference. The route creates the Khala Sync
chat thread through the sanctioned `chat.createThread` mutator with the title
`Sarah`. It then writes the `maintain_owner_contact` authority receipt.

`hasSarahThreadAuthority` is the live gate for every later Sarah action. It
requires the thread to match the owner, an active admin identity, and a
succeeded bootstrap receipt in `sarah_authority_decision_receipts`.

### 3.2 The turn engine already exists

A client sends a message as an ordinary Khala Sync chat message. The client
also admits a `runtime.startTurn` mutation on lane `hosted_khala` through
`POST /api/sync/push`. Push acceptance immediately starts the server dispatch
through `runHostedRuntimeTurnDispatchForEnv`. The one-minute cron drive is the
backstop, not the primary trigger.

`apps/openagents.com/workers/api/src/khala-hosted-runtime-dispatch.ts` is the
server-owned consumer. It claims the queued turn by durably recording
`turn.started`. The claim is atomic because `runtime.recordEvent` takes a row
lock and rejects a duplicate `(turn_id, sequence)` pair.

For an owner conversation the consumer calls `runSarahAgentTurn` in
`apps/openagents.com/workers/api/src/sarah-agent-runtime.ts`. That loop is
bounded to six tool rounds. Each tool result is bounded to 8000 characters.
The last round suppresses tools and forces a conversational answer.

The tools come from `makeSarahRuntimeTools`. The current set is
`codex_workers_capacity`, `codex_workers_start`, `codex_workers_status`,
`full_auto_status`, `full_auto_control`, `sarah_web_comms`,
`sarah_harness_status`, and `sarah_harness_review_history`. Managed-sandbox
tools join the set only when the broker is admitted and healthy.

### 3.3 The event ladder is already typed

The consumer writes these Khala Sync runtime events in order.

| Event kind | Meaning |
| --- | --- |
| `turn.started` | the durable claim, one per turn |
| `tool.call` | Sarah selected a tool, with its authority block |
| `tool.result` | the tool returned a bounded result |
| `tool.error` | the tool refused or failed, with a safe message |
| `text.delta` | the answer text |
| `text.completed` | the answer is final |
| `usage.recorded` | exact provider usage, owner attributed |
| `turn.finished` | `stop` or `error` |
| `turn.interrupted` | a stale claim that the sweep settled |

Each tool event carries an authority block. The block names the allowed flag,
the authority receipt reference, the decision reference, and the tool
reference. `authorizeSarahOperation` writes the matching receipt row before
the target broker runs. An allowed decision is never proof that the target
acted.

The provider call is not a token stream. `runSarahAgentTurn` sets
`stream: false`. The answer arrives as one `text.delta`. The honest liveness
signal for a client is therefore the ordered tool ladder, not token motion.

### 3.4 The transports already exist

| Path | Role |
| --- | --- |
| `/api/mobile/sarah` | principal bootstrap and thread resolution |
| `/api/sync/bootstrap` | consistent snapshot pages for one scope |
| `/api/sync/log` | offset-resumable catch-up pages |
| `/api/sync/push` | transactional mutations, in-band per-mutation results |
| `/api/sync/connect` | authenticated live tail over WebSocket |
| `/api/sync/cvr-pull` | flagged recovery diff after a must-refetch |

### 3.5 A working client already exists

`apps/openagents-desktop/src/desktop-sync-host.ts` builds the complete client
stack from `@openagentsinc/khala-sync-client`. It composes the session, the
conversation, the agent timeline, the runtime interactions, and the runtime
commands over an HTTP transport.

`apps/openagents-desktop/src/desktop-session-pkce.ts` holds the account sign-in.
It runs an OpenAuth PKCE flow with client id `openagents-desktop`, authorize
URL `https://auth.openagents.com/authorize`, token URL
`https://auth.openagents.com/token`, and a loopback receiver on
`127.0.0.1/auth/callback`. `desktop-session-vault.ts` owns credential custody.

The MVP reuses this stack. It does not author a second sync client.

### 3.6 What Omega has today

Omega owns `omega_identity` for isolated Nostr custody and `omega_effectd` for
supervised service life. The service protocol is `openagents.omega.effectd.v1`,
a newline-framed JSON protocol on standard input and output. Frames are capped
at 64 KiB. Generation fencing refuses a stale supervisor generation. A host
bridge lets the service call back into native Omega authorities.

`agent_computer_ui` is the current pattern for a product pane. It is a dock
panel plus an Agent menu entry plus two actions. It sends framed requests to
`omega-effectd`. It stores no durable ledger of its own.

## 4. Named gaps

These gaps are the real work. Each one has an owning packet.

| Gap | Statement | Packet |
| --- | --- | --- |
| G1 | Omega has no OpenAgents human account session | `OMEGA-SW-01` |
| G2 | Omega has no Khala Sync client | `OMEGA-SW-02` |
| G3 | The framed protocol has no conversation methods | `OMEGA-SW-02` |
| G4 | Omega has no workroom pane | `OMEGA-SW-03` |
| G5 | Omega has no receipt inspector | `OMEGA-SW-05` |
| G6 | Omega has no proof for this journey | `OMEGA-SW-07` |

G1 is the blocking gap and it is easy to get wrong. The Agent Computer panel
authenticates with the runtime `OPENAGENTS_AGENT_TOKEN` bearer. That bearer
cannot reach Sarah. The Sarah principal route requires an actor of kind
`human` whose identity matches an OpenAgents admin address. An agent bearer
receives `401`. The MVP therefore needs a real account session in Omega.

The Omega Nostr identity does not satisfy G1 either. It is a different
identity with a different custody model. Keep both identities. Do not fuse
them. Do not derive one from the other.

## 5. Design laws

Laws 1 and 6 hold for Part 1 only. Part 2 supersedes them. Section 17.2
records the exact supersession and the admissions it needs. The other laws
hold for both parts.

1. Khala Sync and Cloud SQL own the Sarah conversation record.
2. Omega is a client of that record and never a second authority.
3. `omega-effectd` is the only Omega path to the OpenAgents API for this
   product. GPUI never opens its own socket to the API.
4. GPUI is projection and command entry only. GPUI stores no durable thread,
   message, receipt, or authority ledger.
5. The Sarah authority profile is unchanged. Omega adds no tool, no
   capability, and no grant.
6. Sarah's owner-private conversation does not go on Nostr in this MVP.
7. A pending intent is never displayed as an applied transition.
8. Silence is never displayed as completion.
9. The pane shows only public-safe references. No raw token, credential,
   private path, or unbounded output reaches the user interface or a log.
10. The MVP is owner-scoped. The admin gate stays. The release text says so.

Law 6 was the revision 1 position. It said that Sarah's owner-private
conversation does not go on Nostr in this MVP. The owner direction of
2026-07-24 reverses it. Part 2 owns the replacement law and its conditions.

Part 1 stays useful under the reversal. The pane, the account session, the
service seam, and the receipt inspector do not change when the record source
changes. Section 17.5 states the exact sequence.

## 6. Architecture

```text
Omega GPUI (Rust)                      OpenAgents API (Cloud Run)
+---------------------------+          +----------------------------------+
| workroom_ui               |          | /api/mobile/sarah                |
|  - roster                 |          | /api/sync/{push,log,bootstrap}   |
|  - transcript             |          | /api/sync/connect                |
|  - activity ladder        |          |                                  |
|  - receipt inspector      |          | hosted runtime dispatch          |
+------------+--------------+          |  -> runSarahAgentTurn            |
             | framed stdio            |  -> Sarah tools + authority       |
             | openagents.omega.       |  -> runtime events                |
             | effectd.v1              +---------------+------------------+
+------------v--------------+                          |
| omega-effectd (Node)      |  HTTPS, human session     |
|  - session custody read   +---------------------------+
|  - khala-sync-client      |
|  - projection + commands  |          Cloud SQL: threads, messages,
+---------------------------+          runtime events, authority receipts
```

The service holds the sync session. The pane holds no network state. A
restart of the service rebuilds projections from the durable record. A
restart of Omega rebuilds the pane from the service.

## 7. Workroom record model

The pane renders five projections. Each projection has one source of truth.

| Projection | Source | Content |
| --- | --- | --- |
| Room | `/api/mobile/sarah` | principal reference, display name, role, thread reference, authority profile and revision |
| Transcript | Khala Sync chat messages | owner messages and Sarah answers in order, with confirmed and pending states |
| Activity | Khala Sync runtime events | ordered tool ladder for the active and recent turns |
| Receipts | tool event authority blocks | allowed flag, authority receipt reference, decision reference, tool reference |
| Run state | `turn.*` events | queued, running, interrupted, finished with reason |

Every row carries a source label, a freshness label, and a gap label. An
unavailable source stays visible and stays honest. A missing source never
becomes an empty success.

The composer is one typed text field. The MVP admits text only. Images,
attachments, voice, and reactions are out of scope.

## 8. Protocol additions

`OMEGA-SW-02` adds these methods to `openagents.omega.effectd.v1`. Names are
proposed and freeze in `OMEGA-SW-00`.

| Method | Direction | Result |
| --- | --- | --- |
| `sarah_session_status` | request | signed-in state, account label, expiry state, no token |
| `sarah_bootstrap` | request | principal projection and thread reference |
| `sarah_room_snapshot` | request | bounded transcript page, activity page, run state, cursors |
| `sarah_send_message` | request | accepted message reference and turn reference |
| `sarah_interrupt_turn` | request | typed intent result, pending until the server settles it |
| `sarah_room_event` | event | one appended record with its cursor |
| `sarah_room_state` | event | connection, freshness, and gap state |

Rules for the additions:

- Every request carries the supervisor generation. A stale generation fails
  closed with `stale_generation`.
- Every page carries a cursor and an explicit gap state.
- A frame stays under the 64 KiB cap. The service pages instead of truncates.
- The service never returns a raw token, a raw provider payload, or an
  unbounded tool result.
- A rejected sync mutation returns as a typed value, not as a transport
  error.

## 9. Packets

### 9.1 `OMEGA-SW-00`: freeze the workroom contract

Inputs: the Sarah ProductSpec, the Sarah authority profile, the runtime event
schema, and the transports in §3.4.

Work: pin the exact upstream digests. Freeze the five projections in §7.
Freeze the protocol names and payload schemas in §8. Freeze the redaction map
for every field the pane can display. Name one writable authority for each
field. Record the paging, gap, duplicate, late event, and revocation rules.

Exit: a reviewer can implement the pane and the service against one contract.
No packet invents a second vocabulary.

### 9.2 `OMEGA-SW-01`: OpenAgents account session in Omega

Work: port the OpenAuth PKCE loopback flow from the Electron application.
Keep the Omega client identity separate from the Electron client identity.
Store credentials in Omega isolated custody under the Omega RC data root.
Never write Zed data. Never read Electron secure storage. Never touch
`~/.codex`.

Add a visible sign-in state to the Omega interface. Show signed out, signed
in, expired, and refused states. A refused admin gate must say that the Sarah
workroom is owner-scoped today. It must not look like a network fault.

Exit: Omega holds a human session. `POST /api/mobile/sarah` returns a
principal projection. No token appears in a log, a crash record, or the
interface.

Falsifier: the workroom works with an agent bearer. That would mean the owner
gate regressed on the server.

### 9.3 `OMEGA-SW-02`: the sync client inside `omega-effectd`

Work: compose `@openagentsinc/khala-sync-client` inside the service. Reuse the
Desktop composition as the reference. Bootstrap the scope, tail
`/api/sync/connect`, and fall back to `/api/sync/log` after a disconnection.
Handle the `must_refetch` path through bootstrap or the flagged diff pull.

Add the framed methods in §8. Keep one client group for one account. Do not
reuse a client group across accounts.

Exit: the service answers snapshots and streams appended records. A restart
reproduces the same state from the durable record. A duplicate delivery does
not duplicate a row.

### 9.4 `OMEGA-SW-03`: the workroom pane

Work: add crate `workroom_ui` with a dock panel, an Agent menu entry, and
actions for open, focus composer, and interrupt. Render the five projections.
Virtualize the transcript and the activity list.

The pane subscribes to service events. It holds no durable state. It restores
focus, scroll position, and pane layout after a restart.

Exit: an owner can read the thread and see live activity. Keyboard use,
visible focus, screen-reader labels, and a minimum window size pass the
roadmap §11 gates.

### 9.5 `OMEGA-SW-04`: send, stream, and interrupt

Work: send a message as a chat message plus a `runtime.startTurn` mutation.
Show the local pending state until the server confirms the row. Show
`turn.started` as running. Render each `tool.call`, `tool.result`, and
`tool.error` as it arrives. Render `text.delta` and `text.completed` as the
answer. Render `turn.finished` with its exact reason.

Interrupt sends a typed intent. The pane shows the intent as pending. The pane
shows an applied state only after the server records the terminal event.

Exit: the sender sees the same accepted record that the mobile client sees.
A restart during a turn shows one honest terminal outcome, never two answers.

### 9.6 `OMEGA-SW-05`: the receipt inspector

Work: add a details view for one activity row. Show the tool reference, the
allowed flag, the authority receipt reference, the decision reference, and
the bounded result reference. Show refusals with their reason class.

Show the Sarah authority profile reference and revision in the room header
area, not in the conversation header. The conversation header says `Sarah`.

Exit: an owner can answer the question "what authorized this" without leaving
Omega and without reading a database.

### 9.7 `OMEGA-SW-06`: proactive updates and attention

Work: render the proactive updates that the autonomous tick appends. Those
updates arrive as ordinary hosted-runtime turns on the same thread. The pane
needs no new source.

Add one unread count and one attention marker for the room. Mark read state
locally in the MVP. Do not invent a cross-device read-state protocol here.

The tick flag `SARAH_AUTONOMOUS_TICK_ENABLED` stays default off. Omega must
not enable it. Omega must render an empty room honestly when it is off.

Exit: an update that Sarah initiates appears in Omega with the same evidence
as an answer she gives to a question.

### 9.8 `OMEGA-SW-07`: prove the journey

Work: run the proof on an installed and signed Omega candidate. Record a
public-safe receipt. Ask an independent reviewer with a distinct execution
identity to check it.

The proof journey:

1. Install the candidate from a clean profile.
2. Sign in to the OpenAgents account through the loopback flow.
3. Open the workroom pane.
4. Confirm the principal reference, the thread reference, and the authority
   revision.
5. Ask Sarah for current release state and read her cited answer.
6. Ask Sarah for coding capacity and observe the tool ladder.
7. Ask Sarah to control an existing Full Auto run and confirm the pending
   state stays pending until the host applies it.
8. Trigger one refusal and confirm the refusal receipt.
9. Interrupt one turn and confirm the terminal event.
10. Restart Omega during a turn and confirm one honest outcome.
11. Kill `omega-effectd` and confirm recovery without a duplicate answer.
12. Disconnect the network and confirm a visible degraded state.
13. Confirm the same thread and history on OpenAgents mobile.
14. Confirm that no token, credential, or private path is in any log.
15. Remove Omega and confirm that Zed and Electron data did not change.

Exit: the receipt exists, the reviewer accepts it, and no release-blocking
defect stays open.

## 10. Non-goals

The MVP does not add:

- a second conversation store, receipt store, or authority model
- a Nostr projection of the owner-private Sarah thread
- multiple rooms, channels, members, roles, or moderation
- direct messages, group threads, presence, or typing state
- attachments, canvases, previews, voice, or huddles
- search across history
- a new Sarah tool, capability, grant, or profile revision
- remote Full Auto start from Omega
- an avatar, a video pipeline, or a speech surface
- a public claim of Buzz parity or Desktop parity

## 11. Failure behavior

The Buzz issue set names the failures that a workroom fakes most easily. Each
one becomes an acceptance test here.

| Risk | Required Omega behavior |
| --- | --- |
| A probe waits forever | every probe has a deadline and a visible degraded state |
| A reply disappears | one typed and acknowledged result channel, no best-effort write |
| An agent looks healthy but does nothing | run state comes from typed events, never from prose |
| A duplicate installation duplicates an actor | one client group for one account, idempotent bootstrap |
| Replay is structurally present but false | a restart test proves one terminal outcome per turn |
| An approval lacks checkable evidence | every action row links its authority receipt |
| A pending intent looks applied | pending renders as pending until the terminal event |

Additional Sarah-specific rules:

- A stale context source stays labeled stale. It does not silently disappear.
- A refused action shows its reserved category. It does not show as an error.
- An empty answer is a failure class, not an answer.

## 12. Falsifiers

The MVP is wrong if any of these becomes true.

1. GPUI or a Rust crate holds durable Sarah conversation or receipt state.
2. Omega writes a Sarah record through any path other than the sanctioned
   Khala Sync mutators.
3. Omega gains a Sarah capability that the admitted profile does not grant.
4. The pane shows a completion that no terminal event supports.
5. A raw token, credential, private path, or unbounded tool result reaches
   the interface, a log, or a receipt.
6. Omega opens a direct socket to the OpenAgents API for this product path.
7. The owner-private thread appears on a relay.

## 13. Relation to the parity program

This lane is a slice of roadmap §8, not a replacement for it.

`OMEGA-BZ-00` freezes the general workroom contract. `OMEGA-SW-00` freezes the
Sarah slice of it. The pane grammar from `OMEGA-BZ-01` is the same grammar
this lane builds. A later Nostr-backed room reuses the pane and swaps the
record source behind the projections in §7.

The value of shipping this slice first is evidence. It proves the pane, the
service seam, the account session, the event ladder, and the receipt
inspector against a record that already exists in production. The
Nostr-primary work then changes the source, not the product.

## 14. Verification commands

| Scope | Command |
| --- | --- |
| Service | `pnpm --dir packages/omega-effectd test` |
| API runtime | `pnpm --dir apps/openagents.com/workers/api test -- src/khala-hosted-runtime-dispatch.test.ts` |
| API Sarah routes | `pnpm --dir apps/openagents.com/workers/api test -- src/sarah-owner-routes.test.ts` |
| Omega crates | `cargo test -p workroom_ui --lib` and `cargo test -p omega_effectd` |
| Repository gate | `pnpm run check` |

A packaged journey receipt is required in addition to these commands. A test
pass is not a product claim.

## 15. Owner blockers

These items need an owner decision before the lane closes.

1. Approve the Omega OpenAuth client identity for the account sign-in. The
   Electron client id must not be reused by a second application.
2. Confirm that the owner-scoped admin gate stays for the MVP release text.
3. Confirm the disposition of the autonomous tick flag during the dogfood
   window.

Record each item in `NEEDS_OWNER.md` as a named screen or action when the
lane starts. Continue every unaffected packet while an item waits.

## 16. Research basis

This specification used the pinned source at `93bbdd70b1` and `87703b753a`.
The main sources were the Sarah ProductSpec revision 5, the Sarah authority
profile revision 6, and the Omega roadmap revision 4. The other sources were
the Buzz parity recommendation and the Buzz teardown. The last sources were
the Omega Full Auto and Agent Computer freezes, and the Electron sync host
and session sources.

The runtime claims come from `sarah-owner-routes.ts`,
`sarah-agent-runtime.ts`, `sarah-runtime-tools.ts`,
`khala-hosted-runtime-dispatch.ts`, `sarah-autonomous-tick.ts`, the sync route
table in `apps/openagents.com/workers/api/src/index.ts`, and the Omega crates
`omega_effectd`, `omega_identity`, and `agent_computer_ui`.

No live relay, live provider turn, or packaged Omega build was exercised for
this document.

# Part 2: the Sarah runtime on an owned Nostr relay

## 17. Direction and scope

### 17.1 The owner direction

The owner directed on 2026-07-24 that Sarah's runtime moves entirely to Nostr,
on a relay that OpenAgents controls.

"Entirely" is exact here. It covers the conversation, the turn lifecycle, and
the activity ladder. It also covers the authority receipts, the memory, the
read state, and the proactive updates. All of them become signed Nostr events.
It does not mean that exact metering, admission authority, or secret custody
move. Section 21 names the parts that must stay where they are, and why.

This direction extends the current owner direction in the Buzz parity
recommendation. That document already makes Nostr the primary workroom
protocol and the signed causal work graph. Sarah was the one lane that
revision 1 held back. The owner has now closed that exception.

### 17.2 What this supersedes

| Superseded text | Source | Replacement |
| --- | --- | --- |
| Khala Sync owns shared conversation and timeline truth | Omega roadmap §5 law 3 | Nostr owns the Sarah conversation record |
| Nostr is an interoperability boundary, not command authority | Omega roadmap §5 law 4 | Nostr carries the record. Admission still gates every effect |
| Sarah's conversation does not go on Nostr in this MVP | this document, revision 1 law 6 | Part 2 |
| Private NIP-17 and NIP-44 data comes later | Omega roadmap §8.9 | encrypted owner conversation is in the first Nostr slice |
| The separate Google Cloud relay deployment is not planned | openagents `#9201` closing note, 2026-07-23 | an owned relay is required again |

Law 4 needs care. The replacement does not make a signature into permission.
A valid signed event is proof that a key signed exact bytes. It is not proof of
authorization and not proof of truth. Relay acceptance does not settle an
OpenAgents action. That rule survives the supersession without change.

### 17.3 What must be re-admitted before code lands

This document cannot make these changes by itself.

| Artifact | Required change |
| --- | --- |
| `specs/openagents/sarah-owner-orchestrator.product-spec.md` | revision 6. `SARAH-AC-01` through `SARAH-AC-03` name the Khala Sync thread and the hosted runtime as the record |
| `docs/authority/SARAH_AUTHORITY.md` | revision 7. Add relay custody, remote signing, and publication conditions |
| `docs/omega/ROADMAP.md` | revision 5. Replace design laws 3 and 4 |
| `INVARIANTS.md` and `apps/openagents.com/INVARIANTS.md` | record the conversation-authority move |
| openagents `#9201` | reopen or replace. The owned relay is required again |
| `packages/behavior-contracts` | the owner statement in §17.1 lands as a contract with an oracle |

Start `SARAH-NR-00` only after the ProductSpec and authority revisions exist.
A packet that writes a Sarah record to a relay before that admission is a
self-amplification, not an implementation.

### 17.4 Why this is credible now

`nostr-effect` is not a thin client. It implements the standard protocol, the
relay server, and every Buzz custom NIP that this runtime needs. Section 18
inventories the exact modules. The missing pieces are a durable production
store, a Node host, a deployment, and the OpenAgents policy layer.

The monorepo already consumes the library. Three packages pin
`nostr-effect` at commit `009fb155`: `packages/nip90`,
`packages/sovereign-identity`, and the `openagents.com` API. The dependency
path is proven. The pin is 27 commits behind `main` and needs a deliberate
bump.

### 17.5 Sequence

Two lanes can run beside each other. They must not merge.

The Part 1 lane builds the client: the account session, the service seam, the
pane, and the receipt inspector. None of that work is wasted, because the
projections in §7 stay the same when the record source changes.

The Part 2 lane builds the record: the contract, the store, the host, the
deployment, the identity, and the turn service. It is the longer lane and the
relay proof gates it.

The recommended order is direct. Start `SARAH-NR-00` through `SARAH-NR-03`
immediately, because the relay build has the longest lead time. Run
`OMEGA-SW-01` through `OMEGA-SW-05` beside them, against the current record.
Then run `SARAH-NR-06` to swap the source behind the pane.

One rule protects the lanes from each other. Do not build a second pane, a
second composer, or a second receipt inspector for the Nostr record. The
Part 1 surfaces are the surfaces.

## 18. What `nostr-effect` already gives us

Every row below was read at pin `c160378`.

### 18.1 Modules this runtime uses directly

| Need | Module | Kind or role |
| --- | --- | --- |
| Owner-private conversation | `src/client/Nip17Service.ts` | kind 14 sealed and gift-wrapped |
| Encryption | `src/services/Nip44Service.ts` | versioned encryption with vectors |
| Gift wrap | `src/wrappers/nip59.ts` | metadata protection |
| Live turn telemetry and control | `src/core/NipAO.ts` | ephemeral kind 24200, both directions |
| Durable turn usage | `src/core/NipAM.ts` | append-only kind 44200, encrypted to owner |
| Agent identity attestation | `src/services/OwnerAttestationService.ts` | NIP-OA `auth` tag, condition clauses |
| Agent authentication | `src/client/AgentAuthService.ts` | kind 22242 with the NIP-OA tag |
| Agent persona and state | `src/client/NipAPService.ts` | addressable kinds 30175 and 30177 |
| Owner-decryptable memory | `src/client/EngramService.ts` | addressable kind 30174, blinded identifier |
| Read state | `src/client/ReadStateService.ts` | kind 30078, max-register merge |
| Reminders | `src/client/EventReminderService.ts` | addressable kind 30300 with expiration |
| Remote signing | `src/client/Nip46Service.ts` | Nostr Connect |
| Relay policy | `src/client/RelayListService.ts` | NIP-65 relay lists |
| Gap recovery | `src/client/Nip77Service.ts` | negentropy set reconciliation |
| Relay authentication | `src/relay/core/nip/modules/Nip42Module.ts` | challenge and verify |
| Relay management | `src/relay/core/nip/modules/Nip86Module.ts` | operator API |
| Relay information | `src/relay/core/nip/modules/Nip11Module.ts` | capability advertisement |
| Job market | `src/client/DVMService.ts` and `src/core/Nip90.ts` | NIP-90 request and result |

The relay core is also present. It has a connection manager, a message
handler, a filter matcher, and a subscription manager. It also has a policy
pipeline, an authentication service, a NIP module registry, and a NIP-86 admin
service.

### 18.2 The two structural gaps

The first gap is storage. `src/relay/storage/EventStore.ts` is a clean
seven-method interface. It has exactly two implementations. One is
`BunSqliteStore`, which imports `bun:sqlite`. The other is `DoSqliteStore`,
which targets Cloudflare Durable Objects. Cloudflare is retired for
OpenAgents and must not return as a runtime.

There is no Postgres store and no Cloud SQL store. A durable
production relay therefore needs a new `EventStore` implementation.

The second gap is the host. `BunServer.ts` uses `Bun.serve`. The package
exports the Cloudflare backend but does not export the Bun backend. The
OpenAgents monorepo targets Node 24 and forbids Bun in its supported paths.
A Node WebSocket host for the relay core does not exist yet.

Neither gap touches the protocol. The library dependencies are
`@noble`, `@scure`, `effect`, and `pako`. Those run on Node without change.

### 18.3 Version skew

`nostr-effect` pins `effect` at `4.0.0-beta.94`. The monorepo catalog pins the
same beta line. The two must be moved together. A packet that bumps one and
not the other creates a type boundary that no test covers.

## 19. What we do not control today

The current canonical market relay is `wss://nos.lol`. It is the default
value of `DefaultForumWorkRequestRelayUrl` in
`apps/openagents.com/workers/api/src/forum-work-requests.ts`. It is a
third-party public relay.

`relay-health.ts` probes that relay with a NIP-11 leg and a WebSocket leg. It
retains outcomes and emits transition events. It is honest monitoring of a
relay that OpenAgents does not operate.

`wss://relay.openagents.com` appears only in a test fixture in
`packages/nip90`. No owned relay serves production traffic today.

The conclusion is direct. "A relay we control" is a build, not a
configuration change. `SARAH-NR-01` and `SARAH-NR-02` own that build.

## 20. Target architecture

```text
Omega GPUI (Rust)          omega-effectd (Node)        relay.openagents.com
+------------------+       +----------------------+    +-------------------+
| workroom pane    |<----->| nostr-effect client  |<-->| nostr-effect      |
| (unchanged)      | framed| NIP-17 / 44 / 59     | wss| relay core        |
+------------------+       | NIP-42 auth          |    | NIP-42 / 11 / 40  |
                           | NIP-77 gap recovery  |    | NIP-70 / 77 / 86  |
                           | NIP-65 relay policy  |    +---------+---------+
                           +----------+-----------+              |
                                      |                 Cloud SQL EventStore
OpenAgents mobile ---------------------+                 (append, query,
(same events, same keys)              |                  replaceable, index)
                                      |
                          +-----------v-----------+
                          | Sarah turn service    |
                          | (Cloud Run)           |
                          | subscribe -> infer -> |
                          | publish signed ladder |
                          +-----------+-----------+
                                      |
                       Cloud SQL: exact usage rows, admission
                       state, and target broker receipts
```

The turn service replaces the hosted-runtime cron consumer. It authenticates
to the relay, subscribes to the owner conversation, runs the same bounded
Sarah agent loop, and publishes the ladder as signed events. It stays a Cloud
Run service on Google Cloud.

The relay is the record. The turn service is a member of the conversation, not
the owner of it. Any authorized client that holds the owner key can read the
same history from the relay without the turn service.

## 21. The boundary: what must not move

Nostr becomes the record. It does not become everything.

| Stays where it is | Reason |
| --- | --- |
| Exact `token_usage_events` rows in Cloud SQL | metering is billing authority. A relay is not a ledger |
| The public served-token counter projection | it reconciles to exact rows, never to relay observations |
| Admission and authority resolution | `resolveAuthorityDecision` decides. A signed event never decides |
| Target broker execution and its receipts | Full Auto, coding capacity, releases, and sandboxes keep their own systems |
| Raw secrets, credentials, and mnemonics | no secret enters an event, a tag, or a log |
| Git objects and refs | Git keeps object safety. NIP-34 carries coordination |
| Provider credentials and model access | runtime only, never in an event |

The metering rule needs one more sentence. A turn may publish a signed
reference to its usage record, and it may publish an encrypted NIP-AM turn
metric to the owner. Neither becomes the counter source. The counter still
reconciles to the exact rows.

## 22. The record on Nostr

### 22.1 Identity and custody

Sarah gets one Nostr identity for `principal.sarah`. The owner gets the
identity that Omega already creates in `omega_identity`.

The service key must not sit in an environment variable in plain text. The
first admitted custody path is Google Secret Manager plus a signing boundary
that returns signatures, not keys. NIP-46 remote signing is the protocol form
of that boundary and `nostr-effect` implements it.

NIP-OA owner attestation binds Sarah's key to the owner with condition
clauses. NIP-AA carries that attestation into relay authentication. A relay
that admits Sarah admits an attested agent key, not an anonymous pubkey.

Key rotation, revocation, and archival need an explicit path from the first
contract. NIP-IA identity archival exists in the library for that purpose.

### 22.2 The conversation

The owner conversation uses NIP-17 private direct messages. A message is a
kind 14 rumor, sealed with NIP-44, and wrapped with NIP-59. The relay stores
ciphertext and cannot read the conversation.

This is a privacy improvement over the current record. Today the thread rows
sit in Cloud SQL where an operator with database access can read them. After
the move, only the owner key and the Sarah key can decrypt.

The stable thread identity moves from `thread.sarah.<digest>` to the pair of
public keys plus a conversation identifier tag. The migration must preserve a
mapping so old references stay resolvable.

### 22.3 The turn ladder

The ladder splits into a live layer and a durable layer. This split is
required, not a preference.

NIP-AO is ephemeral. Kind 24200 sits in the 20000 to 29999 range, so a
compliant relay broadcasts it and does not store it. It is the correct
carrier for live tool activity. It already has a sequence number for drop
detection, session and turn correlation, and a `cancel_turn` control message.

A durable replay needs stored events. The current ladder must therefore have a
durable form.

| Current event | Nostr carrier | Storage |
| --- | --- | --- |
| `turn.started` | durable OpenAgents turn record, encrypted to owner | stored |
| `tool.call` and `tool.result` and `tool.error` | NIP-AO telemetry frame plus a durable turn-record entry | live plus stored |
| `text.delta` and `text.completed` | NIP-17 kind 14 answer from Sarah | stored |
| `usage.recorded` | NIP-AM kind 44200 metric encrypted to owner | stored |
| `turn.finished` and `turn.interrupted` | durable turn record terminal entry | stored |
| interrupt intent | NIP-AO control frame with `cancel_turn` | live |

The durable turn record is a new kind. `SARAH-NR-00` must specify it before
any code writes it. The parity recommendation already sets the rule: no opaque
custom kind without a written specification, canonical fixtures, negative test
vectors, and a NIP-31 `alt` fallback.

Each durable entry links its causal parents. An owner must be able to ask why
an effect happened and read the signed inputs that authorized it.

### 22.4 Authority receipts

An authority decision becomes a durable signed event, encrypted to the owner.
It carries the exact fields of `openagents.authority_decision_receipt.v1`.
Those fields are the receipt reference, the profile reference and revision,
and the program and grant references. They also are the actor role, the
action, the target reference, and the trigger reference. The last fields are
the condition results, the timestamps, the outcome, and the evidence
references.

Two rules keep this honest. The receipt is a record of a decision that the
authority service already made. Publishing a receipt never makes a decision,
and a missing receipt never permits an action.

### 22.5 Memory, read state, and reminders

NIP-AE engrams give Sarah owner-decryptable memory. The conversation key is
symmetric, so the owner can always decrypt what Sarah remembers. That is a
stronger property than the current graph memory store, and it is the sharpest
idea in the Buzz design.

NIP-RS read state uses kind 30078 with a max-register merge. It replaces the
local-only read marker in `OMEGA-SW-06` and makes read state work across
Omega, mobile, and any later client.

NIP-ER reminders use addressable kind 30300 with NIP-40 expiration. The
proactive tick can schedule with them instead of relying only on a cron.

### 22.6 Multi-relay and offline behavior

One relay hostname must never become part of Sarah's identity or the work
identity. NIP-65 relay lists carry the admitted relay set.

The first contract admits more than one relay. The client keeps a local event
store and a signed outbound queue. It may author offline and publish after
reconnection. It reconciles gaps by event identifier through NIP-77
negentropy, not by arrival time.

## 23. The owned relay

### 23.1 Deployment shape

The relay runs on Google Cloud. Cloud Run is the first target because the
monorepo already deploys the API that way. A WebSocket workload with long
connections may need a GCE or a Cloud Run configuration with the correct
timeout and concurrency settings. The packet must measure this, not assume it.

Cloud SQL Postgres is the store. Cloudflare Workers, Durable Objects, D1, and
R2 remain retired and must not appear in any lane of this build.

The public hostname is `relay.openagents.com`. DNS stays with the current
provider in DNS-only mode and points at Google Cloud.

### 23.2 Required relay behavior

| Requirement | Carrier |
| --- | --- |
| Capability advertisement | NIP-11 information document |
| Client authentication | NIP-42 challenge and verify |
| Closed membership | policy pipeline plus attested agent keys |
| Protected events | NIP-70 |
| Expiration | NIP-40 |
| Gap reconciliation | NIP-77 negentropy |
| Operator control | NIP-86 management API |
| Ephemeral routing | kinds 20000 to 29999 broadcast without storage |

Custom kinds advertise through `supported_extensions`, never through
`supported_nips`. That rule is already the convention in the library.

### 23.3 Durability and load proof

The relay is coordination critical path once Sarah runs on it. It needs the
proof that openagents `#9201` specified before that issue was closed.

- A durable append survives a process restart.
- Query and filter contracts match the clients that use them.
- A load report gives the rate, the median latency, the ninety-ninth
  percentile latency, the error classes, and the failure mode under overload.
- Backup, restore, key rotation, and multi-replica behavior have written
  operator notes.

An unproven relay is not admitted as the Sarah record.

## 24. Packets

Each packet names its owning repository.

| Packet | Repository | Outcome |
| --- | --- | --- |
| `SARAH-NR-00` | openagents | freeze the Nostr record contract |
| `SARAH-NR-01` | nostr-effect | Postgres `EventStore` for Cloud SQL |
| `SARAH-NR-02` | nostr-effect | Node WebSocket relay host |
| `SARAH-NR-03` | openagents | deploy the owned relay with load proof |
| `SARAH-NR-04` | openagents | Sarah identity, custody, and remote signing |
| `SARAH-NR-05` | openagents | the Sarah turn service on the relay |
| `SARAH-NR-06` | omega | the pane reads and writes Nostr |
| `SARAH-NR-07` | openagents | memory, read state, and reminders |
| `SARAH-NR-08` | openagents | migration and cutover |
| `SARAH-NR-09` | openagents | prove the Nostr journey |

### 24.1 `SARAH-NR-00`: freeze the Nostr record contract

Specify the durable turn-record kind, the authority-receipt kind, the
conversation identifier, and the causal link rules. Publish canonical fixtures
and negative test vectors for each. State authorship, encryption, deletion,
replacement, and relay derivation rules for every kind. Add a NIP-31 `alt`
value so an unknown client gets a safe non-secret summary.

Map every field in §7 to its carrier. Name one writable authority per field.
Record the boundary in §21 as a contract, not as prose.

Exit: a second implementation could interoperate from the specification alone.

### 24.2 `SARAH-NR-01`: the Postgres `EventStore`

Implement the seven-method `EventStore` interface against Cloud SQL Postgres
on Node. Support append, replaceable, and parameterized replaceable storage.
Support the filter grammar that the clients use, including tag filters.

Prove durability across a restart. Prove that a duplicate insert is idempotent.
Prove that a replaceable event replaces only an older event.

Exit: the store passes the existing relay test suites with the new backend.

### 24.3 `SARAH-NR-02`: the Node relay host

Add a Node WebSocket host for the relay core. Keep the core untouched. Export
the new backend from the package.

Carry the connection discipline that the relay core already assumes. That
includes a connection limit, an authentication challenge, a heartbeat, and a
slow-client policy.

Exit: the relay runs on Node 24 with no Bun and no Cloudflare import in the
served path.

### 24.4 `SARAH-NR-03`: deploy the owned relay

Deploy to Google Cloud. Serve `relay.openagents.com`. Run the load test in
§23.3 and publish the report. Write the operator notes.

Point `relay-health.ts` at the owned relay and keep the third-party relay as a
separate monitored target while the market lane still uses it.

Exit: the relay is live, measured, and monitored, with a public-safe receipt.

### 24.5 `SARAH-NR-04`: identity, custody, and signing

Create the Sarah identity. Store the key material behind a signing boundary
that returns signatures. Bind the key to the owner with NIP-OA. Carry the
attestation into NIP-42 relay authentication with NIP-AA.

Write the rotation, revocation, and archival path. Prove that no raw key
reaches an event, a log, a crash record, or a receipt.

Exit: Sarah can authenticate to the owned relay as an attested agent.

### 24.6 `SARAH-NR-05`: the turn service

Replace the hosted-runtime consumer for the Sarah lane. Subscribe to the owner
conversation. Run the same bounded agent loop with the same tools and the same
authority resolution. Publish the live ladder and the durable ladder.

Keep the claim discipline. Exactly one turn service instance may claim one
turn. Prove that a restart produces one honest terminal outcome and never two
answers.

Keep the exact usage row in Cloud SQL. Publish the NIP-AM metric in addition,
never instead.

Exit: an owner message on the relay produces a signed answer and a signed
ladder, with the same authority receipts as today.

### 24.7 `SARAH-NR-06`: the pane on Nostr

Swap the record source behind the five projections in §7. The pane grammar,
the composer, the activity ladder, and the receipt inspector do not change
shape.

Add relay state to the room header area. Show the connected relay set, the
freshness, the gap state, and the last acknowledged event.

Exit: the Omega pane reads and writes the relay and shows no Khala Sync
dependency for the Sarah lane.

### 24.8 `SARAH-NR-07`: memory, read state, and reminders

Move Sarah's memory to NIP-AE engrams. Move read state to NIP-RS. Move
reminders to NIP-ER. Prove that the owner can decrypt every engram.

Exit: read state agrees across Omega and mobile without a new protocol.

### 24.9 `SARAH-NR-08`: migration and cutover

Run three stages in order.

1. Shadow. The turn service publishes to the relay while Khala Sync stays the
   record. Compare the two records automatically and report drift.
2. Cutover. The relay becomes the record. Khala Sync becomes a derived
   projection for clients that still read it.
3. Retirement. Stop the Khala Sync write path for the Sarah lane after a
   rollback window closes.

The migration must be idempotent, must preserve stable identities, and must
support export and rollback. Do not delete the old rows during the window.

Exit: the relay is the record, the projection agrees, and rollback is proven.

### 24.10 `SARAH-NR-09`: prove the Nostr journey

Extend the §9.8 journey with these steps.

1. Sarah authenticates to the owned relay with an attested key.
2. The owner sends a message that only the owner and Sarah can decrypt.
3. The relay operator cannot read the conversation content.
4. The live ladder arrives as ephemeral frames with no gap in the sequence.
5. The durable ladder replays after a restart from relay history alone.
6. An authority refusal produces a signed receipt with its reserved category.
7. The exact usage row and the signed metric agree.
8. A second admitted relay serves the same history after the first is stopped.
9. An event signed while offline publishes after reconnection.
10. A stale, duplicate, unsigned, revoked, and unauthorized input is rejected
    visibly and does not start a turn.
11. An export verifies the causal chain without reading Cloud SQL.
12. An independent reviewer with a distinct execution identity checks the
    evidence.

Exit: the owner uses the Nostr-backed Sarah daily, with a public-safe receipt
and an accepted independent review.

## 25. Falsifiers for Part 2

The Nostr move is wrong if any of these becomes true.

1. A relay acceptance is treated as an OpenAgents admission.
2. An unsigned, invalid, revoked, or unauthorized event starts a turn or an
   effect.
3. A Cloud SQL row outranks the signed event it came from.
4. The public token counter is computed from relay observations.
5. A raw key, credential, or private path appears in an event, a tag, a log,
   or a receipt.
6. One relay hostname becomes part of Sarah's identity or a work identity.
7. A custom kind ships without a specification and conformance fixtures.
8. The relay operator can read the owner conversation content.
9. A Cloudflare or Bun runtime returns in the served relay path.
10. The Sarah authority profile gains a capability because the record moved.

## 26. Risks

The privacy gain is real and the operational cost is real. Encrypted history
cannot be repaired by an operator. Key loss becomes history loss. The custody,
rotation, and archival path in `SARAH-NR-04` is therefore a first-class
requirement and not a later item.

Search is the second cost. Cloud SQL gives authorized full-text search over
plaintext today. Encrypted events remove that. Client-side indexing over
decrypted content is the honest replacement. Do not put decrypted content in a
server index and call it search.

The third risk is scope. A relay that carries Sarah will attract every other
record. Section 21 is the fence. Every new record type on the relay needs its
own admission, not an inherited one.

## 27. Extended research basis

Part 2 read `nostr-effect` at pin `c160378`. It read
`docs/SUPPORTED_NIPS.md`, `src/relay/storage/EventStore.ts`, the Bun and
Cloudflare backends, the relay core, `src/core/NipAO.ts`, `src/core/NipAM.ts`,
`src/services/OwnerAttestationService.ts`, `src/client/ReadStateService.ts`,
`src/client/EngramService.ts`, and the package manifest.

It read openagents `#9185` and `#9201` including the closing note, the forge
decision audit with its GRASP amendment, `sol-claim-ledger-relay.ts`,
`relay-health.ts`, and `forum-work-requests.ts`.

No relay was deployed, no event was published, and no live turn was run for
this document.

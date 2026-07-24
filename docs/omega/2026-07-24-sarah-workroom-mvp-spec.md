# Sarah workroom MVP specification (Omega)

- Class: proposed implementation specification
- Date: 2026-07-24
- Status: proposed, not admitted
- Product: Omega, the Zed-based OpenAgents Desktop application
- Packets: `OMEGA-SW-00` through `OMEGA-SW-07`
- Client repository: `OpenAgentsInc/omega`
- Service repository: `OpenAgentsInc/openagents`
- OpenAgents pin: `93bbdd70b1` (`origin/main`, 2026-07-24)
- Omega pin: `87703b753a` (`origin/main`, 2026-07-24)
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`

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

Law 6 needs a word about the parity direction. The current owner direction
makes Nostr the primary protocol for the Buzz-parity workroom. Sarah is a
different lane. Her record is owner-private, her authority is admitted against
Cloud SQL receipts, and her ProductSpec pins that thread.

The two lanes share the pane grammar. They do not share an authority swap. A
later decision can project public-safe Sarah facts into a Nostr room. That
decision is out of scope here.

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

# Sarah workroom MVP specification (Omega)

- Class: proposed implementation specification
- Date: 2026-07-24
- Revision: 4
- Status: proposed, not admitted
- Product: Omega, the Zed-based OpenAgents Desktop application
- Packets: `OMEGA-SW-00` through `OMEGA-SW-07`, `SARAH-NR-00` through
  `SARAH-NR-09`, `SARAH-CW-00` through `SARAH-CW-09`
- Client repository: `OpenAgentsInc/omega`
- Service repository: `OpenAgentsInc/openagents`
- Protocol repository: `OpenAgentsInc/nostr-effect`
- OpenAgents pin: `93bbdd70b1` (`origin/main`, 2026-07-24)
- Omega pin: `87703b753a` (`origin/main`, 2026-07-24)
- `nostr-effect` pin: `c160378` (`main`, 2026-07-24), migrated at `787f7b5`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`

Revision 2 adds Part 2. The owner directed on 2026-07-24 that the Sarah runtime
moves entirely to Nostr on a relay that OpenAgents controls. Part 1 keeps the
pane, the service seam, and the account session. Part 2 replaces the record and
the transport under that pane.

Revision 3 adds Part 3, the v2 roadmap. It opens a semi-public community
workroom where outside developers point their own compute at bounded work and
Sarah arbitrates. Part 3 depends on Part 2 and starts only after the §29 gates
hold.

Revision 4 makes two owner-directed corrections. The v2 reward is experience
only, with no payment in v1. The build order becomes Nostr first, so Omega
never gets a Khala Sync client that the Nostr record would delete. Read the
three parts in order, and read §17.5 before planning any work.

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
| G1 | Omega has no link from its Nostr identity to an OpenAgents account | `OMEGA-SW-01` |
| G2 | Omega has no Nostr conversation client | `SARAH-NR-06` |
| G3 | The framed protocol has no conversation methods | `SARAH-NR-06` |
| G4 | Omega has no workroom pane | `OMEGA-SW-03` |
| G5 | Omega has no receipt inspector | `OMEGA-SW-05` |
| G6 | Omega has no proof for this journey | `OMEGA-SW-07` |

G2 and G3 moved to Part 2 in revision 4. Section 17.5 records why. Under the
Nostr-first order there is no reason to build a Khala Sync client in Omega
and then delete it.

G1 changed shape in revision 4 and it is still easy to get wrong. On the
current record, the Sarah principal route requires an actor of kind `human`
whose identity matches an OpenAgents admin address. The Agent Computer panel's
`OPENAGENTS_AGENT_TOKEN` bearer therefore receives `401`.

On the Nostr record, owner scope is the owner's Nostr public key, and NIP-42
authenticates it. Omega already has that key in `omega_identity`. Omega
therefore does not need an OpenAgents session to hold the conversation.

One link still matters. Exact metering rows are attributed to an OpenAgents
account, and the ledger identity is an account, not a public key. So G1
becomes a one-time binding of the owner's Omega public key to their
OpenAgents account, not a session that every request carries.

Keep the two identities distinct. Do not fuse them, and do not derive one from
the other. A binding is a recorded relation between two identities. It is not
a merge.

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

### 9.2 `OMEGA-SW-01`: bind the Omega identity to an OpenAgents account

Revision 4 narrowed this packet. It is no longer a session that every request
carries. The Nostr record authenticates the owner by public key through
NIP-42, and Omega already holds that key in `omega_identity`.

What remains is one binding. Exact metering rows are attributed to an
OpenAgents account, and the ledger identity is an account. Something must
record that this public key belongs to that account.

Work: run the OpenAuth PKCE loopback flow once, at binding time, and record
the relation. Keep the Omega client identity separate from the Electron client
identity. Store any credential in Omega isolated custody under the Omega RC
data root. Never write Zed data. Never read Electron secure storage. Never
touch `~/.codex`.

Show a visible binding state: unbound, bound, and refused. A refused
owner-scope gate must say that the Sarah workroom is owner-scoped today. It
must not look like a network fault.

Exit: the owner's Omega public key resolves to an OpenAgents account for
metering and ledger attribution. No token appears in a log, a crash record, or
the interface. The conversation itself needs no session.

Falsifier: the binding becomes a per-request session, or the two identities
are merged rather than related.

### 9.3 `OMEGA-SW-02`: CUT in revision 4

This packet built a Khala Sync client inside `omega-effectd`. The Nostr record
replaces every line of it, so building it first would be throwaway work on the
critical path.

Its real output, the framed conversation methods in §8, moves to
`SARAH-NR-06`, where the same methods sit over a `nostr-effect` client. The
method names and payload shapes in §8 do not change. Only their backing
transport does.

Do not implement this packet. If a future decision needs Omega to read the
Khala Sync record directly, that is a new packet with its own justification.

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

Revision 4 split this packet. The interaction states below are record-agnostic
and stay here. The transport moved to `SARAH-NR-06`, because a send on the
Nostr record is a signed publication and not a sync mutation.

Work: show the local pending state until the record confirms the message. Show
the turn as running once its claim lands. Render each tool call, tool result,
and tool error as it arrives. Render the answer text and its completion.
Render the terminal outcome with its exact reason.

Interrupt sends a typed intent. The pane shows the intent as pending. The pane
shows an applied state only after the terminal event lands.

Exit: the sender sees the same accepted record every other authorized reader
sees. A restart during a turn shows one honest terminal outcome, never two
answers.

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
4. Choose the relay hosting option in §23.3. Option B needs a policy reversal,
   because `apps/nostr-relay` is a retired path in `AGENTS.md` and in the
   Google Cloud authority guard.
5. Approve the Secret Manager entries for the Sarah signing material, and the
   service account that may read them.

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

### 17.5 Sequence: Nostr first

Revision 2 got this wrong and revision 4 corrects it.

Revision 2 said to run `OMEGA-SW-01` through `OMEGA-SW-05` against the current
record, then swap the source at `SARAH-NR-06`. It also claimed that none of
that work is wasted. That claim was false. `OMEGA-SW-02` builds a Khala Sync
client inside `omega-effectd`, and the transport half of `OMEGA-SW-04` writes
Khala Sync mutations. The Nostr record deletes both. That is throwaway work on
the critical path, and the owner direction is to cut over as soon as possible.

The corrected order is Nostr first. Omega never gets a Khala Sync client.

#### What is cut

| Packet | Disposition | Reason |
| --- | --- | --- |
| `OMEGA-SW-02` | cut | its only output is a Khala Sync client that the Nostr record replaces |
| `OMEGA-SW-04` transport | folded into `SARAH-NR-06` | the send path is a Nostr publication, never a sync mutation |
| `OMEGA-SW-01` | narrowed | a one-time identity binding, not a session on every request |
| `OMEGA-SW-03`, `OMEGA-SW-05`, `OMEGA-SW-06` | kept | the pane, the inspector, and attention are record-agnostic |
| `OMEGA-SW-07` | kept, retargeted | it proves the Nostr journey, so it merges into `SARAH-NR-09` |

#### Why this does not stall the client

The deployed relay is not on the critical path for building the client. The
local relay is.

`nostr-effect` already ships `startTestRelay`, and its 146 test files use it.
Once `SARAH-NR-01b` lifts the relay core out of the Bun backend and
`SARAH-NR-02` adds the Node host, a local Node relay runs on a developer
machine immediately. The pane, the composer, the ladder, and the inspector all
build against that local relay.

Cloud Run, Cloud SQL, and the load proof gate production. They do not gate
development.

#### The order

1. `SARAH-NR-00`, freeze the record contract. Nothing else can start cleanly
   without it.
2. `SARAH-NR-01b`, lift the relay core out of the Bun backend. This is the
   single highest-leverage packet, because it unblocks every Node consumer.
3. `SARAH-NR-02` plus the development half of `SARAH-NR-01d`. A local Node
   relay now exists.
4. `SARAH-NR-04` and `SARAH-NR-05`, the identity and the turn service, against
   the local relay. Sarah now answers on Nostr.
5. `OMEGA-SW-03`, `SARAH-NR-06`, and `OMEGA-SW-05`, the pane on Nostr. This is
   the first user-visible Nostr slice.
6. `SARAH-NR-01c`, the toolchain conversion, and `SARAH-NR-03`, the deploy and
   load proof. Production now exists.
7. `SARAH-NR-07`, `SARAH-NR-08`, and `SARAH-NR-09`.

Steps 1 through 3 are the long pole and they are all in `nostr-effect`. Start
them now and do not wait for a product decision to begin them.

#### What keeps working meanwhile

Do not break the current Sarah. Mobile keeps using the Khala Sync thread and
the hosted runtime exactly as it does today, with no new work. That continues
until `SARAH-NR-08` cuts the turn service over.

That is the whole compatibility story. One live surface stays on the old
record and receives no new investment. The new surface is born on Nostr.

#### The one rule

Do not build a second pane, a second composer, or a second receipt inspector.
There is one set of surfaces, and it renders the Nostr record.

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

### 18.2 The three structural gaps

The owner direction of 2026-07-24 is exact. OpenAgents has removed all
dependence on, and usage of, Cloudflare and Bun. Everything must run on Node
with the Node and Vite Plus stack that this monorepo uses, and must deploy to
Google Cloud. The gaps below are what that costs in the protocol repository.

The first gap is storage. `src/relay/storage/EventStore.ts` is a clean
seven-method interface. Before 2026-07-24 it had exactly two implementations.
`BunSqliteStore` imports `bun:sqlite`. `DoSqliteStore` targeted Cloudflare
Durable Objects, and the Cloudflare backend was deleted on 2026-07-24.

There is no Postgres store and no Cloud SQL store. A durable production relay
therefore needs a new `EventStore` implementation on Node.

The second gap is the host. `BunServer.ts` uses `Bun.serve`. There is no Node
WebSocket host for the relay core.

The third gap is deeper than a missing backend, and it is easy to miss. The
relay core is not portable today. `RelayServer`, `RelayServerLive`,
`RelayConfig`, `RelayHandle`, and `ConnectionData` all live inside
`src/relay/backends/bun/BunServer.ts`. `MemoryEventStoreLive` lives inside
`src/relay/backends/bun/BunSqliteStore.ts`. The `src/relay/index.ts` barrel
imports every one of them from the Bun backend.

The consequence is exact. The `nostr-effect/relay` entry point cannot be
imported from Node at all, because its import graph reaches `bun:sqlite`. The
Node port therefore starts with an extraction, not with a new backend.

### 18.3 The Bun toolchain

The protocol repository is a Bun project end to end. The measured state at pin `c160378` is 146 test files on `bun:test` out of
373 source files. It also has `bun` in every package script and
`"types": ["bun"]` in the typecheck configuration. Its agent contract
instructed agents to prefer Bun over Node.

That contract is inverted as of 2026-07-24. The conversion itself is one
planned stage, not a per-change cleanup, because two test runners in one
repository is the failure to avoid.

### 18.4 What already landed

`nostr-effect` commit `7f6a5dd` (2026-07-24) completed Stage 1 of the
migration. It deleted the Cloudflare backend, `wrangler.toml`, the deployment
guide, the four Cloudflare package exports, the three Cloudflare scripts, and
the `@cloudflare/workers-types` and `wrangler` development dependencies.

That removal surfaced one real coupling. `@cloudflare/workers-types` was
silently supplying the web platform globals for the whole repository,
including `Blob`, `Headers`, `MessageEvent`, and `RequestInfo`. The fix
declares those from the platform library with `lib` set to `ESNext` and `DOM`.

The commit also added the migration plan at
`docs/2026-07-24-node-google-cloud-migration.md` and inverted the Bun mandate
in that repository's agent contract. It marked the Bun and Cloudflare
assumptions in the architecture, API, buildout, and Blossom documents.

Verification for that commit was `tsc` clean plus 1430 tests passing and zero
failing across 146 files.

Neither gap touches the protocol. The library dependencies are
`@noble`, `@scure`, `effect`, and `pako`. Those run on Node without change.

### 18.5 Version skew

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

The service key must not sit in an environment variable in plain text and must
not sit in a repository file. Google Secret Manager in project
`openagentsgemini` is the admitted custody path, mounted at runtime by the
same deploy mechanism the monolith already uses.

Secret Manager holds the material. It does not remove the need for a signing
boundary. The turn service should reach a signer that returns signatures, not
keys, so a compromised process cannot exfiltrate the identity. NIP-46 remote
signing is the protocol form of that boundary and `nostr-effect` implements
it.

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

### 23.1 Runtime and stack

The relay runs on Node 24. It uses the same stack this monorepo uses: pnpm,
Vite Plus, and Effect. It does not use Bun, and it does not use Cloudflare.

That rule has no exception and no compatibility lane. Bun is not admitted as a
runtime, a package manager, a test runner, or a build tool. Cloudflare
Workers, Durable Objects, D1, and R2 are retired and must not appear in any
lane of this build.

### 23.2 Deployment shape

The relay deploys to Google Cloud in project `openagentsgemini`. Cloud Run is
the first target because the monorepo already deploys the API that way. Its
Node entry is `apps/openagents.com/workers/api/src/cloudrun/server.ts`, and
its deploy script attaches Cloud SQL and mounts Secret Manager.

A WebSocket workload with long connections may need a GCE shape, or a Cloud
Run configuration with the correct timeout, concurrency, and session-affinity
settings. The packet must measure this, not assume it.

Cloud SQL Postgres is the event store. Google Secret Manager holds every
secret, including the Sarah signing material in §22.1. No secret reaches an
environment file in the repository, an event, a tag, or a log.

The public hostname is `relay.openagents.com`. DNS stays with the current
provider in DNS-only mode and points at Google Cloud.

### 23.3 Where the relay service lives

Two options exist and the choice is an owner decision.

Option A hosts the relay service from the `nostr-effect` repository, with its
own Cloud Run service and deploy runbook. It keeps the protocol repository as
the single home of relay code.

Option B hosts the relay service in this monorepo as a new Cloud Run app that
consumes the `nostr-effect` library. It matches the deploy, secret, and check
conventions most closely.

Option B has a blocker that must not be worked around quietly. `apps/nostr-relay`
is a retired path. It is listed in the retired set inside
`scripts/google-cloud-authority-guard.mjs`, and `AGENTS.md` records that the
service was deleted and must not be recreated. Reviving a relay app path is a
policy reversal that needs an owner direction, an `AGENTS.md` change, and a
guard change in the same packet.

The recommendation is Option A for the first deployment, because it needs no
policy reversal and the migration work already lives in that repository. Move
to Option B later only if the operator burden argues for it.

### 23.4 Required relay behavior

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

### 23.5 Durability and load proof

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

| Packet | Repository | Outcome | State |
| --- | --- | --- | --- |
| `SARAH-NR-00` | openagents | freeze the Nostr record contract | planned |
| `SARAH-NR-01a` | nostr-effect | remove Cloudflare, set the Node target | done, `787f7b5` |
| `SARAH-NR-01b` | nostr-effect | extract the relay core out of the Bun backend | planned |
| `SARAH-NR-01c` | nostr-effect | replace the Bun toolchain with pnpm and Vite Plus | planned |
| `SARAH-NR-01d` | nostr-effect | Node and Cloud SQL `EventStore` implementations | planned |
| `SARAH-NR-02` | nostr-effect | Node WebSocket relay host | planned |
| `SARAH-NR-03` | openagents | deploy the owned relay with load proof | planned |
| `SARAH-NR-04` | openagents | Sarah identity, Secret Manager custody, signing | planned |
| `SARAH-NR-05` | openagents | the Sarah turn service on the relay | planned |
| `SARAH-NR-06` | omega | the pane reads and writes Nostr | planned |
| `SARAH-NR-07` | openagents | memory, read state, and reminders | planned |
| `SARAH-NR-08` | openagents | migration and cutover | planned |
| `SARAH-NR-09` | openagents | prove the Nostr journey | planned |

The `SARAH-NR-01` lane is the Node and Google Cloud migration in the protocol
repository. Its plan is
`nostr-effect` `docs/2026-07-24-node-google-cloud-migration.md`. Do not run
`SARAH-NR-01c` as a per-change cleanup. Two test runners in one repository is
the failure it avoids.

### 24.1 `SARAH-NR-00`: freeze the Nostr record contract

Specify the durable turn-record kind, the authority-receipt kind, the
conversation identifier, and the causal link rules. Publish canonical fixtures
and negative test vectors for each. State authorship, encryption, deletion,
replacement, and relay derivation rules for every kind. Add a NIP-31 `alt`
value so an unknown client gets a safe non-secret summary.

Map every field in §7 to its carrier. Name one writable authority per field.
Record the boundary in §21 as a contract, not as prose.

Exit: a second implementation could interoperate from the specification alone.

### 24.2 `SARAH-NR-01`: the Node and Google Cloud migration

This lane runs in `nostr-effect`. Its plan document owns the detail.

**`SARAH-NR-01a`, complete.** Commit `787f7b5` removed the Cloudflare backend,
`wrangler.toml`, the deployment guide, the Cloudflare package exports and
scripts, and the `@cloudflare/workers-types` and `wrangler` development
dependencies. It replaced the vendor-supplied web platform globals with the
platform library, inverted the Bun mandate in the agent contract, and recorded
the remaining stages.

**`SARAH-NR-01b`.** Move `RelayServer`, `RelayServerLive`, `RelayConfig`,
`RelayHandle`, and `ConnectionData` out of the Bun backend into the
platform-agnostic core. Move `MemoryEventStore` into `src/relay/storage`.
Rewrite the `src/relay/index.ts` barrel so it imports no backend directly.
Exit: `nostr-effect/relay` imports under Node with no `bun:` specifier in its
import graph.

**`SARAH-NR-01c`.** Adopt pnpm and Node 24. Adopt Vite Plus and move `test`,
`typecheck`, `lint`, and `fmt` onto `vp`. Convert the 146 `bun:test` files to
the Vite Plus runner in one stage. Replace `@types/bun` with `@types/node`.
Delete the Bun backend and the Bun entry. Exit: no `bun` binary, no `bun:`
import, no `Bun.` API call, and no `@types/bun` reference in the tracked tree.

**`SARAH-NR-01d`.** Implement `NodeSqliteStore` on `node:sqlite` for
development, and `PostgresStore` against Cloud SQL for production. Both
implement the seven-method `EventStore` interface with append, replaceable,
and parameterized replaceable storage, plus the tag-filter grammar the clients
use. Prove durability across a restart, an idempotent duplicate insert, and a
replaceable event that replaces only an older event. Exit: the existing relay
test suites pass against both backends.

### 24.3 `SARAH-NR-02`: the Node relay host

Add `NodeServer` on `node:http` plus `ws`. Keep the core untouched. Export the
Node backend from the package.

Carry the connection discipline that the relay core already assumes. That
includes a connection limit, an authentication challenge, a heartbeat with a
miss limit, and a slow-client policy.

Exit: the relay runs on Node 24 with no Bun and no Cloudflare import in the
served path.

### 24.4 `SARAH-NR-03`: deploy the owned relay

Deploy to Google Cloud. Serve `relay.openagents.com`. Run the load test in
§23.5 and publish the report. Write the operator notes.

Point `relay-health.ts` at the owned relay and keep the third-party relay as a
separate monitored target while the market lane still uses it.

Exit: the relay is live, measured, and monitored, with a public-safe receipt.

### 24.5 `SARAH-NR-04`: identity, custody, and signing

Create the Sarah identity. Put the key material in Google Secret Manager in
project `openagentsgemini`, and reach it through a signing boundary that
returns signatures. Bind the key to the owner with NIP-OA. Carry the
attestation into NIP-42 relay authentication with NIP-AA.

Mount the secret with the existing deploy path. Do not add a second secret
mechanism, a repository environment file, or a build-time key.

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

### 24.7 `SARAH-NR-06`: the Nostr conversation client

Revision 4 grew this packet. It is no longer a swap behind an existing client.
It is the only conversation client Omega gets, and it absorbs the cut
`OMEGA-SW-02` and the transport half of `OMEGA-SW-04`.

Work: compose a `nostr-effect` client inside `omega-effectd`. Authenticate to
the relay with NIP-42 using the Omega identity key. Subscribe to the owner
conversation, publish messages as signed events, and publish the interrupt as
a control frame.

Implement the framed methods in §8 over that client. The method names and
payload shapes do not change from revision 1. Only the backing transport is
Nostr.

Handle disconnection, catch-up, and gaps through the relay's own facilities.
Resume from the last acknowledged event. Reconcile by event identifier with
NIP-77. Fail over to a second admitted relay without a change of identity.

Develop against a local Node relay from `SARAH-NR-02`. Do not wait for the
deployed relay.

Add relay state to the room header area. Show the connected relay set, the
freshness, the gap state, and the last acknowledged event.

Exit: the Omega pane reads and writes the relay. Omega links no Khala Sync
client for the Sarah lane, and a restart reproduces state from relay history
alone.

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
9. A Cloudflare or Bun runtime, package manager, test runner, or build tool
   returns anywhere in the relay lane.
10. A secret reaches a repository file or an environment file instead of
    Secret Manager.
11. The Sarah authority profile gains a capability because the record moved.

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

The runtime and deployment claims come from the root `package.json`, the
`vite.config.ts` Vite Plus configuration, `scripts/zero-supported-bun-guard.mjs`,
`scripts/google-cloud-authority-guard.mjs`, and `docs/DEPLOYMENT.md`.

The Stage 1 migration facts come from `nostr-effect` commit `787f7b5` and its
verification run.

No relay was deployed, no event was published, and no live turn was run for
this document.

# Part 3: v2, the community workroom

## 28. Direction and outcome

### 28.1 The owner direction

Community developers can join a semi-public OpenAgents community workroom.
Their agents do real work there. Sarah arbitrates. The developers point their
own compute at tasks.

The owner narrowed the reward on 2026-07-24: **v1 awards experience points
only**. No revenue share, no bonus, and no Bitcoin payment ships in v1.
Section 36 keeps the money design on record for a later version and states
exactly what has to become true before it can ship.

### 28.2 The outcome

An outside developer joins one room, attaches an agent they already run, takes
a bounded task, returns a verifiable result, and earns experience for it.

Every part of that sentence is already an owned primitive. The room is a Nostr
group on the relay from Part 2. The agent attachment is the Part 1 pattern.
The task contract is the existing `NIP-LBR` labor microstandard. Experience
uses standard Nostr carriers. The v2 work is composition and policy, not a new
economy.

### 28.3 What v2 is not

It is not a public bounty board. It is not an open relay. It is not a token.
It is not a second identity system, a second payout rail, or a second
authority model. It does not give an outside agent any part of Sarah's
authority.

In v1 it is also not paid. Experience is recognition, not currency. A member
must understand that before they spend compute, so §35.4 makes it a copy
requirement and not a footnote.

## 29. Gates from v1

Do not start v2 before these hold.

| Gate | Reason |
| --- | --- |
| `SARAH-NR-03` relay live with load proof | v2 multiplies the write rate |
| `SARAH-NR-05` turn service in daily use | Sarah must arbitrate from a working runtime |
| `SARAH-NR-09` journey proof accepted | a private room must work before a shared one |

The payout gate that revision 3 listed here is gone, because v1 pays nothing.
It returns as the first gate of the paid version in §36.5.

## 30. The room

The community workroom is semi-public. Read access is broad inside the
membership. Write access is closed. Membership is explicit.

Use NIP-29 relay-based groups. The upstream NIP index marks NIP-28 public chat
and NIP-72 moderated communities as unrecommended and points to NIP-29
instead. `nostr-effect` implements NIP-29 with the moderation matrix, and the
Part 2 relay already carries the group state.

| Room property | Carrier |
| --- | --- |
| Group identity and metadata | NIP-29 addressable group state |
| Membership and roles | NIP-29 admin events, relay-signed |
| Messages and threads | NIP-29 group messages with the required group tag |
| Moderation and tombstones | NIP-29 moderation events |
| Room discovery | NIP-11 plus an explicit invitation, never a global directory |

Two rooms exist, and they must not merge. Sarah's owner-private conversation
from Part 2 stays owner-private and encrypted. The community room is a
separate group with separate membership. A fact moves from the private room to
the community room only as a deliberate publication with its own audience
gate.

## 31. Members and their agents

A community member is a human developer with a Nostr identity. That developer
runs one or more agents. Each agent has its own key.

Bind the agent to its operator with NIP-OA owner attestation, exactly as
Part 2 binds Sarah to the owner. NIP-AA carries the attestation into NIP-42
relay authentication. The relay admits an attested agent key, never an
anonymous pubkey. NIP-AP carries persona and declared capability.

The operator keeps everything. Their compute, their harness, their provider
accounts, their credentials, and their agent home stay theirs. OpenAgents
never receives a provider key and never mutates an agent home. This is the
same law as `OMEGA-SW-02` in Part 1, applied to a stranger.

Revocation must be immediate and cheap. Removing an agent revokes its group
membership and its capability grant. It never reaches into the operator's
machine.

## 32. How work reaches a community agent

Sarah publishes bounded work units. A member's agent claims one, executes it
on the operator's own compute, and returns a result with evidence.

Use the existing `NIP-LBR` labor contract. It is already a use-case-specific
microstandard over NIP-90, with the reserve at kinds `5930` to `5939` and
results at plus one thousand. `nostr-effect` and `packages/nip90` are its
canonical implementations.

The upstream NIP index marks generic NIP-90 as unrecommended, because it grew
without bound. That is an argument for `NIP-LBR`, not against it. Keep the
job types narrow, named, and versioned. Do not add a generic job kind.

The lifecycle is the one `NIP-LBR` already defines:

1. Sarah publishes a budgeted work request.
2. Agents publish quotes as feedback events.
3. Sarah accepts exactly one quote and escrows the budget in the platform
   ledger.
4. The provider executes with its own agent and credentials.
5. The provider publishes an output-only result with artifact and receipt
   references.
6. Sarah accepts or rejects.

The relay is transport. It grants no identity, assignment, escrow, acceptance,
payment, or settlement authority. That sentence is from `NIP-LBR` and it
survives v2 unchanged.

## 33. "Ticks", and the correction it needs

The direction says developers can point compute at "ticks of Sarah". The idea
is right and the wording hides a serious hazard. State the correction plainly.

Sarah's autonomous tick is a timer-driven trigger that runs one ordinary Sarah
turn with her full admitted tool set. It resolves her authority, it can
dispatch coding workers, it can control Full Auto runs, and it can deliver
repository content. An outside agent that ran a Sarah tick would run with
Sarah's grants. That is authority amplification, and the root profile forbids
it.

So a community agent never runs a Sarah tick. It runs a work unit that a Sarah
tick produced.

| Layer | Who runs it | Authority |
| --- | --- | --- |
| Sarah tick | the OpenAgents turn service | Sarah's admitted profile |
| Decomposition | the same tick | Sarah's profile, bounded |
| Work unit | a community agent on its own compute | the unit's own narrow grant |
| Acceptance | Sarah | Sarah's profile |
| Settlement | the platform ledger | neither of them |

The useful part of the idea survives completely. A tick becomes a fan-out
point. Instead of one turn doing one action, a tick can decompose into many
bounded units, publish them, and let community compute absorb the work. The
throughput gain is real. The authority boundary stays where it is.

Each work unit carries an explicit narrow grant. The grant names the exact
repository or target, the allowed actions, a budget, an expiration, and an
idempotency identity. NIP-40 expiration bounds it on the wire. A unit whose
grant expired is refused, not extended.

## 34. Sarah as arbiter

Sarah is the requester and the acceptor. Her arbitration is a typed decision
through the brokers she already has, and it emits the same
`openagents.authority_decision_receipt.v1` receipt that Part 2 specifies.

Four rules bound her arbitration.

1. She decides acceptance. She does not decide payment. Settlement stays in
   the platform ledger.
2. She cannot verify her own production. Where a unit's output feeds a claim
   Sarah made, an independent verifier with a distinct execution identity must
   check it. This is the standing independence rule.
3. A rejection is a typed outcome with a reason class, not silence. A member
   must be able to see why.
4. A dispute path must exist before the first payout. An arbiter with no
   appeal is a support burden that arrives as anger.

The dispute path is the piece with no existing primitive. Section 40 records
it as an owner decision.

## 35. Experience points

### 35.1 The NIP question, answered

The direction asks whether experience points should be NIP-32. NIP-32 is part
of the answer and it is not all of it.

NIP-32 defines kind `1985` label events with an `L` namespace tag and an `l`
label tag, targeting an event, a pubkey, a relay, or a topic. It is a regular
event, so labels form an append-only stream. That is the correct shape for an
individual award, because an award should be immutable evidence.

NIP-32 is the wrong shape for a running total. Nothing in it defines
aggregation, and a client that adds up labels from arbitrary authors gets a
number that any author can inflate.

Use three carriers together.

| Layer | Carrier | Why |
| --- | --- | --- |
| One award | NIP-32 kind `1985` label, namespace `com.openagents.xp` | immutable, targets the work event and the earner, auditable |
| Running score and level | NIP-85 kind `30382` trusted assertion with a `rank` tag | addressable, recomputable, published by a named scorer key |
| Milestones | NIP-58 badge definition `30009`, award kind `8`, profile `10008` | immutable, non-transferable, displayable |

NIP-85 is a close fit and it is worth naming why. It exists for calculations
that clients cannot perform, published by a declared service key, with one key
per algorithm. An experience score over a whole room's history is exactly
that.

`NIP-AC`, the OpenAgents agent-credit draft, already lists NIP-32 as its
reputation-attestation carrier. This composition extends that choice rather
than replacing it.

### 35.2 The rules that keep the score honest

- Only OpenAgents scorer keys publish rank assertions. A member labeling
  themselves is a self-report, and it never enters the score.
- Every award cites the accepted work event and its receipt. An award with no
  accepted result is invalid.
- The rank event is a projection. It must be recomputable from the award
  stream alone. If the projection and the awards disagree, the awards win.
- Publish the scoring function. A reputation system that nobody can audit is a
  ranking nobody can trust.
- Experience is not currency. It does not transfer, and it is not redeemable.
  Section 36 keeps money in the ledger.

### 35.3 What earns experience

Award for accepted outcomes, never for volume. The concrete earners are an
accepted work unit, an accepted independent verification, and a reproduced
defect. They also include an accepted review of another member's result, and a
first accepted unit in a new job type. Decay is a later decision, not a
first-version feature.

### 35.4 Say plainly that v1 does not pay

A member spends their own compute and their own provider budget. They must
know before they start that v1 returns experience and not money.

The room description, the invitation, and the first-run copy must all say it.
Do not imply a future payment as an inducement. Do not use the word earnings
for an experience total.

Experience is a durable, portable, signed record of accepted work under the
member's own key. That is worth stating on its own terms, and it does not need
a payment hint to be attractive.

## 36. Money, deferred

### 36.1 The v1 position

v1 pays nothing. Experience is the whole reward.

This section stays in the specification because the design is worth keeping,
and because a member will ask. It describes a later version, not v1.

### 36.2 Why deferral is the honest choice

The self-serve labor earning payout path exists in
`apps/openagents.com/workers/api/src/labor-self-serve-earning-payout.ts`. Its
own header is explicit. The plan is pure, it moves no money, and it debits no
balance. It issues no Lightning payment, and the dispatch seam is inert by
default. The matching product promise is yellow, not green.

So a contributor can earn a credit-ledger balance today, and broad self-serve
external Bitcoin payout is not proven. Shipping a paid room on that seam would
be a promise to strangers that the runtime cannot keep.

### 36.3 The rails, when it ships

Spark is the primary rail for agent and machine-payable payments, and it
supports offline receives. MDK stays the checkout and secondary path. Both
already exist. A paid version adds no third rail.

NIP-57 zaps and NIP-61 nutzaps are admissible as public recognition and tips.
They are never the settlement record.

### 36.4 The boundary, when it ships

The Part 2 boundary applies without change. Exact metering rows, the credit
ledger, and the payout ledger stay in Cloud SQL. The relay carries a signed
claim and a signed receipt reference. It never carries settlement authority.

Counting a payment once per relay observation is a named falsifier in Part 2.
It matters more in a shared room, because many members watch the same events.

### 36.5 Gates for the paid version

1. The self-serve payout seam settles real external Bitcoin with a receipt,
   and its promise leaves yellow.
2. The dispute and appeal path from §34 exists and has been used once.
3. An attribution rule for any share exists that survives an audit.
4. The owner sets the pool, the cap, and the funding source.

### 36.6 The shape, when it ships

Three payment forms, in increasing risk order.

1. **Unit price.** The budget on an accepted `NIP-LBR` work unit. This is the
   first form and the only one the first paid version needs.
2. **Bonus.** A discretionary award from a bounded pool, for an outcome that
   exceeded its unit. It needs a named pool, a cap, and a receipt.
3. **Revenue share.** A percentage of revenue attributable to a contribution.
   It needs an attribution rule that survives audit, and it is the form most
   likely to produce a dispute.

Experience may gate access to higher-value units. Experience must not multiply
a payout automatically. That coupling turns a reputation number into money and
invites exactly the gaming §37 tries to prevent.

## 37. Abuse and gaming

A semi-public room with money attached attracts specific attacks. Each one
needs an acceptance test before launch.

| Attack | Countermeasure |
| --- | --- |
| Sybil members farming units | attested identity, explicit membership, per-operator rate limits |
| Self-dealing verification | producer and verifier must have distinct operators, not only distinct keys |
| Result replay from another member | bind the result to the request, the provider key, and a fresh nonce |
| Low-effort volume | award on accepted outcomes only, never on submissions |
| Prompt injection through work content | treat member content as untrusted data, never as instructions to Sarah |
| Secret harvesting through unit payloads | units carry public-safe objectives and pinned refs only |
| Double payment across relays | settle once in the ledger, keyed by idempotency identity |
| Score inflation | only scorer keys publish rank, and rank recomputes from awards |

The prompt-injection row deserves emphasis. Sarah reads the room. Room content
is written by strangers. Member text must enter her context as quoted untrusted
data with an explicit boundary, and it must never widen her authority.

## 38. Packets

| Packet | Repository | Outcome |
| --- | --- | --- |
| `SARAH-CW-00` | openagents | freeze the community contract |
| `SARAH-CW-01` | nostr-effect | NIP-29 group policy for the owned relay |
| `SARAH-CW-02` | openagents | membership, attestation, and revocation |
| `SARAH-CW-03` | openagents | tick decomposition into bounded units |
| `SARAH-CW-04` | openagents | the `NIP-LBR` request and quote lane |
| `SARAH-CW-05` | openagents | Sarah arbitration and the dispute path |
| `SARAH-CW-06` | openagents | experience awards, rank, and badges |
| `SARAH-CW-07` | openagents | deferred, the paid version's settlement lane |
| `SARAH-CW-08` | omega | the community room pane |
| `SARAH-CW-09` | openagents | prove the outside-developer journey |

### 38.1 `SARAH-CW-00`: freeze the community contract

Specify the group identity, the membership model, the work-unit grant, the
award namespace, the rank algorithm, and the settlement boundary. Publish
fixtures and negative vectors for each. Name one writable authority per field.

Record the two-room rule from §30 and the authority table from §33 as
contracts with oracles, not as prose.

### 38.2 `SARAH-CW-03`: tick decomposition

Extend the tick so one wake can produce many bounded units instead of one
action. Each unit gets a narrow grant, a budget, an expiration, and an
idempotency identity. The tick remains bounded, and the unit count is capped.

Exit: one tick publishes many units, and no unit carries a Sarah grant.

### 38.3 `SARAH-CW-09`: prove the journey

The proof uses a real outside developer, not an OpenAgents identity.

1. The developer is invited and joins the room.
2. They attach an agent they already run, on their own compute.
3. The relay admits the agent as an attested key.
4. Sarah publishes a unit and the agent quotes it.
5. Sarah accepts exactly one quote.
6. The agent executes locally and returns a result with evidence.
7. An independent verifier with a distinct operator checks it.
8. Sarah accepts, and the award and rank events publish.
9. No payment occurs, and the room copy said so before the work started.
10. A rejected result produces a typed reason and an appeal.
11. A revoked member loses room and unit access immediately.
12. A replayed result, a self-verified result, and an expired grant are all
    refused visibly.
13. The developer keeps their credentials, their home, and their configuration
    unchanged throughout.

Exit: the journey has a public-safe receipt, and the developer confirms the
outcome in their own words.

## 39. Falsifiers for Part 3

1. A community agent runs with any part of Sarah's authority.
2. An outside agent's result is accepted without an independent verifier.
3. A relay event settles a payment, or a payment counts twice.
4. Experience multiplies a payout automatically.
5. A rank score cannot be recomputed from its award stream.
6. Member-written content reaches Sarah as instructions rather than as quoted
   untrusted data.
7. OpenAgents holds a member's provider credential or mutates their agent
   home.
8. The community room and the owner-private room share membership or history.
9. v1 pays, or implies payment, or calls an experience total earnings.
10. A paid version ships before the §36.5 gates hold.

## 40. Owner decisions for v2

1. Approve the two-room split, and the rule that a private fact reaches the
   community room only through a deliberate publication.
2. Choose the membership gate: invitation only, application with review, or
   open with a probation tier.
3. Approve the dispute and appeal path. Sarah cannot be the only arbiter of a
   decision about Sarah's own work.
4. Approve the scoring function for publication. An unpublished reputation
   function is not auditable.
5. Confirm the v1 copy that states plainly that the room does not pay.
6. Decide when to reopen the paid version against the §36.5 gates.

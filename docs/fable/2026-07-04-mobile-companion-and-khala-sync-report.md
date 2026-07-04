# Mobile Companion × Khala Sync — Considerations, Stack Choice, and the Dogfood Milestone

Date: 2026-07-04
Status: analysis/plan in the Fable lane. No promise state flips
(`mobile.fleet_companion.v1` stays planned; the 2026-07-04.8 refocus
demotions stand), no public copy, no issues filed yet (§8 proposes the
map). Owner direction being answered: add a **companion mobile app** to
the current arc — we're close already — and use it ourselves as the live
test of the sync engine: **make chats on mobile, see them in Khala Code
desktop/web.**

Sources reviewed: the Khala Sync design doc
(`2026-07-04-database-alternatives-and-postgres-sync-engine.md` §4 — there
is no `docs/khala-sync/` folder; that doc is the canonical spec), the
shipping sync embryo (`apps/openagents.com/packages/sync-worker`,
`packages/sync-schema`, `SyncRoomDurableObject` + `sync-notifier.ts` +
`sync-broadcast-throttle.ts` in the Worker), the empty reserved packages
(`packages/khala-sync{,-client,-server}` — name landed 2026-07-04, no
source yet), the live iOS app (`clients/khala-ios/Khala`), and
`docs/mobile/*` (specs, handshake audit, TestFlight runbook, Expo
retirement record).

## 0. The one-paragraph verdict

We are closer than the roadmap suggests. The native SwiftUI **Khala iOS
app already exists and has shipped to TestFlight** (ChatGPT-style client
on the public Khala API, push-to-talk voice, Codex delegation over durable
streams, keychain-held free-tier keys) — the missing piece is exactly the
piece the owner wants to test: its chat history is **on-device SwiftData
only**, and *no* surface in the company syncs chat today. Meanwhile the
sync engine's server half genuinely works (per-scope cursors, monotonic
sequences, mutation idempotency, hibernating WebSocket rooms, broadcast
throttle — serving agent runs, public counters, and feeds in production),
and its client half plus the chat collection are unbuilt. So the mobile
companion and Khala Sync v1 are the **same project**: define the chat
collection + named mutators on the existing outbox, build the thin
TypeScript client for Khala Code desktop/web, port the (deliberately
small) wire protocol to Swift, and the owner's test — type a message on
the phone, watch it appear in the desktop sidebar — becomes the first
Khala Sync receipt. Recommendation on stack: **stay native SwiftUI**
(mandate-compliant, already shipped, best voice/on-device story) and make
the **wire protocol, not a shared runtime, the reuse boundary**; TypeScript
reuse concentrates on desktop/web/CLI via `khala-sync-client`, with a bare
React Native fallback documented but not chosen.

## 1. Where mobile actually stands (honest inventory)

| Asset | State |
| --- | --- |
| `clients/khala-ios/Khala` (SwiftUI, iOS 17+, `com.openagents.khala`) | **Real and uploaded** (TestFlight build 2 / v1.0.0 via the proven `xcrun altool` runbook, Team HQWSG26L43). Chat UI with Recents drawer, markdown/code rendering, push-to-talk with on-device STT, Apple Foundation Models client, Settings/Diagnostics. |
| Backend integration | Plain URLSession: `POST /api/v1/chat/completions` (model `openagents/khala`, SSE streaming), free-key mint → Keychain, fleet inspector (`GET /api/operator/fleet/status`), **Codex delegation** with `workflowClass: codex_agent_task` + durable-stream resume headers and a public-safe prompt validator. No WebSocket, no sync. |
| Chat persistence | **SwiftData, on-device only, by design** ("v1 stores chat history on-device only (no server sync)"). This is the gap the owner's test targets. |
| Android | Nothing exists. |
| Expo/React Native | The old `AutopilotRemoteControl` Expo app was retired and deleted 2026-06-26; the standing owner mandate is **no Expo/EAS cloud** — the current mobile path is pure local Xcode. |
| Registry | `mobile.fleet_companion.v1` planned; the two voice-companion records were demoted to planned in the 2026-07-04.8 refocus. The companion described here is Khala-Code-family work (chat + fleet surfaces), owner-directed; registry evidence accrues on the `khala_code.*` records and any future flips remain receipt-first. |

## 2. Where Khala Sync actually stands

Two-layer truth (full detail in the design doc §4):

**Shipping today — the embryo** (`@openagentsinc/sync-worker` +
`@openagentsinc/sync-schema`, D1-backed, in production):

- Transactional outbox with per-scope monotonic sequences
  (`sync_scopes.last_seq` claimed atomically), `sync_changes`
  (`put|patch|delete|invalidate`), `sync_mutations` with
  idempotent accept/reject — typed end to end in Effect Schema.
- Delivery: `SyncRoomDurableObject` — per-scope **hibernatable WebSocket
  rooms** with a hibernation-safe broadcast throttle (≤3/sec/scope);
  pattern today is notify-then-refetch with cursors
  (`readChangesAfter(scope, cursor)`).
- Scope taxonomy already includes **`thread:`**, `workspace:`, `team:`,
  `agent-run:`, plus public firehose scopes (tokens-served counter,
  Tassadar settled feed, Gym progress — all live consumers).
- Real tests on the store, throttle, and consumers.

**Not built yet — what the name is reserved for:**

- `packages/khala-sync{,-client,-server}` are **empty** (name landed
  2026-07-04; no source, nothing tracked).
- No client half anywhere: no SQLite-materializing store, no optimistic
  named mutators with rebase (the Linear/Replicache model §4 specifies),
  no offline queue (v1 contract is online-optimistic: reads offline,
  writes reject).
- No resumable offset-stream endpoint (design wants
  `GET /khala-sync/log?scope=&cursor=`), no permission-change retraction,
  no E2EE/pairing, no Postgres/Hyperdrive backing (Phase 1/2 of the
  design's plan).
- **No chat collection.** Khala Code desktop does not consume sync at all
  (its cockpit polls); iOS chats never leave the device. The design names
  "Khala Code desktop (fleet cockpit state, chat, assignments)" and the
  mobile companion as the payoff consumers — future tense.

The owner's requested test is therefore *perfectly aimed*: it forces
exactly the unbuilt half (client library + chat collection + mutators)
against exactly the built half (outbox, scopes, rooms, cursors), on the
smallest honest surface.

## 3. Stack choice: Swift/Kotlin vs React Native

### The constraint set

1. **Owner mandate (2026-06-26, standing):** no Expo/EAS cloud; current
   mobile is native SwiftUI, pure Xcode, TestFlight via `altool`. The
   Expo RN app was deliberately retired.
2. **A shipped asset exists** on the native path — including the hard
   parts RN is worst at: on-device STT, Apple Foundation Models bridge,
   push-to-talk audio, SwiftData with corruption recovery.
3. **The TS estate is large**: Effect-based `sync-schema`, the future
   `khala-sync-client`, behavior contracts, khala-tools — all pure
   TypeScript that runs anywhere JS runs.
4. The sync **wire protocol is small by design**: JSON frames over
   WebSocket + cursor refetch + named mutations. v1 is online-optimistic
   — no CRDT merge machinery to port.

### Options, honestly weighed

| Option | TS reuse | Time to the dogfood test | Risks / notes |
| --- | --- | --- | --- |
| **A. Stay native SwiftUI, port the thin protocol to Swift** (recommended) | None on-device; reuse concentrates in desktop/web/CLI clients | **Fastest** — the app, auth, chat UI, and TestFlight path all exist; we add a `KhalaSyncClient.swift` (~WS + cursor + mutation queue) and swap `ConversationStore` writes to mutators with SwiftData as the local cache | A second protocol implementation to keep in lockstep → mitigated by **contract-first discipline**: Effect Schema is the single source, JSON fixtures exported as a conformance suite both clients must pass (the parity-contract pattern Khala Code already uses against Codex upstream) |
| B. Bare React Native (no Expo) | High — `sync-schema` + `khala-sync-client` run on Hermes as-is; op-sqlite for the local store | Slow — rebuild the entire app (chat UI, voice, keychain, delegation) on a stack we just retired; voice/Apple-FM would still need native modules | Fights the spirit of the mandate; throws away the shipped asset; RN's win (shared client) can be captured later for **Android** if we ever want one codebase there |
| C. Embedded JS core in the Swift app (JavaScriptCore/Hermes running `khala-sync-client`, SwiftUI on top) | High for sync logic only | Medium — bridge plumbing, debugging across the boundary, app-size and lifecycle complexity | Clever but the protocol is too simple to justify it; this is the right pattern only if the client logic grows genuinely hairy (full offline rebase, E2EE sessions) |
| D. Kotlin Multiplatform | None (Kotlin) | Slow — new toolchain, no Android demand yet | Revisit when Android is real; KMP would serve iOS+Android from one core but replaces rather than reuses the TS estate |

### Recommendation

**Option A.** Keep the native SwiftUI companion; make the **protocol the
contract**, not the runtime. Concretely:

- `khala-sync-client` (TypeScript, the reserved empty package) is built
  once and consumed by Khala Code desktop, web, and CLI — that's where
  TS reuse pays, and those are the surfaces the test must light up.
- The Swift client implements the same wire protocol against a
  **generated conformance suite**: Effect Schema → JSON Schema + golden
  frame fixtures checked into the repo; a Swift test target replays them.
  Protocol drift becomes a failing test, not a debugging session.
- Kotlin/Android and RN stay documented fallbacks: if Android demand
  arrives, decide then between a Kotlin port of the (still-small)
  protocol and bare RN with the shared TS client.
- Voice, Apple FM, push (APNs later), and the delegation flow stay where
  they already work — native.

## 4. TypeScript reuse map (what's shareable where)

| Package | Desktop (Electrobun) | Web | CLI | iOS (Swift) |
| --- | --- | --- | --- | --- |
| `sync-schema` (types/contracts) | direct | direct | direct | via generated JSON Schema + fixtures |
| `khala-sync-client` (to be built) | direct — **first consumer** | direct | direct | protocol port (`KhalaSyncClient.swift`) |
| `behavior-contracts` | direct (already) | direct | direct | oracle assertions run against the app from the QA harness, not on-device |
| `khala-tools` / delegation contracts | direct | direct | direct | already hand-ported (the Codex-delegation flow in `KhalaClient.swift`) — same conformance-fixture treatment recommended |
| Effect runtime | yes | yes | yes | no (and not needed under Option A) |

## 5. The dogfood milestone: "chat made on mobile appears in Khala Code desktop/web"

What actually has to exist, layer by layer (server → clients), smallest
honest scope — one owner account, online-optimistic, no offline queue:

**Server (the outbox already does most of this):**

1. **Chat collections on existing scopes**: `thread` entities +
   `message` entities under the owner's `workspace:`/`thread:` scopes
   (taxonomy already present). Public-safety: chats are owner-private
   scopes — never public firehose; redaction posture per existing rules.
2. **Named mutators** through `sync_mutations`:
   `chat.createThread`, `chat.appendMessage`, `chat.renameThread`
   (server-authoritative, idempotent via the existing accept/reject
   machinery).
3. **Auth scoping**: subscribe/mutate gated to the owner account token —
   the same boundary the private scopes already assume; the design's
   gatekeeper/retraction work stays Phase-2+.

**TypeScript client (`khala-sync-client` v0 — fills the empty package):**

4. Subscribe (WS room join + snapshot + cursor catch-up), local store
   (in-memory + IndexedDB/SQLite where available), optimistic apply of
   own mutations with server accept/reject reconciliation. Deliberately
   the design's v1 contract, nothing more.

**Khala Code desktop + web (first consumers):**

5. Sidebar session list backed by the sync store: new threads/messages
   appear live. Desktop registers this as UX behavior contracts
   (statement + oracle in the sweep, per the standing mandate) — e.g.
   "a thread created on another device appears in the sidebar without
   restart."

**iOS:**

6. `KhalaSyncClient.swift` (WS + cursor + mutations) with SwiftData as
   the local cache; `ConversationStore` writes route through mutators
   when online (local-only remains the offline behavior, honestly
   labeled). Conformance fixtures shared with the TS client.

**The test itself (the receipt):**

7. Owner creates a chat on the phone → thread + messages appear in Khala
   Code desktop and web within seconds; then the reverse (desktop chat
   visible on phone). Recorded as a public-safe evidence bundle (counts,
   latencies, scope refs — no chat content) — the first Khala Sync
   receipt, and strong `khala_code.*` evidence.

**Explicitly out of scope for the milestone:** offline mutation queue,
E2EE/pairing (the Orca-path companion goal — later), Android, Postgres
migration (the milestone runs fine on the D1 embryo; Phase 1/2 of the
sync design proceeds independently), push notifications, and any registry
flips.

## 6. Effort shape (estimate, not commitment)

The server outbox, rooms, scopes, cursors, and idempotency exist and are
production-tested; the iOS app, auth, and chat UI exist and are shipped.
The genuinely new work is: chat collection + three mutators (small,
Worker-side, well-trodden patterns), `khala-sync-client` v0 (the
largest single piece), the desktop/web sidebar consumers, and the Swift
protocol port with conformance fixtures. Everything is fleet-shaped
except the final on-device testing and the TestFlight upload sitting,
which are owner-adjacent.

## 7. Fit with the revenue focus (and the refocus demotions)

Honest reconciliation with `2026-07-04.8`: mobile *records* were demoted
because standalone mobile ambitions are out of the revenue lane — but
this companion is **Khala Code family infrastructure**, owner-directed,
and doubles as the sync engine's first cross-device proof. The commercial
payoffs are real and near: multi-device chat is table-stakes for Khala
Code as a product (the owner is customer number one, again); Khala Sync
is the substrate the design doc names for cockpit state and assignments;
and the same sync spine is what an Autopilot mobile approval surface
(approve a fulfillment send from your phone) will ride later — the
BF-4.3 approval ladder made ambient. No mobile promise state changes
until the receipts exist.

## 8. Proposed task map (for a follow-up filing pass; not filed here)

| Task | Description | Depends |
| --- | --- | --- |
| KS-1 | Chat collection + named mutators on the sync outbox (thread/message entities, owner-scoped, tests) | — |
| KS-2 | `khala-sync-client` v0 (subscribe/snapshot/cursor/optimistic mutations; fills the empty package) + conformance fixture export from `sync-schema` | — |
| KS-3 | Khala Code desktop + web sidebar consumers with UX behavior contracts | KS-1, KS-2 |
| KS-4 | `KhalaSyncClient.swift` + ConversationStore integration, passing the shared conformance fixtures | KS-1, fixtures from KS-2 |
| KS-5 | The dogfood run: phone↔desktop↔web chat round-trip, public-safe evidence bundle, TestFlight build refresh | KS-3, KS-4 |

Sequencing note: KS-1 and KS-2 start immediately and in parallel; KS-5 is
the owner's requested test and the milestone's only definition of done.

## 9. Superseded-in-part addendum (2026-07-04, same day)

The owner decided **one UI ecosystem — React + Tailwind + Expo React
Native** (recorded in
`2026-07-04-tanstack-start-sites-and-web-app-evaluation.md` §6/§9, with
the AGENTS.md/CLAUDE.md rule updates in the same change). Effect on this
report: §3's Option A is superseded as the *destination* (the companion's
end-state is the Expo RN app, sharing the TanStack DB +
khala-sync-db-collection data layer with web/desktop); the SwiftUI app
becomes the interim companion and native-module reference; KS-2 is
superseded by the khala-sync-db-collection adapter (TS-3) and **KS-4 (the
Swift protocol port) is canceled**. KS-1 (chat collection + mutators),
KS-3 (desktop/web consumers), and KS-5 (the cross-device dogfood test)
stand unchanged, and the near-term test may still run on the interim
SwiftUI app via direct mutation calls + WS refetch. Updates ship through
our own EAS-Updates-replacement server (`apps/oa-updates`, expo-updates
protocol v1 + signed manifests, `updates.openagents.com`) — preserved by
owner direction; builds stay local (prebuild + Xcode/Gradle) for now.

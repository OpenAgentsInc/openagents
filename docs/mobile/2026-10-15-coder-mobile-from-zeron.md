# Coder on the phone: porting to the Zeron iOS app (2026-10-15)

Status: approved plan (owner, 2026-10-15). Epic: [#11840](https://github.com/OpenAgentsInc/openagents/issues/11840). The owner asked:
1. What's needed to move our iOS app onto the Zeron iOS app's UI, which
   ships inside our fork ([OpenAgentsInc/coder-desktop](https://github.com/OpenAgentsInc/coder-desktop),
   `apps/ios`)?
2. Can our Nostr stack replace Zeron's Cloudflare server, or should we build
   something else in Rust on Google Cloud?
3. Should the work happen in the fork or in this monorepo?

Constraints from the owner: **no Cloudflare**, Rust on our infrastructure
(Google Cloud), and port the iOS app **now**.

Related: [Coder Desktop](../coder-desktop/README.md), the
[SDK](../sdk/README.md), [Computers](../sdk/computers.md).

## Recommendation in one paragraph

**Build the phone app in the fork, and build its server in this monorepo.**
- **The phone:** the fork's iOS app is the right base: a Rust core with a
  UIKit shell that already does sessions, transcripts, the composer,
  questions, push and voice. It shares its sync code and protocol with
  Coder Desktop's engine, so keeping them in one repository keeps them in
  step with each other and with upstream.
- **The server:** replace Zeron's Cloudflare server ("edge", about 4,000
  lines of TypeScript) with **`coder-sync`, a Rust server on Google Cloud**:
  one binary, one Postgres, objects in Cloud Storage. It speaks the same
  HTTP and WebSocket protocol, so the phone and desktop clients work
  unchanged apart from their server address and login.
- **Accounts:** sign-in uses **OpenAgents accounts**, not WorkOS.
- **Nostr:** don't carry the transcript sync over Nostr; it needs a
  server-ordered log and resumable downloads that relays don't provide. Do
  use Nostr where it already fits us: device identity, and later the
  phone-to-computer command channel through NIP-HOST.


## What the Zeron phone app is

From the fork at `v0.2.113` plus two of our commits:

| Layer | Where | Size |
| --- | --- | --- |
| UIKit shell: sessions list, transcript painter, composer, sheets, Live Activity | `apps/ios` | about 31,000 lines of Swift |
| UniFFI facade and transcript layout engine (rows, measured display lists, visible ranges) | `crates/mobile` | about 12,700 lines of Rust |
| Viewer client: registry, chat rooms, documents, relay calls, auth, view models, demo data | `crates/client` | about 12,000 |
| Sync: Loro room client, catch-up, backoff, HTTPS fallback | `crates/sync` | about 8,900 |
| Documents: session and registry schemas, command ledger | `crates/doc` | about 11,400 |
| Protocol, RPC, text engine, markdown | `crates/{proto,rpc,text,markdown}` | about 15,700 |

The phone is a **viewer and remote control**: "no agent runs on the phone".
It mirrors the list of sessions, joins one room per chat, and drives a
desktop or headless engine through a durable command ledger.

**What it needs from a server** (Zeron's `edge/`, Cloudflare Workers and
Durable Objects):

| Server piece | What it does | Size (TypeScript) |
| --- | --- | --- |
| Chat rooms (`/chat2/{id}`) | Each chat's Loro document as an append-only log of update rows with server-assigned sequence numbers, plus checkpoints downloadable with HTTP `Range` resume; WebSocket, with HTTPS pull/push as a fallback | `chat-room.ts`, `chat-route.ts`, `chat-log.ts`, `chat-frames.ts`: about 1,100 |
| Registry room (`/registry/{org}`) | One per user: the sessions list, spaces, devices and status rows, as row frames with a `since` cursor; push-target registration | `registry-room.ts`, `registry-core.ts`: about 840 |
| Device room (`/device/{id}`) | A relay between a viewer (phone) and the host engine: catalogs, folders, uploads, queue actions; host liveness and wake nudges | `device-room.ts`, `device-nudges.ts`: about 400 |
| Blobs and attachments | Uploads and downloads in R2 | `blobs.ts` and routes |
| Auth | WorkOS JWT checks; a dev mode for local tests | `auth.ts`, `workos.ts`, `auth-routes.ts`: about 380 |
| Push | APNs on session state changes (finished, needs you, failed) | `apns.ts`, `push-notify.ts` |
| Legacy session room and previews | The pre-cutover session protocol (no current client uses it), and browser preview rooms | `session-room.ts` (1,441, skip), `preview-room.ts` |

**Good news from the client code:** the sync layer already has seams.
- `ChatTransport` and `RegistryTransport` are traits for the HTTPS path.
- The WebSocket path goes through a `UrlProvider`.
- The server address is one config value (`edge_url`; production is
  `https://edge.zeron.sh` in `crates/client/src/auth.rs`).
- There's an in-memory transport for tests.

Pointing the clients at a different server is a configuration change, not a
rewrite.

## Option A: carry it all over Nostr

| Piece | Fits Nostr? |
| --- | --- |
| Device room (phone ⇄ computer commands) | **Yes.** This is what [NIP-HOST](../../nips/openagents/NIP-HOST.md) already does for our phones and `coder` hosts: encrypted request/response through our relay, with signed grants |
| Identity and pairing | **Yes.** Our device keys, NIP-HOST enrollment and QR pairing |
| Push | **Partly.** Our [`push-gateway`](../../crates/push-gateway) (NIP-PL) holds APNs credentials already |
| Registry (sessions list) | **Awkward.** It could be addressable events per row, but tombstones, an exact `since` cursor, and one cheap subscription for the whole list all fight relay semantics (relays order by `created_at`, which is client-chosen) |
| Chat rooms (transcripts) | **No.** They need a server-assigned, gap-free sequence per room, server-held checkpoints with resumable `Range` downloads, and batch-id deduplication. A Nostr relay provides none of these. We'd have to extend our relay into a custom server anyway, and then rewrite `zeron-sync` to match |

**Verdict:** forcing the transcripts onto Nostr means rewriting the
phone's and desktop's sync code and diverging from upstream on the part
that changes most. That's too difficult for "port it now", and not worth
it.

## Option B: `coder-sync`, the same protocol in Rust on Google Cloud (recommended)

A Rust server that answers the routes the current clients use, nothing more:

| Zeron edge | `coder-sync` |
| --- | --- |
| Durable Object per room (single writer) | A room actor per chat and per user in the process, with the sequence in Postgres. Start with **one instance** (Cloud Run `max-instances=1`, or one small GCE VM), which is plenty for now. Scale out later with per-room leases (`crates/actors` fenced leases) and Postgres `LISTEN/NOTIFY` to fan out |
| Durable Object storage | **Postgres**: `chat_rows(chat_id, seq, batch_id, bytes)`, `checkpoints`, `registry_rows`, `devices`, `push_targets` |
| R2 | **Cloud Storage** for blobs, attachments and checkpoints (`Range` downloads served straight from the bucket or through the server) |
| Workers WebSockets | axum/tokio WebSockets. Cloud Run supports WebSockets with a request timeout of up to 60 minutes, and the client already reconnects with backoff. A GCE VM avoids the cap if it matters |
| WorkOS JWT | **OpenAgents accounts**: our session or device-login token (`openagents-login`, device code), checked against the account service. Registry rooms are per account, as Zeron's are per user |
| APNs from the edge | **Our `push-gateway`**: same three triggers (finished, needs you, failed) |
| Device room relay | Same WebSocket relay in v1, so the clients work unchanged. In v2, optionally carry it over NIP-HOST on our relay (see below) |
| Legacy session room | Not built: no current client dials it |
| Preview rooms | Later, with the browser-preview work |

**Home and shape:**
- A crate in this monorepo, `crates/coder-sync`, following our rules: Rust,
  one binary, one Postgres, deployed with the rest of our services in
  `openagentsgemini`.
- Address: `sync.openagents.com`, or a path on `api.openagents.com` if the
  WebSocket behaviour allows.

**Staying compatible with upstream:**
- **Pin and test against the real clients.** `coder-sync`'s integration
  tests depend on the fork's `zeron-sync` and `zeron-client` crates, pinned
  by git commit, and run them against our server: join, catch up from a
  checkpoint, push rows, reconnect, HTTPS fallback, and a device-room round
  trip.
- **Port the edge's own tests.** They become `coder-sync`'s test list:
  `chat-frames`, `registry-core`, `update-log`, `device-frame`,
  `device-host-liveness`, `push-notify`.
- **Watch upstream's `edge/` and `crates/sync` wire types** on every merge.
  A protocol change there is a `coder-sync` change. The fork's
  `FORK.md` should list this as a merge checkpoint.

**Size:** about 4,000 lines of TypeScript, without the 1,441-line legacy
session room. The wire formats already exist as Rust types in `zeron-sync`
and `zeron-proto`, so the server can share them instead of re-deriving them.

## Option C: run Zeron's server as is

Rejected. It's a Cloudflare Worker with Durable Objects and R2, which
breaks the "no Cloudflare" rule, and it's TypeScript.

## Where Nostr does fit (keep it, at the edges)

- **Device identity.** Each phone and engine keeps a Nostr key, as our
  devices already do. `coder-sync` can accept a NIP-98 signed request as
  device proof alongside the account token, so a revoked device is cut off
  without revoking the account.
- **Phone to computer, version 2.** NIP-HOST already carries encrypted
  requests between a phone and a host through `wss://relay.openagents.com`.
  Once v1 works, the device room can become a NIP-HOST profile: the
  same command vocabulary (`zeron_rpc::methods`) inside NIP-HOST's
  encryption and grants. That gives Coder hosts and the phone one pairing
  and one grant system. It's an improvement, not a prerequisite.
- **Payments.** Paid computer time and x402 stay on our Lightning rails, as
  in the [Computers plan](../sdk/computers.md).

## Fork or monorepo?

| | Build in the fork (`coder-desktop`) | Build in this monorepo |
| --- | --- | --- |
| Upstream merges | Natural: `apps/ios` and `crates/{mobile,client,sync,doc,...}` merge as they arrive | We'd copy about 75,000 lines of Rust and 31,000 lines of Swift and lose easy merges |
| Shared protocol with Coder Desktop | Same crates, same commit | Two copies drift |
| Our monorepo rules | The fork is allowed other languages and a thick UIKit shell | Breaks "product code is Rust" and the thin-host rule (31,000 lines of Swift) |
| Our server | — | **Yes, here**: `coder-sync`, accounts, push, Computers |

**So:** the phone app lives in the fork, next to Coder Desktop, and the
servers it talks to live here. That's the same split as the desktop plan.

### What happens to our current iOS app

Today's OpenAgents iOS app (`bins/openagents-ios` with
`crates/openagents-mobile`: about 35,600 lines of Rust and 12,100 of Swift)
has chat (NIP-CJ), Coder on your computers (NIP-HOST), Verse, the Gym, a
Spark wallet, and an account tab. Following the owner's direction (Coder
focuses on coding; a dedicated OpenAgents app may come later with Verse, the
marketplace and the wallet):

- **Coder's phone app is the fork's app,** renamed "Coder", with our icon,
  OpenAgents sign-in, and `coder-sync`.
- **The OpenAgents app stays** as the base for that later app (Verse,
  wallet, Gym). It isn't deleted, and it gets no new Coder work.
- **Carry these over** from the OpenAgents app into the Coder phone app,
  as features, not code:
  - OpenAgents sign-in and account;
  - your OpenAgents chats (as on the desktop, #11824);
  - running Coder on your own computers and on our cloud computers.

  The wallet and Verse don't move.

## The plan

| Step | What | Where |
| --- | --- | --- |
| P0 | Rename the iOS app: **OpenAgents Coder**, our icon, bundle `com.openagents.coder`, Zeron's sign-in and server address off, demo mode working | fork |
| P1 | `coder-sync` v1: chat rooms, registry room, device room, blobs on Cloud Storage, Postgres, and dev-mode auth; integration tests running the pinned `zeron-sync`/`zeron-client` against it | monorepo |
| P2 | OpenAgents sign-in in the phone and desktop clients (device code, or the website's session through the SDK), and `coder-sync` checking our accounts | both |
| P3 | Coder Desktop (or `zeron headless`) hosts its sessions through `coder-sync`. The phone sees and drives them live | fork + deploy |
| P4 | TestFlight build (no push notifications in the first release) | both |
| P5 | Your OpenAgents chats in the phone (shared with desktop #11824) | both |
| P6 | **Cloud computers:** start the headless Coder engine on a computer from the Computers API, so the phone can run sessions with no desktop on | both |
| Later | Push through our `push-gateway`; device room over NIP-HOST; Nostr device proofs; Android, once upstream's Android app exists | both |

**Verification** (see [targets](../verification/targets.md)):
- `coder-sync` keeps the properties the clients rely on: a gap-free, strictly
  increasing sequence per chat; batch-id deduplication (a retried push
  applies once); a checkpoint never ahead of its rows; and registry rows
  visible only to their own account.
- These are a small Stateright model of a room, checked alongside the
  Computers target, with every counterexample becoming a test.

## Decisions (owner, 2026-10-15)

1. **The app is "OpenAgents Coder"** on the existing iOS bundle and App
   Store record, `com.openagents.coder`. The Mac app uses
   `com.openagents.coder.desktop`, so the IDs don't collide.
2. **The server is `sync.openagents.com`.**
3. **The first release has no push notifications.** Push comes later, through
   our `push-gateway`.
4. Cloud Run with one instance first (the recommendation; move to GCE only
   if the WebSocket time limit causes trouble).

Tracking: epic [#11840](https://github.com/OpenAgentsInc/openagents/issues/11840), with children #11841–#11854.

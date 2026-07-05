# Khala Code Desktop ↔ Mobile Tailnet Handshake

Status: live on `main`. Covers the connectivity status dot shipped in
`clients/khala-mobile` and the health beacon it talks to in
`clients/khala-code-desktop`.

## What this is

The mobile app (`clients/khala-mobile`, Expo/React Native) has a single home
screen: a red/green status dot showing whether it can currently see a running
Khala Code desktop instance. There is no login, pairing, or configuration step
— it just probes a fixed local port.

## Desktop side: the health beacon

`clients/khala-code-desktop/src/bun/index.ts` runs a second, minimal HTTP
server alongside the existing preview/RPC server:

- Bound to `0.0.0.0:50099` (not loopback-only) so it is reachable from other
  devices on the same network/Tailnet.
- Only route: `GET /health`, returns
  `{ ok: true, app: "Khala Code Desktop", hostname: os.hostname(), observedAt }`.
- Deliberately separate from the existing preview RPC server on a different
  port, which stays loopback-only because its `/rpc/*` routes carry a
  per-boot secret token. The health beacon carries no secret and exposes
  nothing but liveness + hostname, so widening its bind address is safe.
- Configurable via env: `KHALA_CODE_DESKTOP_TAILNET_HEALTH=0` disables it;
  `KHALA_CODE_DESKTOP_TAILNET_HEALTH_PORT` overrides the port. Both are typed
  entries in `khala-code-config.ts`'s `KhalaCodeEnvKey` union.
- Started once at boot via `startTailnetHealthBeacon()`, right after the
  existing `startPreviewServer()` call.

## Mobile side: the connectivity probe

Logic lives in `clients/khala-mobile/src/status/`, split into two files so the
pure resolution logic stays unit-testable under `bun test` (importing
`expo-device` pulls in React Native's Flow-syntax entry file, which breaks
Bun's plain JS/TS parser):

- `khala-code-connectivity-core.ts` — pure functions, no native imports.
  - `candidateTargets(isDevice, port, tailnetHosts)`: on a simulator, returns
    just `http://127.0.0.1:50099/health` (the simulator shares the host Mac's
    network stack, so localhost reaches whatever is running the desktop app
    on the same machine). On a physical device, returns one URL per
    configured Tailnet hostname (currently `imac-pro-bertha`,
    `macbook-pro-m2`) — edit `KHALA_CODE_TAILNET_CANDIDATE_HOSTS` to add more.
  - `resolveKhalaCodeConnectivity(targets, fetchImpl, timeoutMs)`: tries each
    candidate URL in order with a 1500ms per-host abort timeout, returns the
    first reachable one (`{ reachable, target, hostname, checkedAt }`), or an
    unreachable result if all fail.
- `khala-code-connectivity.ts` — thin wrapper: imports `expo-device` to detect
  `Device.isDevice`, and exposes `checkKhalaCodeConnectivity()` which calls the
  core resolver with real `fetch`.

`app/index.tsx` is the entire mobile home screen: it polls
`checkKhalaCodeConnectivity()` every 5 seconds and again whenever the app
returns to the foreground (`AppState` "active" transition), then renders a
colored dot (gray while checking, green if reachable, red otherwise) plus the
hostname and target URL it found.

## Simulator vs. device targeting

This is the one piece of routing logic in the handshake, and it is
intentional rather than a placeholder:

- **Simulator** (`Device.isDevice === false`): probe `127.0.0.1` only. The iOS
  Simulator shares the host Mac's loopback interface, so this is exactly the
  Khala Code desktop instance running on the same development machine — no
  Tailnet involved.
- **Physical device** (`Device.isDevice === true`): probe the configured
  Tailnet hostnames over the Tailnet, since a real phone is a separate network
  peer from the Mac running Khala Code desktop.

## Verification

Confirmed live in both states via the iOS Simulator: green dot + hostname when
a local `bun run dev` Khala Code desktop instance is up and the beacon
responds on `:50099`; red dot + "no khala code instance found" when the
desktop app (and thus the beacon) is not running.

## Chat streaming over Khala Sync

Built on top of the same Tailnet handshake and the same single mobile home
screen: the desktop's already-flag-gated Khala Sync chat service
(`clients/khala-code-desktop/src/bun/khala-sync-service.ts`, `KHALA_SYNC_CHAT=1`
+ `KHALA_SYNC_CHAT_OWNER_USER_ID`) can create threads, rename them, and now
append messages (`chatAppendMessage`, wired end-to-end through
`shared/rpc.ts` → `khala-sync-service.ts` → `bun/rpc-handlers.ts` →
`ui/main.ts`'s preview-RPC bridge). Those mutations write into the owner's
Khala Sync `scope.thread.<threadId>` (message bodies + thread metadata) and
`scope.user.<owner>` (thread metadata only) scopes via the named
server-authoritative mutators `chat.createThread` / `chat.appendMessage` /
`chat.renameThread` (MC-1, #8352, `packages/khala-sync-server/src/chat-mutators.ts`).

### Mobile side: raw JSON chat feed

`clients/khala-mobile` now has a second piece on the same home screen (below
the connectivity dot): a raw JSON event feed
(`src/sync/khala-chat-feed.tsx` + pure wire-protocol helpers in
`src/sync/khala-chat-feed-core.ts`). It talks directly to the Khala Sync wire
protocol — no TanStack DB collection layer, deliberately, since the ask was
"just show raw ugly json for now":

1. `POST /api/sync/bootstrap` for `scope.thread.<threadId>` (a consistent
   snapshot page + a `cursor`).
2. Opens a WebSocket to `GET /api/sync/connect?scope=…&cursor=…` using React
   Native's `WebSocket` third-argument `{ headers }` extension to carry the
   bearer token (browsers can't set WebSocket headers; RN can).
3. Renders every bootstrap response and every live `LiveFrame` (`DeltaFrame`,
   `MutationAckFrame`, `MustRefetchFrame`, `PingFrame`) as a raw
   `JSON.stringify` block, newest first.

Demo wiring lives in `src/config/khala-sync-demo.ts` — there is no mobile
login flow yet, so the bearer token, owner user id, and thread id are read
from `EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN` /
`EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID` /
`EXPO_PUBLIC_KHALA_SYNC_DEMO_THREAD_ID` at build time (never hardcoded, never
committed).

### Known gap: prod deploy blocked (tracked in #8376)

While wiring this up, `chat.createThread` returned `unknown_mutator` against
production. The mutator code is on `main`, but the `openagents.com` Worker
has not been redeployed since it landed — `deploy:safe` currently fails its
`check:architecture` zero-debt gate (`Worker throw new Error calls` and
`Worker Response return surfaces` budgets, both exceeded by small amounts
accumulated across today's unrelated Khala Sync dual-write PRs, not by this
change). One of the two overages was fixed here (a self-contained
throw-immediately-caught call in `business-domain-store.ts` converted to a
direct log+return); the other needs a real route-mapper extraction and is
tracked in issue #8376 rather than rushed.

Until that lands and someone runs `deploy:safe`, `scope.thread.<threadId>`
has no ownership row in prod, so the mobile feed's bootstrap call honestly
returns `unauthorized_scope` instead of chat content. All the client and
desktop wiring described above is real and complete — this is purely a
server-side deploy gap, not a client-side gap. Verified live: `scope.user.<owner>`
bootstrap reads and the full auth pipeline work correctly against prod today
(confirmed via existing `sync.debugEcho` entities); once the chat mutators are
live, the same mobile screen will show real chat messages with zero further
app changes.

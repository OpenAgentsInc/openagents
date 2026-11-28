# OpenAgents iOS

Native iOS & iPadOS app running **100% in OpenAgents Cloud mode**, rendered on the shared **Coax / Metal GPU** engine (`ios-app.coax` → `gpu-page.coax`) with pure-black OLED modal sheets.

## Capabilities

- **Coax / Metal GPU Surface**: Main chat transcript, Cloud sidebar (`WORKING`, `PINNED`, `CHATS`, `PROJECTS`, `ENVIRONMENTS`), Dashboard, Markdown renderer, unified single-mode execution, automatic OpenAgents Auto routing, bottom-left execution target selector, queued follow-ups, Always-On Usage HUD, and Bi-Directional Live Voice duplex.
- **OpenAgents Account & Integrations**: Strict full-screen OLED Login Gate until logged in, supporting Google, Apple, and 6-digit email code (WorkOS Magic Auth / Email + Password) via `AppAuth` + `AppAccount`, WorkOS organization switching, GitHub integration via WorkOS Pipes, Passkeys, and USD Paid + Promo credit display.
- **OpenAgents Cloud Chat, Tier 1 Tools & Cross-Device Sync**:
  - Live SSE streaming over `POST /api/v1/inference` with Tier 1 Cloud Tools (`POST /api/tools/cloud/v1`: Web Search, Web Fetch, GitHub read tools) and `thought_signature` preservation.
  - Real-time Postgres LISTEN/NOTIFY sync (`GET /api/chat/v2/stream`, `GET /api/chat/v2/conversations/:id/stream`, `POST /api/chat/v2/sync`, `POST /api/chat/v2/conversations/:id/meta`) backed by the `#authority` `account-chat-cloud-wire.coax` codec so iPhone, Mac Desktop, and Web stay in sync.
  - Durable execution on enrolled computers (`$0` local) or Cloud Sandbox (`POST /api/chat/submit/v2alpha1`, `POST /api/chat/placement/v1`, `GET /api/chat/environments/v1`, `GET /api/projects/v1`).

## Build & Check

```sh
apps/openagents-ios/build.sh
apps/openagents-ios/check.sh
```

# OpenAgents iOS

Native iOS & iPadOS app running **100% in OpenAgents Cloud mode**, rendered on the shared **Coax / Metal GPU** engine (`ios-app.coax` → `gpu-page.coax`) with pure-black OLED modal sheets.

## Capabilities

- **Coax / Metal GPU Surface**: Main chat transcript, Cloud sidebar (`WORKING`, `CHATS`, `PROJECTS`, `ENVIRONMENTS`), Dashboard, Markdown renderer, composer, Full Auto toggle, Always-On Usage HUD, and Bi-Directional Live Voice duplex.
- **OpenAgents Account & Passkey**: Sign in with Passkey (`ASAuthorizationController`), 6-digit email code (WorkOS Magic Auth), Email & Password, or Apple / Google / GitHub SSO via `AppAuth` + `AppAccount` + `KitPasskey`.
- **OpenAgents Cloud Chat, Tier 1 Tools & Cross-Device Sync**:
  - Live SSE streaming over `POST /api/v1/inference` with Tier 1 Cloud Tools (`POST /api/tools/cloud/v1`: Web Search, Web Fetch, GitHub read tools) and `thought_signature` preservation.
  - Real-time Postgres LISTEN/NOTIFY sync (`GET /api/chat/v2/stream`, `GET /api/chat/v2/conversations/:id/stream`, `POST /api/chat/v2/sync`, `POST /api/chat/v2/conversations/:id/meta`) backed by the `#authority` `account-chat-cloud-wire.coax` codec so iPhone, Mac Desktop, and Web stay in sync.
  - Durable execution on enrolled computers (`$0` local) or Cloud Sandbox (`POST /api/chat/submit/v2alpha1`, `POST /api/chat/placement/v1`, `GET /api/chat/environments/v1`, `GET /api/projects/v1`).
- **Spark Bitcoin & Lightning Wallet**: Self-custodial mainnet wallet backed by `OASparkWallet` (`BreezSdkSpark`), `OAWalletKeychain`, `#authority` `pay/wallet.coax` & `pay/wallet-backup.coax` spending/backup policies, and read-only NIP-06 Nostr identity (`coax/lib/nostr/nip06.coax`).

## Build & Check

```sh
apps/openagents-ios/build.sh
apps/openagents-ios/check.sh
```

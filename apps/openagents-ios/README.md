# OpenAgents iOS

Native iOS & iPadOS app rendered on the shared **Coax / Metal GPU** engine (`ios-app.coax` → `gpu-page.coax`) with pure-black OLED modal sheets.

## Capabilities

- **Coax / Metal GPU Surface**: Main chat transcript, Universal Introspection sidebar (`WORKING`, `CHATS`, `TUI & TERMINAL`, `OTHER HARNESSES`, `MACHINES`), Dashboard, Markdown renderer, composer, Full Auto toggle, Always-On Usage HUD, and Bi-Directional Live Voice duplex.
- **OpenAgents Account & Passkey**: Sign in with Passkey (`ASAuthorizationController`), 6-digit email code (WorkOS Magic Auth), Email & Password, or Apple / Google / GitHub SSO via `AppAuth` + `AppAccount` + `KitPasskey`.
- **Cloud Chat & Cross-Device Sync**: Standalone Cloud Chat (`POST /api/chat/complete/v1`) and real-time Postgres LISTEN/NOTIFY sync (`GET /api/chat/v2/stream`, `POST /api/chat/v2/sync`) using the `#authority` `account-chat-cloud-wire.coax` codec so iPhone, Mac Desktop, and Web stay in sync.
- **Spark Bitcoin & Lightning Wallet**: Self-custodial mainnet wallet backed by `OASparkWallet` (`BreezSdkSpark`), `OAWalletKeychain`, `#authority` `pay/wallet.coax` & `pay/wallet-backup.coax` spending/backup policies, and read-only NIP-06 Nostr identity (`coax/lib/nostr/nip06.coax`).
- **Direct Tailscale Mac Pairing**: Zero-relay NIP-44 E2E RPCs over Tailscale (`port 47109`) + Bonjour discovery (`_openagents-coder._tcp`) for driving local workspaces, diffs, terminals, and harnesses on your Mac.

## Build & Check

```sh
apps/openagents-ios/build.sh
apps/openagents-ios/check.sh
```

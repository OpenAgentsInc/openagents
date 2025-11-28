# Effect-Based Nostr Implementation: History and Current Status

This report documents the Effect-based Nostr library that previously lived in this repository, what it implemented (including NIP‑01), where the code was located, how it evolved, and when/why it was removed. It also notes the current Nostr approach in this repo.

## TL;DR
- Yes, we implemented a Nostr library using the Effect ecosystem in this repo.
- Core NIP‑01 was implemented with typed schemas, services, and tests in `packages/nostr/`.
- Additional NIPs (notably NIP‑06; beginnings of NIP‑28; helpers for NIP‑02/04/05/09/19/44) were added.
- A full Effect-based relay (`packages/relay/`) with PlanetScale + Drizzle was built and integrated.
- The TypeScript monorepo path (including `packages/nostr`) was later removed in a “Zero base” reset; current strategy uses a Swift `nostr-sdk-ios` fork for Apple platforms.

## What Existed (Effect + TypeScript)

- Library package: `packages/nostr`
  - Core architecture used Effect (Layer/Context/Effect/Schema) with branded types and Zod-like runtime validation via `@effect/schema`.
  - Key components (selected files as of implementation):
    - `packages/nostr/src/core/Schema.ts`
    - `packages/nostr/src/core/Errors.ts`
    - `packages/nostr/src/nip01/index.ts` (NIP‑01 schema and parsing/format)
    - `packages/nostr/src/services/CryptoService.ts`
    - `packages/nostr/src/services/EventService.ts`
    - `packages/nostr/src/services/WebSocketService.ts`
    - `packages/nostr/src/services/RelayService.ts`
    - `packages/nostr/src/services/RelayPoolService.ts`
    - `packages/nostr/src/services/RelayReconnectService.ts`
    - `packages/nostr/test/EphemeralRelay.ts` (in‑memory relay for tests)
    - Tests under `packages/nostr/test/`
  - Additional NIPs/files present at deletion time (evidence of breadth):
    - `packages/nostr/src/nip06/Nip06Service.ts` (NIP‑06)
    - `packages/nostr/src/nip28/Nip28Service.ts` (NIP‑28 – early implementation)
    - Helpers: `packages/nostr/src/nips/nip02.ts`, `nip04*.ts`, `nip05.ts`, `nip09.ts`, `nip19.ts`, `nip44*.ts`

- Relay package: `packages/relay`
  - Effect-based relay with PlanetScale + Drizzle ORM, Psionic integration, and NIP‑01 support.
  - WebSocket handling, subscription management, and real‑time broadcasting.
  - Tests under `packages/relay/test/` including WebSocket and database suites.

## Key Commits (Evidence)

- Implement NIP‑01 library (Effect): 2bd065d6
  - “feat(nostr): Implement NIP‑01 core protocol with relay communication (#914)”
  - Introduced `packages/nostr` with schemas, services, and tests, plus an EphemeralRelay for testing.
  - Files added include: `packages/nostr/src/core/Schema.ts`, `packages/nostr/src/nip01/index.ts`, services for Crypto/Event/WebSocket/Relay, and test suites.

- Implement NIP‑06 (Effect service): a960be46
  - “feat: Implement NIP‑06 Effect service for key derivation (#928)”
  - Added `packages/nostr/src/nip06/Nip06Service.ts` and comprehensive test vectors.
  - Reorganized schemas by NIP to avoid circular imports; all tests passing (noted in commit).

- Effect-based Relay with PlanetScale: 72d64da3
  - “feat: Implement Nostr relay with PlanetScale database and Effect.js architecture (#997)”
  - Created `packages/relay/` with DB schema, WebSocket relay, Psionic plugin, and 20+ passing tests.

- App integration milestones
  - 9898a775: “Replace mock agent identities with real Nostr profiles (#1002) (#1003)”
  - 63b11dc4: “Connect agent-chat component to live Nostr relay (#1006)”
  - 7ba951b2: “Implement full NIP‑28 channel support (Issue #1000) (#1001)”

- Removal / reset
  - 2592ced3: “Clean up unused packages: remove storybook, pylon, and playground (#995)” – removed `apps/pylon` (early relay app), Storybook, Playground.
  - 7b3c985d: “Zero base” – removed the `packages/nostr/` library (and many monorepo artifacts). The deletion diff shows a broad set of Nostr files, including NIP‑01/06/28 and helper NIPs.

## Where To Find It (in Git History)

You can inspect the full implementation by checking out the commits above. Examples:

- Show NIP‑01 addition (files list):
  - `git show --name-only 2bd065d6`
- Browse the NIP‑06 service:
  - `git show --name-only a960be46`
- Browse the relay package:
  - `git show --name-only 72d64da3`
- Confirm removal of `packages/nostr`:
  - `git show --name-only 7b3c985d`

Related internal logs created during the work:
- `docs/logs/20250604/2340-log.md`
- `docs/logs/20250604/2427-log.md`
- `docs/logs/20250615/2505-nip06-log.md`
- `docs/logs/20250619/2424-relay-db-log.md`

## Design Notes (Effect Library)

- Type safety: Extensive use of `@effect/schema` with branded types for EventId, PubKey, Sig, Tag, etc.
- Services: CryptoService (keygen/sign/verify), EventService (creation/validation), WebSocketService (connections), RelayService (REQ/CLOSE/EVENT), RelayPoolService, RelayReconnectService.
- Testing: EphemeralRelay enabled in‑memory end‑to‑end tests; vitest suites covered NIP‑01 flow and pool behavior.
- Extensibility: Modular structure by NIP; early NIP‑28 channels; NIP‑06 key derivation end‑to‑end (mnemonic → npub/nsec).

## Why It’s Not Here Now

- The repo later pivoted to ship a desktop/iOS experience first and adopted a Swift `nostr-sdk-ios` fork to accelerate Nostr support on Apple platforms.
- Evidence of the new direction:
  - iOS references to the fork in package manifests:
    - `ios/OpenAgentsCore/Package.swift`
    - `ios/OpenAgentsCore/Sources/OpenAgentsCore/Nostr/NostrRelayManager.swift`
  - Planning docs describing the fork and rationale:
    - `docs/compute/issues/COMPLETED.md` (see “nostr-sdk-ios Fork Decision”)
    - `354176ec` updated `docs/compute/issues/*` with the fork strategy.

This change reduced duplication and let us leverage 25+ already‑implemented NIPs on iOS/macOS while the TypeScript stack moved toward ACP/Tauri.

## Current Status

- Effect-based Nostr library (`packages/nostr`) and the Effect relay (`packages/relay`) are not present in the current working tree; they exist only in the Git history.
- The app currently integrates Nostr via the `nostr-sdk-ios` fork for Apple platforms and uses Tauri/React on desktop per current architecture.

## If We Want To Revive It

- Starting point: check out commit `2bd065d6` and rebase the `packages/nostr` folder into a new branch.
- Minimal updates likely needed:
  - Align to current `@effect/*` versions and project references.
  - Re-enable tests under `packages/nostr/test` and `packages/relay/test`.
  - Decide on target runtimes (Node/Browser/Cloudflare) and WS layers.
- Alternatively, wrap the Swift SDK with a local bridge for desktop if we want a single code path without fully restoring the TS library.

---

Prepared: 2025‑11‑28

Maintainer notes: If you need deeper diffs or to extract specific files from history, ping and I can script a targeted `git checkout <sha> -- packages/nostr/**` into a separate branch for review.

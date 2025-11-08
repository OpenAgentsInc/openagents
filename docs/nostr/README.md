# Nostr Integration — Current State and How It Works

Last updated: 2025-11-07

This document summarizes the current Nostr SDK integration in OpenAgents, how it’s wired, and how to work with it locally without pulling Nostr into the iOS app.

## What’s Working

- Local path SwiftPM dependency to the OpenAgents fork of the Nostr SDK
  - Path: `/Users/christopherdavid/code/nostr-sdk-ios`
  - Editable locally; no re-import needed — updates are picked up via package resolution
- New macOS-only wrapper target `OpenAgentsNostr`
  - Declared in `ios/OpenAgentsCore/Package.swift`
  - Depends on `NostrSDK` (from the local path package)
  - Minimal shim file compiles on macOS; is a no-op on iOS
- `OpenAgentsCore` conditionally depends on `OpenAgentsNostr` only for macOS builds
  - iOS builds of OpenAgents do not link or import the Nostr SDK
- Project builds on macOS with the local SDK linked

## What’s Not Implemented Yet

- Runtime Nostr client usage (RelayPool connect, filters, subscriptions)
- Marketplace NIPs in code (NIP-57 Zaps, NIP-89 Handlers, NIP-90 DVM)
  - These are specified in `docs/compute/issues/phase-1-mvp/001-nostr-client-library.md`
- Key storage and signer protocol (decision pending: macOS Keychain vs iOS Keychain or both)

## How It’s Wired

Layering
- `OpenAgentsNostr` (new SPM target): wraps `NostrSDK` and is where macOS-only Nostr code should live
- `OpenAgentsCore`: shared code; conditionally depends on `OpenAgentsNostr` on macOS only
- App target: unchanged source-wise; iOS remains free of any Nostr imports

Key files
- `ios/OpenAgentsCore/Package.swift` (products, dependencies, conditional target dependency)
- `ios/OpenAgentsCore/Sources/OpenAgentsNostr/NostrShim.swift` (minimal shim)

## Local Development Workflow

Edit the SDK
- Work in `/Users/christopherdavid/code/nostr-sdk-ios`

Refresh dependencies (no re-import)
- `cd ios && xcodebuild -resolvePackageDependencies -project OpenAgents.xcodeproj -scheme OpenAgents`

Build (macOS)
- `cd ios && xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -sdk macosx -configuration Debug`

Switching package location
- Absolute path (current): edit `ios/OpenAgentsCore/Package.swift`
- Relative path (portable): e.g., `.package(path: "../../nostr-sdk-ios")`
- Git URL/branch (remote): e.g., `.package(url: "https://github.com/OpenAgentsInc/nostr-sdk-ios.git", branch: "main")`
- Submodule: add under `ios/Packages/nostr-sdk-ios` and reference by path

## Design Choices (Why macOS-first)

- Keep iOS clean until we explicitly decide it should run a Nostr client
- Avoid App Store background-execution and downloaded-code pitfalls (see Apple compliance notes below)
- Let macOS host the Nostr client and worker-side responsibilities first

## Next Steps (per Issue #1459)

- Implement marketplace NIPs in the fork: NIP-57, NIP-89, NIP-90
- Add a thin `NostrClient` wrapper inside `OpenAgentsNostr` (connect, subscribe, publish)
- Define a signer protocol in `OpenAgentsCore` and implement macOS signer with Keychain + Secure Enclave
- Decide on key location(s): macOS-only initially; optionally add iOS later
- Optional: split app into separate iOS/macOS targets to avoid resolving Nostr SDK on iOS entirely

## Apple Compliance Notes (Summary)

- iOS: no Nostr linkage/imports; coordination-only remains the plan
- macOS: Nostr client acceptable; follow general privacy/safety principles
- Foundation Models AUP: policy module filters prohibited job types (see `docs/compute/apple-terms-research.md`)

## Troubleshooting

- Package not found: verify path in `ios/OpenAgentsCore/Package.swift` matches your local clone
- Xcode not picking changes: run resolve step or clean build folder; ensure the scheme targets macOS when building locally
- iOS build errors referencing Nostr: confirm the conditional dependency remains `platforms: [.macOS]` only

## References

- Issue: Nostr Client Library (Swift) — `docs/compute/issues/phase-1-mvp/001-nostr-client-library.md`
- Apple terms and constraints — `docs/compute/apple-terms-research.md`
- ADR overview — `docs/adr/README.md`


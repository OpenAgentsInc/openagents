# Khala App — API Handshake Audit

> **Honest-scope header.** This is an engineering status audit, not a product
> promise. It records what the native Khala iOS app actually does against the
> live `openagents.com` Khala API as of the date below, the exact gap that
> blocked a voice-free end-to-end test, and the change that closed it. Live
> behavior is only what the running app and the live API actually do.

- **Date:** 2026-06-26
- **App:** `clients/khala-ios/Khala/` (native SwiftUI, bundle `com.openagents.khala`)
- **Spec:** `docs/mobile/2026-06-26-khala-voice-app-spec.md`
- **Toolchain:** Xcode 26.5 (build 17F42), iOS 17 deployment target

## What the owner wants

A real round-trip: a message sent from the mobile app reaches the Khala API and
returns a response. Hardcoded key / minimal UI is acceptable for v1 — the point
is the **end-to-end handshake** working
(mobile → `POST /api/v1/chat/completions` → response back).

## Handshake status (before this change)

The scaffold was **more complete than its status note implied**. The full HTTP
round-trip already existed and the voice path already wired it up. What was
missing was a **voice-free way to exercise it**, which made the handshake
effectively untestable in the simulator or before microphone/speech permission.

### Already present and correct

- `Net/KhalaClient.swift`
  - `complete(prompt:apiKey:)` — `POST https://openagents.com/api/v1/chat/completions`,
    body `{ "model": "openagents/khala", "messages": [{role:"user", content:…}] }`,
    `Authorization: Bearer <key>`, decodes `choices[0].message.content`.
  - `mintFreeKey()` — `POST https://openagents.com/api/keys/free`, reads
    `credential.token`.
  - Typed errors incl. explicit **HTTP 402 → `quotaExceeded`** ("free quota
    reached") and a generic non-2xx path.
- `Store/KeychainStore.swift` — key stored in the iOS Keychain
  (service `com.openagents.khala`), never `UserDefaults`/plist. Mint and paste
  both persist here; `SettingsView` exposes mint / paste / remove plus the
  honest free-tier data-sharing disclosure.
- `Voice/VoiceController.send(_:)` — already loaded the key, set `.thinking`,
  called `KhalaClient.complete`, rendered the response, and (optionally) spoke
  it. So the voice path **already round-trips**.
- Baseline `xcodebuild` for the simulator: **BUILD SUCCEEDED**.

### The exact gap

The **only** entry point to `send(_:)` was the push-to-talk pipeline
(`AVAudioEngine` capture → `SFSpeechRecognizer` STT → `send`). `send` was
`private`, and `ContentView` had no text field. Consequences:

- No way to trigger the round-trip without granting mic + speech permission and
  physically speaking — i.e. not testable in the simulator or in CI-like flows.
- The "type/say a message → call the API → show the response" path the owner
  asked for had no **type** half.

Auth handling, request/response decode, and 402 handling were **not** gaps —
they were already implemented.

## Change in this commit (gap closed)

Added a minimal, voice-free text-input path that reuses the exact same
round-trip the voice path uses:

- `VoiceController.sendText(_:)` (new, non-private) — trims input, guards on
  busy state, sets the transcript, and calls the shared `send(_:)`. No new
  network code; it funnels into the identical `KhalaClient.complete` call.
- `ContentView` composer — a `TextField` (+ send button, `.onSubmit` send),
  disabled when there is no key or the controller is busy, that calls
  `sendText`. Sits alongside the push-to-talk button so both paths are visible.

Rebuild after the change: **BUILD SUCCEEDED** (simulator,
`iPhone` iOS-17 destination).

## Is a real mobile → API round-trip now achievable?

**Yes.** Concrete steps to demonstrate the handshake:

1. Open `clients/khala-ios/Khala/Khala.xcodeproj` in Xcode (or
   `xcodebuild … -scheme Khala`), run on an iOS 17+ simulator.
2. Tap the gear → **Mint a free key** (calls `POST /api/keys/free`, stores the
   `oa_agent_…` token in the Keychain) — or paste an existing key.
3. Type a message in the composer and tap send (no mic needed).
4. The app issues `POST /api/v1/chat/completions` with `model: openagents/khala`
   and renders `choices[0].message.content`. Over-quota shows the 402 message.

The voice path (hold-to-talk → STT → same `send`) reaches the same endpoint on a
physical device or a simulator with microphone input.

## Remaining / follow-ups (tracked as issues)

- **(a)** Round-trip hardening: a small in-repo smoke/contract check for
  `KhalaClient` (request shape, 402 decode, content extraction) so the handshake
  cannot silently regress. The wire path itself works today.
- **(b)** The text-input composer landed here; an issue tracks any UX polish
  (multi-turn, clearing, error affordances) on top of it.
- **(c)** **Codex steering from the app (follow-up the owner is excited about):**
  eventually let the app dispatch a `codex_agent_task` through the gateway (the
  Khala → Pylon → Codex delegation path) from a mobile request, not just a plain
  chat completion. v1 is intentionally a plain `openagents/khala` chat round-trip;
  this is a roadmap item, not v1 scope.

> Note: per the repo strict-bug issue policy, GitHub issues are normally for
> concrete reproducible bugs only. These handshake items are implementation
> tasks; the owner explicitly directed opening issues for this work, overriding
> the strict-bug default.

# Retirement — Autopilot Remote Control (Expo mobile app)

- **Date:** 2026-06-26
- **Retired path:** `clients/khala-ios/AutopilotRemoteControl` (deleted from the
  repo in this change)
- **Replaced by:** `clients/khala-ios/Khala` (new native SwiftUI voice app)
- **Spec for the replacement:** `docs/mobile/2026-06-26-khala-voice-app-spec.md`

## What it was

`AutopilotRemoteControl` was the Expo (React Native) single-operator mobile
client for observing and steering a paired Pylon Autopilot Coder node
(roadmap CL-4 / issue #4906). It used `@openagentsinc/autopilot-control-protocol`,
`expo-secure-store` for pairing credentials, MMKV for non-secret state, and our
local-build + own-OTA delivery path (`updates.openagents.com`, never EAS).

## Why it was retired

The owner is retiring the Expo React-Native mobile surface entirely and
replacing it with a **brand-new, very basic native Swift voice app ("Khala")**.
The new app's only job is push-to-talk voice → on-device speech-to-text → the
Khala API → show/speak the response. It uses **native Swift APIs only** — no
Expo / React Native / EAS, and no Pylon remote-control / operator features.

This is a clean break, not a port: none of the React-Native code, the
remote-control protocol wiring, or the Expo/OTA delivery machinery carries over.
The new app is pure local Xcode (see the spec, Section 8).

## What replaces each concern

| Retired concern | Replacement |
| --- | --- |
| Expo RN app shell | Native SwiftUI app (`clients/khala-ios/Khala`) |
| Pylon pairing / node steering / projections | **Dropped** (out of scope for Khala v1) |
| `expo-secure-store` for credentials | iOS Keychain (`Security`) |
| MMKV non-secret state | none in v1 |
| Local build + own OTA (`updates.openagents.com`) | Local Xcode build only; no OTA (native Swift can't OTA) |
| Bundle id `com.openagents.autopilot-mobile` | `com.openagents.khala` |

## References updated for the retirement

Tracked references in this repo that pointed at the old path were updated to
note the retirement and point at the Khala app/spec where relevant. Several of
those are dated historical audits (e.g. `docs/launch/…`, `docs/autopilot-coder/…`)
that describe past state; those are left as historical record and were not
rewritten. The active places that should not dangle (repo-root `AGENTS.md`
mobile policy, the new app's own docs) reference Khala.

## Owner-gated follow-ups

- **App Store Connect:** a new app record + bundle id `com.openagents.khala`
  under Apple Team `HQWSG26L43` is owner-gated. The retired app's ASC record
  (`com.openagents.autopilot-mobile`, ascAppId `6779949704`) is not reused.
- **Root workspace `~/work/CLAUDE.md`** still references
  `clients/khala-ios/AutopilotRemoteControl` as "the current mobile operator path."
  That file lives in a **different repo** (the root workspace), so it was not
  edited here; it should be updated separately to point at `clients/khala-ios/Khala`
  (or to note that the mobile operator surface was retired).

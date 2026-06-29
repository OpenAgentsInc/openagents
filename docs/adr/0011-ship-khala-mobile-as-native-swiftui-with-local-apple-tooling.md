---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, docs/DEPLOYMENT.md, docs/mobile/2026-06-26-khala-voice-app-spec.md, docs/mobile/2026-06-26-autopilot-remote-control-retirement.md
informed: OpenAgents contributors, agents, and release operators
---

# Ship Khala mobile as native SwiftUI with local Apple tooling

## Context and Problem Statement

The repository previously carried an Expo React Native mobile surface for
remote control. That surface was retired. The current Khala iOS client is the
native SwiftUI app under `clients/khala-ios/Khala`, and mobile release
instructions require local Xcode tooling and Apple-native upload paths.

## Decision Drivers

* Keep mobile builds on owner-controlled local tooling.
* Avoid Expo/EAS cloud build, submit, or update paths.
* Make native SwiftUI the current Khala iOS implementation boundary.
* Prevent OTA assumptions from leaking into a native app that has no Expo
  update path.

## Considered Options

* Native SwiftUI Khala iOS app with local Xcode and Apple upload tooling
* Revive the retired Expo React Native app
* Use Expo/EAS cloud for native build, submit, or OTA updates

## Decision Outcome

Chosen option: "Native SwiftUI Khala iOS app with local Xcode and Apple upload
tooling", because root instructions and mobile docs settle the mobile product
boundary on `clients/khala-ios/Khala`, local `xcodebuild`, and Apple-native
TestFlight upload, with no Expo/EAS cloud path.

### Consequences

* Good, because the mobile build and signing chain stays on local controlled
  infrastructure.
* Good, because contributors do not need to reason about Expo prebuild or OTA
  behavior for the current Khala iOS app.
* Bad, because JavaScript-only OTA release techniques do not apply to native
  SwiftUI app changes.

### Confirmation

Compliance is confirmed by root mobile policy, the Khala voice app spec, the
Autopilot Remote Control retirement record, the TestFlight release runbook, and
code review rejecting `eas build`, `eas submit`, or `eas update` for current
mobile release work.

## Pros and Cons of the Options

### Native SwiftUI Khala iOS app with local Xcode and Apple upload tooling

* Good, because it matches the current repository app and release policy.
* Good, because it avoids cloud build dependency for mobile releases.
* Bad, because release operators must have the local Apple signing environment.

### Revive the retired Expo React Native app

* Good, because Expo can be fast for JavaScript UI iteration.
* Bad, because the retired app is no longer the current product boundary.

### Use Expo/EAS cloud for native build, submit, or OTA updates

* Good, because hosted build and OTA flows are convenient.
* Bad, because root policy explicitly forbids Expo/EAS cloud for the current
  mobile app.

## More Information

* `AGENTS.md` ("Mobile build/ship policy")
* `docs/DEPLOYMENT.md`
* `docs/mobile/2026-06-26-khala-voice-app-spec.md`
* `docs/mobile/2026-06-26-autopilot-remote-control-retirement.md`
* `docs/mobile/2026-06-26-khala-testflight-release-runbook.md`

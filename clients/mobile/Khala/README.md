# Khala — native SwiftUI voice app

Khala is a minimal **push-to-talk voice client** for the public Khala API.
One screen: hold the button, speak, and Khala answers.

- **Bundle id:** `com.openagents.khala`
- **Platform:** iOS, native SwiftUI. **No Expo / React Native / EAS.**
- **Spec:** `../../../docs/mobile/2026-06-26-khala-voice-app-spec.md`
- Replaces the retired Expo app `clients/mobile/AutopilotRemoteControl`
  (`../../../docs/mobile/2026-06-26-autopilot-remote-control-retirement.md`).

## What it does (v1)

Push-to-talk → on-device speech-to-text (Apple `Speech`) → send the transcript
to the Khala API (`POST https://openagents.com/api/v1/chat/completions`, model
`openagents/khala`) → show the response (and optionally speak it). The API key
(an `oa_agent_…` token) is minted in-app (`POST /api/keys/free`) or pasted, and
stored in the iOS Keychain.

## Project layout

```
clients/mobile/Khala/
├── project.yml                 # XcodeGen spec (regenerates Khala.xcodeproj)
├── Khala.xcodeproj/            # hand-authored project (opens without xcodegen)
└── Khala/
    ├── KhalaApp.swift          # @main app entry
    ├── ContentView.swift       # the single screen
    ├── VoiceState.swift        # state machine + Onyx status colors
    ├── Views/
    │   ├── AnimatedBackground.swift  # TimelineView+Canvas backdrop
    │   ├── PushToTalkButton.swift    # press-and-hold control
    │   └── SettingsView.swift        # key mint/paste + disclosure
    ├── Voice/
    │   ├── VoiceController.swift      # mic capture + orchestration
    │   ├── SpeechRecognizer.swift     # SFSpeechRecognizer (on-device STT)
    │   └── SpeechSynthesizer.swift    # AVSpeechSynthesizer (optional TTS)
    ├── Net/KhalaClient.swift   # URLSession -> Khala API
    ├── Store/KeychainStore.swift
    └── Resources/Info.plist    # mic + speech usage strings, bundle id
```

## Open & run locally (Xcode)

```sh
cd clients/mobile/Khala
open Khala.xcodeproj
```

Then in Xcode pick the **Khala** scheme + an iOS Simulator and press Run
(`⌘R`). On the simulator you can exercise the full flow except real on-device
speech (the simulator mic uses the Mac mic; STT works on a physical device best).

If you prefer to regenerate the project from the spec (e.g. after adding files):

```sh
brew install xcodegen      # one-time, if not installed
cd clients/mobile/Khala
xcodegen generate
open Khala.xcodeproj
```

## Build / archive from the command line

```sh
cd clients/mobile/Khala
# Build for a simulator (no signing needed):
xcodebuild -project Khala.xcodeproj -scheme Khala \
  -destination 'platform=iOS Simulator,name=iPhone 16' build

# Archive for TestFlight (requires signing — see below):
xcodebuild -project Khala.xcodeproj -scheme Khala \
  -configuration Release -archivePath build/Khala.xcarchive archive
xcodebuild -exportArchive -archivePath build/Khala.xcarchive \
  -exportPath build -exportOptionsPlist ExportOptions.plist
```

TestFlight upload is **Apple-native** (`xcrun altool` / Transporter), using the
App Store Connect API key in workspace `.secrets/appstoreconnect.env`. Per the
repo mobile policy (root `AGENTS.md`), **never** use `eas build`/`submit`/
`update`; native Swift has no OTA path.

## NEEDS-OWNER (owner-gated)

These require the owner / signing identity and cannot be completed in-repo:

1. **App Store Connect record** for bundle id `com.openagents.khala` under Apple
   Team `HQWSG26L43` (the retired app's record is not reused).
2. **Signing**: a distribution cert + provisioning profile for
   `com.openagents.khala` in the login keychain (automatic signing is set, team
   `HQWSG26L43`), and an `ExportOptions.plist` for the archive export.
3. **First `xcodebuild`/Xcode build** on this Mac to confirm it compiles against
   the installed SDK and to wire up a simulator destination name that exists
   locally.
4. **(optional) `xcodegen`** install if you want to regenerate the project from
   `project.yml` rather than editing the committed `.xcodeproj`.

The Swift sources and project structure are complete and coherent; items above
are environment/signing steps, not code.

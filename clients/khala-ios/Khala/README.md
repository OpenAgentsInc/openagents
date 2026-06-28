# Khala — native SwiftUI app (ChatGPT-style)

Khala is a **ChatGPT-style mobile client** for the public Khala API with a
retained **push-to-talk voice visualization**. A left slide-over drawer holds
chat history; the main surface is a chat `NavigationStack`.

- **Bundle id:** `com.openagents.khala`
- **Platform:** iOS 17+, native SwiftUI. **No Expo / React Native / EAS.**
- **Spec:** `../../../docs/mobile/2026-06-26-khala-chatgpt-style-app-spec.md`
  (voice-runtime reference: `../../../docs/mobile/2026-06-26-khala-voice-app-spec.md`)
- Replaces the retired Expo app `clients/khala-ios/AutopilotRemoteControl`
  (`../../../docs/mobile/2026-06-26-autopilot-remote-control-retirement.md`).

## What it does

Chat with `openagents/khala` (`POST https://openagents.com/api/v1/chat/completions`)
— typed or push-to-talk → on-device speech-to-text (Apple `Speech`). Conversations
persist **locally on device** (SwiftData) and show up in the drawer Recents
(new / rename / delete, sorted by most-recent). The API key (an `oa_agent_…`
token) is minted in-app (`POST /api/keys/free`) or pasted, stored in the iOS
Keychain.

## Xcode project: file-system-synchronized group

`Khala.xcodeproj` uses an Xcode 16 `PBXFileSystemSynchronizedRootGroup` for the
`Khala/` (and `KhalaTests/`) folders. **New Swift files added under `Khala/` are
compiled automatically — no per-file `project.pbxproj` edit is required.** This
keeps parallel feature lanes free of `pbxproj` merge conflicts. (`Resources/Info.plist`
is excluded from the synchronized build via a membership exception.)

## Project layout

```
clients/khala-ios/Khala/
├── project.yml                 # XcodeGen spec (regenerates Khala.xcodeproj)
├── Khala.xcodeproj/            # sync-group project (opens without xcodegen)
└── Khala/
    ├── KhalaApp.swift          # @main app entry (creates ConversationStore)
    ├── ContentView.swift       # thin wrapper around RootView (preview/compat)
    ├── VoiceState.swift        # state machine + Onyx status colors
    ├── Shell/
    │   ├── RootView.swift            # app shell: chat NavigationStack + drawer
    │   ├── DrawerContainer.swift     # left slide-over (scrim + drag, RM-safe)
    │   ├── DrawerContentView.swift   # Recents/search/New Chat (#6344 seam)
    │   └── ChatView.swift            # chat surface + composer + voice (#6345 seam)
    ├── Model/
    │   ├── Conversation.swift        # SwiftData @Model Conversation/Message
    │   └── ConversationStore.swift   # local persistence façade
    ├── Views/
    │   ├── AnimatedBackground.swift  # TimelineView+Canvas backdrop
    │   ├── PushToTalkButton.swift    # press-and-hold control
    │   └── SettingsView.swift        # key mint/paste + disclosure
    ├── Voice/
    │   ├── VoiceController.swift      # mic capture + orchestration
    │   ├── SpeechRecognizer.swift     # SFSpeechRecognizer (on-device STT)
    ├── Net/KhalaClient.swift   # URLSession -> Khala API
    ├── Store/KeychainStore.swift
    └── Resources/Info.plist    # mic + speech usage strings, bundle id
```

### Env demo / test hooks (env-gated; no-op in normal use)

- `KHALA_API_KEY` — inject the bearer key (skips Keychain) for simulator smokes.
- `KHALA_DEMO_PROMPT` — auto-send a prompt on launch to exercise the round-trip.
- `KHALA_SKIP_PERMISSIONS` — skip the mic/speech prompt for launch-render
  screenshots / CI (real users still get prompted on first push-to-talk).

## Open & run locally (Xcode)

```sh
cd clients/khala-ios/Khala
open Khala.xcodeproj
```

Then in Xcode pick the **Khala** scheme + an iOS Simulator and press Run
(`⌘R`). On the simulator you can exercise the full flow except real on-device
speech (the simulator mic uses the Mac mic; STT works on a physical device best).

If you prefer to regenerate the project from the spec (e.g. after adding files):

```sh
brew install xcodegen      # one-time, if not installed
cd clients/khala-ios/Khala
xcodegen generate
open Khala.xcodeproj
```

## Build / archive from the command line

```sh
cd clients/khala-ios/Khala
# Build for a simulator (no signing needed):
xcodebuild -project Khala.xcodeproj -scheme Khala \
  -destination 'platform=iOS Simulator,name=iPhone 16' build

# Archive for TestFlight (requires signing — see below):
xcodebuild -project Khala.xcodeproj -scheme Khala \
  -configuration Release -archivePath build/Khala.xcarchive archive
xcodebuild -exportArchive -archivePath build/Khala.xcarchive \
  -exportPath build -exportOptionsPlist ExportOptions.plist
```

## macOS Apple FM packaging gate

Khala macOS builds that advertise Apple FM support must bundle the Pylon
Foundation Models helper at:

```text
Khala.app/Contents/Resources/app/apple-fm-bridge/foundation-bridge
```

The Xcode target runs `scripts/copy-packaged-apple-fm-bridge.sh` as a macOS-only
post-build phase. It copies `apps/pylon/bin/foundation-bridge` by default; build
it first with:

```sh
bash ../../../apps/pylon/swift/foundation-bridge/build.sh
```

Before signing/notarization, run:

```sh
bun scripts/verify-packaged-apple-fm-bridge.ts /path/to/Khala.app
```

The verifier fails if the helper is missing, empty, or not executable. To ship an
intentional Apple-FM-less build, set `KHALA_SKIP_APPLE_FM_BRIDGE_CHECK=1` during
the Xcode build and during verification; the copy phase writes an explicit
`APPLE_FM_UNAVAILABLE.txt` marker into the bundle instead of silently omitting
the helper.

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

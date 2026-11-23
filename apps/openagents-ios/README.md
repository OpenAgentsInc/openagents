# OpenAgents for iOS

OpenAgents for iOS is a dedicated, first-class mobile client exclusively for **Antigravity over Tailscale** (GitHub `#13993`).

- **Architecture (Shared Cross-Apple SwiftUI + Lean Coax `#authority` Core with OpenAgents macOS):**
  - **Shared Engine (`apps/shared/antigravity/`):** Compiled Coax C core (`client-core.coax` → `Generated/client_core.c` + `client-core-bridge.{h,c}`) and cross-Apple Swift engine (`AntigravityStore`, `TailscaleClient`, `DeviceIdentityStore`, `AntigravityModels`, `TranscriptComponents`, `WorkspaceInspectorView`, `SubagentInspectorView`) shared 1:1 with `apps/openagents-macos/`.
    - NIP-44 sealing/opening & secp256k1 identity (`apps/shared/app/device/seal.coax`)
    - Tailscale admission, relay-free direct grants, direct transport & real-time push frames (`coax/lib/nostr/host/tailnet.coax`)
    - Environment runtime wire with Review Mode (`auto`/`default`/`plan`) & Workspace Git/PR wire (`coax/lib/agent/environment/wire.coax`, `coax/lib/agent/workspace/wire.coax`)
    - `#authority` Antigravity-only model & effort policy (`coax/lib/agent/environment/phone-agent-policy.coax`) — non-Antigravity models are refused at the wire boundary before sealing.
  - **Native iOS/iPadOS UI (`Sources/`):** Pure-black OLED (`#000000`, `#262626` hairlines) SwiftUI shell with adaptive iPad split sidebar, native keyboard docking, `+` Attachments (`PhotosPicker` & `fileImporter`), `/` Slash Commands, interactive Approval/Question cards, Subagent Inspector sheet, Workspace Diff/Branches/Worktrees/PRs sheet, and one-tap Pair over Tailscale.
  - **Zero bloat:** No JavaScript, WebKit, Metal GPU canvas, cloud accounts, Passkeys, public relays (`relay.openagents.com`), Claude Code, Inkling, Foreman, or Projects in `OpenAgents.app`. Enforced mechanically by `apps/openagents-ios/check.sh`.

Bundle ID `com.openagents.app`, team `HQWSG26L43`, version `1.0.0`.

## Files

| File | What it is |
| --- | --- |
| `mobile-core.coax` | Lean compiled Coax C root: `seal.coax` + `tailnet.coax` + `phone-agent-policy.coax` + policy-enforced `EaRequest`/`EaView` helpers, with `#test` checks |
| `mobile-core-bridge.{h,c}` | Opaque C ABI shim over `Generated/mobile_core.h` and `apps/shared/native/tailnet-codec.c` (heap-allocated `oa_mobile_work_t`; no 64 MB stack hack) |
| `Bridge.h` | Swift bridging header exposing `mobile-core-bridge.h` and `tailnet-codec.h` |
| `Sources/OpenAgentsApp.swift` | `@main` SwiftUI entry point enforcing dark OLED theme and `scenePhase` lifecycle |
| `Sources/Theme.swift` | Pure-black OLED color tokens (`#000000`, `#262626`), Antigravity mark, age formatting |
| `Sources/MobileCoreBridge.swift` | Swift wrapper over the Coax C bridge (NIP-44 seal/open, Antigravity model/effort normalization, `EaRequest` encoding, `EaView` validation) |
| `Sources/KeychainStore.swift` | Device-only Keychain storage (`com.openagents.app.auth`: `own-computer-key-v1` and `own-computer-paired-v1`) + `AppTailnetBook` + per-computer Antigravity defaults |
| `Sources/AntigravityModels.swift` | View models (`ComputerEntry`, `ChatSessionSummary`, `TranscriptItem`, `AntigravityModelOption`, `ThinkingEffort`, `ReviewMode`) and `EaView` JSON parser with Coax-backed Antigravity filtering |
| `Sources/TailscaleClient.swift` | `Network.framework` Bonjour discovery (`_openagents-coder._tcp`), `NWPathMonitor`, manual MagicDNS/IP probing, and direct Tailscale TCP NIP-44 RPC over port `47109` |
| `Sources/AppStore.swift` | `@MainActor` store coordinating computers, sessions (`Working` / `Recents`), transcript, Antigravity composer preferences, and RPC calls |
| `Sources/RootView.swift` | Top navigation bar, status pill, and slide-over sidebar container |
| `Sources/SidebarView.swift` | Calm pure-black sidebar: `New chat`, search, collapsible `Working`, flat chronological `Recents`, and `Computers` footer |
| `Sources/ChatView.swift` | Native transcript (selectable Markdown, copyable code blocks, collapsible Thinking/Tools/Diffs), Tailscale onboarding empty state, and sticky composer with Model / Thinking / Review Mode chips |
| `Sources/ComputersView.swift` | Paired/discovered Tailscale computers, live reachability & refusal diagnostics (`tagged`, `not_owner`, `not_tailnet`, `unavailable`), manual MagicDNS/IP add, and local secp256k1 device key |
| `Tests/MobileCoreTests.swift` | Unit tests covering Coax C bridge, Antigravity-only policy enforcement, NIP-44 RPC sealing, Tailscale direct codec, and `EaView` filtering |
| `UITests/AntigravityUITests.swift` | Simulator UI test verifying pure-black Antigravity shell, sidebar, Computers sheet, composer controls, and absence of legacy Login/Projects/Foreman UI |
| `project.yml` | XcodeGen specification (`OpenAgents`, `OpenAgentsTests`, `OpenAgentsUITests`) |
| `generate.sh` | Compiles `mobile-core.coax` to `Generated/mobile_core.{c,h}` and checks the 512 KB stack-frame budget at `-Os` and `-Os -fno-inline` |
| `build.sh` | Runs `generate.sh`, `xcodegen generate`, and builds `OpenAgents.app` for iOS Simulator |
| `check.sh` | Fails if `OpenAgents.app` links JS/WebKit, bundles `.js`/`.wasm`/`.html`/`.cnu`/`.metal`, or contains legacy GPU/AppCore/account/relay symbols or URLs |
| `release.sh` | Archives, runs `check.sh`, exports, and uploads to App Store Connect (`tools/app_store.py`) |
| `OpenAgents/` | `Assets.xcassets` (`AppIcon`), `Info.plist` (`NSBonjourServices`, `UIUserInterfaceStyle = Dark`), and minimal `OpenAgents.entitlements` |

Shared code compiled into the iOS app: `apps/shared/native/AppTailnet.swift` and `apps/shared/native/tailnet-codec.{h,c}`.

> **Note on unlinked legacy harness files:** Legacy GPU-page files (`chat.coax`, `chat-library-checks.coax`, `app.coax`, `editor.coax`, `composer_screen.coax`, `host.{c,h}`, `core-host.{c,h}`, `Tests/IdentityTests.swift`, `Tests/tester/catalog.coax`) remain temporarily on disk outside `project.yml` and `OpenAgents.app` only because out-of-tree shared monorepo checks (`coax/native/{project,usage}-screen-check.coax`, `apps/shared/tester/cli.coax`, `openagents/book/chapters.json`) still reference them. None of them are compiled or linked into the iOS app.

## Build, check, and test

```sh
# 1. Coax unit tests & #authority mutation check
coax/bin/coax test apps/openagents-ios/mobile-core.coax
coax/bin/coax mutate coax/lib/agent/environment/phone-agent-policy.coax

# 2. Generate C bridge, build iOS Simulator app, and verify clean binary invariants
apps/openagents-ios/build.sh -destination 'generic/platform=iOS Simulator'
apps/openagents-ios/check.sh

# 3. Run unit tests & UI tests on an iOS Simulator (isolated bundle ID)
xcodebuild test -project apps/openagents-ios/OpenAgents.xcodeproj -scheme OpenAgentsTests \
  -configuration Debug -sdk iphonesimulator -destination 'platform=iOS Simulator,name=Coax Native Starter' \
  -derivedDataPath apps/openagents-ios/build CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- \
  OA_IOS_BUNDLE_ID=com.openagents.app.test

xcodebuild test -project apps/openagents-ios/OpenAgents.xcodeproj -scheme OpenAgentsUITests \
  -configuration Debug -sdk iphonesimulator -destination 'platform=iOS Simulator,name=Coax Native Starter' \
  -derivedDataPath apps/openagents-ios/build CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- \
  OA_IOS_BUNDLE_ID=com.openagents.app.test
```

## Release

```sh
apps/openagents-ios/release.sh /absolute/fresh/output/directory
python3 apps/openagents-ios/tools/app_store.py status --build BUILD
```

The version and build number are `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION` in `project.yml`. Credentials are read from `~/work/.secrets/appstoreconnect.env`.

# OpenAgents iOS Host Integration

OpenAgents iOS runs **100% in OpenAgents Cloud mode** on the shared **Coax / Metal GPU** engine (`ios-app.coax` → `gpu-page.coax`) with pure-black OLED SwiftUI modal sheets for Account, Spark Wallet, Environments & Projects, and Usage.

## Architecture

| Layer | Files | Responsibility |
| --- | --- | --- |
| Coax root & `#authority` policies | `ios-app.coax`, `gpu-page.coax` | Compiles Spark wallet & NIP-06 policies, Cloud Chat v2 wire codec, Tier 1 Cloud Tools allowlist, Auto placement policy, Full Auto, Voice Duplex VAD/speech sanitizer, atlas tables, and the iOS Metal GPU surface into `Generated/gallery.{c,h}` |
| Metal GPU host & C bridge | `gpu-host.{c,h}`, `gpu-carrier.c`, `ComponentsHost.m`, `cloud-chat-bridge.{c,h}`, `Bridge.h` | Exposes `oa_ios_*` (Metal rendering, pointer/scroll, VoiceOver semantics, action queue) and `oa_cc_*` (`#authority` Cloud Chat v2, durable turns, Tier 1 tools, Full Auto, Voice Duplex) to Swift |
| UIKit / SwiftUI Cloud shell | `Sources/OpenAgentsApp.swift`, `Sources/RootView.swift`, `Sources/GpuRootViewController.swift` | Full-screen `CADisplayLink` Metal loop, native `UIScrollView` scroll driver, `ComposerEditor` text overlay, `KitAccess` VoiceOver bridge, Photos/Files pickers, and OLED modal sheets |
| Cloud Chat & Sync | `Sources/CloudCore.swift`, `Sources/MobileCloudChat.swift` | `/api/v1/inference` streaming (with `reasoning_effort`, multimodal images, Tier 1 `/api/tools/cloud/v1` tool loop, and `thought_signature` preservation), dual live SSE sync (`/api/chat/v2/stream`, `/api/chat/v2/conversations/:id/stream`), durable turns (`/api/chat/submit/v2alpha1`), placement (`/api/chat/placement/v1`), environments & projects (`/api/chat/environments/v1`, `/api/projects/v1`), and usage (`/api/v1/inference/usage`) |
| Account, Wallet & Voice | `Sources/MobileAccountController.swift`, `Sources/AccountView.swift`, `Sources/MobileWalletController.swift`, `Sources/WalletView.swift`, `Sources/EnvironmentsView.swift`, `Sources/UsageDashboardView.swift`, `Sources/VoiceDictationController.swift`, `Sources/TurnNotifications.swift` | Passkey / WorkOS Magic Auth / Email + Password / Apple-Google-GitHub SSO with Paid + Promo credit display, self-custodial Breez Spark mainnet wallet + NIP-06 identity, execution target & project selection, and bi-directional Live Voice duplex |

## Build & Verification

```sh
coax/bin/coax test apps/openagents-ios/ios-app.coax
coax/bin/coax test apps/openagents-ios/gpu-page.coax --library
coax/bin/coax test apps/openagents-ios/Tests/tester/catalog.coax --library
apps/openagents-ios/build.sh
apps/openagents-ios/check.sh
bash apps/openagents-ios/e2e-sim-test.sh
```

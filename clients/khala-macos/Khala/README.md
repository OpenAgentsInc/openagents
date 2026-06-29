# Khala Desktop - native macOS SwiftUI app

Khala Desktop is the native macOS sibling of the iOS Khala app at `clients/khala-ios/Khala`. It opens to a desktop `NavigationSplitView` shell with local chat history, Keychain-backed Khala API auth, and visible local node readiness panels.

- **Bundle id:** `com.openagents.khala-macos`
- **Platform:** macOS 14+, native SwiftUI. No Expo, EAS, Electron, or web shell.
- **Spec:** `../../../docs/desktop/2026-06-28-khala-desktop-spec.md`
- **Model:** `openagents/khala` through `https://openagents.com/api/v1`
- **Signing team:** `HQWSG26L43`

## Current MVP

- Chat with the public Khala API after storing an `oa_agent_...` key in Keychain.
- Local conversation history persisted as JSON in Application Support.
- Desktop shell with conversation sidebar, main chat pane, and right inspector.
- Pylon supervisor: attaches to a running local Pylon control endpoint when one is available, otherwise boots the bundled Pylon runtime with an app-managed `PYLON_HOME`.
- Node inspector: shows Pylon mode, control URL, isolated Pylon home, account/capacity summaries, and assignment summaries.
- XcodeGen `project.yml` plus a committed `.xcodeproj` using synchronized source folders so the app opens without XcodeGen installed.

## Build locally

```sh
cd clients/khala-macos/Khala
xcodebuild -project Khala.xcodeproj -scheme Khala -destination 'platform=macOS' build
```

Run tests:

```sh
cd clients/khala-macos/Khala
xcodebuild -project Khala.xcodeproj -scheme Khala -destination 'platform=macOS' test
```

## Environment hooks

- `KHALA_API_KEY` injects a bearer key for local smoke runs and bypasses Keychain reads. Normal user flows store the key in Keychain only.

## Not in this scaffold

The app does not print control tokens or API keys. Bundled Pylon runs with its own Application Support home and does not set or write the default Codex home.

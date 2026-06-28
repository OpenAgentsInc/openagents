# Khala Desktop - native macOS SwiftUI app

Khala Desktop is the native macOS sibling of `clients/khala-ios/Khala`. This
first scaffold follows `docs/desktop/2026-06-28-khala-desktop-spec.md`: a
buildable SwiftUI app shell with a desktop chat layout, local history,
Keychain-backed Khala API auth, streaming `openagents/khala` chat, and visible
local-node readiness surfaces for the future Pylon / Apple FM supervisor.

- **Bundle id:** `com.openagents.khala-macos`
- **Platform:** macOS 14+, native SwiftUI. **No Expo / React Native / EAS.**
- **Project:** XcodeGen source in `project.yml`, with a committed
  `Khala.xcodeproj` so the app opens and builds without xcodegen installed.
- **Signing team:** `HQWSG26L43`

## Layout

```
clients/khala-macos/Khala/
├── project.yml
├── Khala.xcodeproj/
└── Khala/
    ├── KhalaApp.swift
    ├── Model/Conversation.swift
    ├── Model/ConversationStore.swift
    ├── Net/KhalaClient.swift
    ├── Net/KhalaStream.swift
    ├── Store/KeychainStore.swift
    ├── Views/ChatView.swift
    ├── Views/DesktopRootView.swift
    ├── Views/MarkdownMessage.swift
    ├── Views/MessageBubble.swift
    ├── Views/NodeInspectorView.swift
    └── Resources/Info.plist
```

## Build

```sh
cd clients/khala-macos/Khala
xcodebuild -project Khala.xcodeproj -scheme Khala \
  -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO build
```

Regenerate the project from the XcodeGen spec if needed:

```sh
cd clients/khala-macos/Khala
xcodegen generate
```

## Scope

This scaffold does not claim a live bundled Pylon or Apple FM backend yet. The
right inspector shows truthful `not connected` / `unavailable` states until the
supervisor and packaged bridge lifecycle are implemented.

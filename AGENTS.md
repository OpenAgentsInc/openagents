# Repository Guidelines

## Codebase Summary

**OpenAgents v0.3+** is a native Swift application for iOS and macOS that enables mobile/desktop management of coding agents.

- **Purpose**: Mobile and desktop command center for coding agents (OpenAI Codex, Claude Code CLI). Run agent sessions from your iPhone or Mac, stay updated on progress, and nudge agents along when needed.
- **Architecture**: Native Swift using SwiftUI for UI, Foundation Models for on-device intelligence, and WebSocket-based bridge for iOS ↔ macOS connectivity
- **Platforms**: iOS 16.0+, macOS 13.0+
- **Previous versions**: v0.2 and earlier (Expo/React Native + Rust + Tauri) are deprecated and no longer maintained

### Key Components

- **iOS App** (`ios/OpenAgents/`)
  - Native Swift iOS app with SwiftUI views
  - Agent Client Protocol (ACP) implementation for agent communication
  - WebSocket client for connecting to macOS companion app
  - Foundation Models integration for conversation titles and summaries
  - Liquid Glass UI for Apple's new translucent material system

- **macOS App** (`ios/OpenAgents/`)
  - Native Swift macOS app (same Xcode project, different targets)
  - Three‑pane chat interface (NavigationSplitView: sidebar + chat + inspector)
  - WebSocket server for iOS pairing and bridge communication
  - Desktop agent session management (Codex/Claude Code CLI integration)
  - Bonjour/mDNS discovery for zero-config LAN pairing
  - Settings and Developer tools accessible via toolbar/menu (⌘,, ⌥⌘D)

- **Shared Core** (`ios/OpenAgentsCore/`)
  - SwiftPM package with shared logic for both platforms
  - ACP protocol implementation (`AgentClientProtocol/`)
  - WebSocket bridge components (`DesktopWebSocketServer`, `MobileWebSocketClient`)
  - JSON-RPC 2.0 transport layer
  - Bridge messages and configuration

### Repository Layout

```
ios/                              # Xcode project root
├── OpenAgents/                   # Main app target
│   ├── Views/                    # SwiftUI views
│   │   ├── macOS/                # macOS-specific views
│   │   │   ├── ChatMacOSView.swift      # Root NavigationSplitView
│   │   │   ├── SessionSidebarView.swift # Session history sidebar
│   │   │   ├── ChatAreaView.swift       # Main chat timeline
│   │   │   ├── ComposerMac.swift        # NSTextView-based composer
│   │   │   ├── Settings/                # Settings sheets (gear)
│   │   │   │   └── SettingsView.swift
│   │   │   └── Developer/               # Developer tools (wrench)
│   │   │       └── DeveloperView.swift
│   ├── Bridge/                   # Bridge integration
│   └── ACP/                      # ACP renderers and components
├── OpenAgentsCore/               # Shared SwiftPM package
│   ├── Sources/
│   │   └── OpenAgentsCore/
│   │       ├── AgentClientProtocol/
│   │       ├── DesktopBridge/
│   │       ├── MobileBridge/
│   │       └── Bridge/
│   └── Tests/
│       └── OpenAgentsCoreTests/
├── OpenAgents.xcodeproj          # Xcode project file
└── OpenAgents.xcworkspace        # Xcode workspace

docs/                             # Documentation
├── adr/                          # Architecture Decision Records
├── liquid-glass/                 # Liquid Glass UI documentation
├── ios-bridge/                   # Bridge protocol documentation
└── logs/                         # Development logs

packages/tricoder/                # DEPRECATED npm package (kept for responsible deprecation)
```

## Architecture

### v0.3 Swift-Only Architecture

**No Expo. No Rust. No Tauri. No TypeScript.** All application code is native Swift.

- **Native Swift**: iOS 16.0+ and macOS 13.0+ using SwiftUI and UIKit where needed
- **Agent Client Protocol (ACP)**: Swift implementation of ACP for agent communication
- **WebSocket Bridge**: JSON-RPC 2.0 over WebSocket for iOS ↔ macOS connectivity
- **Bonjour Discovery**: Zero-config LAN pairing via mDNS (`_openagents._tcp`)
- **Foundation Models**: On-device Apple Intelligence for conversation titles and summaries
- **Liquid Glass UI**: Apple's translucent material system for structural UI (iOS 26+, macOS 15+)

### macOS Chat Interface (v0.3.1+)

The macOS app uses a NavigationSplitView‑based chat layout (sidebar + content, with inspector reserved for future use). The root is `ChatMacOSView` and adopts OATheme black surfaces by default.

- Sidebar: `SessionSidebarView` lists recent sessions (Tinyvex), supports search, ⌘N for New Chat, and Delete with confirmation.
- Chat: `ChatAreaView` renders ACP updates with shared renderers; `ComposerMac` provides a Berkeley Mono input (Return = send, Shift+Return = newline).
- Inspector: reserved for tool details; currently hidden.

Bridge on macOS uses a local JSON‑RPC adapter (Option A) that calls `DesktopWebSocketServer` handlers directly and subscribes to `session/update` via a Combine publisher. See ADR‑0007. Screenshots live under `docs/images/chat-desktop/`.

### ACP Implementation

The Swift ACP implementation is the canonical contract for agent communication:

- All agent updates use ACP `SessionUpdate` messages
- Tool calls, text content, thinking blocks all follow ACP schema
- No custom JSONL or proprietary formats
- See `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/` for implementation
- Reference: ADR-0002 (Agent Client Protocol)

### Bridge Protocol

iOS and macOS communicate via WebSocket with JSON-RPC 2.0:

- **Desktop (macOS)**: Runs `DesktopWebSocketServer`, advertises via Bonjour, accepts JSON-RPC methods
- **Mobile (iOS)**: Runs `MobileWebSocketClient`, discovers via `NetServiceBrowser`, sends JSON-RPC requests
- **Protocol**: `initialize`, `session/new`, `session/prompt`, `session/cancel`, `session/update` (notification)
- **Discovery**: Automatic via `_openagents._tcp` Bonjour service
- See `docs/ios-bridge/` for detailed protocol documentation
- Reference: ADR-0004 (iOS ↔ Desktop WebSocket Bridge)

## Development

### Prerequisites

- **Xcode 16.0+** (required for Swift 5.9+ and iOS 16.0/macOS 13.0 SDKs)
- **macOS 13.0+** (for Xcode and macOS target development)
- **OpenAI Codex or Claude Code CLI** (for desktop agent integration)

### Opening the Project

```bash
cd ios
open OpenAgents.xcworkspace  # Use workspace (includes SwiftPM packages)
```

**Important**: Always use `OpenAgents.xcworkspace`, not `OpenAgents.xcodeproj` directly, to ensure SwiftPM packages are loaded.

### Building

#### iOS

```bash
# Command line (simulator)
cd ios
xcodebuild -workspace OpenAgents.xcworkspace -scheme OpenAgents -sdk iphonesimulator -configuration Debug

# Or use Xcode UI: Product > Build (⌘B)
# Select iOS simulator or device from scheme picker
```

#### macOS

```bash
# Command line
cd ios
xcodebuild -workspace OpenAgents.xcworkspace -scheme OpenAgents -sdk macosx -configuration Debug

# Or use Xcode UI: Product > Build (⌘B)
# Select "My Mac" from scheme picker
```

### Running

- **iOS**: Select an iOS simulator or connected device, then press ⌘R
- **macOS**: Select "My Mac" scheme, then press ⌘R
- **TestFlight**: iOS builds available at https://testflight.apple.com/join/dvQdns5B

### Testing

```bash
# Run tests in Xcode
# Product > Test (⌘U)

# Or command line
cd ios
xcodebuild test -workspace OpenAgents.xcworkspace -scheme OpenAgents -sdk iphonesimulator
```

Key test suites:
- `BridgeServerClientTests.swift` - WebSocket bridge integration tests
- `DesktopWebSocketServerComprehensiveTests.swift` - Server-side bridge tests
- `MessageClassificationRegressionTests.swift` - ACP message classification
- `ToolCallViewRenderingIntegrationTests.swift` - UI rendering tests

## Coding Style & Conventions

### Swift Style

- **Language**: Swift 5.9+ (strict mode)
- **Formatting**: Follow Swift standard conventions (SwiftFormat/SwiftLint configs if present)
- **Indentation**: 4 spaces (Xcode default for Swift)
- **Fonts**:
  - All monospace text MUST use Berkeley Mono across iOS and macOS.
  - SwiftUI: use `OAFonts.mono(...)` (which resolves to Berkeley Mono).
  - AppKit/UIKit: resolve via `BerkeleyFont.defaultName()` and construct `NSFont/UIFont` with that family.
  - Do NOT use system monospace (`.monospacedSystemFont`, SF Mono) in app code.
- **Naming**:
  - Types: `PascalCase` (e.g., `BridgeManager`, `DesktopWebSocketServer`)
  - Functions/properties: `camelCase` (e.g., `connectToServer()`, `isConnected`)
  - Constants: `camelCase` (e.g., `defaultPort`, `serviceType`)
  - Files: Match primary type name (e.g., `BridgeManager.swift`)

### SwiftUI Conventions

- Use `@State`, `@StateObject`, `@ObservedObject`, `@EnvironmentObject` appropriately
- Prefer composition over inheritance
- Extract view components when views exceed ~100 lines
- Use `PreviewProvider` for Xcode Previews

### Architecture Patterns

- **MVVM-ish**: Views consume `ObservableObject` view models or managers
- **Dependency Injection**: Pass dependencies explicitly (e.g., `BridgeManager` via environment)
- **Async/Await**: Use Swift concurrency for async operations
- **Actors**: Use for thread-safe state management where appropriate

### Type Safety

- **Never use `Any` or force casts** unless absolutely necessary
- Prefer protocol conformance over type erasure
- Use generics for reusable, type-safe code
- ACP types are defined in `OpenAgentsCore` - import and use them directly

## Build Discipline (Mandatory)

### Before Committing

1. **Build succeeds**: Press ⌘B in Xcode for both iOS and macOS schemes
2. **Tests pass**: Press ⌘U to run full test suite
3. **No warnings**: Fix or suppress warnings appropriately (don't leave them)
4. **SwiftLint passes**: If configured, ensure no linter errors

### Build Breakage Policy

- If you break the build, **fix forward immediately** or revert the breaking change
- Never leave the main branch in a broken state
- Run a full build before pushing to shared branches
- For large changes, test on a clean clone to catch missing files

### Pre‑release Policy

- avoid feature gates/flags and any backwards compability changes - since our app is still unreleased

### iOS/macOS Testing

- **Always test on both platforms** if you touch shared code (`OpenAgentsCore`)
- Use iOS Simulator for quick iteration
- Test on real devices periodically (especially for bridge/networking features)
- macOS app must build and run without errors

## Git Workflow

### Branching Policy

- **Main branch**: `main` (production-ready code)
- **Default branch for work**: Commit directly to `main` unless instructed otherwise
- **Feature branches**: Only create when explicitly requested by user
- **No destructive operations**: Never use `git reset --hard`, `git clean -fdx`, `git stash`, or force pushes unless explicitly requested

### Commit Guidelines

- **Imperative mood**: "Add feature" not "Added feature" or "Adds feature"
- **Concise subject**: ≤50 characters
- **Body when needed**: Explain why, not what (the diff shows what)
- **Commit often**: Small, focused commits with immediate pushes
- **No stashing**: Always commit work in progress, never stash

### Staging Discipline

- **Only stage files you changed**: Never use `git add .` or `git add -A` unless you changed all those files
- **Use explicit paths**: `git add path/to/file.swift` not `git add .`
- **Review before committing**: `git status` and `git diff --staged` to verify staged changes
- **Leave unrelated changes untouched**: If you see unstaged changes you didn't make, leave them alone

### Multi-Agent Safety

- **Assume concurrent work**: Other agents may be working on the same branch
- **Never delete untracked files**: They may be in-progress work by another agent
- **No history rewriting**: No rebases, amended commits, or force pushes without explicit permission
- **Respect local changes**: Don't revert or restore files you didn't modify

## Architecture Decision Records (ADRs)

This project uses ADRs to document significant architectural decisions.

### Reading ADRs

- **Location**: `docs/adr/`
- **Current ADRs**:
  - ADR-0001: Adopt Architectural Decision Records
  - ADR-0002: Agent Client Protocol (ACP) as Canonical Runtime Contract
  - ADR-0003: Swift Cross-Platform App (macOS + iOS)
  - ADR-0004: iOS ↔ Desktop WebSocket Bridge and Pairing
  - ADR-0005: Adopt Liquid Glass for Apple Platforms
  - ADR-0006: Use Apple Foundation Models for On-Device Intelligence

### Creating ADRs

Use the provided script for consistency:

```bash
cd docs/adr
./new.sh "Your ADR Title Here"
```

**For AI Agents**: Read `docs/adr/AGENTS.md` before creating or modifying ADRs. It contains important guidelines on tone, voice, and content principles.

### ADR Guidelines

- Focus on **why**, not just what
- Document alternatives considered and why they were rejected
- Include consequences (both positive and negative)
- Be direct and honest about trade-offs
- Use specific examples from the OpenAgents codebase
- See `docs/adr/AGENTS.md` for detailed AI agent guidelines

## Key Technologies

### Liquid Glass

Apple's new translucent material system for iOS 26+, iPadOS 26+, macOS 15+ (Sequoia).

- **Usage**: Structural UI (bars, sidebars, sheets, toolbars, cards)
- **APIs**: `glassEffect(_:in:)`, `GlassEffectContainer`, `UIGlassEffect`
- **Fallback**: Standard materials (`Material.regular`, `Material.ultraThin`) on older OS versions
- **Docs**: `docs/liquid-glass/`
- **Reference**: ADR-0005

### Foundation Models

Apple's on-device language models for local intelligence tasks.

- **Usage**: Conversation titles, summaries, tags/classification
- **APIs**: `SystemLanguageModel`, `LanguageModelSession`, `GenerationOptions`
- **Availability**: iOS 26+, macOS 15+ with Apple Intelligence enabled
- **Fallback**: Deterministic local logic when unavailable
- **Privacy**: All processing on-device, no network calls
- **Reference**: ADR-0006

### Agent Client Protocol (ACP)

Open standard for agent communication.

- **Implementation**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/`
- **Message Types**: `SessionUpdate`, `ToolCall`, `ContentBlock`, `ThinkingBlock`
- **Transport**: JSON over WebSocket
- **Spec**: https://agentclientprotocol.com/
- **Reference**: ADR-0002

## Security & Privacy

- **No secrets in code**: Use Xcode's secret management or environment variables
- **iOS Bundle ID**: `com.openagents.app`
- **TestFlight**: Managed via App Store Connect
- **On-device processing**: Foundation Models run entirely on-device
- **WebSocket security**: LAN-only by default; future: TLS + pairing tokens

## Common Tasks

### Adding a New SwiftUI View

1. Create `MyNewView.swift` in appropriate directory under `ios/OpenAgents/Views/`
2. Define view conforming to `View` protocol
3. Add Xcode Preview for development
4. Import necessary dependencies (e.g., `OpenAgentsCore`)
5. Build (⌘B) and preview in Xcode Canvas

### Adding a New Chat Message Renderer (shared ACP UI)

1. Implement a renderer in `ios/OpenAgents/ACP/` using OATheme/OAFonts.
2. Integrate in the appropriate switch (e.g., message row) with minimal logic.
3. Add an optional detail sheet for rich/structured content.
4. Verify on iOS and macOS.
5. Add a focused unit/integration test if applicable.

Example snippet:
```
// inside a message row switch
case .custom(let payload):
    CustomMessageView(payload: payload)
        .font(OAFonts.mono(.body, 14))
        .foregroundStyle(OATheme.Colors.textPrimary)
```

### Accessing Settings or Developer Tools (macOS)

- Settings: Toolbar gear or ⌘, (Connection, Workspace, Agents, Orchestration)
- Developer Tools: Menu Developer → Developer Tools or ⌥⌘D (Database, Nostr, Logs, Diagnostics)

### Modifying Bridge Protocol

1. Update message definitions in `ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/BridgeMessages.swift`
2. Update server/client handlers in `DesktopWebSocketServer.swift` or `MobileWebSocketClient.swift`
3. Update `docs/ios-bridge/` documentation
4. Add/update tests in `OpenAgentsCoreTests/`
5. Consider creating an ADR if it's a significant protocol change

### Adding Foundation Models Usage

1. Check availability: `SystemLanguageModel.default.availability`
2. Create session with instructions
3. Implement fallback for unavailable/unsupported devices
4. Cache results to avoid recomputation
5. Add logging for diagnostics (development builds)
6. Reference ADR-0006 for guidelines

### Updating ADRs

1. For new decisions: `cd docs/adr && ./new.sh "Your Title"`
2. For updates to existing ADRs: Edit the markdown file directly
3. Change status as appropriate (Proposed → In Review → Accepted)
4. Update references in related ADRs
5. Commit with clear message explaining the architectural change

## Troubleshooting

### "No such module 'OpenAgentsCore'"

- Make sure you opened `OpenAgents.xcworkspace` not `OpenAgents.xcodeproj`
- Clean build folder: Product > Clean Build Folder (⌘⇧K)
- Close Xcode, delete `~/Library/Developer/Xcode/DerivedData/OpenAgents-*`, reopen

### Bridge Connection Issues

- Check macOS app is running and Bonjour service is advertised
- Check iOS app can browse `_openagents._tcp` services
- Use simulator: ensure both simulator and Mac are on same network
- Check logs: Server logs in Console.app, client logs in Xcode debug console

### Foundation Models Not Available

- Requires iOS 26+ or macOS 15+ with Apple Intelligence enabled
- Check device support: Settings > Apple Intelligence
- Models may be downloading: wait and retry
- Fallback logic should handle gracefully (see ADR-0006)

### Xcode Build Errors

- **"Command SwiftCompile failed"**: Check for syntax errors in Swift files
- **"Cycle in dependencies"**: Check framework/target dependencies in Xcode project settings
- **"Failed to build module"**: Clean build folder (⌘⇧K) and rebuild

## Testing Guidelines

### Unit Tests

- **Location**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/`
- **Naming**: `*Tests.swift` (e.g., `BridgeMessageTests.swift`)
- **Target**: Test shared core logic (ACP parsing, bridge messages, protocols)
- **Run**: Press ⌘U in Xcode

### Integration Tests

- Test WebSocket bridge end-to-end (server + client)
- Test ACP message flow through the system
- Test UI rendering with sample data
- Examples: `BridgeServerClientTests.swift`, `ToolCallViewRenderingIntegrationTests.swift`

### UI Tests

- **Framework**: XCTest UI Testing
- **Target**: `OpenAgentsUITests` (if configured)
- **Scope**: Critical user flows (session creation, agent prompting, message rendering)

### Test Coverage

- Aim for 70%+ coverage on new code
- Critical paths (ACP parsing, bridge protocol) should have 90%+ coverage
- Use Xcode's coverage reports: Product > Test (⌘U), then coverage tab

## Deprecation Notes

### v0.2 and Earlier (Deprecated)

The following are **NO LONGER SUPPORTED** as of v0.3:

- ❌ Expo/React Native mobile app (`expo/` - deleted)
- ❌ Rust WebSocket bridge (`crates/oa-bridge/` - deleted)
- ❌ Tauri desktop app (`tauri/` - deleted)
- ❌ TypeScript packages (`packages/openagents-core`, `packages/openagents-theme`, `packages/tinyvex` - deleted)
- ❌ npm package `tricoder` (v0.3.0 published as deprecated, last working version v0.2.5)
- ❌ Bun/npm build system (replaced with Xcode/SwiftPM)
- ❌ Maestro E2E tests (`.maestro/` - deleted)

### Migration from v0.2

If you were familiar with v0.2:

- **No more Expo**: All UI is now SwiftUI
- **No more Rust bridge**: Bridge is now Swift WebSocket server/client with JSON-RPC
- **No more TypeScript**: All application code is Swift
- **No more Tauri**: macOS app is native Swift
- **No more bun/npm**: Use Xcode and SwiftPM
- **No data migration**: v0.3 is a fresh start

The repository was cleaned up in PR #1414. See that PR and issue #1413 for the full deletion list and rationale.

### v0.3.0 Dashboard (Deprecated in v0.3.1)

The original macOS dashboard view (`SimplifiedMacOSView`) has been replaced by the chat‑first layout:

- Dashboard cards (Bridge Status, Working Directory, Agent Config, Dev Tools) moved into Settings (⌘,) and Developer (⌥⌘D) views.
- The main window now opens into `ChatMacOSView` with a session sidebar and chat area.
- See ADR‑0007 for the rationale and architecture details.

## Additional Documentation

- **ADRs**: `docs/adr/` - All architectural decisions
  - ADR‑0007: macOS Chat Interface Architecture
- **Liquid Glass**: `docs/liquid-glass/` - Visual design, APIs, examples
- **iOS Bridge**: `docs/ios-bridge/` - WebSocket protocol specification
- **Logs**: `docs/logs/` - Historical development logs and decisions

## Getting Help

- **GitHub Issues**: https://github.com/OpenAgentsInc/openagents/issues
- **ADRs**: Check `docs/adr/` for architectural context
- **TestFlight**: https://testflight.apple.com/join/dvQdns5B

## Summary for AI Agents

**This is a native Swift iOS/macOS project. Use Xcode, not Expo/npm/Rust tooling.**

- Build with Xcode (⌘B)
- Test with Xcode (⌘U)
- All code is Swift (no TypeScript, no Rust)
- Read ADRs in `docs/adr/` before making architectural changes
- Read `docs/adr/AGENTS.md` before creating/modifying ADRs
- Always build and test before committing
- Commit frequently with explicit file staging
- Never use destructive git operations unless explicitly requested

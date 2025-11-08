# OpenAgents

A native iOS and macOS app for managing your coding agents on the go.

<img width="1000" height="470" alt="OpenAgents" src="https://github.com/user-attachments/assets/0569c202-e7d8-43a7-b6ad-829fe761d31b" />

## Why

There is no good mobile app for managing coding agents while AFK.

Coding agent CLIs like Claude Code are powerful enough to handle most day-to-day coding, but developers are still glued to their computers because there's no good way to code on your phone.

Remote terminals are clunky. The labs' bolted-on "Code" features are underpowered afterthoughts.

We want coding agents running async, keeping us just updated enough to nudge them along when needed.

A good agent frees you from your computer, doesn't tie you to it.

## Version 0.3 - Swift Native

**v0.3+** is a complete rewrite using native Swift for iOS and macOS.

Previous versions (v0.2 and earlier) were proof-of-concept implementations using Expo/React Native, Tauri, and Rust. Those versions are **deprecated** and no longer maintained.

## Platforms

**Supported (v0.3+)**:
- iOS 16.0+
- macOS 13.0+

**Future** (contributions welcome):
- Android
- Windows
- Linux
- Web

## Tech Stack

- Swift & SwiftUI
- Agent Client Protocol (ACP)
- WebSocket bridge for desktop connectivity
- OpenAI Codex and Claude Code CLI integration
- Native Apple technologies:
  - Liquid Glass UI (iOS 26+, macOS 15+)
  - Foundation Models (on-device intelligence)

## Architecture

The v0.3 architecture is Swift-only:

- **iOS/macOS App**: Native Swift app in `ios/`
- **ACP Implementation**: Swift implementation of Agent Client Protocol under `ios/OpenAgentsCore/`
- **Desktop Bridge**: macOS companion app for desktop agent connectivity
- **WebSocket Protocol**: iOS ↔ macOS communication via WebSocket

All ACP translation and processing happens in Swift. No Rust, no TypeScript, no web technologies.

## Getting Started

### iOS

1. Download from [TestFlight](https://testflight.apple.com/join/dvQdns5B)
2. Pair with your Mac running the companion app
3. Start managing your coding agents

### macOS

1. Build from source (for now):
   ```bash
   cd ios
   xcodebuild -scheme OpenAgents -configuration Debug
   ```
2. Run the app
3. Pair with your iOS device

### Requirements

- Xcode 16.0+
- Swift 5.9+
- iOS 16.0+ / macOS 13.0+
- OpenAI Codex or Claude Code CLI (installed on macOS)

## Development

```bash
# Open in Xcode (use the workspace)
cd ios
open OpenAgents.xcworkspace

# Build iOS
xcodebuild -scheme OpenAgents -sdk iphonesimulator

# Build macOS
xcodebuild -scheme OpenAgents -sdk macosx
```

## Contributing

PRs welcome! Please ensure:

- Code follows Swift style guidelines
- iOS/macOS builds pass
- Changes are tested on both platforms
- Documentation is updated

## Migration from v0.2

If you used previous versions (Expo/Rust/Tauri):

- **Data**: No automatic migration available. v0.3 is a clean start.
- **Pairing**: Use new WebSocket-based pairing in v0.3
- **Desktop**: Install new macOS companion app (not Rust bridge)
- **npm package**: The `tricoder` npm package ([npmjs.com/package/tricoder](https://www.npmjs.com/package/tricoder)) is deprecated. Last working version was v0.2.5.

The v0.2 codebase is archived but no longer maintained.

## Documentation

- Architecture Decision Records: `docs/adr/`
- Liquid Glass UI: `docs/liquid-glass/`
- Agent Client Protocol: `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/`

## License

MIT

## Links

- [GitHub Issues](https://github.com/OpenAgentsInc/openagents/issues)
- [ADRs](docs/adr/)
- [TestFlight](https://testflight.apple.com/join/dvQdns5B)

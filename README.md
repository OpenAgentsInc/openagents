# OpenAgents

Claude Code wrapper with desktop and mobile apps featuring **two-way sync** between platforms. Built as a Bun workspace monorepo with Convex real-time backend.

Being built in public. See [intro video](https://x.com/OpenAgentsInc/status/1948214004268064771)

## Quick Start

```bash
# Install all dependencies
bun install

# Run desktop app
bun run desktop

# Run mobile app  
bun run mobile

# Start Convex backend
bun run convex
```

## ✨ Features

- **Two-way Claude Code sync** - Sessions created on desktop/mobile sync in real-time
- **Cross-platform messaging** - Send/receive messages between desktop and mobile
- **Real-time backend** - Powered by Convex for instant updates
- **Mobile session creation** - Start desktop Claude Code sessions from mobile

## Commands

**Development:**
- `bun run desktop` - Run desktop app in development
- `bun run mobile` - Run mobile app in development  
- `bun run ios` - Run on iOS simulator
- `bun run android` - Run on Android emulator
- `bun run convex` - Start Convex backend

**Building:**
- `bun run build:desktop` - Build desktop app
- `bun run build:ios` - Build iOS production app
- `bun run build:android` - Build Android production app

**Deployment:**
- `bun run submit` - Submit iOS app to App Store
- `bun run update` - Publish OTA update to production

**Utilities:**
- `bun install` - Install all dependencies
- `bun run clean` - Clean node_modules and dist folders

## Status
✅ **Two-way sync implemented** - Desktop ↔ Mobile Claude Code session synchronization working!

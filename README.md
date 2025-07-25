# OpenAgents

Claude Code wrapper with desktop and mobile apps, built as a Bun workspace monorepo

Being built in public. See [intro video](https://x.com/OpenAgentsInc/status/1948214004268064771)

## Quick Start

```bash
# Install all dependencies
bun install

# Run desktop app
bun run desktop

# Run mobile app
bun run mobile
```

## Commands

**Workspace (from root):**
- `bun install` - Install all dependencies
- `bun run desktop` - Run desktop app in development
- `bun run mobile` - Run mobile app in development
- `bun run ios` - Run mobile app on iOS simulator
- `bun run android` - Run mobile app on Android emulator
- `bun run build:desktop` - Build desktop app
- `bun run build:mobile` - Build mobile app

**Desktop-specific:**
- `cd apps/desktop && bun run dev` - Direct desktop development
- See [Desktop README](apps/desktop/README.md) for more

## Status
WIP: Not yet usable

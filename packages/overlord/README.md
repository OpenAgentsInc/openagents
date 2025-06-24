# Overlord - Claude Code Sync Service

Overlord is a StarCraft-themed CLI service that bridges local Claude Code instances with OpenAgents.com, enabling centralized management of AI development sessions.

## Overview

Named after the StarCraft Zerg unit that provides oversight and coordination, Overlord monitors your local Claude Code JSONL files and synchronizes them with a cloud database, enabling:

- 📊 **Centralized Dashboard** - View all Claude Code sessions from OpenAgents.com
- 🔄 **Real-time Sync** - Automatic synchronization of conversation data
- 🎮 **Remote Control** - Execute commands on your local machine from the web (future)
- 📈 **Analytics** - Track usage, costs, and productivity metrics
- 🤝 **Collaboration** - Share sessions with team members

## Installation

```bash
# Install globally
pnpm install -g @openagentsinc/overlord

# Or run directly with pnpm
pnpm exec overlord
```

## Usage

### Spawn the Daemon

Start Overlord to monitor your Claude Code sessions:

```bash
overlord spawn --user-id=your-user-id --api-key=your-api-key
```

### Detect Claude Installations

Find Claude Code installations on your machine:

```bash
overlord detect
```

### Transport Sessions

Manually sync sessions to the cloud:

```bash
# Sync all sessions
overlord transport all --user-id=your-user-id --api-key=your-api-key

# Sync specific session
overlord transport abc-123-def --user-id=your-user-id --api-key=your-api-key
```

### Check Status

View daemon status:

```bash
overlord status
```

## StarCraft Theme

All commands follow StarCraft Overlord terminology:

- **spawn** - Start the daemon (like spawning a unit)
- **detect** - Reconnaissance of Claude installations
- **transport** - Move data to the cloud (like transporting units)
- **burrow/unburrow** - Background/foreground operation
- **evolve** - Update to latest version

## Architecture

Overlord consists of:

1. **File Watcher** - Monitors `~/.claude/projects/` for JSONL changes
2. **WebSocket Client** - Real-time connection to OpenAgents.com
3. **JSONL Parser** - Processes Claude Code conversation data
4. **Sync Engine** - Handles bidirectional data synchronization

## Development

```bash
# Build the package
pnpm build

# Run tests
pnpm test

# Type checking
pnpm check
```

## Database Schema

See `database/schema.sql` for the PlanetScale schema used to store:
- Claude sessions
- Conversation messages
- Machine registry
- Remote commands

## Security

- API keys are never logged or stored locally
- WebSocket connections use secure authentication
- All data is encrypted in transit
- Remote command execution requires explicit permissions

## Future Features

- 🖥️ Remote terminal access
- 📁 File browser and editor
- 🤖 Start Claude Code sessions remotely
- 📊 Advanced analytics and insights
- 👥 Team collaboration features
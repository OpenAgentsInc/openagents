# Overlord - Claude Code Sync Service

Overlord watches your Claude Code conversations on your computer and syncs them to the cloud so you can view them on OpenAgents.com.

## What Problem Does This Solve?

Right now, when you use Claude Code, all your conversations are stored as files on your computer in hidden folders. You can't:
- See them in a nice interface
- Access them from another computer  
- Search through old conversations easily
- Track how much you're spending
- Share interesting conversations

Overlord fixes this by automatically syncing your conversations to the cloud.

## How to Use It

### 1. Check if Overlord can find your Claude Code files

```bash
overlord detect
```

This shows you where Claude Code is storing your conversations. You should see something like:
```
✅ Found 1 Claude Code installation(s):
   📁 /Users/yourname/.claude/projects/
      Sessions: 42
      Last active: 2025-06-24T12:30:00Z
```

### 2. Start syncing your conversations

```bash
overlord spawn --user-id=your-email --api-key=your-key
```

This starts a background process that watches for new messages in your Claude Code conversations.

### 3. What happens automatically

- Every time you send a message to Claude Code
- Or Claude responds to you  
- Overlord sees the change and sends it to OpenAgents.com
- You can then log into OpenAgents.com and see all your conversations

## What You'll See on OpenAgents.com (Coming in Phase 2)

- All your Claude Code sessions organized by project
- How much each conversation cost
- Search across all your conversations
- Nice formatting for code blocks and responses
- Usage analytics and insights

## Current Status

**Phase 1 (Complete)**: The "watcher" part that monitors your files
**Phase 2 (Next)**: The website interface to view your synced conversations
**Phase 3 (Future)**: Remote control of Claude Code from the web
**Phase 4 (Future)**: Team collaboration features

Think of it like Dropbox for your Claude Code conversations - it watches for changes and syncs them automatically.

## Technical Details

### Installation

```bash
# Install globally
pnpm install -g @openagentsinc/overlord

# Or run from the package directory
cd packages/overlord
pnpm build
node dist/esm/bin.js --help
```

### All Commands

```bash
overlord spawn --user-id=xxx --api-key=yyy  # Start the sync daemon
overlord detect                             # Find Claude installations
overlord transport                          # Manually sync sessions
overlord status                             # Check if daemon is running
overlord evolve                             # Update to latest version
```

### StarCraft Theme

The commands are themed after StarCraft's Overlord unit:
- **spawn** - Start the daemon (like spawning a unit)
- **detect** - Reconnaissance of Claude installations  
- **transport** - Move data to the cloud (like transporting units)
- **burrow/unburrow** - Background/foreground operation
- **evolve** - Update to latest version

### Architecture

Under the hood, Overlord uses:
1. **File Watcher** - Monitors `~/.claude/projects/` for JSONL changes
2. **WebSocket Client** - Real-time connection to OpenAgents.com  
3. **JSONL Parser** - Processes Claude Code conversation data
4. **Sync Engine** - Handles uploading changes to the cloud

### Security

- API keys are never logged or stored locally
- All data is encrypted in transit
- You control what gets synced
- Remote features require explicit permission

## Development

```bash
# Build the package
pnpm build

# Run tests  
pnpm test

# Type checking
pnpm check
```

## Troubleshooting

### "Service not found" error
Make sure you're running the command from the right directory or have installed it globally.

### Can't find Claude installations
Claude Code stores files in `~/.claude/projects/` or `~/.config/claude/projects/`. Make sure you have used Claude Code at least once.

### WebSocket connection fails
The OpenAgents.com sync endpoint isn't deployed yet. This is coming in Phase 2.
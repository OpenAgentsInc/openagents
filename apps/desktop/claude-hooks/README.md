# Claude Code Convex Sync Hooks

This directory contains hook scripts that enable real-time synchronization of Claude Code sessions to the Convex backend.

## Setup Instructions

### 1. Environment Configuration

Set the Convex deployment URL in your environment:

```bash
export VITE_CONVEX_URL="https://your-deployment.convex.cloud"
# or 
export CONVEX_URL="https://your-deployment.convex.cloud"
```

### 2. Claude Code Configuration

Add the hooks configuration to your Claude Code settings. The configuration can be added in several ways:

#### Option A: Global Configuration
Add to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/openagents/apps/desktop/claude-hooks/sync-to-convex.js",
            "description": "Sync user prompts to Convex backend"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command", 
            "command": "/path/to/openagents/apps/desktop/claude-hooks/sync-to-convex.js",
            "description": "Sync tool results to Convex backend"
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/openagents/apps/desktop/claude-hooks/sync-to-convex.js", 
            "description": "Sync final session state to Convex backend"
          }
        ]
      }
    ]
  }
}
```

#### Option B: Project-specific Configuration
Create `.claude/settings.json` in your project directory:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "$CLAUDE_PROJECT_DIR/apps/desktop/claude-hooks/sync-to-convex.js",
            "description": "Sync user prompts to Convex backend"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command", 
            "command": "$CLAUDE_PROJECT_DIR/apps/desktop/claude-hooks/sync-to-convex.js",
            "description": "Sync tool results to Convex backend"
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "$CLAUDE_PROJECT_DIR/apps/desktop/claude-hooks/sync-to-convex.js", 
            "description": "Sync final session state to Convex backend"
          }
        ]
      }
    ]
  }
}
```

### 3. Testing the Setup

#### Enable Debug Logging
```bash
export CLAUDE_HOOK_DEBUG=true
```

#### Test the Hook Script
```bash
echo '{"session":{"id":"test-123","project_path":"/test"},"event":"test","messages":[]}' | ./sync-to-convex.js
```

#### Check Logs
Hook execution logs are stored in `~/.claude/hook-logs/convex-sync-YYYY-MM-DD.log`

## Hook Events

The sync script responds to these Claude Code events:

### UserPromptSubmit
- Triggered when a user submits a prompt
- Syncs the user message to Convex
- Creates session if it doesn't exist

### PostToolUse  
- Triggered after Claude completes a tool operation
- Syncs tool usage and results to Convex
- Updates session activity timestamp

### Stop
- Triggered when Claude finishes responding  
- Syncs final session state to Convex
- Useful for capturing complete conversation turns

## Data Flow

```
Claude Code Session
       ↓
   Hook Trigger
       ↓
 sync-to-convex.js
       ↓
  Convex Backend
       ↓
   Mobile App
```

## Troubleshooting

### Hook Not Executing
1. Check that the script has execute permissions: `ls -la sync-to-convex.js`
2. Verify the path in your Claude settings is correct
3. Check that the Convex URL is set: `echo $VITE_CONVEX_URL`

### Sync Failures
1. Check the hook logs: `tail -f ~/.claude/hook-logs/convex-sync-*.log`
2. Test the Convex connection manually
3. Verify your Convex deployment is running

### Debug Mode
Enable debug output:
```bash
export CLAUDE_HOOK_DEBUG=true
```

This will log detailed information about hook execution to stderr.

## Manual Sync Script

For testing or one-off syncing, you can create a script to inspect Claude Code session structure:

```bash
#!/bin/bash
# inspect-session.js - Inspect Claude Code session data

claude --help  # Check if Claude Code CLI is available
claude sessions list  # List current sessions
claude sessions inspect <session-id>  # Get detailed session data
```

This can help understand the exact message structure for debugging sync issues.

## Security Notes

- Hook scripts run with your user permissions
- Be cautious about what data you sync to external services
- Consider encrypting sensitive project information
- Review hook scripts before enabling them globally

## Related Files

- `sync-to-convex.js` - Main hook script
- `claude-config-example.json` - Example configuration
- `../../packages/convex/convex/claude.ts` - Convex functions for sync
- `../../packages/convex/convex/schema.ts` - Database schema
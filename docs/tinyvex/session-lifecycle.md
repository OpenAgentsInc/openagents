# Tinyvex Session Lifecycle and Historical Sessions

## Overview

This document explains how Claude Code sessions are created, persisted, and why historical sessions loaded from Tinyvex DB cannot be resumed.

## Session Creation Flow

### 1. New Session in OpenAgents

When a user starts a new conversation in the OpenAgents app:

```
User sends first message
  ↓
Bridge generates new UUID (e.g., "A50150EE-6C13-45BB-BF8C-025C9343253C")
  ↓
Bridge calls: claude --session-id A50150EE-6C13-45BB-BF8C-025C9343253C "user message"
  ↓
Claude Code CREATES new session in ~/.claude/sessions/ with that ID
  ↓
Session exists in BOTH places:
  - Claude Code's local storage (~/.claude/sessions/)
  - Tinyvex DB (via session/update notifications)
```

### 2. Continuing a Session

For subsequent messages in the same conversation:

```
User sends second message
  ↓
Bridge uses same UUID from step 1
  ↓
Bridge calls: claude --resume A50150EE-6C13-45BB-BF8C-025C9343253C "follow up message"
  ↓
Claude Code RESUMES existing session from ~/.claude/sessions/
  ↓
Conversation continues with full context
```

**Key distinction**:
- `--session-id <uuid>`: **Creates** a new, empty session with that ID
- `--resume <uuid>`: **Resumes** an existing session that was previously created

## Session Storage

Sessions exist in two places:

### Claude Code Local Storage
- Location: `~/.claude/sessions/`
- Purpose: Claude Code's own session management
- Lifetime: Managed by Claude Code (may expire, be cleaned up, etc.)
- Format: Internal to Claude Code

### Tinyvex Database
- Location: `~/Library/Application Support/OpenAgents/tinyvex.sqlite`
- Purpose: OpenAgents' persistent record of all ACP events
- Lifetime: Permanent (unless manually deleted)
- Format: `acp_events` table with session_id, seq, ts, update_json columns

## The Historical Session Problem

### Why Historical Sessions Are Orphaned

When you view historical sessions in the OpenAgents drawer, you're loading data from **Tinyvex DB only**. These sessions may no longer exist in Claude Code's local storage due to:

1. **Time**: Claude Code may have expired/cleaned up old sessions
2. **Reinstallation**: User may have reinstalled Claude Code, losing local session data
3. **Different Machine**: Tinyvex DB synced across devices, but Claude's sessions are local
4. **Manual Cleanup**: User may have cleared Claude Code's session cache

### Why Resume Fails

When you try to send a message to a historical session:

```
User loads historical session BF618DB3-A768-4ABB-BABB-112616B7FB69 from Tinyvex
  ↓
Messages display correctly (loaded from Tinyvex DB)
  ↓
User sends new message "continue the story"
  ↓
Bridge attempts: claude --resume BF618DB3-A768-4ABB-BABB-112616B7FB69 "continue the story"
  ↓
Claude Code searches ~/.claude/sessions/ for that ID
  ↓
❌ Session not found!
  ↓
Claude Code responds: "❌ No conversation found with session ID: BF618DB3-..."
```

### Why --session-id Doesn't Help

You might think: "Just use `--session-id` instead of `--resume` to recreate the session!"

**This doesn't work because**:

```
Bridge calls: claude --session-id BF618DB3-A768-4ABB-BABB-112616B7FB69 "continue the story"
  ↓
Claude Code CREATES a NEW, EMPTY session with that ID
  ↓
The new session has NO MEMORY of the previous conversation
  ↓
Claude responds without context from the historical messages
```

The `--session-id` flag creates a **fresh session** with that ID. It doesn't restore the previous conversation context from anywhere. Claude Code has no way to reconstruct the session state from our Tinyvex DB.

## Current Solution: View-Only Historical Sessions

As of v0.3, historical sessions are **view-only**:

```
User loads historical session from drawer
  ↓
Bridge calls: tinyvex/history.sessionTimeline (our custom endpoint)
  ↓
Historical messages display in the UI (read-only)
  ↓
currentSessionId is set to nil
  ↓
User sends new message
  ↓
Bridge sees currentSessionId == nil
  ↓
Bridge creates FRESH session with new UUID
  ↓
New conversation begins (without automatic context from historical messages)
```

### User Experience

**What users see**:
1. Browse historical conversation messages ✅
2. Send a new message → starts a fresh conversation
3. Historical messages remain visible for reference (scroll up)
4. New messages appear below, in a new session

**What users DON'T see** (but happens behind the scenes):
- Session ID changes when they send the first new message
- Old and new messages are in different sessions
- No automatic context carryover (unless we implement future features)

## Future Improvements

Possible enhancements to make historical sessions more useful:

### Option 1: Context Injection
When starting a new session after viewing historical messages, inject a summary:

```
System: "Resuming conversation from <date>. Previous context: <AI-generated summary>"
User's new message: "continue the story"
```

### Option 2: Claude Code Session Import
If Claude Code adds an API to import/restore sessions from external sources, we could:
1. Export session data from Tinyvex DB
2. Import into Claude Code's local storage
3. Resume normally with `--resume`

### Option 3: Read-Only Indicator
Add visual indicator in UI: "📖 Viewing historical session (read-only)"

## Implementation Details

### BridgeManager.swift
```swift
func loadSessionTimeline(sessionId: String) {
    // ...
    currentSessionId = nil  // Force new session for any new prompts
    // Historical messages loaded for viewing only
}
```

### DesktopWebSocketServer.swift
```swift
// First prompt in a NEW session
if !claudeStartedBySession[sidStr] {
    process.arguments = ["--session-id", sidStr, prompt]
    claudeStartedBySession[sidStr] = true
}
// Subsequent prompts in EXISTING session
else {
    process.arguments = ["--resume", sidStr, prompt]
}
```

### Tinyvex DB Schema
```sql
CREATE TABLE acp_events (
    session_id TEXT,
    seq INTEGER,
    ts INTEGER,
    update_json TEXT,
    PRIMARY KEY (session_id, seq)
);
```

## Related Documentation

- [Tinyvex Database Overview](../tinyvex/README.md)
- [Agent Client Protocol](../../ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/)
- [iOS Bridge Protocol](../ios-bridge/)
- [ADR-0006: Tinyvex as Persistent ACP Event Store](../adr/0006-tinyvex-persistence.md) _(if exists)_

## Debugging Tips

### Check if session exists in Claude Code storage

```bash
# List Claude Code sessions
ls -la ~/.claude/sessions/

# Search for specific session
find ~/.claude/sessions/ -name "*BF618DB3*"
```

### Check Tinyvex DB for session

```bash
sqlite3 ~/Library/Application\ Support/OpenAgents/tinyvex.sqlite \
  "SELECT session_id, COUNT(*) as event_count, MAX(ts) as last_ts
   FROM acp_events
   WHERE session_id = 'BF618DB3-A768-4ABB-BABB-112616B7FB69';"
```

### Verify session creation/resume behavior

Watch bridge logs when sending messages:

```
[Bridge][server] arguments (claude): --session-id <uuid> "first message"
[Bridge][server] arguments (claude): --resume <uuid> "second message"
```

If you see `--resume` on a historical session followed by "No conversation found", the session is orphaned.

## Summary

**Key Takeaways**:

1. `--session-id` **creates** sessions, `--resume` **continues** them
2. Sessions must exist in Claude Code's local storage to be resumed
3. Tinyvex DB persists sessions permanently, but Claude Code storage is transient
4. Historical sessions become orphaned when Claude Code's local copy expires
5. Current solution: Historical sessions are view-only; new messages start fresh sessions
6. Future: Context injection or session import could bridge the gap

---

*Last updated: 2025-11-07*
*Related issue: #1430 - Tinyvex History — Recent Sessions Drawer + Load Messages*

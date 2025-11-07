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

## Addendum: Practical Workarounds for Session Continuation

The above explains why naive session resumption doesn't work. However, there are **practical workarounds** that could enable true historical session continuation.

### Approach 1: Context Injection on Resume Failure

When `--resume` fails with "No conversation found", automatically retry with context:

```
User loads historical session and sends "continue the story"
  ↓
Bridge tries: claude --resume BF618DB3-... "continue the story"
  ↓
❌ "No conversation found"
  ↓
Bridge detects failure, loads historical messages from Tinyvex
  ↓
Bridge condenses/formats previous conversation
  ↓
Bridge retries: claude --session-id <NEW-UUID> "<CONTEXT>\n\nUser: continue the story"
  ↓
✅ Claude responds with full context
```

**Implementation**:
```swift
func sendPrompt(text: String, sessionId: String?) {
    if let sid = sessionId {
        // Try resume first
        sendWithResume(sid, text) { success in
            if !success {
                // Resume failed - inject context and create new session
                let context = loadAndCondenseHistory(sid)
                let newSid = UUID().uuidString
                sendWithContext(newSid, context: context, userMessage: text)
            }
        }
    }
}
```

### Approach 2: Preemptive Context Injection

Don't even try to resume. When user loads a historical session and sends first message, **always** create a new session with context:

```swift
func loadSessionTimeline(sessionId: String) {
    // Load historical messages
    // ...

    // Set flag indicating we need context injection
    self.pendingHistoricalContext = sessionId
    self.currentSessionId = nil
}

func sendPrompt(text: String) {
    if let historicalSid = self.pendingHistoricalContext {
        // Build context from historical session
        let context = buildContextString(historicalSid)
        let newSid = UUID().uuidString

        let fullPrompt = """
        [Previous conversation from \(formatDate(historicalSid))]
        \(context)

        [Current message]
        User: \(text)
        """

        sendWithSessionId(newSid, fullPrompt)
        self.pendingHistoricalContext = nil
    } else {
        // Normal flow
    }
}
```

### Approach 3: Smart Condensing of Historical Messages

The main challenge: historical conversations can be **huge** (tool calls, thinking blocks, etc.). Solution: condense intelligently.

**Condensing Strategy**:

```swift
func buildContextString(_ sessionId: String) -> String {
    let updates = loadHistoricalUpdates(sessionId)
    var condensed = ""

    for update in updates {
        switch update.sessionUpdate {
        case .agent_message_chunk:
            // Keep full assistant messages
            condensed += "Assistant: \(update.content.text)\n\n"

        case .user_message:
            // Keep full user messages
            condensed += "User: \(update.content.text)\n\n"

        case .agent_thought_chunk:
            // Optionally skip or summarize thoughts
            // condensed += "[Thinking: \(truncate(update.content.text, 50))]\n"
            break

        case .tool_call:
            // Condense tool calls to just name and purpose
            condensed += "[Tool: \(update.toolCall.name)]\n"

        case .tool_call_update:
            // Keep only successful tool output, truncate long outputs
            if update.toolCallUpdate.status == .completed {
                let output = truncate(update.toolCallUpdate.output, maxChars: 200)
                condensed += "[Result: \(output)]\n"
            }

        default:
            break
        }
    }

    return condensed
}

func truncate(_ text: String, maxChars: Int) -> String {
    if text.count <= maxChars {
        return text
    }
    return text.prefix(maxChars) + "... (truncated)"
}
```

**Example condensed context**:

```
[Previous conversation from 2 hours ago]

User: Hey there
Assistant: Hey there! What can I help you with today?

User: Tell me a story about space
[Thinking: Preparing creative story...]
Assistant: Once there was an orbital botanist named Mira who farmed constellations...

[Tool: shell]
[Result: README.md CLAUDE.md ios/ docs/]

User: Continue the story
Assistant: ...Mira realized the cosmos had been quietly tending to her all along.

[Current message]
User: write the next few sentences
```

**Benefits**:
- Massive reduction in prompt size (10KB conversation → 2KB context)
- Preserves essential conversation flow
- Claude gets enough context to continue meaningfully
- Works even when original session is orphaned

### Approach 4: Hybrid with Foundation Models

Use Apple's on-device Foundation Models to generate summaries:

```swift
func buildSmartContext(_ sessionId: String) async -> String {
    let updates = loadHistoricalUpdates(sessionId)
    let messages = updates.filter {
        $0.sessionUpdate == .agent_message_chunk || $0.sessionUpdate == .user_message
    }

    // Summarize conversation with Foundation Models
    let summary = await ConversationSummarizer.summarizeConversation(messages)

    return """
    [Previous conversation summary]
    \(summary)

    [Last few exchanges]
    \(formatLastNMessages(messages, n: 4))
    """
}
```

**Benefits**:
- On-device processing (privacy)
- Intelligent summarization of key points
- Works for very long conversations
- Reduces prompt size even further

### Approach 5: Multi-Tier Context Strategy

Combine techniques based on conversation length:

```swift
func buildContextForSession(_ sessionId: String) async -> String {
    let updates = loadHistoricalUpdates(sessionId)
    let messageCount = updates.filter { isUserOrAssistantMessage($0) }.count

    switch messageCount {
    case 0...10:
        // Short conversation: Include everything (condensed)
        return buildCondensedContext(updates)

    case 11...50:
        // Medium: Smart filtering + recent messages
        return buildMediumContext(updates)

    case 51...:
        // Long: Foundation Models summary + recent exchanges
        return await buildSmartContext(sessionId)

    default:
        return buildCondensedContext(updates)
    }
}
```

### Approach 6: Progressive Context Loading

Start with minimal context, add more if needed:

```
User loads historical session, sends "continue the story"
  ↓
Bridge sends: claude --session-id <uuid> "[Summary: Space botanist story]\n\nUser: continue the story"
  ↓
If Claude responds: "I need more context about..."
  ↓
Bridge detects insufficient context
  ↓
Bridge sends follow-up with more detail
```

### Implementation Recommendation

**Best approach for v0.3.1**:

1. **Implement Approach 2** (Preemptive Context Injection) + **Approach 3** (Smart Condensing)
2. Add a `pendingHistoricalContext` field to `BridgeManager`
3. When loading historical session, set flag and store session ID
4. When user sends first message, build condensed context automatically
5. Create new session with context + user message
6. Subsequent messages use that new session normally

**Why this works**:
- No dependency on Claude Code's session storage
- Works for any historical session age
- Gives Claude enough context to continue meaningfully
- Automatic, no user action required
- Can be enhanced later with Foundation Models (Approach 4)

**Rough estimates**:
- Average conversation: 50 updates × 200 chars = 10KB raw
- After smart condensing: ~2KB context string
- Well within Claude's prompt limits
- Fast enough for real-time user experience

### Edge Cases to Handle

1. **Very first message was tool-heavy**
   - Don't include user message in context (it's all tool use)
   - Just include summary: "[Previous: analyzed codebase]"

2. **Conversation was mostly thinking**
   - Skip thought chunks entirely
   - Preserve only user/assistant messages

3. **Massive tool outputs** (file reads, directory listings)
   - Truncate aggressively: "Read README.md (2000 lines)..."
   - Or skip entirely if not relevant

4. **Context exceeds limits**
   - Fallback: Use only last N messages
   - Or: Use Foundation Models to summarize

### Testing Strategy

```swift
// Test context builder with real historical sessions
let testSessionId = "66147D82-FE0D-43C8-8351-0B29C38008AD"
let context = buildContextString(testSessionId)

print("Original size: \(calculateOriginalSize(testSessionId)) bytes")
print("Condensed size: \(context.utf8.count) bytes")
print("Reduction: \(calculateReduction())%")
print("\nContext preview:")
print(context.prefix(500))
```

### Summary of Workarounds

| Approach | Complexity | Effectiveness | Latency | Dependencies |
|----------|-----------|---------------|---------|--------------|
| Approach 1: Retry on failure | Medium | High | +2s | None |
| Approach 2: Preemptive inject | Low | High | +0.5s | None |
| Approach 3: Smart condensing | Low | Medium | +0.1s | None |
| Approach 4: Foundation Models | High | Very High | +1s | iOS 26+/macOS 15+ |
| Approach 5: Multi-tier | High | Very High | Variable | Approach 4 |
| Approach 6: Progressive | Medium | Medium | Variable | Retry logic |

**Recommendation**: Start with **Approach 2 + 3**, add **Approach 4** in v0.3.2+ for iOS 26+/macOS 15+ devices.

## Summary

**Key Takeaways**:

1. `--session-id` **creates** sessions, `--resume` **continues** them
2. Sessions must exist in Claude Code's local storage to be resumed
3. Tinyvex DB persists sessions permanently, but Claude Code storage is transient
4. Historical sessions become orphaned when Claude Code's local copy expires
5. Current solution (v0.3): Historical sessions are view-only; new messages start fresh sessions
6. **Practical workarounds exist**: Context injection + smart condensing can enable true continuation
7. Recommended implementation: Preemptive context injection with intelligent message filtering
8. Future enhancement: Foundation Models for on-device summarization of long conversations

---

*Last updated: 2025-11-07*
*Related issue: #1430 - Tinyvex History — Recent Sessions Drawer + Load Messages*

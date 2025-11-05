# iOS ACP Implementation Status Analysis

## Executive Summary

The iOS codebase has **Codex ACP implementation fully in place** with working translators, renderers, and bridge integration. However, **Claude Code integration is incomplete** — it has directory scanning but **NO translator** to convert Claude Code JSONL to ACP events.

**Current Provider Status:**
- **Codex**: Production-ready (translators + renderers + UI)
- **Claude Code**: Scanning only (no translator/renderer support)

---

## 1. Folder Structure & Architecture

### iOS Directory Layout
```
ios/
├── OpenAgents/                          # Main iOS app
│   ├── ACP/Renderers/                   # ACP event renderers (SwiftUI)
│   │   ├── ToolCallView.swift
│   │   ├── ToolResultView.swift
│   │   ├── PlanStateView.swift
│   │   └── RawEventView.swift
│   ├── Bridge/                          # WebSocket bridge management
│   │   ├── BridgeManager.swift
│   │   ├── MobileWebSocketClient.swift
│   │   ├── DesktopWebSocketServer.swift
│   │   └── BridgeStatusChip.swift
│   ├── History/                         # Local file scanning
│   │   ├── LocalCodex.swift             # Codex file discovery
│   │   └── LocalClaude.swift            # Claude Code file discovery
│   ├── AcpThreadView.swift              # Main thread rendering
│   └── HistorySidebar.swift             # History list UI
│
└── OpenAgentsCore/                      # Swift Package (shared library)
    └── Sources/OpenAgentsCore/
        ├── ACP/                         # ACP type definitions
        │   ├── ACPCommon.swift          # Common enums & JSONValue
        │   ├── ACPEvent.swift           # ACPEvent wrapper
        │   ├── ACPMessage.swift         # Message structure
        │   ├── ACPTool.swift            # Tool calls/results
        │   ├── ACPPlanState.swift       # Plan state tracking
        │   ├── ACPContent.swift         # Content parts
        │   └── ACPThread.swift          # Thread wrapper
        ├── Translators/
        │   └── CodexAcpTranslator.swift # CODEX ONLY — converts JSONL to ACP
        ├── Providers/
        │   ├── CodexScanner.swift       # Codex JSONL file discovery
        │   └── ClaudeScanner.swift      # Claude Code JSONL file discovery
        ├── AgentClientProtocol/         # ACP protocol types (Rust-mirrored)
        │   ├── acp.swift                # ACP namespace & docs
        │   ├── client.swift             # Client-side SessionUpdate enum
        │   ├── session.swift            # SessionNotificationWire
        │   ├── rpc.swift                # JSON-RPC protocol
        │   └── ...more types
        └── Bridge/
            ├── BridgeMessages.swift
            ├── JSONRPC.swift
            └── BridgeConfig.swift
```

---

## 2. Current ACP Components

### A. ACP Type Models (OpenAgentsCore/ACP/)

**Core Structures:**
- `ACPMessage`: id, thread_id, role, parts (text/tool), ts
- `ACPToolCall`: id, tool_name, arguments (JSONValue), ts
- `ACPToolResult`: call_id, ok, result (JSONValue), error, ts
- `ACPPlanState`: status, summary, steps, ts
- `ACPEvent`: wrapper with kind enum (message, tool_call, tool_result, plan_state)
- `ACPThread`: id, title, created_at, updated_at, events[]

**Key Type:**
```swift
public enum JSONValue: Equatable, Codable {
    case string(String), number(Double), object([String: JSONValue])
    case array([JSONValue]), bool(Bool), null
}
```
Strong typing with NO `Any` cast (per coding rules).

### B. ACP Protocol Types (AgentClientProtocol/)

**SessionUpdate Enum** (lines 254-376 of client.swift):
```swift
public enum SessionUpdate: Codable {
    case userMessageChunk(ContentChunk)
    case agentMessageChunk(ContentChunk)
    case agentThoughtChunk(ContentChunk)
    case plan(Plan)
    case availableCommandsUpdate(AvailableCommandsUpdate)
    case currentModeUpdate(CurrentModeUpdate)
    case toolCall(ACPToolCallWire)
    case toolCallUpdate(ACPToolCallUpdateWire)
}
```

**SessionNotificationWire** (session.swift:72):
```swift
public struct SessionNotificationWire: Codable {
    public var session_id: ACPSessionId
    public var update: SessionUpdate
    public var _meta: [String: AnyEncodable]?
}
```

### C. Bridge Integration

**Path 1: Mobile (iOS) — Typed ACP Updates**
- `MobileWebSocketClient` connects to Rust bridge via WebSocket
- `BridgeManager` receives `SessionNotificationWire` via JSON-RPC notification `session.update`
- AcpThreadView: `computeTimelineFromUpdates()` processes typed updates → timeline

**Path 2: Desktop (macOS) — Local JSONL File Loading**
- Loads from `~/.codex/sessions/` or `~/.claude/projects/`
- `CodexAcpTranslator.translateLines()` converts JSONL → ACPThread
- AcpThreadView: `computeTimelineFromLines()` processes lines → timeline

---

## 3. Current Translators

### CodexAcpTranslator (Full Implementation)
**File:** `/Users/christopherdavid/code/openagents/ios/OpenAgentsCore/Sources/OpenAgentsCore/Translators/CodexAcpTranslator.swift`

**What it does:**
- Parses Codex JSONL lines (array of JSON objects)
- Detects event types: messages, tool calls, tool results, plan states, reasoning
- Extracts timestamps, thread IDs, and metadata
- Generates stable IDs for events/calls using deterministic hash

**Supported Event Types:**
1. **session_meta** / **thread.started** → thread metadata
2. **user_message** → ACPMessage(role: .user)
3. **agent_message** → ACPMessage(role: .assistant)
4. **agent_reasoning** → reasoning message
5. **tool_call** → ACPToolCall
6. **tool_result** → ACPToolResult
7. **response_item** → provider-native function calls/outputs
8. **plan_state** → ACPPlanState
9. Lines with nested "type" in payloads are normalized

**Special Handling:**
- Tool command prettifier for shell tools
- Reasoning summary parsing from response_item.summary
- Flexible timestamp extraction (ts field + payload.ts)
- Argument unwrapping (string-encoded JSON)

**Tests:** CodexAcpTranslatorTests.swift (basic timeline, session meta generation)

### Claude Code Translator ⚠️ **MISSING**
**Status:** No `ClaudeAcpTranslator.swift` exists

**Gap:** ClaudeScanner can find Claude Code JSONL files, but there's no translator to convert them to ACP. The codebase has:
- `ClaudeScanner` (Providers/ClaudeScanner.swift): Finds files in `~/.claude/projects/`
- Local file discovery in LocalClaude.swift
- UI support showing `source: "claude_code"` in ThreadSummary

But no mechanism to convert Claude Code JSONL → ACPEvent.

---

## 4. UI Renderers (ACP/Renderers/)

### ToolCallView.swift
**Renders:** ACPToolCall

**Features:**
- Shows tool name with wrench icon
- Pretty-prints shell commands: detects `bash -lc <cmd>` pattern
- Falls back to pretty JSON for non-shell tools
- Handles argument unwrapping (string-encoded JSON payloads)
- Horizontal scroll for long args

**Code snippet:**
```swift
struct ToolCallView: View {
    let call: ACPToolCall
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack { Image(systemName: "wrench.and.screwdriver") ...
            if let cmd = prettyShellCommand(call: call) {
                Text(cmd) // shell-specific rendering
            } else if let pretty = try? prettyJSON(call.arguments) {
                Text(pretty) // generic JSON
            }
        }
    }
}
```

### ToolResultView.swift
**Renders:** ACPToolResult

**Features:**
- Shows success/error icon (checkmark.seal / xmark.seal)
- Displays error string in danger color if present
- Pretty-prints result JSON with horizontal scroll
- Color-coded by ok status

### PlanStateView.swift
**Renders:** ACPPlanState

**Features:**
- Status indicator circle (idle→gray, running→yellow, completed→green, failed→red)
- Summary text
- Numbered steps list with indentation
- Supports multi-step plans

### RawEventView.swift
**Renders:** Unknown/unmapped events

**Features:**
- JSON preview with truncation
- Tap-to-expand sheet for full content
- Used for fallback when event type not recognized

---

## 5. Data Flow & Integration Points

### iOS Bridge Flow (Current)
```
User opens app
  ↓
BridgeManager.start() [line 63]
  ↓
connect(host: "127.0.0.1", port: 8787)
  ↓
MobileWebSocketClient.connect()
  ↓
sendInitialize() [line 48] → JSON-RPC init
  ↓
Bridge connects, client receives JSON-RPC notification
  ↓
method == "session.update" [line 145]
  ↓
Decode ACP.Client.SessionNotificationWire
  ↓
BridgeManager.updates append [line 156+]
  ↓
AcpThreadView observes updates
  ↓
computeTimelineFromUpdates() [line 1033]
  ↓
Render timeline with typed components
```

### macOS Desktop Flow (Current)
```
User selects file in History sidebar
  ↓
HistorySidebar.onSelect
  ↓
AcpThreadView(url: selectedURL)
  ↓
loadThread() async [background]
  ↓
readLines(from: url) → [String]
  ↓
CodexAcpTranslator.translateLines() ← CODEX ONLY
  ↓
returns ACPThread with events[]
  ↓
computeTimelineFromLines() [line 958]
  ↓
Process events, attach to timeline[]
  ↓
Render timeline
```

### ⚠️ Claude Code Flow (Incomplete)
```
User has ~/.claude/projects/.../*.jsonl files
  ↓
LocalClaudeDiscovery.scanExactProjectTopK() [LocalClaude.swift:139]
  ↓
Returns [LocalThreadSummary] with source: "claude_code"
  ↓
HistorySidebar shows item with "claude_code" source
  ↓
User selects Claude Code file
  ↓
AcpThreadView(url: claudeCodeFile)
  ↓
readLines(from: url) → [String]
  ↓
CodexAcpTranslator.translateLines() ← STILL USES CODEX TRANSLATOR!
  ↓
??? FAILS OR PRODUCES WRONG EVENTS ???
```

**Issue:** Claude Code JSONL format differs from Codex. No translator checks the source; it blindly applies CodexAcpTranslator logic.

---

## 6. Provider Support Status

### Codex
- ✅ Scanner: CodexScanner finds `~/.codex/sessions/YYYY/MM/DD/*.jsonl`
- ✅ Translator: CodexAcpTranslator converts JSONL → ACP
- ✅ Tests: CodexAcpTranslatorTests validates basic flow
- ✅ UI: Renderers for tool calls, results, plan states
- ✅ Local file access: Desktop app loads and renders
- ✅ Live streaming: iOS bridge consumes typed SessionNotificationWire

### Claude Code
- ✅ Scanner: ClaudeScanner finds `~/.claude/projects/**/*.jsonl`
- ✅ Local discovery: LocalClaudeDiscovery enumerates files
- ❌ **Translator: MISSING** — no ClaudeAcpTranslator
- ✅ UI: Renderers exist but no data to render
- ❌ Data flow broken: Tries to use CodexAcpTranslator on incompatible format
- ❌ Live streaming: Not yet supported (no Claude Code agent integration)

---

## 7. Key Gaps & TODOs

### Critical
1. **No Claude Code Translator** (Lines 254-376 of client.swift show SessionUpdate doesn't distinguish source)
   - Need to parse Claude Code JSONL format
   - Implement `ClaudeAcpTranslator.swift` with similar API to CodexAcpTranslator
   - Add source detection logic to AcpThreadView or translator dispatcher

2. **No translator dispatcher** in AcpThreadView
   - Currently hardcodes CodexAcpTranslator at line 977 and 1028
   - Should check source and route to correct translator
   - Example:
     ```swift
     let translator = url.path.contains("claude") ? ClaudeAcpTranslator.self : CodexAcpTranslator.self
     let thread = translator.translateLines(lines, options: ...)
     ```

3. **Claude Code JSONL format unknown** to iOS team
   - Need to document Claude Code event structure (messages, tool calls, etc.)
   - Determine if format differs significantly from Codex
   - Collect sample Claude Code JSONL for testing

### High Priority
4. **Tests for ClaudeAcpTranslator** (once implemented)
   - Mirror CodexAcpTranslatorTests with Claude Code samples
   - Validate timestamp extraction, event parsing, ID generation

5. **Live Claude Code streaming** on iOS
   - Bridge currently hardcoded for Codex agents
   - No support for Claude Code agent sessions yet
   - Would require upstream bridge changes

### Medium Priority
6. **Provider metadata in ThreadSummary**
   - Currently just a string: `source: String // "codex" | "claude_code"`
   - Could be enum or typed struct for better type safety

7. **Unified event filtering**
   - Both translators hide metadata (session_meta, instructions, environment_context)
   - Could extract to shared helper

---

## 8. File Inventory & Line Numbers

### ACP Type Models
| File | Purpose | Key Lines |
|------|---------|-----------|
| ACPCommon.swift | JSONValue enum, ACPRole, ACPId | 1-56 |
| ACPMessage.swift | Message structure | 1-20 |
| ACPTool.swift | ACPToolCall, ACPToolResult | 1-36 |
| ACPPlanState.swift | ACPPlanStatus, ACPPlanState | 1-26 |
| ACPEvent.swift | ACPEvent wrapper | 1-41 |
| ACPContent.swift | ACPContentPart, ACPText | 1-46 |
| ACPThread.swift | ACPThread wrapper | (brief) |

### Bridge & Protocol
| File | Purpose | Key Lines |
|------|---------|-----------|
| AgentClientProtocol/client.swift | SessionUpdate enum | 254-376 |
| AgentClientProtocol/session.swift | SessionNotificationWire | 72-78 |
| Bridge/BridgeManager.swift | iOS: loads latest typed | 121-132 |
| MobileBridge/MobileWebSocketClient.swift | JSON-RPC client | 1-100 |
| Bridge/BridgeMessages.swift | Envelope, message types | 1-98 |

### Translators & Scanners
| File | Purpose | Key Lines |
|------|---------|-----------|
| **CodexAcpTranslator.swift** | **ONLY translator** | 1-245 |
| Providers/CodexScanner.swift | File discovery | 1-150+ |
| Providers/ClaudeScanner.swift | File discovery | 1-100 |
| History/LocalCodex.swift | Local file scanning | 1-278 |
| History/LocalClaude.swift | Local file scanning | 1-197 |

### UI Renderers
| File | Purpose | Lines |
|------|---------|-------|
| ACP/Renderers/ToolCallView.swift | Tool call UI | 1-112 |
| ACP/Renderers/ToolResultView.swift | Tool result UI | 1-39 |
| ACP/Renderers/PlanStateView.swift | Plan state UI | 1-56 |
| ACP/Renderers/RawEventView.swift | Fallback UI | (brief) |

### Timeline Processing
| File | Purpose | Key Lines |
|------|---------|-----------|
| AcpThreadView.swift | Main rendering logic | 1-1100+ |
| — computeTimelineFromLines() | JSONL processing | 958-1030 |
| — computeTimelineFromUpdates() | ACP update processing | 1033-1110 |

---

## 9. Recommendations for Claude Code Parity

### Phase 1: Immediate (Foundation)
1. **Investigate Claude Code JSONL format**
   - Collect 3-5 sample Claude Code session files
   - Document structure (event types, timestamp format, nesting)
   - Compare vs. Codex format

2. **Implement ClaudeAcpTranslator**
   - Mirror CodexAcpTranslator.swift structure
   - Handle Claude Code event types (may differ from Codex)
   - Use same JSONValue/ACPMessage/ACPToolCall models
   - Public static method: `translateLines(_ lines: [String], options: Options) -> ACPThread`

3. **Add translator dispatch logic in AcpThreadView**
   - Detect source from URL or ThreadSummary
   - Route to appropriate translator

### Phase 2: Validation (Testing)
4. **Unit tests for ClaudeAcpTranslator**
   - Create ClaudeAcpTranslatorTests.swift (mirror Codex tests)
   - Test basic timeline, metadata, all event types

5. **Integration test**
   - Load actual Claude Code JSONL file
   - Verify timeline renders without crashes

### Phase 3: Enhancement (Polish)
6. **Live Claude Code streaming** (iOS bridge feature)
   - Coordinate with desktop bridge to support Claude Code agents
   - Route SessionNotificationWire from Claude Code sessions

7. **Type-safe provider enum**
   - Replace `source: String` with `enum Provider { case codex, claude_code }`
   - Better compile-time safety

---

## 10. Current Implementation Quality

### Strengths
✅ **Type-safe ACP models** — No `Any` casts, uses JSONValue enum
✅ **Tested translator** — CodexAcpTranslatorTests covers basic flows
✅ **Good renderer separation** — Dedicated SwiftUI views per event type
✅ **Flexible parser** — Handles nested payloads, variant field names
✅ **Robust timeline building** — Deduplication, reasoning grouping, timestamp fallback
✅ **Both local & streaming** — Desktop file loading + iOS bridge updates

### Weaknesses
⚠️ **Single translator** — No Claude Code support
⚠️ **Hardcoded in UI** — CodexAcpTranslator called without dispatch logic
⚠️ **Source string** — Untyped "codex" | "claude_code" everywhere
⚠️ **Limited tests** — Only CodexAcpTranslator tested
⚠️ **No provider docs** — Each provider's JSONL format not formally documented

---

## Conclusion

The iOS ACP implementation is **production-ready for Codex** with full translator, renderers, and bridge support. **Claude Code parity requires:**

1. **Implement ClaudeAcpTranslator** to convert Claude Code JSONL → ACP events
2. **Add source detection** to dispatch correct translator
3. **Add tests** validating Claude Code parsing
4. **Document provider formats** for future maintenance

Estimated effort: **2-3 days** to implement translator + tests + integration.

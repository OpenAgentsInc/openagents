# Plan: APM (Actions Per Minute) Measurement for MechaCoder

## Goal

Measure and display APM for MechaCoder, porting the algorithm from the original Rust/Tauri implementation to the new Effect-TS orchestrator.

## Background

### Original APM Algorithm (from deleted Rust code in commit `7bfcb4bc`)

**Formula:** `APM = (message_count + tool_count) / duration_minutes`

**Data Source:** Claude Code conversation files at `~/.claude/projects/*/*.jsonl`

**Time Windows:** 1h, 6h, 1d, 1w, 1m, lifetime (wall-clock time, not active time)

**Metrics Tracked:**
- Messages (user + assistant)
- Tool calls (extracted from `message.content[].type == "tool_use"`)
- Tool categories: Code Generation, File Operations, System Operations, Search, Planning, Other
- Productivity by time of day (morning/afternoon/evening/night)
- Per-session breakdown

### Current Architecture Integration Points

| Component | Location | Purpose |
|-----------|----------|---------|
| Event emitter | `overnight.ts:511-564` | Already emits `OrchestratorEvent` for all actions |
| MetricsCollector | `src/bench/metrics.ts` | Turn-level metrics pattern to follow |
| HUD callbacks | `overnight.ts:508` | Real-time UI updates via WebSocket |
| Session tracking | `src/agent/session.ts` | JSONL session persistence |

## Implementation Plan

### Phase 1: Core APM Module

**Create `src/agent/apm.ts`** with:

```typescript
// APM types (following Effect Schema pattern like metrics.ts)
export const APMStats = S.Struct({
  apm1h: S.Number,
  apm6h: S.Number,
  apm1d: S.Number,
  apm1w: S.Number,
  apm1m: S.Number,
  apmLifetime: S.Number,
  totalSessions: S.Number,
  totalMessages: S.Number,
  totalToolCalls: S.Number,
  totalDurationMs: S.Number,
  toolUsage: S.Array(ToolUsage),
  productivityByTime: ProductivityByTime,
});

// APM collector (following MetricsCollector pattern)
export class APMCollector {
  recordAction(type: "message" | "tool_call", toolName?: string): void
  getSessionAPM(): number
  finalize(): SessionAPMStats
}
```

### Phase 2: Hook into Orchestrator Event Stream

**Modify `overnight.ts`** emit callback to track APM:

```typescript
// In overnightLoopOrchestrator, around line 510
const apmCollector = new APMCollector();

const emit = (event: OrchestratorEvent) => {
  // Track APM-relevant events
  if (event.type === "subtask_complete") {
    apmCollector.recordAction("tool_call", event.result.agent);
  }
  // ... existing logging
};
```

### Phase 3: Claude Code Conversation Parser

**Create `src/agent/apm-parser.ts`** to read historical Claude Code data:

```typescript
// Port the Rust analyzer logic
export const parseClaudeConversations = Effect.gen(function* () {
  const claudeDir = path.join(os.homedir(), ".claude", "projects");
  // Scan *.jsonl files
  // Parse each line as ConversationEntry
  // Extract messages, tool calls, timestamps
  // Calculate APM per time window
});
```

### Phase 4: CLI Command

**Create `src/cli/apm.ts`** for user-facing APM display:

```bash
# Usage
bun src/cli/apm.ts              # Show lifetime APM stats
bun src/cli/apm.ts --session    # Show current session APM
bun src/cli/apm.ts --watch      # Live-update APM display
```

### Phase 5: HUD Integration (Optional)

**Add APM message type to `src/hud/protocol.ts`:**

```typescript
| { type: "apm_update"; apm: number; window: string }
```

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `src/agent/apm.ts` | Create | Core APM types and collector |
| `src/agent/apm-parser.ts` | Create | Claude Code conversation parser |
| `src/cli/apm.ts` | Create | CLI command for APM stats |
| `src/agent/overnight.ts` | Modify | Hook APM collector into emit callback |
| `src/hud/protocol.ts` | Modify | Add APM message type (optional) |
| `docs/apm.md` | Create | APM spec as mentioned in video |

## Key Algorithm Details (from Rust source)

### Tool Categories

```typescript
const getToolCategory = (name: string): string => {
  switch (name) {
    case "Edit": case "MultiEdit": case "Write": return "Code Generation";
    case "Read": case "LS": case "Glob": return "File Operations";
    case "Bash": return "System Operations";
    case "Grep": case "WebSearch": case "WebFetch": return "Search";
    case "TodoWrite": case "TodoRead": return "Planning";
    default: return "Other";
  }
};
```

### APM Calculation (Lifetime)

```typescript
// Lifetime APM uses wall-clock time from first to last conversation
const apmLifetime = totalDurationMinutes > 0
  ? (totalMessages + totalToolCalls) / totalDurationMinutes
  : 0;
```

### Time Windows

```typescript
// APM for window = (actions in window) / (window duration in minutes)
const apm1h = calculateWindowAPM(sessions, 1, 60);      // 1 hour
const apm6h = calculateWindowAPM(sessions, 6, 360);     // 6 hours
const apm1d = calculateWindowAPM(sessions, 24, 1440);   // 1 day
const apm1w = calculateWindowAPM(sessions, 168, 10080); // 1 week
const apm1m = calculateWindowAPM(sessions, 720, 43200); // 30 days
```

## Testing

- Unit tests for APM calculation (`apm.test.ts`)
- Integration test with mock JSONL data
- Verify against known baseline (2.3 APM from video)

## Design Decisions (from user feedback)

1. **Data Sources:** Track BOTH Claude Code and MechaCoder separately
   - Compare APM between direct Claude Code usage vs MechaCoder
   - Break down by source to see differences
   - Note: Need to investigate if MechaCoder's Claude Code usage saves to `~/.claude/projects/`

2. **Display:** Both CLI command + HUD widget
   - CLI for on-demand stats with comparison view
   - HUD for live monitoring during MechaCoder runs

3. **Persistence:** Hybrid approach
   - Store historical metrics in `.openagents/apm.json` for fast access
   - Compute on-demand for fresh data when requested
   - Cache invalidation when new sessions detected

## Updated Data Model

```typescript
// APM broken out by source
export const APMBySource = S.Struct({
  claudeCode: APMStats,      // Direct Claude Code usage
  mechaCoder: APMStats,      // MechaCoder orchestrator runs
  combined: APMStats,        // Aggregate total
  comparison: S.Struct({
    apmDelta: S.Number,      // mechaCoder.apm - claudeCode.apm
    efficiencyRatio: S.Number, // mechaCoder.apm / claudeCode.apm
  }),
});

// Persisted cache
export const APMCache = S.Struct({
  lastUpdated: S.String,     // ISO timestamp
  lastSessionId: S.String,   // Most recent session processed
  stats: APMBySource,
});
```

## Investigation Needed

Before implementation, verify:
1. Does MechaCoder's Claude Code invocation save to `~/.claude/projects/`?
2. If yes, how to distinguish MechaCoder sessions from direct usage?
3. If no, MechaCoder APM must be tracked via orchestrator events only

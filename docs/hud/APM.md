# APM in the HUD

> **APM = Actions Per Minute** - the core velocity metric for MechaCoder and Claude Code.

This document describes how APM integrates with the HUD WebSocket protocol for real-time display in the Electrobun desktop app.

## Overview

APM measures agent velocity as:

```
APM = (messages + tool_calls) / duration_minutes
```

The HUD displays:
1. **Real-time session APM** - Current session velocity (updated as work happens)
2. **Historical comparison** - MechaCoder vs Claude Code efficiency
3. **Time window metrics** - APM over 1h, 6h, 1d, 1w, 1m, lifetime

## HUD Message Types

### APM Update (Real-time)

Sent periodically during MechaCoder execution to update the HUD display:

```typescript
interface APMUpdateMessage {
  type: "apm_update";
  sessionId: string;
  sessionAPM: number;        // Current session APM
  recentAPM: number;         // Last 5 minutes
  totalActions: number;      // Messages + tool calls this session
  durationMinutes: number;   // Session duration
}
```

### APM Snapshot (Session Start/End)

Sent at session boundaries with full historical context:

```typescript
interface APMSnapshotMessage {
  type: "apm_snapshot";
  combined: {
    apm1h: number;
    apm6h: number;
    apm1d: number;
    apm1w: number;
    apm1m: number;
    apmLifetime: number;
    totalSessions: number;
    totalActions: number;
  };
  comparison: {
    claudeCodeAPM: number;
    mechaCoderAPM: number;
    efficiencyRatio: number;  // mechaCoder / claudeCode
  };
}
```

### Tool Usage Update

Sent when tool breakdown changes significantly:

```typescript
interface APMToolUsageMessage {
  type: "apm_tool_usage";
  tools: Array<{
    name: string;
    count: number;
    percentage: number;
    category: string;  // "Code Generation" | "File Operations" | etc.
  }>;
}
```

## Event Flow

```
┌─────────────────────┐
│   APMCollector      │  Records actions during session
│   (overnight.ts)    │
└──────────┬──────────┘
           │ recordAction("tool_call", "Edit")
           ▼
┌─────────────────────┐
│   apm_update        │  Periodic updates (every 30s or on significant change)
│   HudMessage        │
└──────────┬──────────┘
           │ ws://localhost:4242
           ▼
┌─────────────────────┐
│   Electrobun HUD    │  Renders APM widget
│   APM Widget        │
└─────────────────────┘
```

## Integration Points

### 1. Orchestrator (overnight.ts)

Create APMCollector at session start, record actions in emit callback:

```typescript
const apmCollector = new APMCollector(sessionId, projectName);

const emit = (event: OrchestratorEvent) => {
  // Track APM-relevant events
  switch (event.type) {
    case "subtask_start":
      apmCollector.recordAction("message");
      break;
    case "subtask_complete":
      // Count tool calls from result
      apmCollector.recordAction("tool_call", event.result.agent);
      break;
  }

  // Periodic APM updates to HUD
  if (shouldSendAPMUpdate()) {
    hudClient.send({
      type: "apm_update",
      sessionId,
      sessionAPM: apmCollector.getSessionAPM(),
      recentAPM: apmCollector.getRecentAPM(5),
      totalActions: apmCollector.actions.length,
      durationMinutes: apmCollector.getDurationMinutes(),
    });
  }

  // Forward other events
  hudEmit(event);
};
```

### 2. Protocol (src/hud/protocol.ts)

Add APM message types to the HudMessage union:

```typescript
export interface APMUpdateMessage {
  type: "apm_update";
  sessionId: string;
  sessionAPM: number;
  recentAPM: number;
  totalActions: number;
  durationMinutes: number;
}

export interface APMSnapshotMessage {
  type: "apm_snapshot";
  combined: APMTimeWindows;
  comparison: APMComparison;
}

export interface APMToolUsageMessage {
  type: "apm_tool_usage";
  tools: ToolUsageItem[];
}

export type HudMessage =
  | SessionStartMessage
  | SessionCompleteMessage
  // ... existing types ...
  | APMUpdateMessage
  | APMSnapshotMessage
  | APMToolUsageMessage;
```

### 3. HUD Client (src/hud/emit.ts)

Add helpers for emitting APM messages:

```typescript
export const createAPMEmitter = (client: HudClient) => {
  let lastUpdate = 0;
  const UPDATE_INTERVAL = 30000; // 30 seconds

  return (collector: APMCollector) => {
    const now = Date.now();
    if (now - lastUpdate < UPDATE_INTERVAL) return;
    lastUpdate = now;

    client.send({
      type: "apm_update",
      sessionId: collector.sessionId,
      sessionAPM: collector.getSessionAPM(),
      recentAPM: collector.getRecentAPM(5),
      totalActions: collector.actions.length,
      durationMinutes: collector.getDurationMinutes(),
    });
  };
};
```

## HUD Widget Spec

The APM widget renders as an **SVG overlay** in the Electrobun mainview, following the same architecture as the Flow visualization.

### Widget Layout

```
┌─────────────────────────────────────┐
│  APM: 18.97  ▲ 4.2x faster          │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━    │
│  Session: 12.3 APM | 47 actions     │
│  ─────────────────────────────────  │
│  1h: 12.9 | 6h: 20.1 | 24h: 12.3    │
│  MechaCoder vs Claude Code: +322%   │
└─────────────────────────────────────┘
```

### SVG Rendering

Following the pattern in `src/flow-host-svg/render.ts`:

```typescript
// APM overlay SVG group (positioned in viewport, not canvas)
const apmWidget: SVGGroup = {
  type: "group",
  transform: "translate(20, 20)", // Top-left corner
  children: [
    // Background rect with theme colors
    { type: "rect", x: 0, y: 0, width: 280, height: 120, fill: "#141017", stroke: "rgba(245, 158, 11, 0.25)" },
    // APM value text
    { type: "text", x: 16, y: 32, text: `APM: ${apm.toFixed(2)}`, fill: "#f59e0b", fontSize: 24 },
    // ... more elements
  ],
};
```

### Colors (matching Flow theme)

| APM Range | Color | Meaning |
|-----------|-------|---------|
| 0-5 | `#6b7280` (gray) | Baseline/idle |
| 5-15 | `#3b82f6` (blue) | Active |
| 15-30 | `#22c55e` (green) | High velocity |
| 30+ | `#f59e0b` (amber/gold) | Elite performance |

### Message Handling in mainview/index.ts

```typescript
messages: {
  hudMessage: (message: HudMessage) => {
    switch (message.type) {
      case "apm_update":
        updateAPMWidget(message.sessionAPM, message.recentAPM, message.totalActions);
        break;
      case "apm_snapshot":
        updateAPMSnapshot(message.combined, message.comparison);
        break;
    }
  },
}
```

## Related Files

| File | Purpose |
|------|---------|
| `src/agent/apm.ts` | Core APM types and APMCollector |
| `src/agent/apm-parser.ts` | Historical data parser |
| `src/cli/apm.ts` | CLI command |
| `src/hud/protocol.ts` | HUD message types |
| `src/hud/emit.ts` | Event conversion + APM emitters |
| `src/agent/overnight.ts` | Orchestrator integration |
| `src/mainview/index.ts` | HUD controller (add APM widget here) |
| `src/flow-host-svg/render.ts` | SVG rendering patterns |
| `docs/apm.md` | APM specification |

## Tasks

- [x] `oa-4cbc15` - Core APM types and collector
- [x] `oa-d2ad64` - Claude Code conversation parser
- [x] `oa-20d3d1` - CLI command
- [x] `oa-5a59e1` - Integrate APM into overnight.ts orchestrator
- [x] `oa-710f93` - Add APM message types to HUD protocol
- [x] `oa-2248cf` - Create APM emitter helpers in emit.ts
- [ ] `oa-25ca85` - Build APM SVG widget in mainview/index.ts

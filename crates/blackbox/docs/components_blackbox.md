# BlackBox Components

UI components for displaying BlackBox session logs. Follows atomic design: atoms → molecules → organisms → templates.

---

## Atoms

### StepBadge

```
[42]
```

**Purpose:** Displays step number from `step=N`.

**Behavior:**
- Monospace font, muted background
- Click → scrolls to/highlights that step
- Tooltip: "Step 42"

---

### TimestampBadge

```
00:15:23        ← elapsed time (# t=HH:MM:SS)
03:21:08Z       ← wall clock (ts=...)
```

**Purpose:** Shows time in either elapsed or ISO format.

**Behavior:**
- Elapsed time from `# t=` comments (session-relative)
- Wall clock from `ts=` fields (absolute)
- Hover shows full ISO timestamp
- Click copies to clipboard

---

### CallIdBadge

```
call_47
```

**Purpose:** Shows call ID for correlation (`id=call_N`).

**Behavior:**
- Clickable → highlights all lines with same ID (call + observations)
- Color-coded by call type (tool=yellow, mcp=cyan, subagent=red)

---

### CostBadge

```
$0.0045
```

**Purpose:** Shows cost from metrics lines.

**Behavior:**
- Green for low, yellow for medium, red for high
- Hover shows token breakdown

---

### TokenBadge

```
1.2k in · 89 out
```

**Purpose:** Compact token count display.

**Behavior:**
- Shows prompt_tokens / completion_tokens
- Cached tokens shown dimmed if present
- Abbreviates: 1200 → 1.2k

---

### LatencyBadge

```
340ms
```

**Purpose:** Shows `latency_ms=N` for tool calls.

**Behavior:**
- Green <1s, yellow 1-5s, red >5s
- Hover shows exact value

---

### AttemptBadge

```
2/3
```

**Purpose:** Shows retry attempt from `attempt=N/M`.

**Behavior:**
- Only shown when attempt > 1
- Orange color to indicate retry

---

### TidBadge

```
tid:2
```

**Purpose:** Shows thread/agent ID for concurrent operations.

**Behavior:**
- Color-coded per thread (tid:1=default, tid:2=blue, tid:3=green, etc.)
- Click filters to show only that thread

---

### StatusDot

```
●  ← green (success)
●  ← blue (running)
●  ← yellow (pending)
●  ← red (error)
○  ← gray (skipped)
```

**Purpose:** Visual status indicator.

**Behavior:**
- Pulses when running
- Tooltip shows status text

---

### LineTypeLabel

```
USER         ← user message
AGENT        ← agent message
TOOL         ← tool call
OBSERVATION  ← deferred result
SKILL        ← skill activation
PLAN         ← plan action
MODE         ← mode change
RECALL       ← memory recall
SUBAGENT     ← subagent spawn
MCP          ← mcp call
QUESTION     ← question/clarification
#            ← comment/meta
@            ← lifecycle event
◐            ← phase transition
```

**Purpose:** Type label for each line.

**Behavior:**
- Uppercase, monospace
- Preceded by StatusDot (colored by state)
- Comment/lifecycle/phase use symbol instead of word

---

### BlobRef

```
@blob sha256=a1b2c3d4 · 12.8KB · text/markdown
```

**Purpose:** Shows external blob reference.

**Behavior:**
- Click opens BlobViewer modal
- Shows size and mime type
- Truncated hash (first 8 chars)

---

### RedactedValue

```
[redacted:api_key]
```

**Purpose:** Shows redacted sensitive value.

**Behavior:**
- Red background, lock icon
- Tooltip: "Redacted: api_key"
- Not copyable

---

### ResultArrow

```
→
```

**Purpose:** Separator between call and result.

**Behavior:**
- Dimmed color
- Stretches to fill space in flex layout

---

## Molecules

### LineMeta

```
[42] 00:15:23 call_47 340ms
```

**Purpose:** Combines step + timestamp + call_id + latency badges.

**Behavior:**
- Right-aligned in line header
- Badges appear/hide based on available data
- Compact on mobile

---

### LineHeader

```
● TOOL  read                     [42] 00:15:23
```

**Purpose:** Full line header with status dot, type label, name, and metadata.

**Behavior:**
- Status dot (colored by state) + type label
- Tool/skill/subagent name
- LineMeta on right

---

### ResultDisplay

```
→ [ok]
→ [186 lines]
→ [err: permission denied]
→ @blob sha256=a1b2c3d4 · 12.8KB
```

**Purpose:** Renders result portion after arrow.

**Behavior:**
- `[ok]` → green checkmark
- `[N lines/files]` → count badge
- `[err: msg]` → red with error message
- `@blob` → BlobRef component
- Long results truncated with expand

---

### ModeIndicator

```
┌──────────────┐
│ MODE: auto   │  ← green background
└──────────────┘

┌──────────────┐
│ MODE: plan   │  ← blue background
└──────────────┘

┌──────────────┐
│ MODE: chat   │  ← gray background
└──────────────┘
```

**Purpose:** Shows current session mode.

**Behavior:**
- Sticky in sidebar or floating
- Updates on `m:` lines
- Tooltip shows mode description

---

### PhaseIndicator

```
PLAN MODE
┌─────────────────────────────────────┐
│ ● explore  ● design  ○ review  ○ final  ○ exit │
└─────────────────────────────────────┘
```

**Purpose:** Shows progress through plan mode phases.

**Behavior:**
- Only visible during `m:plan`
- Filled dots for completed phases
- Current phase highlighted
- Click phase → jumps to that `@phase` line

---

### BudgetMeter

```
Budget: ████████░░ $38.17 / $50.00
```

**Purpose:** Visual budget remaining display.

**Behavior:**
- Progress bar fills as budget used
- Green → yellow → red as depleted
- Updates on `# budget:` lines
- Hover shows token count

---

### CostAccumulator

```
Session cost: $12.47  ↑$0.02
```

**Purpose:** Running total with delta.

**Behavior:**
- Accumulates from `# metrics` lines
- Shows delta since last update
- Sticky in header/sidebar

---

## Organisms

### UserLine

```
┌─────────────────────────────────────────────────┐
│ ● USER                           [43] 00:09:05  │
├─────────────────────────────────────────────────┤
│ Hey, looks good! I'm heading to bed. Quick      │
│ thing - the Daytona API needs auth.             │
└─────────────────────────────────────────────────┘
```

**Purpose:** Full user message display.

**Behavior:**
- Distinct background (user color)
- Avatar on left
- Full message content
- Markdown rendering for code blocks

---

### AgentLine

```
┌─────────────────────────────────────────────────┐
│ ● AGENT                          [46] 00:09:18  │
├─────────────────────────────────────────────────┤
│ Acknowledged. I'll use the token for Daytona    │
│ API calls. Continuing with implementation.      │
├─────────────────────────────────────────────────┤
│ tokens: 2.4k in · 62 out · $0.0018              │
└─────────────────────────────────────────────────┘
```

**Purpose:** Full agent message with optional metrics.

**Behavior:**
- Distinct background (agent color)
- Collapsible metrics footer
- Markdown rendering
- Links `# metrics step=N` to this line

---

### ToolLine

```
┌─────────────────────────────────────────────────┐
│ ● TOOL  read                     [8] 00:01:00   │
│         docs/decisions/userstory.md             │
├─────────────────────────────────────────────────┤
│ [▸] → [19 lines]                                │
└─────────────────────────────────────────────────┘

[Expanded:]
┌─────────────────────────────────────────────────┐
│ ● TOOL  read                     [8] 00:01:00   │
│         docs/decisions/userstory.md             │
├─────────────────────────────────────────────────┤
│ [▾]                                             │
│   User logs in with GitHub, selects repo        │
│   Single button: full auto toggle (starts off)  │
│   Single text input to communicate with Auto... │
│   Routes: / (home), /logout, /settings          │
│                                                 │
│ → [19 lines]                                    │
└─────────────────────────────────────────────────┘
```

**Purpose:** Collapsible tool call display.

**Behavior:**
- Shows tool name + primary arg
- Collapsible content area
- Result indicator (lines/ok/err/blob)
- Left border color by status
- Links to observation if async

---

### ToolStreamLine

```
┌─────────────────────────────────────────────────┐
│ ● TOOL  test                     [56] 00:35:00  │
│         cargo test -p platform sandbox          │
├─────────────────────────────────────────────────┤
│ ● [running]                     ← start         │
│   running 4 tests...            ← progress      │
│   running 4 tests...            ← progress      │
│ ● [ok] 4 tests passed · 8.4s    ← complete      │
└─────────────────────────────────────────────────┘
```

**Purpose:** Streaming tool with start/progress/complete phases.

**Behavior:**
- Groups `t!:`, `t~:`, `t:` with same ID
- Shows progress updates inline
- Animates status dot while running
- Final latency shown on complete

---

### ObservationLine

```
┌─────────────────────────────────────────────────┐
│ ● OBSERVATION                    id=call_92     │
├─────────────────────────────────────────────────┤
│ → [ok] 128 tests passed                         │
│                                                 │
│ ↳ Links to: TOOL test [step 56]                 │
└─────────────────────────────────────────────────┘
```

**Purpose:** Deferred result linked to original call.

**Behavior:**
- Shows call_id prominently
- Links back to originating tool/mcp/subagent
- Can be partial (`partial=`) or final

---

### SubagentLine

```
┌─────────────────────────────────────────────────┐
│ ● SUBAGENT  explore              [24] 00:04:45  │
│             "What are the main architectural    │
│              components?"                       │
├─────────────────────────────────────────────────┤
│ tid:2  session_id=sess_sub_1                    │
│                                                 │
│ → summary: "6 crates identified"                │
│   [View full trajectory →]                      │
└─────────────────────────────────────────────────┘
```

**Purpose:** Subagent spawn with trajectory reference.

**Behavior:**
- Shows subagent type (explore/plan)
- Task description in quotes
- Thread ID badge
- Link to full subagent trajectory (opens SubagentViewer)
- Summary inline

---

### QuestionLine

```
┌─────────────────────────────────────────────────┐
│ ● QUESTION                       [q_1] 00:04:22 │
│   "Which auth library should we use?"           │
├─────────────────────────────────────────────────┤
│ Options:                                        │
│   ○ JWT                                         │
│   ● OAuth        ← selected                     │
│   ○ Session                                     │
├─────────────────────────────────────────────────┤
│ → [selected: OAuth]                             │
└─────────────────────────────────────────────────┘
```

**Purpose:** Question with options and answer.

**Behavior:**
- Shows question text
- Radio/checkbox options if provided
- Selected answer highlighted
- `[pending]` shows waiting spinner
- `[auto: X, reason="..."]` shows auto-decision with rationale

---

### McpLine

```
┌─────────────────────────────────────────────────┐
│ ● MCP  github.issues             [13] 00:02:00  │
│        state=open                               │
├─────────────────────────────────────────────────┤
│ → [8 issues]                                    │
│   #47 Admin dashboard (P0)                      │
│   #48 Lease expiration job                      │
│   #49 Issue service tests                       │
│   [5 more...]                                   │
└─────────────────────────────────────────────────┘
```

**Purpose:** MCP server call display.

**Behavior:**
- Shows server.method
- Arguments as tags
- Collapsible result list
- Same expand/collapse as ToolLine

---

### RecallLine

```
┌─────────────────────────────────────────────────┐
│ ● RECALL                         [4] 00:00:15   │
│   "platform" "priorities"                       │
├─────────────────────────────────────────────────┤
│ → [2 matches]                                   │
│   sess_20251217: discussed Phase 1 roadmap      │
│   sess_20251215: reviewed issue system design   │
└─────────────────────────────────────────────────┘
```

**Purpose:** Memory recall with matches.

**Behavior:**
- Query terms highlighted
- Match count badge
- Session IDs linkable (if available)
- Expandable match details

---

### PhaseLine

```
┌─────────────────────────────────────────────────┐
│ ◐ PHASE                                         │
│   ━━━━━━━━━━ EXPLORE ━━━━━━━━━━                 │
└─────────────────────────────────────────────────┘
```

**Purpose:** Plan mode phase transition marker.

**Behavior:**
- Full-width divider style
- Phase name centered
- Color-coded by phase
- Updates PhaseIndicator in sidebar

---

### LifecycleLine

```
┌─────────────────────────────────────────────────┐
│ @ START                          [1] 00:00:00   │
│   id=sess_12h budget=$50 duration=12h           │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│ @ CHECKPOINT                     [72] 04:00:00  │
│   hour=4 tokens=145000 cost=$12.30              │
├─────────────────────────────────────────────────┤
│   Progress: 4/12 hours                          │
│   Budget: ████████░░ $37.70 remaining           │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│ @ END                            [125] 12:00:00 │
│   summary=@blob sha256=f1a2b3c4d5e6             │
├─────────────────────────────────────────────────┤
│   duration: 12h 0m                              │
│   issues_completed: 8                           │
│   prs_merged: 5                                 │
│   cost: $42.17                                  │
└─────────────────────────────────────────────────┘
```

**Purpose:** Session lifecycle events.

**Behavior:**
- @start: Shows session config
- @checkpoint: Progress summary, updates meters
- @end: Final summary with all stats
- @pause/@resume: Show reason
- @notify/@escalate: Alert styling

---

### MetricsLine

```
┌─────────────────────────────────────────────────┐
│ # METRICS                        step=7         │
├─────────────────────────────────────────────────┤
│ prompt: 2,400  completion: 62  cached: 1,800    │
│ cost: $0.0018                                   │
└─────────────────────────────────────────────────┘
```

**Purpose:** Per-step token/cost metrics.

**Behavior:**
- Links to associated agent line by step
- Compact inline display
- Hover shows full breakdown

---

### TimeMarker

```
━━━━━━━━━━━━━━ 04:00:00 ━━━━━━━━━━━━━━
```

**Purpose:** Elapsed time divider from `# t=HH:MM:SS`.

**Behavior:**
- Full-width horizontal rule
- Time centered
- Click copies timestamp
- Used for visual scanning

---

### HourDivider

```
═══════════════════════════════════════════════════
 HOUR 4-5: AUTOPILOT REFACTORING
═══════════════════════════════════════════════════
```

**Purpose:** Section divider for multi-hour sessions.

**Behavior:**
- Parsed from `# ═══` comment patterns
- Collapsible section
- Jump target for navigation

---

## Section Organisms

### SessionHeader

```
┌─────────────────────────────────────────────────┐
│ SESSION  sess_12h_20251218_001           [▾]    │
├─────────────────────────────────────────────────┤
│ Model:    sonnet-4                              │
│ Mode:     auto                                  │
│ Repo:     OpenAgentsInc/platform @ 215db51      │
│ Branch:   main                                  │
│ Runner:   daytona (dtn_platform_12h_001)        │
├─────────────────────────────────────────────────┤
│ Budget:   $50 for 12h                           │
│ Skills:   (none)                                │
│ MCP:      github                                │
└─────────────────────────────────────────────────┘
```

**Purpose:** Collapsible session metadata header.

**Behavior:**
- Parsed from YAML header
- Sticky at top or in sidebar
- Collapse to single line: `sess_12h · sonnet-4 · platform`
- Links to repo, sandbox

---

### SessionStats

```
┌─────────────────────────────────────────────────┐
│ STATISTICS                               [▾]    │
├─────────────────────────────────────────────────┤
│ Lines:        728                               │
│ Duration:     12h 0m                            │
│ Cost:         $42.17                            │
├─────────────────────────────────────────────────┤
│ User msgs:    3                                 │
│ Agent msgs:   14                                │
│ Tool calls:   55                                │
│ MCP calls:    21                                │
│ Subagents:    4                                 │
│ Questions:    0                                 │
│ Phases:       5                                 │
├─────────────────────────────────────────────────┤
│ Blobs:        4                                 │
│ Redacted:     1                                 │
└─────────────────────────────────────────────────┘
```

**Purpose:** Session statistics panel.

**Behavior:**
- Collapsible in sidebar
- Counts update if streaming live
- Click stat → filters to that type

---

### ToolIndex

```
┌─────────────────────────────────────────────────┐
│ TOOLS USED                               [▾]    │
├─────────────────────────────────────────────────┤
│ read     18 calls                               │
│ grep     12 calls                               │
│ edit      8 calls                               │
│ git       6 calls                               │
│ test      4 calls                               │
│ shell     3 calls                               │
│ glob      3 calls                               │
└─────────────────────────────────────────────────┘
```

**Purpose:** Tool usage breakdown.

**Behavior:**
- Sorted by count
- Click tool name → filters to those calls
- Hover shows avg latency

---

### StepNavigator

```
┌─────────────────────────────────────────────────┐
│ STEPS                    1 ──●────── 125        │
├─────────────────────────────────────────────────┤
│ [◀ Prev]  Step: [___42___]  [Next ▶]           │
└─────────────────────────────────────────────────┘
```

**Purpose:** Jump to specific step.

**Behavior:**
- Slider or input field
- Prev/Next buttons
- Keyboard: ↑/↓ for step navigation
- Shows current step in context

---

### SearchFilter

```
┌─────────────────────────────────────────────────┐
│ 🔍 [Search logs...                    ]         │
├─────────────────────────────────────────────────┤
│ Type:  [All ▾]  [Tool ▾]  [Agent ▾]  ...       │
│ Time:  [00:00] to [12:00]                       │
│ Has:   □ errors  □ blobs  □ redacted           │
└─────────────────────────────────────────────────┘
```

**Purpose:** Search and filter log lines.

**Behavior:**
- Full-text search with regex support
- Type dropdown filters
- Time range slider
- Checkbox filters for special content
- Results count: "Showing 47 of 728 lines"

---

## Templates

### SessionTimeline

```
┌─────────────────────────────────────────────────┐
│ [SessionHeader - collapsed]                     │
├─────────────────────────────────────────────────┤
│                                                 │
│ [LifecycleLine @start]                          │
│                                                 │
│ [AgentLine]                                     │
│ [MetricsLine]                                   │
│                                                 │
│ [TimeMarker 00:00:15]                           │
│                                                 │
│ [RecallLine]                                    │
│ [RecallLine]                                    │
│                                                 │
│ [AgentLine]                                     │
│                                                 │
│ [ToolLine read]                                 │
│ [ToolLine read]                                 │
│ [ToolLine read]                                 │
│                                                 │
│ [HourDivider HOUR 1-4]                          │
│                                                 │
│ ...                                             │
│                                                 │
│ [LifecycleLine @end]                            │
│                                                 │
└─────────────────────────────────────────────────┘
```

**Purpose:** Main scrollable timeline view.

**Behavior:**
- Virtual scroll for large logs
- Grouped by time/hour sections
- Sticky time markers while scrolling
- Auto-scroll to bottom if following live
- Click-to-expand for all collapsible lines

---

### SessionSidebar

```
┌───────────────────┐
│ [SessionHeader]   │
├───────────────────┤
│ [ModeIndicator]   │
│ [PhaseIndicator]  │
├───────────────────┤
│ [BudgetMeter]     │
│ [CostAccumulator] │
├───────────────────┤
│ [SessionStats]    │
├───────────────────┤
│ [ToolIndex]       │
├───────────────────┤
│ [StepNavigator]   │
├───────────────────┤
│ [SearchFilter]    │
└───────────────────┘
```

**Purpose:** Sidebar with metadata, stats, navigation.

**Behavior:**
- Collapsible sections
- Sticky on scroll
- Responsive: collapses to icons on mobile
- Updates in real-time if streaming

---

### SessionViewer

```
┌─────────────────────────────────────────────────────────────────┐
│ [Breadcrumb: Sessions > sess_12h_20251218_001]    [⚙] [↓ JSON]  │
├───────────────────┬─────────────────────────────────────────────┤
│                   │                                             │
│ [SessionSidebar]  │  [SessionTimeline]                          │
│                   │                                             │
│                   │                                             │
│                   │                                             │
│                   │                                             │
│                   │                                             │
│                   │                                             │
│                   │                                             │
│                   │                                             │
│                   │                                             │
└───────────────────┴─────────────────────────────────────────────┘
```

**Purpose:** Full page layout for viewing sessions.

**Behavior:**
- Two-column: sidebar + timeline
- Breadcrumb navigation
- Settings gear (display options)
- Export button (JSON, ATIF)
- Keyboard shortcuts for navigation

---

## Overlays

### BlobViewer

```
┌─────────────────────────────────────────────────┐
│ BLOB  sha256=a1b2c3d4...              [✕ Close] │
├─────────────────────────────────────────────────┤
│ Size: 12,847 bytes                              │
│ Type: text/markdown                             │
├─────────────────────────────────────────────────┤
│ # Agent Algorithms                              │
│                                                 │
│ Core thesis: "Don't rebuild agents. Build       │
│ skills."                                        │
│                                                 │
│ Autopilot = general agent + skills marketplace  │
│ ...                                             │
│                                     [↓ Download] │
└─────────────────────────────────────────────────┘
```

**Purpose:** View blob content.

**Behavior:**
- Modal overlay
- Syntax highlighting by mime type
- Download button
- Copy button
- Line numbers for code

---

### SubagentViewer

```
┌─────────────────────────────────────────────────┐
│ SUBAGENT  sess_sub_1  (explore)       [✕ Close] │
├─────────────────────────────────────────────────┤
│ Task: "What are the main architectural          │
│        components?"                             │
├─────────────────────────────────────────────────┤
│                                                 │
│ [Embedded SessionTimeline for subagent]         │
│                                                 │
│ t:glob **/*.rs → [12 files]                     │
│ t:read crates/server/src/lib.rs → [45 lines]    │
│ t:read crates/agent/src/lib.rs → [38 lines]     │
│ ...                                             │
│                                                 │
│ Summary: 6 crates identified                    │
└─────────────────────────────────────────────────┘
```

**Purpose:** Inline view of subagent trajectory.

**Behavior:**
- Modal or slide-out panel
- Loads from `path=` reference
- Recursive: subagent can have subagents
- Scroll syncs with parent if linked

---

### StepJumper

```
┌─────────────────────────────────────────────────┐
│ JUMP TO STEP                          [✕ Close] │
├─────────────────────────────────────────────────┤
│                                                 │
│ Enter step number: [____42____]                 │
│                                                 │
│ Range: 1 - 125                                  │
│                                                 │
│                              [Cancel]  [Go →]   │
└─────────────────────────────────────────────────┘
```

**Purpose:** Quick jump to specific step.

**Behavior:**
- Keyboard shortcut: `g` then type number
- Validates range
- Highlights target line after jump

---

## Streaming Components

### LiveIndicator

```
● LIVE                 ← pulses green
○ ENDED at 12:00:00    ← static gray
```

**Purpose:** Shows if session is live/streaming.

**Behavior:**
- Pulses when receiving events
- Shows end time when complete
- Click to toggle auto-scroll

---

### StreamBuffer

**Purpose:** Handles incoming SSE events for live sessions.

**Behavior:**
- Buffers events for batched DOM updates
- Throttles to 60fps
- Preserves scroll position unless at bottom
- Shows "N new lines" button if scrolled up

---

## Implementation Notes

**File structure:**
```
crates/server/src/views/blackbox/
├── mod.rs              # Exports
├── atoms.rs            # StepBadge, TimestampBadge, etc.
├── molecules.rs        # LineMeta, LineHeader, ResultDisplay
├── organisms.rs        # UserLine, ToolLine, etc.
├── sections.rs         # SessionHeader, SessionStats
├── templates.rs        # SessionTimeline, SessionViewer
├── overlays.rs         # BlobViewer, SubagentViewer
└── streaming.rs        # LiveIndicator, StreamBuffer
```

**Routes:**
```
GET /sessions/{id}           → SessionViewer
GET /sessions/{id}/timeline  → SessionTimeline (HTMX partial)
GET /sessions/{id}/stats     → SessionStats (HTMX partial)
GET /sessions/{id}/blob/{sha} → BlobViewer
GET /sessions/{id}/export    → JSON/ATIF download
```

**SSE endpoint (live sessions):**
```
GET /sessions/{id}/stream    → Server-Sent Events
```

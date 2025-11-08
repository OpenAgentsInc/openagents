# Overnight Agents - UI Specification

**Last Updated**: 2025-11-08
**Platform**: macOS 13.0+ (primary), iOS 16.0+ (monitoring only)
**Design System**: Liquid Glass UI (where available), Apple HIG
**Status**: Design Phase

---

## Table of Contents

1. [Design Principles](#design-principles)
2. [Screen Hierarchy](#screen-hierarchy)
3. [macOS Views](#macos-views)
4. [Shared Components](#shared-components)
5. [iOS Views](#ios-views)
6. [Visual Design](#visual-design)
7. [Interactions & Animations](#interactions--animations)
8. [States & Feedback](#states--feedback)
9. [Implementation Notes](#implementation-notes)

---

## Design Principles

### 1. **Transparency & Trust**
- Show all decision-making process (FM reasoning visible)
- Real-time streaming of agent work
- No hidden operations
- Clear indication when agents are active

### 2. **Calm Computing**
- Agents work in background, UI is for monitoring/approval
- Minimal interruptions (notifications only for decisions requiring input)
- Glanceable status indicators
- Progressive disclosure of details

### 3. **Apple Platform Native**
- Liquid Glass materials (macOS 15+, iOS 26+)
- SF Symbols throughout
- Native controls and patterns
- Dark mode support (primary)

### 4. **Mobile-First Components**
- All components designed to work on iOS (even if disabled for orchestration)
- Responsive layouts
- Touch-friendly targets (44pt minimum)
- Reusable SwiftUI components

### 5. **Real-Time Feedback**
- Streaming updates via AsyncStream
- Live progress indicators
- No polling, all push-based updates
- Optimistic UI for user actions

---

## Screen Hierarchy

```
macOS App (DesktopWebSocketServer)
├── Main Window
│   ├── Sidebar
│   │   ├── Sessions (existing)
│   │   ├── Agents (existing)
│   │   └── Overnight ← NEW
│   └── Content Area
│       └── OvernightOrchestrationView (selected via sidebar)
│           ├── Header (status, controls)
│           ├── SchedulerCard
│           ├── TaskQueueSection
│           ├── ActiveSessionsSection
│           └── RecentDecisionsSection

iOS App (MobileWebSocketClient)
├── Tab Bar
│   ├── Sessions (existing)
│   ├── Agents (existing)
│   └── Overnight ← NEW
│       └── OvernightMonitoringView
│           ├── StatusHeader
│           ├── TaskQueueCard
│           ├── ActiveSessionsList
│           └── RecentDecisionsList
```

---

## macOS Views

### 1. OvernightOrchestrationView

**Purpose**: Main control center for overnight agent orchestration on macOS.

**Layout**: Single-column, scrollable content area with fixed header.

```
┌─────────────────────────────────────────────────────────────┐
│  Overnight Agent Orchestration                        [ ]  │  ← Header
│  ● Running  Next wake: 2:30 AM (in 1h 23m)                │
│                                                             │
│  [▶ Start] [⏸ Pause] [⏹ Stop] [⚙ Settings]              │  ← Controls
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌───────────────────────────────────────────────────────┐ │
│  │ 🕐 Scheduler                                          │ │  ← SchedulerCard
│  │                                                       │ │
│  │ Schedule: Every 30 min (1:00 AM - 5:00 AM)          │ │
│  │ Constraints: ✓ Plugged in  ✓ WiFi                   │ │
│  │ Manifest: nightly-refactor-v1                        │ │
│  │                                                       │ │
│  │ Last run: 2:00 AM (completed in 34s)                │ │
│  │ Next run: 2:30 AM                                    │ │
│  └───────────────────────────────────────────────────────┘ │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐ │
│  │ 📋 Task Queue                            [View All]  │ │  ← TaskQueueSection
│  │                                                       │ │
│  │  Pending: 3    In Progress: 1    Completed: 5       │ │
│  │                                                       │ │
│  │  ┌─────────────────────────────────────────────────┐ │ │
│  │  │ ⚡ IN PROGRESS                                  │ │ │
│  │  │ Refactor BridgeManager error handling           │ │ │
│  │  │ Agent: Claude Code  •  Started: 2:15 AM         │ │ │
│  │  │ ▓▓▓▓▓▓▓▓░░░░░░░░ 12 tool calls  •  45s elapsed  │ │ │
│  │  └─────────────────────────────────────────────────┘ │ │
│  │                                                       │ │
│  │  ┌─────────────────────────────────────────────────┐ │ │
│  │  │ ⏸ PENDING                                       │ │ │
│  │  │ Generate tests for WebSocketServer              │ │ │
│  │  │ Agent: Codex  •  Priority: High                 │ │ │
│  │  └─────────────────────────────────────────────────┘ │ │
│  │                                                       │ │
│  └───────────────────────────────────────────────────────┘ │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐ │
│  │ 🤖 Active Sessions                       [View All]  │ │  ← ActiveSessionsSection
│  │                                                       │ │
│  │  ┌─────────────────────────────────────────────────┐ │ │
│  │  │ Claude Code  •  Session abc123                  │ │ │
│  │  │                                                  │ │ │
│  │  │ [tool_call] edit_file: BridgeManager.swift      │ │ │
│  │  │ [tool_call] run_bash: xcodebuild test...        │ │ │
│  │  │ [thinking] Refactoring error handling...        │ │ │
│  │  │                                                  │ │ │
│  │  │ 12 tool calls  •  45s elapsed  •  ~15s remaining │ │ │
│  │  │                                   [Cancel]       │ │ │
│  │  └─────────────────────────────────────────────────┘ │ │
│  └───────────────────────────────────────────────────────┘ │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐ │
│  │ 💡 Recent Decisions                      [View All]  │ │  ← RecentDecisionsSection
│  │                                                       │ │
│  │  ┌─────────────────────────────────────────────────┐ │ │
│  │  │ 2:15 AM  •  Confidence: 87%                     │ │ │
│  │  │                                                  │ │ │
│  │  │ Task: Refactor BridgeManager error handling    │ │ │
│  │  │ Agent: Claude Code                              │ │ │
│  │  │                                                  │ │ │
│  │  │ Rationale:                                       │ │ │
│  │  │ "BridgeManager.swift touched 25 times in recent │ │ │
│  │  │  sessions with user frequently requesting error │ │ │
│  │  │  handling improvements. Current implementation  │ │ │
│  │  │  uses optional returns which hides error        │ │ │
│  │  │  context."                                       │ │ │
│  │  │                                                  │ │ │
│  │  │ [View Session]                                   │ │ │
│  │  └─────────────────────────────────────────────────┘ │ │
│  └───────────────────────────────────────────────────────┘ │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐ │
│  │ ✅ Completed Work                        [View All]  │ │
│  │                                                       │ │
│  │  PR #42: Refactor BridgeManager error handling      │ │
│  │    ✓ Merged 5 minutes ago                           │ │
│  │                                                       │ │
│  │  PR #41: Add tests for DesktopWebSocketServer       │ │
│  │    ⏳ Awaiting review                                │ │
│  └───────────────────────────────────────────────────────┘ │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Dimensions**:
- Min width: 800pt
- Preferred width: 1000pt
- Max width: 1400pt
- Content padding: 20pt
- Section spacing: 24pt

**Materials**:
- Background: `.regularMaterial` (or `.glassEffect(.regular)` on macOS 15+)
- Cards: `.ultraThinMaterial` (or `.glassEffect(.thin)`)
- Header: `.thickMaterial`

---

### 2. SchedulerCard

**Purpose**: Display scheduler status, schedule, and controls.

**States**:
- `idle`: Not running, ready to start
- `running`: Active, shows next wake time
- `paused`: Temporarily paused (constraints not met)
- `stopped`: Manually stopped

**Layout**:

```swift
struct SchedulerCard: View {
    @Binding var state: SchedulerState
    let schedule: ScheduleConfig

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Header
            HStack {
                Image(systemName: "clock.arrow.circlepath")
                    .font(.title2)
                    .foregroundColor(.accentColor)

                Text("Scheduler")
                    .font(.headline)

                Spacer()

                StatusBadge(state: state)
            }

            Divider()

            // Schedule info
            VStack(alignment: .leading, spacing: 8) {
                InfoRow(label: "Schedule", value: schedule.description)
                InfoRow(label: "Constraints", value: constraintsDescription)
                InfoRow(label: "Manifest", value: schedule.manifestId)
            }

            Divider()

            // Timing
            if case .running(let nextWake) = state {
                VStack(alignment: .leading, spacing: 8) {
                    InfoRow(label: "Last run", value: lastRunDescription)
                    InfoRow(label: "Next run", value: nextWakeDescription(nextWake))
                }
            }
        }
        .padding(16)
        .background(Material.ultraThin)
        .cornerRadius(12)
    }
}
```

**Visual Specs**:
- Padding: 16pt
- Corner radius: 12pt
- Icon size: 24pt (title2)
- Label font: `.body` (regular weight)
- Value font: `.body` (medium weight)
- Divider color: `.separator`

**Status Badge Colors**:
- `idle`: Gray (systemGray)
- `running`: Green (systemGreen)
- `paused`: Orange (systemOrange)
- `stopped`: Red (systemRed)

---

### 3. TaskCard

**Purpose**: Display individual task status and metadata.

**Variants**:
- `pending`: Waiting to be picked up
- `in_progress`: Agent actively working
- `completed`: Finished successfully
- `failed`: Encountered error

**Layout (In Progress)**:

```
┌─────────────────────────────────────────────────┐
│ ⚡ IN PROGRESS                                  │  ← Status badge
│                                                  │
│ Refactor BridgeManager error handling           │  ← Task title (large)
│                                                  │
│ Agent: Claude Code  •  Started: 2:15 AM         │  ← Metadata
│                                                  │
│ ▓▓▓▓▓▓▓▓░░░░░░░░ 12 tool calls  •  45s elapsed  │  ← Progress
│                                                  │
│ Confidence: 87%                                  │  ← Decision metadata
│                                                  │
│ Rationale: "BridgeManager.swift touched 25      │  ← Collapsible
│ times..."                            [Show More] │
│                                                  │
│                            [View Session] [❌]   │  ← Actions
└─────────────────────────────────────────────────┘
```

**Visual Specs**:
- Padding: 16pt
- Corner radius: 8pt
- Title font: `.title3` (semibold)
- Metadata font: `.subheadline` (regular)
- Progress bar height: 8pt
- Border: 1pt solid (color based on status)

**Status Indicators**:
- Icon + text on top left
- Icons: ⏸ (pending), ⚡ (in_progress), ✅ (completed), ❌ (failed)
- Background tint matches status color (subtle, 5% opacity)

---

### 4. SessionStreamView

**Purpose**: Live stream of ACP updates from active agent session.

**Layout**:

```
┌─────────────────────────────────────────────────┐
│ Claude Code  •  Session abc123         [Cancel] │  ← Header
├─────────────────────────────────────────────────┤
│                                                  │
│ [tool_call] read_file                           │  ← Tool call (collapsed)
│   BridgeManager.swift                           │
│                                                  │
│ [tool_call] edit_file                [Expand]   │  ← Tool call (collapsed)
│   BridgeManager.swift                           │
│   • 15 lines changed                            │
│                                                  │
│ [thinking] 🧠                           [Expand] │  ← Thinking block
│   Refactoring error handling to use Result<T>   │
│   types for better error propagation...         │
│                                                  │
│ [tool_call] run_bash                  [Expand]  │
│   xcodebuild test -workspace...                 │
│                                                  │
│ ▼                                                │  ← Auto-scroll indicator
├─────────────────────────────────────────────────┤
│ 12 tool calls  •  45s elapsed  •  ~15s remaining│  ← Footer
└─────────────────────────────────────────────────┘
```

**Behavior**:
- Auto-scrolls to bottom on new updates
- Collapsed by default, expandable on click
- Thinking blocks show spinner animation while streaming
- Tool calls show icon based on type (file, bash, etc.)

**Visual Specs**:
- Background: `.ultraThinMaterial`
- Max height: 400pt (scrollable)
- Item spacing: 12pt
- Monospace font for code/paths: `.system(.body, design: .monospaced)`

---

### 5. DecisionCard

**Purpose**: Display Foundation Models decision with full rationale.

**Layout**:

```
┌─────────────────────────────────────────────────┐
│ 💡 Decision • 2:15 AM                           │  ← Header
│                                        87% ████  │  ← Confidence bar
├─────────────────────────────────────────────────┤
│                                                  │
│ Task                                             │
│ Refactor BridgeManager error handling with      │  ← Task (large, bold)
│ Swift Result types                               │
│                                                  │
│ Agent: Claude Code • Priority: High             │  ← Metadata
│ Estimated: 30 minutes                            │
│                                                  │
│ ──────────────────────────────────────────────  │  ← Divider
│                                                  │
│ Rationale                                        │  ← Section header
│                                                  │
│ "BridgeManager.swift touched 25 times in recent │  ← FM explanation
│ sessions with user frequently requesting error  │
│ handling improvements. Current implementation   │
│ uses optional returns which hides error         │
│ context. Refactoring to Result<T, Error> will   │
│ provide clear error propagation and better      │
│ debugging experience."                           │
│                                                  │
│ ──────────────────────────────────────────────  │
│                                                  │
│ Context                                          │  ← Collapsible section
│ • 15 recent Claude Code sessions analyzed       │
│ • BridgeManager.swift: 25 touches               │
│ • User intents: refactor, error handling        │
│ • Test coverage: 68% (target: 80%)              │
│                                      [Show More] │
│                                                  │
│                        [Approve] [Edit] [Skip]  │  ← Actions
└─────────────────────────────────────────────────┘
```

**Visual Specs**:
- Padding: 20pt
- Corner radius: 12pt
- Confidence bar: gradient from orange (50%) → green (100%)
- Task font: `.title2` (semibold)
- Rationale font: `.body` (regular), quoted style
- Context font: `.footnote` (regular)

**Confidence Visualization**:
- 0-50%: Red gradient
- 50-70%: Orange gradient
- 70-85%: Yellow/green gradient
- 85-100%: Green gradient
- Show percentage number + visual bar

---

### 6. PRPreviewSheet

**Purpose**: Preview PR before creation/push, allow editing.

**Layout** (Modal sheet, 600pt width):

```
┌─────────────────────────────────────────────────┐
│ Pull Request Preview                     [✕]    │  ← Sheet header
├─────────────────────────────────────────────────┤
│                                                  │
│ Branch: agent/nightly-refactor/abc123           │  ← Branch name
│ Base: main                                       │
│                                                  │
│ ──────────────────────────────────────────────  │
│                                                  │
│ Title                                            │
│ ┌──────────────────────────────────────────────┐│
│ │ Refactor BridgeManager error handling      ││  ← Editable
│ └──────────────────────────────────────────────┘│
│                                                  │
│ Description                                      │
│ ┌──────────────────────────────────────────────┐│
│ │ ## Autonomous Agent Work                   ││  ← Editable (markdown)
│ │                                             ││
│ │ **Task**: Refactor BridgeManager...        ││
│ │                                             ││
│ │ **Agent**: Claude Code                     ││
│ │                                             ││
│ │ ... (scrollable)                           ││
│ └──────────────────────────────────────────────┘│
│                                                  │
│ ──────────────────────────────────────────────  │
│                                                  │
│ Files Changed (3)                     [View Diff]│
│ • BridgeManager.swift            +45 / -30      │  ← File list
│ • BridgeManagerTests.swift       +120 / -0      │
│ • DesktopWebSocketServer.swift   +5 / -2        │
│                                                  │
│ ──────────────────────────────────────────────  │
│                                                  │
│ ☐ Create as draft                                │  ← Options
│ ☑ Auto-assign reviewers                         │
│ ☐ Request review from @username                 │
│                                                  │
├─────────────────────────────────────────────────┤
│                        [Cancel] [Create PR]     │  ← Footer actions
└─────────────────────────────────────────────────┘
```

**Behavior**:
- Modal sheet presentation
- Title/description editable via TextEditor
- Markdown preview toggle
- Files list expandable to show diff
- Create button disabled if title empty
- Validation: warn if no files changed

---

### 7. SettingsView (Overnight Tab)

**Purpose**: Configure scheduler, manifests, constraints.

**Layout** (Settings window tab):

```
┌─────────────────────────────────────────────────┐
│ Overnight Orchestration                          │
├─────────────────────────────────────────────────┤
│                                                  │
│ General                                          │
│                                                  │
│ Upgrade Manifest                                 │
│ ┌──────────────────────────────────────────────┐│
│ │ nightly-refactor-v1              [Browse...] ││
│ └──────────────────────────────────────────────┘│
│                                                  │
│ ☑ Start scheduler automatically at launch       │
│ ☑ Show notifications for decisions              │
│ ☐ Require approval before creating PRs          │
│                                                  │
│ ──────────────────────────────────────────────  │
│                                                  │
│ Schedule                                         │
│                                                  │
│ Cron Expression                                  │
│ ┌──────────────────────────────────────────────┐│
│ │ */30 1-5 * * *                               ││
│ └──────────────────────────────────────────────┘│
│ Every 30 minutes, 1 AM - 5 AM                   │
│                                                  │
│ Time Window                                      │
│ Start: [01:00 ▾]    End: [05:00 ▾]             │
│                                                  │
│ Timezone: [America/Los_Angeles ▾]               │
│                                                  │
│ ──────────────────────────────────────────────  │
│                                                  │
│ Constraints                                      │
│                                                  │
│ ☑ Only run when plugged into power              │
│ ☑ Only run on WiFi (not cellular)               │
│ ☐ Pause when Do Not Disturb is enabled          │
│ ☐ Pause when user is actively using Mac         │
│                                                  │
│ CPU Usage Limit: [80% ▾]                        │
│                                                  │
│ ──────────────────────────────────────────────  │
│                                                  │
│ Advanced                                         │
│                                                  │
│ Jitter (random delay): [300 ▾] seconds          │
│ On missed run: [○ Skip  ⦿ Run at next chance]  │
│ Max concurrent agents: [2 ▾]                    │
│                                                  │
│                           [Restore Defaults]    │
└─────────────────────────────────────────────────┘
```

**Validation**:
- Cron expression validated on blur
- Show human-readable description below cron input
- Time window: start must be before end (or allow overnight crossing)
- All changes auto-saved

---

## Shared Components

### StatusBadge

**Purpose**: Consistent status indicator across all views.

```swift
struct StatusBadge: View {
    let state: SchedulerState

    var body: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(statusColor)
                .frame(width: 8, height: 8)

            Text(statusText)
                .font(.caption)
                .fontWeight(.medium)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(statusColor.opacity(0.15))
        .cornerRadius(8)
    }

    private var statusColor: Color {
        switch state {
        case .idle: return .secondary
        case .running: return .green
        case .paused: return .orange
        case .stopped: return .red
        }
    }

    private var statusText: String {
        switch state {
        case .idle: return "Idle"
        case .running: return "Running"
        case .paused(let reason): return "Paused"
        case .stopped: return "Stopped"
        }
    }
}
```

**Variants**:
- Small (caption font, 8pt dot)
- Medium (body font, 10pt dot)
- Large (title3 font, 12pt dot)

---

### InfoRow

**Purpose**: Label-value pair for metadata display.

```swift
struct InfoRow: View {
    let label: String
    let value: String
    var icon: String? = nil

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            if let icon = icon {
                Image(systemName: icon)
                    .font(.body)
                    .foregroundColor(.secondary)
                    .frame(width: 20)
            }

            Text(label)
                .font(.body)
                .foregroundColor(.secondary)

            Spacer()

            Text(value)
                .font(.body)
                .fontWeight(.medium)
                .multilineTextAlignment(.trailing)
        }
    }
}
```

**Usage**:
```swift
InfoRow(label: "Schedule", value: "Every 30 min (1-5 AM)")
InfoRow(label: "Agent", value: "Claude Code", icon: "cpu")
```

---

### ProgressRing

**Purpose**: Circular progress indicator for task/session progress.

```swift
struct ProgressRing: View {
    let progress: Double  // 0.0 - 1.0
    let lineWidth: CGFloat = 8

    var body: some View {
        ZStack {
            Circle()
                .stroke(Color.secondary.opacity(0.2), lineWidth: lineWidth)

            Circle()
                .trim(from: 0, to: progress)
                .stroke(
                    AngularGradient(
                        colors: [.blue, .green],
                        center: .center
                    ),
                    style: StrokeStyle(lineWidth: lineWidth, lineCap: .round)
                )
                .rotationEffect(.degrees(-90))
                .animation(.easeInOut, value: progress)
        }
    }
}
```

**Sizes**:
- Small: 24pt diameter
- Medium: 40pt diameter
- Large: 60pt diameter

---

### ConfidenceBar

**Purpose**: Visual representation of FM decision confidence (0-100%).

```swift
struct ConfidenceBar: View {
    let confidence: Double  // 0.0 - 1.0

    var body: some View {
        HStack(spacing: 8) {
            Text("\(Int(confidence * 100))%")
                .font(.caption)
                .fontWeight(.semibold)
                .foregroundColor(confidenceColor)

            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    // Background
                    RoundedRectangle(cornerRadius: 4)
                        .fill(Color.secondary.opacity(0.2))

                    // Filled portion
                    RoundedRectangle(cornerRadius: 4)
                        .fill(confidenceGradient)
                        .frame(width: geometry.size.width * confidence)
                }
            }
            .frame(height: 8)
        }
    }

    private var confidenceColor: Color {
        switch confidence {
        case 0..<0.5: return .red
        case 0.5..<0.7: return .orange
        case 0.7..<0.85: return .yellow
        default: return .green
        }
    }

    private var confidenceGradient: LinearGradient {
        LinearGradient(
            colors: [confidenceColor.opacity(0.7), confidenceColor],
            startPoint: .leading,
            endPoint: .trailing
        )
    }
}
```

---

### ToolCallRow

**Purpose**: Display individual ACP tool call in session stream.

```swift
struct ToolCallRow: View {
    let toolCall: ACPToolCallWire
    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Header
            HStack {
                Image(systemName: toolIcon)
                    .foregroundColor(.accentColor)

                Text("[tool_call]")
                    .font(.caption.monospaced())
                    .foregroundColor(.secondary)

                Text(toolCall.name)
                    .font(.body.monospaced())

                Spacer()

                Button(isExpanded ? "Collapse" : "Expand") {
                    withAnimation {
                        isExpanded.toggle()
                    }
                }
                .font(.caption)
            }

            // Arguments (when expanded)
            if isExpanded {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(sortedArguments, id: \.key) { arg in
                        HStack(alignment: .top) {
                            Text("\(arg.key):")
                                .font(.caption.monospaced())
                                .foregroundColor(.secondary)

                            Text(String(describing: arg.value))
                                .font(.caption.monospaced())
                                .lineLimit(10)
                        }
                    }
                }
                .padding(.leading, 24)
            }
        }
        .padding(12)
        .background(Material.ultraThin)
        .cornerRadius(8)
    }

    private var toolIcon: String {
        switch toolCall.name {
        case "read_file", "write_file", "edit_file":
            return "doc.text"
        case "run_bash":
            return "terminal"
        case "grep":
            return "magnifyingglass"
        default:
            return "function"
        }
    }
}
```

---

## iOS Views

### OvernightMonitoringView

**Purpose**: iOS monitoring interface (read-only, no orchestration controls).

**Layout**:

```
┌─────────────────────────────────────────────┐
│ < Overnight                                 │  ← Nav bar
├─────────────────────────────────────────────┤
│                                             │
│ ┌─────────────────────────────────────────┐ │
│ │ 🟢 Running                              │ │  ← StatusHeader
│ │ Next wake: 2:30 AM (in 1h 23m)         │ │
│ │                                         │ │
│ │ macOS: Christopher's MacBook Pro        │ │
│ └─────────────────────────────────────────┘ │
│                                             │
│ ┌─────────────────────────────────────────┐ │
│ │ 📋 Task Queue                           │ │  ← TaskQueueCard
│ │                                         │ │
│ │   ⏸ 3     ⚡ 1     ✅ 5                │ │  ← Counts
│ │ Pending  Active  Done                   │ │
│ │                                         │ │
│ │ ───────────────────────────────────── │ │
│ │                                         │ │
│ │ ⚡ Refactor BridgeManager               │ │  ← Active task
│ │ Claude Code • 45s                       │ │
│ │ ▓▓▓▓▓▓▓▓░░░░░░░░ 12 calls              │ │
│ └─────────────────────────────────────────┘ │
│                                             │
│ Active Sessions                             │  ← Section header
│                                             │
│ ┌─────────────────────────────────────────┐ │
│ │ Claude Code • abc123                    │ │  ← SessionRow
│ │ Refactor BridgeManager...               │ │
│ │ 12 tool calls • 45s • ~15s left     ›  │ │
│ └─────────────────────────────────────────┘ │
│                                             │
│ Recent Decisions                            │
│                                             │
│ ┌─────────────────────────────────────────┐ │
│ │ 💡 2:15 AM • 87%                        │ │  ← DecisionRow
│ │                                         │ │
│ │ Refactor BridgeManager error handling  │ │
│ │ Agent: Claude Code                      │ │
│ │                                         │ │
│ │ "BridgeManager.swift touched 25...     │ │
│ │                              [More]  › │ │
│ └─────────────────────────────────────────┘ │
│                                             │
│ ┌─────────────────────────────────────────┐ │
│ │ 💡 2:00 AM • 92%                        │ │
│ │ Generate tests for WebSocketServer     │ │
│ │ ...                                  › │ │
│ └─────────────────────────────────────────┘ │
│                                             │
└─────────────────────────────────────────────┘
```

**Dimensions**:
- Full screen width
- Safe area insets respected
- Content padding: 16pt
- Card padding: 16pt
- Card corner radius: 12pt
- Section spacing: 24pt

**Interactions**:
- Pull to refresh (re-sync with macOS)
- Tap session row → detail view with full stream
- Tap decision row → detail view with full rationale
- No editing or control actions (monitoring only)

---

### StatusHeader (iOS)

**Purpose**: Glanceable status at top of iOS view.

```swift
struct StatusHeader: View {
    let state: SchedulerState
    let nextWake: Date?
    let macName: String

    var body: some View {
        VStack(spacing: 12) {
            HStack {
                StatusBadge(state: state)

                Spacer()

                if let nextWake = nextWake {
                    Text("Next wake: \(nextWake, style: .relative)")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }

            HStack {
                Image(systemName: "desktopcomputer")
                    .foregroundColor(.secondary)

                Text(macName)
                    .font(.subheadline)
                    .foregroundColor(.secondary)

                Spacer()
            }
        }
        .padding(16)
        .background(Material.regular)
        .cornerRadius(12)
    }
}
```

---

### TaskQueueCard (iOS)

**Purpose**: Compact task queue visualization for iOS.

```swift
struct TaskQueueCard: View {
    let pending: Int
    let active: Int
    let completed: Int
    let currentTask: OvernightTask?

    var body: some View {
        VStack(spacing: 16) {
            // Header
            HStack {
                Image(systemName: "list.bullet.clipboard")
                    .foregroundColor(.accentColor)

                Text("Task Queue")
                    .font(.headline)

                Spacer()
            }

            // Counts
            HStack(spacing: 32) {
                CountBadge(icon: "pause.circle", count: pending, label: "Pending")
                CountBadge(icon: "bolt.circle.fill", count: active, label: "Active")
                CountBadge(icon: "checkmark.circle.fill", count: completed, label: "Done")
            }

            if let task = currentTask {
                Divider()

                // Active task preview
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Image(systemName: "bolt.circle.fill")
                            .foregroundColor(.green)

                        Text(task.decision.task)
                            .font(.subheadline)
                            .fontWeight(.medium)
                            .lineLimit(2)
                    }

                    HStack {
                        Text(task.decision.agent.rawValue)
                            .font(.caption)
                            .foregroundColor(.secondary)

                        Text("•")
                            .foregroundColor(.secondary)

                        Text(task.elapsedTime, style: .timer)
                            .font(.caption.monospacedDigit())
                            .foregroundColor(.secondary)
                    }

                    ProgressView(value: task.progress)
                        .tint(.green)
                }
            }
        }
        .padding(16)
        .background(Material.regular)
        .cornerRadius(12)
    }
}

struct CountBadge: View {
    let icon: String
    let count: Int
    let label: String

    var body: some View {
        VStack(spacing: 4) {
            Image(systemName: icon)
                .font(.title2)
                .foregroundColor(.accentColor)

            Text("\(count)")
                .font(.title2)
                .fontWeight(.bold)

            Text(label)
                .font(.caption)
                .foregroundColor(.secondary)
        }
    }
}
```

---

## Visual Design

### Color Palette

**System Colors** (adapt to light/dark mode):
- Primary: `.accentColor` (blue)
- Success: `.green`
- Warning: `.orange`
- Error: `.red`
- Secondary: `.secondary`

**Status Colors**:
- Idle: `.systemGray`
- Running: `.systemGreen`
- Paused: `.systemOrange`
- Failed: `.systemRed`
- Completed: `.systemGreen`

**Confidence Colors**:
- 0-50%: `.systemRed`
- 50-70%: `.systemOrange`
- 70-85%: `.systemYellow`
- 85-100%: `.systemGreen`

### Typography

**Font Scale** (iOS/macOS):
- Large Title: `.largeTitle` / 34pt
- Title: `.title` / 28pt
- Title 2: `.title2` / 22pt
- Title 3: `.title3` / 20pt
- Headline: `.headline` / 17pt (semibold)
- Body: `.body` / 17pt (regular)
- Callout: `.callout` / 16pt
- Subheadline: `.subheadline` / 15pt
- Footnote: `.footnote` / 13pt
- Caption: `.caption` / 12pt
- Caption 2: `.caption2` / 11pt

**Font Weights**:
- Regular: task descriptions, body text
- Medium: values, metadata
- Semibold: headings, labels
- Bold: counts, emphasis

**Monospace** (for code/data):
- `.system(.body, design: .monospaced)`
- Use for: file paths, cron expressions, session IDs, timestamps

### Spacing

**Padding**:
- Card padding: 16pt
- Section padding: 20pt
- Content margins: 20pt (macOS), 16pt (iOS)

**Spacing**:
- Between cards: 16pt
- Between sections: 24pt
- Between elements in card: 12pt
- Between label-value pairs: 8pt

**Corner Radius**:
- Cards: 12pt
- Buttons: 8pt
- Badges: 8pt
- Input fields: 6pt

### Materials (macOS 15+ / iOS 26+)

**Liquid Glass** (where available):
- `.glassEffect(.regular)` - Main background
- `.glassEffect(.thin)` - Cards, overlays
- `.glassEffect(.thick)` - Headers, toolbars

**Fallback Materials** (older OS):
- `.regularMaterial` - Main background
- `.ultraThinMaterial` - Cards
- `.thickMaterial` - Headers

---

## Interactions & Animations

### Transitions

**Card Appear/Disappear**:
```swift
.transition(.asymmetric(
    insertion: .move(edge: .trailing).combined(with: .opacity),
    removal: .scale.combined(with: .opacity)
))
```

**List Items**:
```swift
.transition(.move(edge: .top).combined(with: .opacity))
```

**Modal Sheets**:
```swift
.transition(.move(edge: .bottom))
```

### Animations

**Progress Updates**:
```swift
withAnimation(.easeInOut(duration: 0.3)) {
    progress = newValue
}
```

**Status Changes**:
```swift
withAnimation(.spring(response: 0.5, dampingFraction: 0.7)) {
    state = newState
}
```

**Auto-Scroll (Session Stream)**:
```swift
ScrollViewReader { proxy in
    // ...
    .onChange(of: latestUpdateID) { id in
        withAnimation {
            proxy.scrollTo(id, anchor: .bottom)
        }
    }
}
```

### Gestures

**Swipe Actions (iOS)**:
```swift
.swipeActions(edge: .trailing) {
    Button(role: .destructive) {
        cancelTask()
    } label: {
        Label("Cancel", systemImage: "xmark")
    }
}
```

**Context Menu (macOS)**:
```swift
.contextMenu {
    Button("View Session") { ... }
    Button("Copy Session ID") { ... }
    Divider()
    Button("Cancel Task", role: .destructive) { ... }
}
```

---

## States & Feedback

### Loading States

**Skeleton Loading**:
```swift
struct SkeletonCard: View {
    @State private var isAnimating = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            RoundedRectangle(cornerRadius: 4)
                .fill(Color.secondary.opacity(0.2))
                .frame(width: 200, height: 20)

            RoundedRectangle(cornerRadius: 4)
                .fill(Color.secondary.opacity(0.2))
                .frame(height: 16)

            RoundedRectangle(cornerRadius: 4)
                .fill(Color.secondary.opacity(0.2))
                .frame(width: 150, height: 16)
        }
        .padding(16)
        .opacity(isAnimating ? 0.5 : 1.0)
        .animation(.easeInOut(duration: 1.0).repeatForever(), value: isAnimating)
        .onAppear {
            isAnimating = true
        }
    }
}
```

### Empty States

**No Tasks**:
```
┌─────────────────────────────────────────────────┐
│                                                  │
│              📋                                  │
│                                                  │
│         No Tasks Yet                             │
│                                                  │
│  The scheduler will create tasks automatically  │
│  based on the configured upgrade manifest.      │
│                                                  │
│                [Start Scheduler]                 │
│                                                  │
└─────────────────────────────────────────────────┘
```

**No Active Sessions**:
```
┌─────────────────────────────────────────────────┐
│                                                  │
│              🤖                                  │
│                                                  │
│      No Active Agent Sessions                    │
│                                                  │
│  Agents will start working on the next          │
│  scheduled wake-up.                              │
│                                                  │
└─────────────────────────────────────────────────┘
```

### Error States

**Scheduler Error**:
```
┌─────────────────────────────────────────────────┐
│ ⚠️ Scheduler Error                               │
│                                                  │
│ Failed to start scheduler:                       │
│ Invalid cron expression "* * * *"               │
│                                                  │
│ Please check your schedule configuration.       │
│                                                  │
│              [Open Settings]  [Dismiss]         │
└─────────────────────────────────────────────────┘
```

**Agent Failed**:
```
┌─────────────────────────────────────────────────┐
│ ❌ FAILED                                        │
│                                                  │
│ Refactor BridgeManager error handling           │
│                                                  │
│ Agent: Claude Code  •  Failed after 45s         │
│                                                  │
│ Error: Process exited with code 1               │
│ Time budget exceeded (estimated 30m, ran 45m)   │
│                                                  │
│              [View Logs]  [Retry]  [Skip]       │
└─────────────────────────────────────────────────┘
```

### Success States

**Task Completed**:
```
┌─────────────────────────────────────────────────┐
│ ✅ COMPLETED                                     │
│                                                  │
│ Refactor BridgeManager error handling           │
│                                                  │
│ Agent: Claude Code  •  Completed in 34s         │
│                                                  │
│ ✓ 12 tool calls executed                        │
│ ✓ All tests passing                             │
│ ✓ PR #42 created                                │
│                                                  │
│              [View PR]  [View Session]          │
└─────────────────────────────────────────────────┘
```

**PR Created**:
```
┌─────────────────────────────────────────────────┐
│ 🎉 Pull Request Created                         │
│                                                  │
│ PR #42: Refactor BridgeManager error handling   │
│                                                  │
│ Branch: agent/nightly-refactor/abc123           │
│ Status: Open • Checks passing ✓                 │
│                                                  │
│              [Open in GitHub]  [Dismiss]        │
└─────────────────────────────────────────────────┘
```

---

## Implementation Notes

### SwiftUI Architecture

**View Models**:
```swift
@MainActor
class OvernightOrchestrationViewModel: ObservableObject {
    @Published var schedulerState: SchedulerState = .idle
    @Published var tasks: [OvernightTask] = []
    @Published var activeSessions: [AgentSessionInfo] = []
    @Published var recentDecisions: [TaskDecision] = []

    private let bridgeManager: BridgeManager
    private var cancellables = Set<AnyCancellable>()

    init(bridgeManager: BridgeManager) {
        self.bridgeManager = bridgeManager
        subscribeToUpdates()
    }

    func subscribeToUpdates() {
        // Subscribe to bridge notifications
        NotificationCenter.default.publisher(for: .orchestrationUpdate)
            .sink { [weak self] notification in
                self?.handleUpdate(notification)
            }
            .store(in: &cancellables)
    }

    func startScheduler() async throws {
        try await bridgeManager.sendRequest(
            method: "orchestration/start",
            params: [:]
        )
    }
}
```

**Environment Objects**:
```swift
@main
struct OpenAgentsApp: App {
    @StateObject private var bridgeManager = BridgeManager()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(bridgeManager)
        }
    }
}
```

### Bridge Integration

**JSON-RPC Methods** (macOS → iOS):
- `orchestration/status` - Get current state
- `orchestration/task_queued` - New task added
- `orchestration/task_started` - Agent started
- `orchestration/task_completed` - Task finished
- `orchestration/decision_made` - FM decision
- `orchestration/pr_created` - PR created

**Notification Names**:
```swift
extension Notification.Name {
    static let orchestrationUpdate = Notification.Name("orchestrationUpdate")
    static let schedulerStateChanged = Notification.Name("schedulerStateChanged")
    static let taskQueueUpdated = Notification.Name("taskQueueUpdated")
}
```

### Accessibility

**VoiceOver Labels**:
```swift
.accessibilityLabel("Task: \(task.decision.task)")
.accessibilityValue("Status: \(task.status), Agent: \(task.decision.agent)")
.accessibilityHint("Tap to view details")
```

**Dynamic Type**:
- All text uses system font scales
- Test at accessibility sizes (XXL, XXXL)
- Ensure layouts don't break at large sizes

**Keyboard Navigation** (macOS):
- All controls keyboard accessible
- Tab order logical
- Return key activates primary action
- Escape key dismisses sheets/modals

### Performance

**Lazy Loading**:
```swift
ScrollView {
    LazyVStack {
        ForEach(tasks) { task in
            TaskCard(task: task)
        }
    }
}
```

**Virtualization** (for long lists):
```swift
List {
    ForEach(sessions) { session in
        SessionRow(session: session)
    }
}
.listStyle(.plain)
```

**Debouncing** (for search/filter):
```swift
.onChange(of: searchText) { newValue in
    searchTask?.cancel()
    searchTask = Task {
        try await Task.sleep(nanoseconds: 300_000_000)  // 300ms
        await performSearch(newValue)
    }
}
```

---

## Future Enhancements

### Phase 2 (Post-Demo)

1. **Timeline View**
   - Gantt chart of scheduled vs actual runs
   - Historical performance metrics
   - Success/failure trends

2. **Analytics Dashboard**
   - Agent performance comparison
   - Task completion rates
   - Average time per task type
   - FM decision accuracy (user feedback)

3. **Advanced Filters**
   - Filter tasks by status, agent, date range
   - Search by task description
   - Saved filter presets

4. **Notifications**
   - macOS notifications for decisions requiring approval
   - iOS push notifications for completed work
   - Customizable notification rules

5. **Multi-Device Sync**
   - View overnight work across devices
   - Start scheduler from iOS (triggers macOS)
   - Approve PRs from iPhone

6. **Manifest Editor**
   - Visual cron expression builder
   - Pipeline operation drag-and-drop
   - Validation and preview

7. **Session Replay**
   - Step through agent session like debugger
   - Jump to specific tool calls
   - Export session as video/GIF

---

## Appendix: Component Checklist

### macOS Components
- [ ] OvernightOrchestrationView
- [ ] SchedulerCard
- [ ] TaskCard (pending/in_progress/completed/failed variants)
- [ ] SessionStreamView
- [ ] DecisionCard
- [ ] PRPreviewSheet
- [ ] SettingsView (Overnight tab)
- [ ] StatusBadge
- [ ] InfoRow
- [ ] ProgressRing
- [ ] ConfidenceBar
- [ ] ToolCallRow

### iOS Components
- [ ] OvernightMonitoringView
- [ ] StatusHeader
- [ ] TaskQueueCard
- [ ] CountBadge
- [ ] SessionRow (list item)
- [ ] DecisionRow (list item)
- [ ] SessionDetailView
- [ ] DecisionDetailView

### Shared Components
- [ ] StatusBadge (reused)
- [ ] ConfidenceBar (reused)
- [ ] ToolCallRow (reused)

### View Models
- [ ] OvernightOrchestrationViewModel (macOS)
- [ ] OvernightMonitoringViewModel (iOS)
- [ ] SchedulerViewModel
- [ ] TaskQueueViewModel
- [ ] SessionStreamViewModel

---

**End of UI Specification**

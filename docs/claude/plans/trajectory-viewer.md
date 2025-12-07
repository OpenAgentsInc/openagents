# Plan: HuggingFace Trajectory Browser Widget

## Overview

Build an Effuse widget system for browsing and viewing 15,209 OpenThoughts SFT trajectories:
- **Sidebar (260px)**: Searchable, filterable list of trajectories
- **Main area**: Selected trajectory detail view (starting with raw JSON)

## Recommended Approach: Hybrid Strategy

After analyzing three perspectives (simplicity, performance, UX), I recommend a **phased hybrid approach** that balances quick delivery with scalability:

### Phase 1: MVP (This PR)
- Two separate widgets following existing patterns
- Paginated list (100 items per page, Next/Prev buttons)
- Simple text search (client-side substring matching on agent/task/episode)
- **Formatted trajectory view** with step-by-step accordion (like ATIF Details widget)
- **Delivery: 10-12 hours, ~900 lines**

### Phase 2: Performance & UX Enhancements (Future PR)
- Virtual scrolling for seamless browsing (replace pagination)
- Multi-field filtering (task, agent, model, date range, outcome)
- Search indexing for instant multi-field search
- Export trajectory to file

---

## Architecture

### Two Widget Design

**Why two widgets instead of one?**
1. Follows proven Effuse patterns (TrajectoryPaneWidget + detail views)
2. Cleaner state isolation and independent testing
3. Matches physical layout separation (sidebar vs main)
4. Easier to enhance each widget independently

### Widget 1: HFTrajectoryListWidget (Sidebar)

**Purpose:** Paginated, searchable list of trajectories

**State:**
```typescript
interface HFTrajectoryListState {
  trajectories: TrajectoryMetadata[]    // Current page (100 items)
  filteredTrajectories: TrajectoryMetadata[]
  selectedSessionId: string | null
  searchQuery: string
  currentPage: number
  pageSize: number
  totalCount: number
  loading: boolean
  error: string | null
  collapsed: boolean
}

interface TrajectoryMetadata {
  sessionId: string
  agentName: string
  modelName: string
  task: string
  episode: string
  date: string
  stepCount: number
}
```

**Events:**
```typescript
type HFTrajectoryListEvent =
  | { type: "loadPage"; page: number }
  | { type: "search"; query: string }
  | { type: "select"; sessionId: string }
  | { type: "toggleCollapse" }
  | { type: "refresh" }
```

**Key Features:**
- Pagination controls (Prev/Next, "Showing X-Y of 15,209")
- Search input (filters current page by agent/task/episode)
- Click item to select (highlights, emits select event)
- Collapsed state to save space

### Widget 2: HFTrajectoryDetailWidget (Main Area)

**Purpose:** Display selected trajectory with formatted step-by-step view

**State:**
```typescript
interface HFTrajectoryDetailState {
  trajectory: Trajectory | null
  sessionId: string | null
  loading: boolean
  error: string | null
  collapsed: boolean
  expandedStepId: number | null  // For accordion behavior
  viewMode: "formatted" | "json"  // Toggle for future
}
```

**Events:**
```typescript
type HFTrajectoryDetailEvent =
  | { type: "load"; sessionId: string; trajectory: Trajectory }
  | { type: "toggleCollapse" }
  | { type: "toggleStep"; stepId: number }
  | { type: "clear" }
  | { type: "toggleViewMode" }  // Future: switch between formatted/JSON
```

**Key Features (Phase 1):**
- **Metadata header:** Session ID, agent name, model, task, episode, step count, date
- **Step-by-step accordion:** Expand/collapse individual steps (one at a time)
- **Step header:** Step #, source badge (user/agent/system), timestamp, tool count indicator
- **Expanded step details:**
  - Message content (formatted JSON if structured)
  - Tool calls (function name + arguments in violet)
  - Observations (results in emerald)
  - Metrics (tokens, cost if available)
- **Color coding:**
  - User steps: blue (`bg-blue-900/40 text-blue-300`)
  - Agent steps: emerald (`bg-emerald-900/40 text-emerald-300`)
  - System steps: gray (`bg-zinc-800/40 text-zinc-300`)
- **Empty state:** "No trajectory selected"
- **Loading state:** Spinner while fetching
- **Pattern:** Modeled after ATIFDetailsWidget (src/effuse/widgets/atif-details.ts)

**Future (Phase 2):**
- Toggle between formatted view and raw JSON
- Export trajectory to file (JSON, Markdown)
- Copy individual steps
- Collapsible metadata sections

---

## Data Flow

### Loading Trajectories

```
User opens page
  ↓
HFTrajectoryListWidget mounts
  ↓
Calls OpenThoughtsService.getTrajectories(0, 100)
  ↓
Renders first 100 items in list
```

### Selecting Trajectory

```
User clicks list item
  ↓
HFTrajectoryListWidget emits { type: "select", sessionId }
  ↓
Mount code listens to list widget events
  ↓
Calls OpenThoughtsService.getTrajectory(index)
  ↓
Sends to HFTrajectoryDetailWidget via { type: "load", sessionId }
  ↓
Detail widget renders JSON
```

### Searching

```
User types in search box
  ↓
Widget emits { type: "search", query }
  ↓
handleEvent filters current trajectories array
  ↓
Updates state.filteredTrajectories
  ↓
Re-render shows filtered list
```

---

## Implementation Plan

### Files to Create

1. **`src/effuse/widgets/hf-trajectory-list.ts`** (~300 lines)
   - List widget implementation
   - Pagination logic
   - Client-side search filtering
   - Event delegation for clicks

2. **`src/effuse/widgets/hf-trajectory-detail.ts`** (~400 lines)
   - Detail widget implementation (modeled after ATIFDetailsWidget)
   - Formatted step-by-step accordion view
   - Step expansion logic (one at a time)
   - Source badges, tool call display, observations
   - JSON formatting utilities
   - Empty/loading/error states

3. **`src/effuse/widgets/hf-trajectory-list.test.ts`** (~150 lines)
   - Test pagination
   - Test search filtering
   - Test selection events
   - Test loading states

4. **`src/effuse/widgets/hf-trajectory-detail.test.ts`** (~150 lines)
   - Test formatted step rendering
   - Test accordion expand/collapse
   - Test tool call display
   - Test observation display
   - Test empty/loading/error states
   - Test source badge colors

### Files to Modify

5. **`src/mainview/index.html`** (+10 lines)
   - Add container divs in sidebar and main areas:
   ```html
   <aside class="w-[260px] flex-shrink-0 bg-zinc-950">
     <div class="h-full p-4">
       <div id="hf-trajectory-list-widget"></div>
     </div>
   </aside>

   <main class="flex-1 bg-zinc-950">
     <div class="h-full p-4">
       <div id="hf-trajectory-detail-widget"></div>
     </div>
   </main>
   ```

6. **`src/mainview/effuse-main.ts`** (+50 lines)
   - Mount both widgets
   - Wire up event forwarding (list select → detail load)
   - Error handling

7. **`src/effuse/index.ts`** (+2 lines)
   - Export new widgets

### Files to Reference (No Changes)

- `src/huggingface/openthoughts.ts` - Service API
- `src/huggingface/schema.ts` - Data types
- `src/atif/schema.ts` - Trajectory types
- `src/effuse/widgets/trajectory-pane.ts` - Pattern reference
- `src/effuse/widgets/tb-results.ts` - Search/filter patterns

---

## UI Design

### Sidebar List

```
┌─────────────────────────────────────────┐
│ HuggingFace Trajectories          [↕]  │ <- Header with collapse
├─────────────────────────────────────────┤
│ [Search...                          ]   │ <- Search input
├─────────────────────────────────────────┤
│ Showing 1-100 of 15,209                 │ <- Count
├─────────────────────────────────────────┤
│                                         │
│ ┌─────────────────────────────────────┐ │
│ │ claude-code      Dec 6, 10:32       │ │ <- Selected (bg-zinc-800)
│ │ task: fm-bench-reasoning            │ │
│ │ episode-001 • 45 steps              │ │
│ └─────────────────────────────────────┘ │
│                                         │
│ ┌─────────────────────────────────────┐ │
│ │ mechacoder       Dec 5, 14:21       │ │ <- Hover state
│ │ task: fm-bench-coding               │ │
│ │ episode-002 • 128 steps             │ │
│ └─────────────────────────────────────┘ │
│                                         │
│ ... (more items)                        │
│                                         │
├─────────────────────────────────────────┤
│ [← Prev]              [Next →]         │ <- Pagination
└─────────────────────────────────────────┘
```

### Main Detail View

```
┌──────────────────────────────────────────────────────────┐
│ HF Trajectory Details                          [▲]      │ <- Header (collapsible)
├──────────────────────────────────────────────────────────┤
│ Session: abc123-def456                                   │
│ Agent: claude-code (claude-sonnet-4)                     │
│ Task: fm-bench-reasoning • Episode: episode-001          │
│ Steps: 45 • Date: Dec 6, 2024 10:32                      │
├──────────────────────────────────────────────────────────┤
│                                                          │
│ ┌────────────────────────────────────────────────────┐  │
│ │ #1  [USER]  10:32:01  ▼                            │  │ <- Step header (collapsed)
│ └────────────────────────────────────────────────────┘  │
│                                                          │
│ ┌────────────────────────────────────────────────────┐  │
│ │ #2  [AGENT]  10:32:05  🔧 3 tools  ▲               │  │ <- Step header (expanded)
│ ├────────────────────────────────────────────────────┤  │
│ │ Message:                                           │  │
│ │ ┌────────────────────────────────────────────────┐ │  │
│ │ │ "I'll help you with that task..."             │ │  │
│ │ └────────────────────────────────────────────────┘ │  │
│ │                                                    │  │
│ │ Tool Calls:                                        │  │
│ │ ┌────────────────────────────────────────────────┐ │  │
│ │ │ read_file                                      │ │  │
│ │ │ { "path": "/app/src/main.ts" }                 │ │  │
│ │ └────────────────────────────────────────────────┘ │  │
│ │ ┌────────────────────────────────────────────────┐ │  │
│ │ │ grep_search                                    │ │  │
│ │ │ { "pattern": "export.*function" }              │ │  │
│ │ └────────────────────────────────────────────────┘ │  │
│ │                                                    │  │
│ │ Observation:                                       │  │
│ │ ┌────────────────────────────────────────────────┐ │  │
│ │ │ File contents: ...                             │ │  │
│ │ └────────────────────────────────────────────────┘ │  │
│ │                                                    │  │
│ │ Metrics: 1,234 prompt tokens • 456 completion      │  │
│ └────────────────────────────────────────────────────┘  │
│                                                          │
│ ┌────────────────────────────────────────────────────┐  │
│ │ #3  [AGENT]  10:32:12  ▼                           │  │ <- Another step (collapsed)
│ └────────────────────────────────────────────────────┘  │
│                                                          │
│ ... (more steps, scrollable)                             │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

### Tailwind Classes

Following existing widget patterns:

**List item:**
- Normal: `bg-zinc-900/40 border border-zinc-800/40 rounded-lg p-3 mb-2 cursor-pointer`
- Hover: `hover:bg-zinc-900/60`
- Selected: `bg-zinc-800/60 border-zinc-700/50`

**Search input:**
- `w-full px-3 py-2 bg-zinc-900 border border-zinc-800 rounded text-zinc-100 focus:border-zinc-700`

**Step accordion:**
- Step header: `flex items-center justify-between px-4 py-2 hover:bg-zinc-900/40 cursor-pointer transition-colors`
- Expanded content: `px-4 py-3 bg-zinc-900/20 space-y-3`
- Message/tool call blocks: `text-xs font-mono bg-zinc-950/60 p-2 rounded border border-zinc-800/40 overflow-x-auto`
- Tool calls: `text-violet-300` (function name), `text-zinc-400` (args)
- Observations: `text-emerald-300`

---

## Testing Strategy

### Unit Tests (Bun Test + makeTestLayer)

**HFTrajectoryListWidget:**
1. Renders empty state when no trajectories
2. Renders list of trajectories
3. Filters trajectories on search
4. Emits select event on click
5. Pagination updates current page
6. Shows loading state

**HFTrajectoryDetailWidget:**
1. Renders empty state when no trajectory selected
2. Renders metadata header correctly
3. Renders step list with accordion behavior
4. Expands/collapses steps correctly (one at a time)
5. Displays tool calls with violet styling
6. Displays observations with emerald styling
7. Shows correct source badges (user=blue, agent=emerald, system=gray)
8. Shows loading state while fetching
9. Handles error state

### Integration Test (Manual)
1. Open app, verify sidebar shows first 100 trajectories
2. Type in search box, verify list filters
3. Click trajectory, verify JSON appears in main area
4. Click Next, verify page 2 loads
5. Click Copy JSON, verify clipboard has content

---

## Performance Considerations

### Phase 1 Acceptable Limitations
- **100 items per page**: Fast rendering, no virtual scroll needed
- **Client-side search**: Works fine for 100 items (filters in <1ms)
- **No caching**: Parquet reads are fast enough (~200ms per page load)
- **JSON display**: May freeze on huge trajectories (>1000 steps), acceptable for MVP

### Phase 2 Optimizations (If Needed)
- Virtual scrolling for infinite list
- Search indexing for instant multi-field search
- Trajectory caching to avoid re-fetching
- Formatted view instead of raw JSON

---

## Dependencies

**All dependencies already exist:**
- ✅ OpenThoughtsService with getTrajectories(), count(), getTrajectory()
- ✅ Dataset downloaded (15,209 trajectories, 104.6 MB)
- ✅ DomService, StateService, SocketService from Effuse
- ✅ ATIF types (Trajectory, Step, Agent)
- ✅ Tailwind CSS via index.html

**No new npm packages needed.**

---

## Success Criteria

### Phase 1 (This PR)
1. ✅ User can browse 100 trajectories at a time in sidebar
2. ✅ User can search by agent/task/episode (instant client-side filter)
3. ✅ User can click trajectory to view formatted steps in main area
4. ✅ User can expand/collapse individual steps to see details
5. ✅ User can see tool calls, observations, and metrics for each step
6. ✅ User can paginate through all 15,209 trajectories
7. ✅ Initial load completes in <3 seconds
8. ✅ Search/selection responds in <100ms
9. ✅ Step expansion is smooth (accordion behavior)

### Phase 2 (Future)
- Virtual scrolling for seamless browsing
- Multi-field advanced filtering
- Formatted trajectory view with step timeline
- Export trajectory to file
- Trajectory comparison mode

---

## Estimated Effort

**Total: 10-12 hours**

| Task | Hours |
|------|-------|
| HFTrajectoryListWidget | 2.5 |
| HFTrajectoryDetailWidget (formatted view) | 4.0 |
| Widget tests | 3.0 |
| Integration (mount, events) | 1.5 |
| HTML changes | 0.5 |
| Manual testing | 0.5 |

---

## Risk Analysis

**Low Risk:**
- Following proven widget patterns (TrajectoryPane, TBResults)
- Service layer already validated via CLI
- Effect.gen patterns well-established

**Medium Risk:**
- Large trajectories (>500 steps) might render slowly
  - **Mitigation:** Limit initial display to first 100 steps, add "Load more" button
  - **Mitigation:** Steps are collapsed by default (only headers rendered), so DOM is manageable

**High Risk:** None

---

## Critical Files Reference

1. **`src/huggingface/openthoughts.ts`** - Service implementation
2. **`src/atif/schema.ts`** - Trajectory type definitions
3. **`src/effuse/widgets/atif-details.ts`** - **Detail view pattern (model after this!)**
4. **`src/effuse/widgets/trajectory-pane.ts`** - List widget pattern
5. **`src/effuse/widgets/tb-results.ts`** - Search/filter pattern
6. **`src/mainview/effuse-main.ts`** - Widget mounting pattern
7. **`src/effuse/testing/layers/test.ts`** - Testing utilities

---

## Alternative Approaches Considered

### Virtual Scrolling (Performance-First)
- **Pros:** Handle all 15k items smoothly, no pagination
- **Cons:** Complex implementation (windowing, scroll math), 6+ extra hours
- **Decision:** Defer to Phase 2, pagination is acceptable for MVP

### Single Widget (Simpler)
- **Pros:** One less file, shared state
- **Cons:** Violates separation of concerns, harder to test
- **Decision:** Two widgets follows Effuse patterns better

### Rich Formatted View (UX-First)
- **Pros:** Better user experience, step timeline, tool call inspection
- **Cons:** 2x implementation time
- **Decision:** ✅ **Chosen approach** - User selected formatted view. Model after ATIFDetailsWidget.

---

## Future Enhancements

### Phase 2: Performance & UX
- Virtual scrolling for seamless browsing
- Multi-field filtering (task, agent, model, date range, outcome)
- Search indexing for instant results
- Formatted trajectory view with tabs (Overview, Timeline, JSON)

### Phase 3: Analytics
- Error grouping (show common failure patterns)
- Success rate by task/agent/model
- Token usage visualization
- Duration histograms

### Phase 4: Advanced Features
- Trajectory comparison (side-by-side)
- Export to file (JSON, Markdown)
- Bookmark/favorite trajectories
- Annotate trajectories with notes

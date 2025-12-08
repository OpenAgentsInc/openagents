# Refactor TestGen UI to ATIF Thread Format

## Goal

Transform the TestGen UI from a sectioned layout (reflections boxed at top, test cards below) into a chronological thread-based display using reusable ATIF-style components.

## Current State (Problems)

**File:** `src/effuse/widgets/tb-command-center/tbcc-testgen.ts`

**Current Layout:**
```
┌─────────────────────────────────────┐
│ Header (fixed)                      │
├─────────────────────────────────────┤
│ Controls (fixed)                    │
├─────────────────────────────────────┤
│ Environment Panel (fixed)           │
├─────────────────────────────────────┤
│ Progress Indicator (fixed)          │
├─────────────────────────────────────┤
│ ┌─────────────────────────────────┐ │ ← PROBLEM: Reflections boxed separately
│ │ Reflection Panel (boxed)        │ │
│ │ (max-h-32, scrolls internally)  │ │
│ └─────────────────────────────────┘ │
│ ┌─────────────────────────────────┐ │ ← PROBLEM: Tests batched below
│ │ Test Cards (category-grouped)   │ │
│ └─────────────────────────────────┘ │
└─────────────────────────────────────┘
```

**Issues:**
1. **Non-chronological**: Reflections shown separately from tests
2. **Poor UX**: Can't see when reflection occurred relative to tests
3. **Not reusable**: Custom rendering logic tightly coupled to TestGen
4. **Scrolling confusion**: Reflections scroll internally, tests scroll separately

## New Design (Thread-Based)

**Desired Layout:**
```
┌─────────────────────────────────────┐
│ Header (fixed)                      │
├─────────────────────────────────────┤
│ Controls (fixed)                    │
├─────────────────────────────────────┤
│ Environment Panel (fixed)           │
├─────────────────────────────────────┤
│ Progress Indicator (fixed)          │
├─────────────────────────────────────┤
│ ┌─────────────────────────────────┐ │
│ │ Thread Container                │ │ ← Chronological thread
│ │ ┌─────────────────────────────┐ │ │
│ │ │ [15:30:01] PROGRESS          │ │ │ ← Item type: progress
│ │ │ "Generating anti_cheat..."  │ │ │
│ │ ├─────────────────────────────┤ │ │
│ │ │ [15:30:05] TEST              │ │ │ ← Item type: test
│ │ │ 🔴 anti_cheat_1 (95%)       │ │ │
│ │ │ ▼ (click to expand)         │ │ │
│ │ ├─────────────────────────────┤ │ │
│ │ │ [15:30:06] TEST              │ │ │
│ │ │ 🔴 anti_cheat_2 (90%)       │ │ │
│ │ ├─────────────────────────────┤ │ │
│ │ │ [15:30:10] REFLECTION        │ │ │ ← Item type: reflection
│ │ │ 💭 [anti_cheat] Need more...│ │ │
│ │ ├─────────────────────────────┤ │ │
│ │ │ [15:30:15] TEST              │ │ │
│ │ │ 🔵 existence_1 (85%)        │ │ │
│ │ └─────────────────────────────┘ │ │
│ └─────────────────────────────────┘ │
└─────────────────────────────────────┘
```

## Architecture

### 1. Reusable ATIF Thread Components

**New File:** `src/effuse/components/atif-thread.ts`

**Exports:**
```typescript
// Thread container
export function renderThreadContainer(items: ThreadItem[], options?: ThreadOptions): TemplateResult

// Individual item renderers
export function renderProgressItem(progress: ProgressData, options?: ItemOptions): TemplateResult
export function renderReflectionItem(reflection: ReflectionData, options?: ItemOptions): TemplateResult
export function renderTestItem(test: TestData, expanded: boolean, options?: ItemOptions): TemplateResult

// Generic thread item wrapper
export function renderThreadItem(item: ThreadItem, state: ThreadItemState): TemplateResult
```

**ThreadItem Types:**
```typescript
type ThreadItem =
  | { type: "progress"; timestamp: number; data: ProgressData }
  | { type: "reflection"; timestamp: number; data: ReflectionData }
  | { type: "test"; timestamp: number; data: TestData }
  | { type: "complete"; timestamp: number; data: CompleteData }
  | { type: "error"; timestamp: number; data: ErrorData }

interface ProgressData {
  phase: string
  category: string | null
  round: number
  status: string
}

interface ReflectionData {
  category: string | null
  text: string
  action: "refining" | "assessing" | "complete"
}

interface TestData {
  id: string
  category: string
  input: string
  expectedOutput: string | null
  reasoning: string
  confidence: number
}
```

### 2. TestGen Widget Refactoring

**File:** `src/effuse/widgets/tb-command-center/tbcc-testgen.ts`

**State Changes:**
```typescript
// OLD: Separate arrays for reflections and tests
reflections: Array<{ category, text, action }>
tests: Array<{ id, category, input, ... }>

// NEW: Unified chronological thread
threadItems: ThreadItem[]  // Reflections, tests, progress all in timestamp order
expandedItemId: string | null  // For accordion expansion
```

**Rendering Changes:**
```typescript
// OLD: Separate rendering
const reflectionPanel = ...
const testCards = ...
const scrollableContent = html`${reflectionPanel} ${testCards}`

// NEW: Unified thread rendering
import { renderThreadContainer } from "../../components/atif-thread.js"
const threadView = renderThreadContainer(state.threadItems, {
  expandedItemId: state.expandedItemId,
  onToggle: (itemId) => ctx.emit({ type: "toggleItem", itemId })
})
```

**Message Handling Changes:**
```typescript
// OLD: Push to separate arrays
if (msg.type === "testgen_reflection") {
  state.reflections.push({ ... })
}
if (msg.type === "testgen_test") {
  state.tests.push({ ... })
}

// NEW: Push to unified thread with timestamps
if (msg.type === "testgen_reflection") {
  state.threadItems.push({
    type: "reflection",
    timestamp: Date.now(),
    data: { ... }
  })
}
if (msg.type === "testgen_test") {
  state.threadItems.push({
    type: "test",
    timestamp: Date.now(),
    data: { ... }
  })
}
```

### 3. ATIF Thread Component Implementation

**Accordion Pattern:**
- Click item header to expand/collapse
- Only one item expanded at a time (accordion mode)
- Expanded item shows full details (input, output, reasoning for tests)
- Collapsed items show summary line only

**Visual Design:**
```
┌─────────────────────────────────────────────┐
│ [15:30:05] 🔴 TEST: anti_cheat_1 (95%) ▼   │ ← Collapsed
├─────────────────────────────────────────────┤
│ [15:30:06] 🔴 TEST: anti_cheat_2 (90%) ▲   │ ← Expanded
│ ┌─────────────────────────────────────────┐ │
│ │ Input:                                   │ │
│ │ which R 2>/dev/null || echo 'not found' │ │
│ │                                          │ │
│ │ Expected Output:                         │ │
│ │ not found                                │ │
│ │                                          │ │
│ │ Reasoning:                               │ │
│ │ R should not be installed for R→Python   │ │
│ │ conversion task. Verify prohibited tool. │ │
│ │                                          │ │
│ │ Confidence: ████████████████░░░░ 90%     │ │
│ └─────────────────────────────────────────┘ │
├─────────────────────────────────────────────┤
│ [15:30:10] 💭 REFLECTION ▼                  │
│ [anti_cheat] Need more edge cases for...   │
└─────────────────────────────────────────────┘
```

**Color Coding (Match Current):**
- 🔴 `anti_cheat`: Red/Pink
- 🔵 `existence`: Blue
- 🟢 `correctness`: Emerald
- 🟡 `boundary`: Yellow
- 🟣 `integration`: Purple
- 💭 `reflection`: Blue (softer)
- ⚙️ `progress`: Gray

## Implementation Steps

### Step 1: Create Reusable Thread Components

**New File:** `src/effuse/components/atif-thread.ts`

```typescript
import { html, joinTemplates, type TemplateResult } from "../template/html.js"

// Thread item types
export type ThreadItem = /* ... */

// Render full thread container
export function renderThreadContainer(
  items: ThreadItem[],
  options: {
    expandedItemId: string | null
    onToggle?: (itemId: string) => void
  }
): TemplateResult {
  const itemElements = items.map(item => {
    const itemId = getItemId(item)
    const isExpanded = itemId === options.expandedItemId
    return renderThreadItem(item, { isExpanded, onToggle: options.onToggle })
  })

  return html`
    <div class="flex flex-col gap-2">
      ${joinTemplates(itemElements)}
    </div>
  `
}

// Render individual thread item
export function renderThreadItem(
  item: ThreadItem,
  state: { isExpanded: boolean; onToggle?: (id: string) => void }
): TemplateResult {
  const timestamp = formatTimestamp(item.timestamp)

  switch (item.type) {
    case "progress":
      return renderProgressItem(timestamp, item.data, state)
    case "reflection":
      return renderReflectionItem(timestamp, item.data, state)
    case "test":
      return renderTestItem(timestamp, item.data, state)
    case "complete":
      return renderCompleteItem(timestamp, item.data, state)
    case "error":
      return renderErrorItem(timestamp, item.data, state)
  }
}

// Individual item renderers
function renderTestItem(
  timestamp: string,
  test: TestData,
  state: { isExpanded: boolean }
): TemplateResult {
  const categoryBadge = getCategoryBadge(test.category)
  const confidenceBar = renderConfidenceBar(test.confidence)

  const header = html`
    <div
      class="flex items-center justify-between p-3 bg-zinc-900/60 border border-zinc-800/60 rounded-lg cursor-pointer hover:bg-zinc-900/80 transition-colors"
      data-action="toggleItem"
      data-item-id="${test.id}"
    >
      <div class="flex items-center gap-3">
        <span class="text-xs text-zinc-500 font-mono">${timestamp}</span>
        ${categoryBadge}
        <span class="text-sm text-zinc-200 font-mono">${test.id}</span>
        <span class="text-xs text-zinc-400">(${Math.round(test.confidence * 100)}%)</span>
      </div>
      <span class="text-zinc-500">${state.isExpanded ? "▲" : "▼"}</span>
    </div>
  `

  if (!state.isExpanded) {
    return header
  }

  const details = html`
    <div class="mt-2 p-4 bg-zinc-950/60 border border-zinc-800/40 rounded-lg space-y-3">
      <div>
        <label class="text-xs font-mono text-zinc-500 uppercase">Input</label>
        <pre class="mt-1 p-2 bg-zinc-900/60 rounded text-sm font-mono text-emerald-300 overflow-x-auto">${test.input}</pre>
      </div>

      ${test.expectedOutput ? html`
        <div>
          <label class="text-xs font-mono text-zinc-500 uppercase">Expected Output</label>
          <pre class="mt-1 p-2 bg-zinc-900/60 rounded text-sm font-mono text-blue-300 overflow-x-auto">${test.expectedOutput}</pre>
        </div>
      ` : ""}

      <div>
        <label class="text-xs font-mono text-zinc-500 uppercase">Reasoning</label>
        <p class="mt-1 text-sm text-zinc-300 leading-relaxed">${test.reasoning}</p>
      </div>

      <div>
        <label class="text-xs font-mono text-zinc-500 uppercase">Confidence</label>
        ${confidenceBar}
      </div>
    </div>
  `

  return html`${header} ${details}`
}

// Helper: category badges
function getCategoryBadge(category: string): TemplateResult {
  const badges = {
    anti_cheat: { emoji: "🔴", class: "bg-red-900/40 text-red-300 border-red-700/50" },
    existence: { emoji: "🔵", class: "bg-blue-900/40 text-blue-300 border-blue-700/50" },
    correctness: { emoji: "🟢", class: "bg-emerald-900/40 text-emerald-300 border-emerald-700/50" },
    boundary: { emoji: "🟡", class: "bg-yellow-900/40 text-yellow-300 border-yellow-700/50" },
    integration: { emoji: "🟣", class: "bg-purple-900/40 text-purple-300 border-purple-700/50" },
  }

  const badge = badges[category] || { emoji: "⚪", class: "bg-zinc-800/40 text-zinc-300 border-zinc-700/50" }

  return html`
    <span class="px-2 py-1 text-xs font-mono border rounded ${badge.class}">
      ${badge.emoji} ${category}
    </span>
  `
}
```

### Step 2: Update TestGen Widget State

**File:** `src/effuse/widgets/tb-command-center/tbcc-testgen.ts`

```typescript
// Replace state fields:
export interface TBTestGenState {
  // ... existing fields ...

  // REMOVE these:
  // tests: Array<{ ... }>
  // reflections: Array<{ ... }>

  // ADD these:
  threadItems: ThreadItem[]  // Unified chronological thread
  expandedItemId: string | null  // Accordion expansion state
}

// Update events:
export type TBTestGenEvent =
  | { type: "loadSuite" }
  | { type: "selectTask"; taskId: string | null }
  | { type: "generate" }
  | { type: "clear" }
  | { type: "cancel" }
  | { type: "toggleItem"; itemId: string }  // NEW: Toggle accordion
```

### Step 3: Update Message Handling

**File:** `src/effuse/widgets/tb-command-center/tbcc-testgen.ts`

```typescript
// In subscriptions, update each message handler:

if (msg.type === "testgen_progress") {
  yield* ctx.state.update(s => ({
    ...s,
    threadItems: [
      ...s.threadItems,
      {
        type: "progress",
        timestamp: Date.now(),
        data: {
          phase: msg.phase,
          category: msg.currentCategory ?? null,
          round: msg.roundNumber,
          status: msg.status
        }
      }
    ],
    currentPhase: msg.phase,
    currentCategory: msg.currentCategory ?? null,
    currentRound: msg.roundNumber,
    progressStatus: msg.status
  }))
}

if (msg.type === "testgen_test") {
  yield* ctx.state.update(s => ({
    ...s,
    threadItems: [
      ...s.threadItems,
      {
        type: "test",
        timestamp: Date.now(),
        data: msg.test
      }
    ]
  }))
}

if (msg.type === "testgen_reflection") {
  yield* ctx.state.update(s => ({
    ...s,
    threadItems: [
      ...s.threadItems,
      {
        type: "reflection",
        timestamp: Date.now(),
        data: {
          category: msg.category ?? null,
          text: msg.reflectionText,
          action: msg.action
        }
      }
    ]
  }))
}
```

### Step 4: Update Rendering

**File:** `src/effuse/widgets/tb-command-center/tbcc-testgen.ts`

```typescript
import { renderThreadContainer } from "../../components/atif-thread.js"

// In render function:
const threadView = state.threadItems.length > 0 ? html`
  <div class="flex-1 overflow-y-auto p-4">
    ${renderThreadContainer(state.threadItems, {
      expandedItemId: state.expandedItemId,
      onToggle: (itemId) => Effect.runFork(ctx.emit({ type: "toggleItem", itemId }))
    })}
  </div>
` : ""

// Update main container:
const result = html`
  <div class="h-full flex flex-col bg-zinc-950">
    ${header}
    ${controls}
    ${environmentPanel}
    ${taskDescPanel}
    ${progressIndicator}
    ${errorPanel}
    ${emptyState}
    ${loadingState}
    ${threadView}  <!-- SINGLE unified thread view -->
    ${completionSummary}
  </div>
`
```

### Step 5: Handle Accordion Events

**File:** `src/effuse/widgets/tb-command-center/tbcc-testgen.ts`

```typescript
// In setupEvents:
yield* ctx.dom.delegate(ctx.container, "[data-action='toggleItem']", "click", (_e, target) => {
  const itemId = (target as HTMLElement).dataset.itemId
  if (itemId) {
    Effect.runFork(ctx.emit({ type: "toggleItem", itemId }))
  }
})

// In handleEvent:
case "toggleItem": {
  yield* ctx.state.update(s => ({
    ...s,
    expandedItemId: s.expandedItemId === event.itemId ? null : event.itemId
  }))
  break
}
```

## Files to Create/Modify

### New Files

| File | Purpose |
|------|---------|
| `src/effuse/components/atif-thread.ts` | Reusable thread components (container, item renderers, helpers) |

### Modified Files

| File | Changes |
|------|---------|
| `src/effuse/widgets/tb-command-center/tbcc-testgen.ts` | Refactor to use thread components, update state, rendering, events |
| `src/effuse/index.ts` | Export thread components for reuse |

## Benefits

1. **Chronological**: See exact timeline of generation (progress → tests → reflections → more tests)
2. **Reusable**: Thread components can be used in other widgets (ATIF viewer, agent logs, etc.)
3. **Cleaner UX**: No separate scrolling regions, unified thread view
4. **Expandable Details**: Accordion pattern for test details (collapsed by default)
5. **Type-safe**: Generic `ThreadItem` type can support any item type

## Migration Path

1. Create thread components first (backward-compatible)
2. Update TestGen widget to use threads (breaking change to state)
3. Test with existing test generation flow
4. Update E2E tests for new state structure
5. Roll out to other widgets (ATIF viewer, etc.)

## Future Enhancements

- Add filtering by item type (only tests, only reflections, etc.)
- Add search within thread
- Add export thread as ATIF trajectory
- Add thread replay (step through chronologically)
- Add virtual scrolling for very long threads

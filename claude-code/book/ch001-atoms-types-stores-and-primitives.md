# 1. The Atoms: Identity, Reactive Stores, Lazy Schemas, and Async Generators

> **Status: written.** Reverse-engineered from the Claude Code 2.1.88 source snapshot (see the Prologue (`prologue-how-to-read-a-1900-file-agent.md`) for snapshot caveats).
>
> **Layer:** Layer 0 (Leaf Primitives & Zero-Dependency Foundations)
> **Prerequisites:** [Chapter 00](prologue-how-to-read-a-1900-file-agent.md)
> **Primary Source Files:**
> - `types/ids.ts` — compile-time branded `SessionId` and `AgentId` (44 lines)
> - `state/store.ts` — minimal reactive store (34 lines)
> - `state/AppStateStore.ts` & `state/AppState.tsx` — immutable session state and `useSyncExternalStore` bindings
> - `bootstrap/state.ts` — process-wide leaf singleton
> - `utils/lazySchema.ts` — deferred Zod schema factory (8 lines)
> - `utils/generators.ts` — bounded concurrent async-generator multiplexer (88 lines)
> - `utils/fileStateCache.ts` — path-normalizing, byte-bounded LRU file cache (142 lines)

---

## Why This Layer Exists

Before an agentic coding tool can prompt an LLM, execute a shell command, or draw a single pixel to a terminal, it needs six low-level guarantees:

1. **Unambiguous identity** — never accidentally pass a subagent ID where a session ID is expected.
2. **Reactive session state without re-render storms** — hundreds of UI components and non-React services must read and update shared state cleanly.
3. **Process-wide state without circular imports** — deep leaf utilities need the current working directory, session ID, and cost counters without importing React or high-level orchestrators.
4. **Zero-cost cold-start declarations** — defining 60+ tool schemas must not burn CPU when the process boots.
5. **Bounded streaming concurrency** — running multiple read-only tools at once must stream progress incrementally while respecting a concurrency cap.
6. **Bounded file awareness** — remembering what files the model has read so edits don't clobber unseen changes, without leaking gigabytes of RAM.

Remarkably, the codebase solves all six with small, crisp, self-contained primitives at the bottom of the import graph. Let's dissect each one.

Four of those primitives are short enough to read whole. Browse them here, then follow the sections below:

```text
types/ids.ts          (44 lines)  — Branded SessionId & AgentId
state/store.ts        (34 lines)  — Minimal reactive external store
utils/lazySchema.ts   (8 lines)   — Deferred Zod schema factory
utils/generators.ts   (88 lines)  — Bounded async-generator multiplexer
```

---

## 1. Branded Identity (`types/ids.ts`)

### The Problem

An agent process quickly juggles multiple string identifiers: the top-level conversation's session UUID, subagent IDs, teammate names, and tool-use IDs. In plain TypeScript, all of them are `string`. Passing a `SessionId` to a function that writes a subagent sidechain transcript—or passing an `AgentId` to a session telemetry event—compiles silently and corrupts disk state at runtime.

### The Implementation

`types/ids.ts` is 44 lines with **zero imports**. It uses TypeScript structural branding (`string & { readonly __brand: ... }`) to make `SessionId` and `AgentId` mutually incompatible at compile time while costing zero bytes at runtime:

```ts
// types/ids.ts:10-17
export type SessionId = string & { readonly __brand: 'SessionId' }
export type AgentId = string & { readonly __brand: 'AgentId' }
```

Even better, `toAgentId()` acts as a runtime parsing boundary between arbitrary user/team strings and genuine agent IDs:

```ts
// types/ids.ts:35-44
const AGENT_ID_PATTERN = /^a(?:.+-)?[0-9a-f]{16}$/

export function toAgentId(s: string): AgentId | null {
  return AGENT_ID_PATTERN.test(s) ? (s as AgentId) : null
}
```

### Why This Matters Upward

Look at `Tool.ts:245`:
```ts
agentId?: AgentId // Only set for subagents; use getSessionId() for session ID.
```
And look at `bootstrap/state.ts:178`: invoked skills are keyed by `${agentId ?? ''}:${skillName}` so a subagent invoking a skill never overwrites the main thread's record. Because `AgentId` is distinct from `SessionId` at the type level, hooks, transcript storage, and telemetry cannot confuse main-thread calls with subagent calls.

---

## 2. The 34-Line Reactive Store (`state/store.ts` → `state/AppState.tsx`)

### The Problem

Claude Code has two very different consumers of session state:
- **React/Ink terminal components** (389 files under `components/` and `screens/`), which must re-render *only* when the exact slice of state they display changes—otherwise every spinner tick or token chunk would re-render the whole terminal screen.
- **Non-React imperative code** (`query.ts`, `Tool.call()`, `QueryEngine.ts`, background tasks, settings file watchers), which needs synchronous `getState()` / `setState(prev => next)` access outside the React tree.

### The Implementation

Instead of pulling in Redux or a large external state framework, `state/store.ts` builds the entire reactive primitive in **34 lines**:

```ts
// state/store.ts:1-34
type Listener = () => void
type OnChange<T> = (args: { newState: T; oldState: T }) => void

export type Store<T> = {
  getState: () => T
  setState: (updater: (prev: T) => T) => void
  subscribe: (listener: Listener) => () => void
}

export function createStore<T>(
  initialState: T,
  onChange?: OnChange<T>,
): Store<T> {
  let state = initialState
  const listeners = new Set<Listener>()

  return {
    getState: () => state,

    setState: (updater: (prev: T) => T) => {
      const prev = state
      const next = updater(prev)
      if (Object.is(next, prev)) return
      state = next
      onChange?.({ newState: next, oldState: prev })
      for (const listener of listeners) listener()
    },

    subscribe: (listener: Listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}
```

Pause and admire four quiet design choices in those 34 lines:

1. **Functional updaters only (`updater: (prev: T) => T`).** Callers cannot pass a raw partial object that silently clobbers concurrent updates; every write computes from the latest `prev`.
2. **Exact `Object.is(next, prev)` bailout.** If an updater returns `prev` unchanged, neither `onChange` nor any subscriber fires.
3. **Synchronous `onChange({ newState, oldState })` side-effect hook.** Runs *before* React subscribers—used by `state/onChangeAppState.ts` to react to cross-cutting transitions (like mode changes or notifications) regardless of what is mounted.
4. **Zero React dependency.** Headless SDK mode (`QueryEngine.ts`) and tests can instantiate `createStore(getDefaultAppState())` without ever importing React or Ink!

### Bridging to React (`state/AppState.tsx`)

When the interactive terminal UI *does* mount, `AppStateProvider` creates the store **once** in `useState(() => createStore(...))` and puts the stable `store` object (never the `state` value!) into `AppStoreContext`. Because the context value never changes, the provider never forces cascading re-renders.

Components read slices through `useAppState(selector)`, which plugs directly into React 18+'s `useSyncExternalStore`:

```ts
// From state/AppState.tsx (original source)
export function useAppState<T>(selector: (state: AppState) => T): T {
  const store = useAppStore()
  const get = () => {
    const state = store.getState()
    const selected = selector(state)
    // In internal ('ant') builds, throws if a selector returns the entire state!
    return selected
  }
  return useSyncExternalStore(store.subscribe, get, get)
}
```

And components that only *write* state call `useSetAppState()`, which returns `store.setState`—a reference that never changes, guaranteeing zero re-renders.

> [!IMPORTANT]
> **Notice the architectural split between `AppStateStore.ts` and `AppState.tsx`:**
> Look at the comment in `state/AppState.tsx:23-25`:
> *"Kept for back-compat during migration so `.ts` callers can incrementally move off the `.tsx` import and stop pulling React."*
> Keeping the `AppState` type and `getDefaultAppState()` in a pure `.ts` file (`state/AppStateStore.ts`) prevents headless code paths from accidentally bundling React.

---

## 3. The Process-Wide Leaf Singleton (`bootstrap/state.ts`)

### The Problem

Wait—if `AppState` already exists, why does `bootstrap/state.ts` exist too?

Because **process lifetime ≠ React/session lifetime**, and **import graphs hate cycles**.

Think about everything that happens *before* `AppState` is even created—or deep inside a utility that doesn't receive a `ToolUseContext`:
- What was `originalCwd` at launch, and what is the stable `projectRoot` (which must NOT change if the agent enters a Git worktree mid-session)?
- What is the active `sessionId`?
- How much USD (`totalCostUSD`), API latency (`totalAPIDuration`), and tokens (`modelUsage`) has this process spent across turns and subagents?
- What hooks were registered by SDK callbacks or plugins (`registeredHooks`)?
- Which skills have been invoked so compaction can preserve them (`invokedSkills`)?

If a low-level logger or permission helper imported `AppState` or `QueryEngine`, the module graph would knot into circular dependencies immediately.

### The Solution: A Strictly Isolated Leaf

`bootstrap/state.ts` holds a single private `State` object exposed via focused getter/setter functions—and is protected by a custom lint rule (`custom-rules/bootstrap-isolation`) to keep it a **leaf of the import DAG**.

Look at the real scars visible right in `bootstrap/state.ts`'s comments:

```ts
// bootstrap/state.ts:46-50
originalCwd: string
// Stable project root - set once at startup (including by --worktree flag),
// never updated by mid-session EnterWorktreeTool.
// Use for project identity (history, skills, sessions) not file operations.
projectRoot: string
```

And look at line 121-123—a textbook example of using a leaf store to break a 4-module import cycle:

```ts
// CLAUDE.md content cached by context.ts for the auto-mode classifier.
// Breaks the yoloClassifier → claudemd → filesystem → permissions cycle.
cachedClaudeMdContent: string | null
```

And line 144-149—why mutable session registries live here instead of scattered module-level variables:

```ts
// Teams created this session via TeamCreate. ...
// Lives here (not teamHelpers.ts) so resetStateForTests() clears it between tests.
sessionCreatedTeams: Set<string>
```

### The Mental Rule: `bootstrap/state.ts` vs. `AppState`

| Dimension | `bootstrap/state.ts` | `AppState` (`state/AppStateStore.ts`) |
| --- | --- | --- |
| **Lifetime** | Process-wide | Session / UI store instance |
| **Mutability** | Mutable singleton fields + getters/setters | `DeepImmutable<...>` replaced via pure updaters |
| **Reactivity** | Non-reactive (imperative reads) | Reactive (`useSyncExternalStore` + `onChange`) |
| **Import position** | Strict leaf of import DAG | Mid-layer (imports tool, task, MCP, plugin types) |
| **Examples** | `sessionId`, `projectRoot`, `totalCostUSD`, `cachedClaudeMdContent`, `invokedSkills` | `toolPermissionContext`, `tasks`, `mcp`, `plugins`, `mainLoopModel`, `expandedView` |

---

## 4. Deferred Schema Construction (`utils/lazySchema.ts`)

### The Problem

Every tool, permission rule, hook, and setting in Claude Code validates structured data with Zod v4 (`zod/v4`). Constructing dozens of deeply nested Zod schemas eagerly at module import time burns tens of milliseconds on every single CLI invocation—even if the user only ran a quick local command or only used 2 of the 60+ tools.

### The Implementation

The entire file `utils/lazySchema.ts` is **8 lines**:

```ts
// utils/lazySchema.ts:1-8
/**
 * Returns a memoized factory function that constructs the value on first call.
 * Used to defer Zod schema construction from module init time to first access.
 */
export function lazySchema<T>(factory: () => T): () => T {
  let cached: T | undefined
  return () => (cached ??= factory())
}
```

### How It Pairs with JavaScript Getters

On a tool definition (like `tools/GlobTool/GlobTool.ts:26-75`), `lazySchema` is paired with a JS `get` accessor:

```ts
const inputSchema = lazySchema(() =>
  z.strictObject({
    pattern: z.string().describe('...'),
    path: z.string().optional().describe('...'),
  }),
)

export const GlobTool = buildTool({
  ...
  get inputSchema() {
    return inputSchema()
  },
})
```

Notice the subtlety: if you wrote `inputSchema: inputSchema()`, you would evaluate the thunk eagerly when `GlobTool` is defined! Wrapping it inside `get inputSchema()` defers Zod execution until the exact moment the API serializer or validator actually reads `tool.inputSchema`.

> [!WARNING]
> **One subtle invariant of `buildTool` + getters:** Object spread `{ ...TOOL_DEFAULTS, ...def }` invokes enumerable getters on `def` when `buildTool` runs! That is worth checking whenever you optimize cold-start paths: `lazySchema` still guarantees single construction per schema and avoids module-top-level evaluation for standalone schemas like `permissionModeSchema` (`utils/permissions/PermissionMode.ts:21`) that are called as functions `permissionModeSchema()`.

---

## 5. Bounded Async-Generator Multiplexing (`utils/generators.ts`)

### The Problem

Suppose the LLM asks to call 12 read-only tools in parallel (`Read` x8, `Grep` x3, `Glob` x1). Each tool doesn't just return a final `Promise`—it is an `AsyncGenerator` that yields intermediate progress updates (`ProgressMessage`) along the way before yielding its final `tool_result`!

`Promise.all()` cannot solve this:
1. `Promise.all()` waits until *everything* finishes, throwing away incremental progress streaming.
2. `Promise.all()` launches all N tasks simultaneously with no concurrency cap.

### The Implementation

`utils/generators.ts` implements `all()`—an 88-line file whose heart is a 40-line concurrent async-generator multiplexer:

```ts
// utils/generators.ts:32-72
export async function* all<A>(
  generators: AsyncGenerator<A, void>[],
  concurrencyCap = Infinity,
): AsyncGenerator<A, void> {
  const next = (generator: AsyncGenerator<A, void>) => {
    const promise: Promise<QueuedGenerator<A>> = generator
      .next()
      .then(({ done, value }) => ({
        done,
        value,
        generator,
        promise,
      }))
    return promise
  }
  const waiting = [...generators]
  const promises = new Set<Promise<QueuedGenerator<A>>>()

  // Start initial batch up to concurrency cap
  while (promises.size < concurrencyCap && waiting.length > 0) {
    const gen = waiting.shift()!
    promises.add(next(gen))
  }

  while (promises.size > 0) {
    const { done, value, generator, promise } = await Promise.race(promises)
    promises.delete(promise)

    if (!done) {
      promises.add(next(generator))
      if (value !== undefined) {
        yield value
      }
    } else if (waiting.length > 0) {
      // Start a new generator when one finishes
      const nextGen = waiting.shift()!
      promises.add(next(nextGen))
    }
  }
}
```

Trace the trick in `next(generator)`:
- Each in-flight `.next()` promise resolves to an object **containing a reference to itself (`promise`)** plus its `generator`, `done`, and `value`.
- `Promise.race(promises)` wakes up as soon as *any* generator yields a single item or finishes.
- `promises.delete(promise)` removes the settled promise from the active `Set` in O(1).
- If `!done`, it immediately queues the next `.next()` call on the same generator and yields `value` to the caller.
- If `done`, a concurrency slot just opened up—so it shifts the next generator off `waiting` and starts it!

Alongside `all()`, `utils/generators.ts` provides four tiny companions used across the codebase: `lastX` (consume to final yielded value), `returnValue` (drain a generator and extract its `Return` value—crucial because `query()` returns `Terminal`), `toArray`, and `fromArray`.

---

## 6. Bounded File Awareness (`utils/fileStateCache.ts`)

### The Problem

Coding agents fail catastrophically when they edit files blindly:
- If the model tries to edit a file it never read, it hallucinates line contents.
- If the user (or a linter/formatter) modified a file on disk *after* the model read it, the model's patch silently overwrites the user's changes.
- Conversely, if the agent caches every file read forever in an unbounded `Map`, a long session inspecting large logs or generated bundles leaks hundreds of megabytes of RAM.

### The Implementation

`utils/fileStateCache.ts` wraps `LRUCache<string, FileState>` with three critical invariants:

```ts
// utils/fileStateCache.ts:4-23
export type FileState = {
  content: string
  timestamp: number
  offset: number | undefined
  limit: number | undefined
  // True when this entry was populated by auto-injection (e.g. CLAUDE.md) and
  // the injected content did not match disk (stripped HTML comments, stripped
  // frontmatter, truncated MEMORY.md). The model has only seen a partial view;
  // Edit/Write must require an explicit Read first. `content` here holds the
  // RAW disk bytes (for getChangedFiles diffing), not what the model saw.
  isPartialView?: boolean
}

export const READ_FILE_STATE_CACHE_SIZE = 100
const DEFAULT_MAX_CACHE_SIZE_BYTES = 25 * 1024 * 1024
```

Look closely at the three defenses baked right into `FileStateCache`:

1. **Path normalization on every key (`normalize(key)`):** Every `get`, `set`, `has`, and `delete` normalizes redundant segments (`/foo/../bar`) and Windows slash variants (`\` vs `/`) so the same file can never masquerade under two cache keys.
2. **Dual eviction caps (`max: 100` entries AND `maxSize: 25 MB`):** Size is calculated with `Math.max(1, Buffer.byteLength(value.content))` (the `Math.max(1, ...)` guards against empty 0-byte files violating `lru-cache`'s positive-size requirement!).
3. **`isPartialView` safety flag:** Auto-injected files like `CLAUDE.md` or truncated `MEMORY.md` store raw disk bytes for diff detection, while flagging `isPartialView: true` so `FileEditTool` / `FileWriteTool` refuse to edit until the model executes an explicit `Read`.
4. **Serialization (`dump()` / `load()` / `cloneFileStateCache()`):** Lets subagents clone the parent's read-file cache (`QueryEngine.ts:56`, `createSubagentContext`) and lets compaction serialize recently read files so the agent can re-attach them after summarizing history!

> [!NOTE]
> **Watch the downstream ripple of LRU eviction:** Because `readFileState` is an LRU cache that *can* evict entries in busy sessions, `ToolUseContext` (`Tool.ts:217-222`) maintains a *separate* `loadedNestedMemoryPaths: Set<string>` just for deduplicating `CLAUDE.md` injection—otherwise evicting a `CLAUDE.md` from `readFileState` would cause it to be re-injected dozens of times!

---

## Putting Layer 0 Together

```text
Layer 0 Atoms (Zero High-Level Dependencies)
├── types/ids.ts            --> SessionId, AgentId (compile-time identity)
├── state/store.ts          --> createStore() (34-line reactive store)
├── bootstrap/state.ts      --> process-wide leaf singleton & cycle breaker
├── utils/lazySchema.ts     --> 8-line deferred Zod initializer
├── utils/generators.ts     --> all() bounded async-generator multiplexer
└── utils/fileStateCache.ts --> path-normalized, 25 MB LRU file tracker
```

Notice what we **haven't** needed yet: no Anthropic SDK network calls, no prompts, no tools, no permissions, no React components, no terminal ANSI codes. Every single atom above is small enough to hold in your head in one sitting.

---

## How This Snaps Into the Next Layer

Every single Layer 0 atom plugs directly into **Layer 1 (`Tool.ts`, `tools.ts`)**:
- `ToolUseContext` carries `readFileState: FileStateCache`, `agentId?: AgentId`, and `getAppState` / `setAppState` wired directly to `createStore()`.
- Individual tools define their Zod schemas through `lazySchema()`.
- `services/tools/toolOrchestration.ts` passes concurrent tool generators straight into `all(..., getMaxToolUseConcurrency())`.

In [Chapter 2: The Tool Contract](ch002-the-tool-contract.md), we snap these atoms together to define what an agent can actually *do*.

---

## Reading Exercises & Code Checkpoints

1. **Check the `bootstrap-isolation` footprint:**
   Run `rg "bootstrap-isolation" bootstrap/` and inspect why `bootstrap/state.ts` has a special comment for `randomUUID`.
2. **Trace `Object.is` bailout:**
   Read `state/selectors.ts` and `state/onChangeAppState.ts` to see how selectors and state-change listeners hook into `createStore()`.
3. **Inspect `cloneFileStateCache`:**
   Read lines 120–142 of `utils/fileStateCache.ts` and trace how `QueryEngine.ts` and `AgentTool` isolate file caches across agents.

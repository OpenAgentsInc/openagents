# 2. The Tool Contract: Defining What an Agent Can Do

> **Status: written.** Reverse-engineered from the Claude Code 2.1.88 source snapshot (see the Prologue (`prologue-how-to-read-a-1900-file-agent.md`) for snapshot caveats).
>
> **Layer:** Layer 1 (Core Capability Contracts)
> **Prerequisites:** [Chapter 01](ch001-atoms-types-stores-and-primitives.md)
> **Primary Source Files:**
> - `Tool.ts` — `Tool`, `ToolDef`, `buildTool()`, `ToolUseContext`, `ToolPermissionContext`, `ToolResult` (792 lines)
> - `tools/GlobTool/GlobTool.ts` — clean, complete real-world tool implementation (198 lines)
> - `tools.ts` — tool catalog, DCE conditional loading, deny-rule pre-filtering, and prompt-cache-aware sorting (389 lines)

---

## Why This Layer Exists

An LLM by itself can only predict text. What turns a language model into a coding *agent* is a **tool contract**—a uniform interface that lets the runtime:

1. Advertise a tool's name, prompt instructions, and JSON Schema to the model.
2. Validate malformed model arguments *before* bothering the user with a permission prompt.
3. Decide whether two tool calls can safely run in parallel or must execute serially.
4. Enforce permissions, hooks, sandboxing, and security classifiers uniformly across built-in and MCP tools.
5. Stream progress and render rich terminal UI *without* polluting what gets sent back to the model.
6. Assemble and order the final tool list without busting the LLM prompt cache.

In Claude Code, every one of those responsibilities converges in a single contract: `Tool<Input, Output, P>` in `Tool.ts`.

---

## 1. The Four Faces of `Tool<Input, Output, P>` (`Tool.ts`)

If you read `Tool` (`Tool.ts:362-695`) from top to bottom, its ~35 properties and methods fall cleanly into **four orthogonal faces**:

```text
                       Tool<Input, Output, P>
    ┌──────────────────────┬───────────────────────┐
    │ 1. IDENTITY & SCHEMA │ 2. SAFETY & SCHEDULE  │
    │  name, aliases       │  isConcurrencySafe()  │
    │  inputSchema (Zod)   │  isReadOnly()         │
    │  inputJSONSchema     │  isDestructive()      │
    │  outputSchema        │  interruptBehavior()  │
    │  prompt(), searchHint│  isEnabled()          │
    │  shouldDefer         │  maxResultSizeChars   │
    ├──────────────────────┼───────────────────────┤
    │ 3. LIFECYCLE & EXEC  │ 4. UI & PROJECTION    │
    │  validateInput()     │  userFacingName()     │
    │  checkPermissions()  │  renderToolUseMessage │
    │  preparePermission-  │  renderToolResult...  │
    │    Matcher()         │  renderToolUseProgress│
    │  toAutoClassifier-   │  renderGroupedToolUse │
    │    Input()           │  extractSearchText()  │
    │  call() -> ToolResult│  mapToolResultToTool- │
    │                      │    ResultBlockParam() │
    └──────────────────────┴───────────────────────┘
```

Let's examine the subtle, high-leverage design choices hidden inside each face:

### Face 1: Identity, Schema, and Deferred Loading
- **`aliases?: string[]` & `findToolByName()` (`Tool.ts:348-360`):** Lets tools be renamed without breaking older transcripts or prompts.
- **`inputSchema` vs. `inputJSONSchema`:** Built-in tools define Zod v4 schemas (`inputSchema`); MCP tools pass raw JSON Schema directly via `inputJSONSchema` without round-tripping through Zod.
- **`shouldDefer` / `alwaysLoad` / `searchHint` (`Tool.ts:378, 442-449`):** When dozens of MCP servers are connected, dumping every tool schema into turn 1 wastes tens of thousands of tokens. Deferred tools are sent with `defer_loading: true` and discovered on demand by `ToolSearchTool` using `searchHint` (a 3–10 word keyword phrase). Tools marked `alwaysLoad: true` bypass deferral so the model always sees them on turn 1.
- **`maxResultSizeChars` (`Tool.ts:466`):** Controls when `utils/toolResultStorage.ts` spills a huge tool output to disk as `<persisted-output>`. Crucially, `FileReadTool` sets this to `Infinity`: persisting `Read`'s output to a file that the model must `Read` back would create an infinite Read→file→Read loop!

### Face 2: Safety & Concurrency Metadata
- **`isConcurrencySafe(input)` and `isReadOnly(input)` take `input`!**
  Why aren't these static booleans? Because `BashTool` depends on the command! `git status` or `ls -la` can be classified as read-only/concurrency-safe, whereas `rm -rf` or `npm install` is neither. Passing parsed `input` makes scheduling and permissions *input-sensitive*.
- **`interruptBehavior?(): 'cancel' | 'block'` (`Tool.ts:416`):** Declares what happens if the user submits a new message while this tool is mid-flight—either cancel and discard, or block the new message until the tool finishes (default: `'block'`).

### Face 3: The Execution & Validation Lifecycle
Notice the strict ordering documented right in `Tool.ts:483-503`:
1. **`backfillObservableInput?(input)`** — Runs on a *copy* of `tool_use` input before observers (SDK, hooks, transcripts) see it; the original API-bound input is never mutated so the prompt cache stays intact!
2. **`validateInput?(input, context)`** — Runs *before* permission prompts. Returns `{ result: true }` or `{ result: false, message, errorCode }` so invalid paths or bad arguments bounce straight back to the model without nagging the human.
3. **`checkPermissions(input, context)`** — Tool-specific permission logic (e.g., filesystem path rules for `GlobTool`, AST command rules for `BashTool`), layered with the general permission engine in `utils/permissions/permissions.ts`.
4. **`call(args, context, canUseTool, parentMessage, onProgress)`** — Executes the tool and returns `Promise<ToolResult<Output>>`.

Look at `ToolResult<T>` (`Tool.ts:321-336`):
```ts
export type ToolResult<T> = {
  data: T
  newMessages?: (UserMessage | AssistantMessage | AttachmentMessage | SystemMessage)[]
  // contextModifier is only honored for tools that aren't concurrency safe.
  contextModifier?: (context: ToolUseContext) => ToolUseContext
  mcpMeta?: {
    _meta?: Record<string, unknown>
    structuredContent?: Record<string, unknown>
  }
}
```
Notice `contextModifier`: a tool can return a pure function `(context) => nextContext` to update `ToolUseContext` for subsequent tools in the turn (for example, mutating session flags or caches), and the orchestrator controls *when* modifiers apply so concurrent tools never race on context mutation!

### Face 4: Separating Model Serialization from Human UI
A tool never returns a raw string directly to both the LLM and the screen. Instead, `call()` returns typed `data: Output`, and **three separate projections** read that `Output`:
1. **`mapToolResultToToolResultBlockParam(output, toolUseID)`** → Produces the Anthropic API `ToolResultBlockParam` the model sees.
2. **`renderToolResultMessage(output, progress, options)`** → Produces the React/Ink terminal UI the human sees (with `renderToolUseMessage` accepting `Partial<Input>` so the UI can render *while the JSON arguments are still streaming in*!).
3. **`extractSearchText?(output)`** → Flattens the *exact text visible in transcript mode* (excluding `<system-reminder>` wrappers and chrome like `"Found 3 files in 12ms"`) so in-terminal transcript search counts match highlighted screen cells 1-to-1 (`Tool.ts:582-599`).

---

## 2. Fail-Closed Defaults: `buildTool()` (`Tool.ts:703-792`)

### The Problem

With 60+ tools in `tools/`, making every tool implement boilerplate stubs like `isDestructive: () => false` or `isConcurrencySafe: () => false` causes two failures: repetitive noise, and worse, an engineer forgetting a safety check or callers littering the engine with `tool.isConcurrencySafe?.(input) ?? false`.

### The Implementation

Every tool is constructed through `buildTool(def)` (`Tool.ts:757-792`), which spreads a fail-closed `TOOL_DEFAULTS` object underneath the tool's definition:

```ts
// Tool.ts:757-792
const TOOL_DEFAULTS = {
  isEnabled: () => true,
  isConcurrencySafe: (_input?: unknown) => false,
  isReadOnly: (_input?: unknown) => false,
  isDestructive: (_input?: unknown) => false,
  checkPermissions: (
    input: { [key: string]: unknown },
    _ctx?: ToolUseContext,
  ): Promise<PermissionResult> =>
    Promise.resolve({ behavior: 'allow', updatedInput: input }),
  toAutoClassifierInput: (_input?: unknown) => '',
  userFacingName: (_input?: unknown) => '',
}

export function buildTool<D extends AnyToolDef>(def: D): BuiltTool<D> {
  return {
    ...TOOL_DEFAULTS,
    userFacingName: () => def.name,
    ...def,
  } as BuiltTool<D>
}
```

Look closely at the **fail-closed defaults**:
- `isConcurrencySafe` defaults to **`false`** — a new tool runs serially until explicitly audited and marked safe.
- `isReadOnly` defaults to **`false`** — assume a tool mutates state unless proven otherwise.
- `checkPermissions` returns `{ behavior: 'allow', updatedInput: input }`, which does *not* bypass security—it defers to the general permission engine (`permissions.ts`), where modes, deny rules, ask rules, and classifiers still apply.
- At the TypeScript level, `BuiltTool<D>` (`Tool.ts:735-741`) mirrors the runtime `{ ...TOOL_DEFAULTS, ...def }` spread so callers retain each tool's exact literal `name`, `Input`, and `Output` types with zero `?.()` optional checks needed at runtime.

---

## 3. The Capability Bag: `ToolUseContext` & `ToolPermissionContext`

Every tool's `call()` and `checkPermissions()` receives a `ToolUseContext` (`Tool.ts:158-300`). Reading its fields is like looking at an X-ray of the entire agent runtime—every Layer 0 atom we studied in Chapter 1 shows up here:

- **Static turn options (`options`):** `tools`, `commands`, `mainLoopModel`, `thinkingConfig`, `mcpClients`, `mcpResources`, `agentDefinitions`, `isNonInteractiveSession`, `refreshTools`.
- **Execution & cancellation:** `abortController`, `messages`, `queryTracking: { chainId, depth }`, `toolUseId`.
- **State bridges (from Chapter 1):**
  - `readFileState: FileStateCache`
  - `getAppState()` / `setAppState()`
  - **`setAppStateForTasks?` (`Tool.ts:184-192`)** — A classic production scar! For async background subagents, `setAppState` is replaced with a **no-op** so a subagent cannot clobber the main UI. Yet background subagents still need to register or clean up session-wide background tasks and hooks. `setAppStateForTasks` punches a narrow, intentional channel straight through to the root store.
  - **`localDenialTracking?` (`Tool.ts:279-283`)** — Same root cause! Because an async subagent's `setAppState` is a no-op, denial counts written via `setAppState` would never accumulate; `localDenialTracking` gives the subagent a mutable local counter so fallback-to-prompting still triggers.
- **Prompt-cache & budget state:**
  - `contentReplacementState?: ContentReplacementState` — tracks which tool outputs were spilled to disk so forked subagents make identical replacement decisions.
  - `renderedSystemPrompt?: SystemPrompt` — freezes the parent's rendered system prompt bytes at turn start so forked subagents share the parent's prompt cache instead of re-evaluating GrowthBook flags and busting it.
- **UI & notification hooks (optional—undefined in headless SDK mode!):**
  `setToolJSX`, `addNotification`, `appendSystemMessage`, `sendOSNotification`, `requestPrompt`, `setStreamMode`, `setHasInterruptibleToolInProgress`.

Alongside it sits `ToolPermissionContext` (`Tool.ts:123-138`), wrapped in `DeepImmutable<...>`:
- `mode: PermissionMode` (`default`, `plan`, `acceptEdits`, `bypassPermissions`, `dontAsk`, or internal `auto`/`bubble`)
- `additionalWorkingDirectories: Map<string, AdditionalWorkingDirectory>`
- `alwaysAllowRules`, `alwaysDenyRules`, `alwaysAskRules` (indexed by `PermissionRuleSource`)
- `shouldAvoidPermissionPrompts` (auto-deny prompts for headless background agents) and `prePlanMode` (remembers which mode to restore when exiting Plan Mode).

---

## 4. Concrete Walkthrough: `tools/GlobTool/GlobTool.ts`

Abstract interfaces come alive when you read a real 198-line tool end to end. `GlobTool` is one of the cleanest tools in the repo, and it exercises nearly every concept in this chapter:

1. **Lazy Zod schemas behind getters (`GlobTool.ts:26-75`):**
   Both `inputSchema` (`z.strictObject({ pattern, path? })`) and `outputSchema` (`{ durationMs, numFiles, filenames, truncated }`) are wrapped in `lazySchema(...)` and exposed via `get inputSchema()` / `get outputSchema()`.
2. **Explicit opt-in to concurrency & read-only (`GlobTool.ts:76-87`):**
   Overrides `buildTool`'s fail-closed defaults with `isConcurrencySafe() { return true }`, `isReadOnly() { return true }`, and `isSearchOrReadCommand() { return { isSearch: true, isRead: false } }` (allowing the UI to collapse consecutive search calls).
3. **Result size cap (`GlobTool.ts:60`):**
   `maxResultSizeChars: 100_000` bounds inline output before `toolResultStorage.ts` spills it to disk.
4. **Defensive pre-permission validation (`GlobTool.ts:94-134`):**
   Look at lines 100–103 inside `validateInput`:
   ```ts
   // SECURITY: Skip filesystem operations for UNC paths to prevent NTLM credential leaks.
   if (absolutePath.startsWith('\\\\') || absolutePath.startsWith('//')) {
     return { result: true }
   }
   ```
   On Windows, calling `fs.stat('//attacker/share')` triggers an outbound SMB handshake that leaks NTLM hashes *before the user has even been asked for permission*. Skipping `fs.stat` on UNC paths in `validateInput` pushes the decision safely into `checkPermissions`! For normal local paths, `validateInput` verifies the directory exists and even suggests a corrected path under `cwd` (`suggestPathUnderCwd`) when `ENOENT` occurs—giving the model actionable self-correction feedback without a permission prompt.
5. **Tool-specific permission check (`GlobTool.ts:135-142`):**
   Delegates to `checkReadPermissionForTool(GlobTool, input, appState.toolPermissionContext)`.
6. **Execution & token-saving relativization (`GlobTool.ts:154-176`):**
   Runs `glob(...)` with `abortController.signal` and `limit = globLimits?.maxResults ?? 100`, then maps results through `toRelativePath` specifically to save model tokens.
7. **Dual projection (`GlobTool.ts:151-153, 177-197`):**
   - `extractSearchText` returns `filenames.join('\n')` (matching the rendered list).
   - `mapToolResultToToolResultBlockParam` emits `'No files found'` when empty, or newline-joined paths plus a truncation hint for the LLM.

---

## 5. Assembling the Tool Catalog (`tools.ts`)

Having 60+ `Tool` objects on disk is only half the story. Before any turn begins, `tools.ts` decides *which* tools exist in this build, *which* are permitted right now, and *in what exact order* they are handed to the API.

### Step 1: Build-Time Filtering & Cycle Breaking (`tools.ts:16-156, 193-251`)
`getAllBaseTools()` builds the master array using three gating mechanisms:
- **Compile-time DCE:** `feature('PROACTIVE')`, `feature('AGENT_TRIGGERS')`, `feature('COORDINATOR_MODE')`, `process.env.USER_TYPE === 'ant'`.
- **Circular-dependency thunks:** `getSendMessageTool()`, `getTeamCreateTool()`, `getTeamDeleteTool()`, and `getPowerShellTool()` use lazy `require()` calls to break `tools.ts → Team*Tool → ... → tools.ts`.
- **Environment adaptation:** On internal native builds with embedded `bfs`/`ugrep` (`hasEmbeddedSearchTools()`), `GlobTool` and `GrepTool` are omitted because the shell's `find`/`grep` are already aliased to fast embedded binaries.

### Step 2: Mode & Blanket-Deny Filtering (`tools.ts:262-327`)
`getTools(permissionContext)` applies runtime filtering:
- **`CLAUDE_CODE_SIMPLE` mode:** Collapses the entire catalog down to just `[BashTool, FileReadTool, FileEditTool]` (plus coordinator tools if active).
- **Blanket deny filtering (`filterToolsByDenyRules`):** Removes any tool (or entire MCP server prefix like `mcp__server`) matched by a blanket deny rule *before the model ever sees the tool schema*—saving tokens and eliminating futile tool calls.
- **`isReplModeEnabled()`:** When the ant-only VM `REPLTool` is active, hides raw primitive tools (`REPL_ONLY_TOOLS`) so the model routes through the VM.
- **`tool.isEnabled()`:** Final runtime enablement filter.

### Step 3: Partitioned Sorting for Prompt-Cache Stability (`tools.ts:345-367`)
Finally, `assembleToolPool(permissionContext, mcpTools)` merges built-in tools and dynamic MCP tools. Read lines 354–366 of `tools.ts` carefully—this is one of the best systems-engineering gems in the entire repository:

```ts
// tools.ts:354-366
// Sort each partition for prompt-cache stability, keeping built-ins as a
// contiguous prefix. The server's claude_code_system_cache_policy places a
// global cache breakpoint after the last prefix-matched built-in tool; a flat
// sort would interleave MCP tools into built-ins and invalidate all downstream
// cache keys whenever an MCP tool sorts between existing built-ins. uniqBy
// preserves insertion order, so built-ins win on name conflict.
const byName = (a: Tool, b: Tool) => a.name.localeCompare(b.name)
return uniqBy(
  [...builtInTools].sort(byName).concat(allowedMcpTools.sort(byName)),
  'name',
)
```

Think about what would happen if you naively ran `[...builtInTools, ...allowedMcpTools].sort(byName)`:
- Connecting a single MCP server whose tool is named `mcp__alpha__query` would insert it *between* `Edit` and `Read`—invalidating the global prompt cache prefix for **every** built-in tool after it and the entire system prompt below it!
- By sorting `[builtInTools]` and `[allowedMcpTools]` as **two separate contiguous partitions**, built-in tools stay byte-identical across users and MCP configurations, AND `uniqBy(..., 'name')` guarantees a malicious or colliding MCP tool can never shadow a built-in tool.

---

## Putting Layer 1 Together

```text
Individual Tool Modules (GlobTool, FileReadTool, BashTool, ...)
   │  wrapped by buildTool() (fail-closed defaults + BuiltTool<D> types)
   ▼
getAllBaseTools()              ── compile-time feature() DCE + lazy cycle requires
   │
   ▼
getTools(permissionContext)    ── CLAUDE_CODE_SIMPLE + filterToolsByDenyRules + isEnabled()
   │
   ▼
assembleToolPool(ctx, mcpTools)── [sorted built-ins] ++ [sorted allowed MCPs] + uniqBy
   │
   ▼
Ready for API serialization (Ch 4-5) & Tool Execution / StreamingToolExecutor (Ch 6)
```

---

## How This Snaps Into the Next Layer

We now have the complete contract for **what the model can invoke** (`Tool` + `assembleToolPool`). Next, we need the other half of Layer 1: **what the user can invoke and how conversation messages flow**—the three-variant `Command` contract (`types/command.ts`), `utils/messages.ts`, and `utils/processUserInput/processUserInput.ts` (Chapter 3).

Then, in Part II, we will feed these tools and messages into `context.ts`, `services/api/claude.ts`, `services/tools/toolOrchestration.ts`, `StreamingToolExecutor.ts`, and `query.ts` to bring the entire agent loop to life.

---

## Reading Exercises & Code Checkpoints

1. **Compare a read-only tool with a mutating tool:**
   Read `tools/FileEditTool/FileEditTool.ts` alongside `tools/GlobTool/GlobTool.ts`. How does `FileEditTool` use `ToolUseContext.readFileState` (from Chapter 1) inside `validateInput`?
2. **Find every `maxResultSizeChars: Infinity` opt-out:**
   Run `rg "maxResultSizeChars" tools/` and observe *which* tools opt out of disk persistence and why.
3. **Trace `assembleToolPool` callers:**
   Run `rg "assembleToolPool"` to see how both interactive REPL (`useMergedTools`) and subagents (`runAgent.ts`) share the exact same partitioned-sort pipeline.

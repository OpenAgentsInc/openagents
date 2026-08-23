# cc tool rendering, delegation, and agent-fleet TUI reproduction spec — 2026-08-23

Subject: the commit-pinned `cc` source import (`AtlantisPleb/cc`, commit
`813c06acfa2d705076df6193b405c81eb11a18d1`, "Import cc sources") read as an
implementation reference for one question: how does a terminal agent host
define tools, run many child coding agents at once, and draw all of that on a
single screen without the transcript turning into noise?

Provenance and boundary:

- Everything in sections 1-8 is **observed source**, with file paths and
  verbatim shapes. The import is post-React-Compiler output, so component
  bodies contain `_c(n)` memo slots; the shapes and control flow are intact and
  every quoted type or string below is copied from the file named above it.
- Section 9 is **inferred mapping** to OpenAgents. It is a proposal, not a
  requirement, and it does not override `AGENTS.md`, `INVARIANTS.md`, or the
  Sol roadmap.
- Prior teardowns already cover the surrounding system:
  [Claude Code architecture](./2026-07-10-claude-code-teardown.md) (query loop,
  authority, persistence) and
  [Claude subagent histories](./2026-07-10-claude-subagents-rendering-analysis.md)
  (sidechain JSONL, topology reconstruction across versions). This document does
  not repeat them. It adds the layer neither covered at reproduction depth: the
  per-tool render contract, the task/progress state machine, and the compact
  multi-agent surfaces.
- The port plan that consumes this document is
  [OpenAgents Coder agent-fleet port plan](./2026-08-23-openagents-coder-tui-agent-fleet-port-plan.md).

## TL.DR

1. **A tool owns its own rendering.** `Tool` is a single object that carries
   the input schema, the permission check, execution, result serialization, and
   *eleven* render methods — use, progress, result, queued, rejected, error,
   grouped, tag, plus three text/summary accessors. There is no `switch` on tool
   name anywhere in the message renderer.
2. **Rendering is defined over partial input.** Every render method takes
   `Partial<z.infer<Input>>`, because the model streams arguments and the row
   must draw before the arguments are complete.
3. **Compact and full are separate outputs of the same tool**, selected by
   `verbose`, `style: 'condensed'`, `isTranscriptMode`, and
   `terminalSize`. Nothing re-derives a summary from the full rendering.
4. **The tool pool is assembled once, deterministically, and sorted.**
   Deny rules filter before the model ever sees a tool; built-ins beat MCP tools
   on name collision; partitions are sorted so the prompt prefix stays cacheable.
5. **Delegation is a tool, but its state lives in a task registry.** `AgentTool`
   spawns a child and immediately registers a `local_agent` task; the fleet
   surfaces (footer pill, dialog, per-agent line) read the registry, not the
   transcript.
6. **Progress is aggregated, not replayed.** A child streams normalized
   messages up as `agent_progress`; the parent keeps `toolUseCount`,
   `latestInputTokens + cumulativeOutputTokens`, and a `recentActivities` ring
   bounded at `MAX_RECENT_ACTIVITIES = 5`. The compact row shows the last
   activity; the transcript shows all of it.
7. **Every list surface degrades to one line.** N agents collapse to
   `3 local agents`; a too-small terminal collapses in-progress detail to
   `In progress… · 12 tool uses · 4.1k tokens · ctrl+o to expand`.
8. **Memoization is the load-bearing performance trick.** A resolved,
   non-streaming message row is memoized against width, screen, verbosity, and
   resolution; only unresolved or streaming rows re-render.

## 1. The tool contract (`Tool.ts`)

One interface couples execution, authority, and presentation. This coupling is
deliberate and is the single most portable idea in the file.

### 1.1 Execution and metadata

```ts
name: string
inputSchema: Input                 // zod, built lazily
prompt(...): Promise<string>       // model-facing description
description(...): Promise<string>
isEnabled(): boolean               // runtime availability
isReadOnly(input?): boolean
isDestructive(input?): boolean
isConcurrencySafe(input?): boolean // may run in parallel with siblings
checkPermissions(input, ctx?): Promise<PermissionResult>
toAutoClassifierInput(input?): string
validateInput?(input, ctx): Promise<{ result: boolean; message?: string; errorCode?: number }>
call(input, toolUseContext, canUseTool, parentMessage, onProgress): ...
mapToolResultToToolResultBlockParam(data, toolUseID): ToolResultBlockParam
maxResultSizeChars?: number
shouldDefer?: boolean
aliases?: string[]
searchHint?: string
```

`ToolUseContext` is what a tool is allowed to see. It carries the full option
set plus the mutable session handles:

```ts
export type ToolUseContext = {
  options: {
    commands: Command[]
    debug: boolean
    mainLoopModel: string
    tools: Tools
    verbose: boolean
    thinkingConfig: ThinkingConfig
    mcpClients: MCPServerConnection[]
    mcpResources: Record<string, ServerResource[]>
    isNonInteractiveSession: boolean
    agentDefinitions: AgentDefinitionsResult
    maxBudgetUsd?: number
    customSystemPrompt?: string
    appendSystemPrompt?: string
    querySource?: QuerySource
    refreshTools?: () => Tools
  }
  abortController: AbortController
  readFileState: FileStateCache
  getAppState: () => AppState
  setAppState: SetAppState
  messages: Message[]
  ...
}
```

Two fields matter for delegation: `querySource` (which is how a forked child
recognizes itself and refuses to fork again) and `messages` (which is how a
child inherits the parent's exact prefix).

Permission state is a separate immutable value, threaded rather than ambient:

```ts
export type ToolPermissionContext = DeepImmutable<{
  mode: PermissionMode
  additionalWorkingDirectories: Map<string, AdditionalWorkingDirectory>
  alwaysAllowRules: ToolPermissionRulesBySource
  alwaysDenyRules: ToolPermissionRulesBySource
  alwaysAskRules: ToolPermissionRulesBySource
  isBypassPermissionsModeAvailable: boolean
  isAutoModeAvailable?: boolean
  strippedDangerousRules?: ToolPermissionRulesBySource
  shouldAvoidPermissionPrompts?: boolean
  awaitAutomatedChecksBeforeDialog?: boolean
  prePlanMode?: PermissionMode
}>
```

### 1.2 The render contract

Three text accessors and eight node renderers. Every input parameter is
`Partial`, and every renderer is optional except `renderToolUseMessage`.

```ts
userFacingName(input: Partial<z.infer<Input>> | undefined): string
getToolUseSummary?(input: Partial<z.infer<Input>> | undefined): string | null
getActivityDescription?(input: Partial<z.infer<Input>> | undefined): string | null

renderToolUseMessage(
  input: Partial<z.infer<Input>>,
  options: { theme: ThemeName; verbose: boolean; commands?: Command[] },
): React.ReactNode

renderToolUseProgressMessage?(
  progressMessagesForMessage: ProgressMessage<P>[],
  options: {
    tools: Tools
    verbose: boolean
    terminalSize?: { columns: number; rows: number }
    inProgressToolCallCount?: number
    isTranscriptMode?: boolean
  },
): React.ReactNode

renderToolResultMessage?(
  content: Output,
  progressMessagesForMessage: ProgressMessage<P>[],
  options: {
    style?: 'condensed'
    theme: ThemeName
    tools: Tools
    verbose: boolean
    isTranscriptMode?: boolean
    isBriefOnly?: boolean
    input?: unknown
  },
): React.ReactNode

renderToolUseQueuedMessage?(): React.ReactNode
renderToolUseRejectedMessage?(input, options): React.ReactNode
renderToolUseErrorMessage?(result, options): React.ReactNode
renderToolUseTag?(input: Partial<z.infer<Input>>): React.ReactNode

renderGroupedToolUse?(
  toolUses: Array<{
    param: ToolUseBlockParam
    isResolved: boolean
    isError: boolean
    isInProgress: boolean
    progressMessages: ProgressMessage<P>[]
    result?: { param: ToolResultBlockParam; output: unknown }
  }>,
  options: { shouldAnimate: boolean; tools: Tools },
): React.ReactNode | null

extractSearchText?(out: Output): string
isResultTruncated?(output: Output): boolean
```

Reproduction notes:

- `renderToolUseProgressMessage` receives **all** progress messages for that
  tool use plus the terminal size and the count of concurrently in-flight tool
  calls. That is what allows a tool to shrink itself when the screen is busy,
  instead of the frame owner truncating it from outside.
- `renderToolResultMessage` receives the progress history *as well as* the
  result, so a completed delegation can still render its nested transcript.
- `renderGroupedToolUse` is the multi-call surface: the presence of this method
  is what makes a tool eligible for grouping at all (section 4).
- Failure has three distinct renderings: rejected (user denied), error (tool
  threw), queued (not started). None of these is a special case of the result.

### 1.3 Conservative defaults

```ts
const TOOL_DEFAULTS = {
  isEnabled: () => true,
  isConcurrencySafe: (_input?: unknown) => false,
  isReadOnly: (_input?: unknown) => false,
  isDestructive: (_input?: unknown) => false,
  checkPermissions: (input, _ctx?) =>
    Promise.resolve({ behavior: 'allow', updatedInput: input }),
  toAutoClassifierInput: (_input?: unknown) => '',
  userFacingName: (_input?: unknown) => '',
}

export function buildTool<D extends AnyToolDef>(def: D): BuiltTool<D> {
  return { ...TOOL_DEFAULTS, userFacingName: () => def.name, ...def } as BuiltTool<D>
}
```

A tool that says nothing is serial, mutating, and named after itself. The
scheduler can only parallelize what opted in.

## 2. Tool assembly (`tools.ts`)

One catalog function, feature-gated at the array level:

```ts
export function getAllBaseTools(): Tools {
  return [
    AgentTool,
    TaskOutputTool,
    BashTool,
    ...(hasEmbeddedSearchTools() ? [] : [GlobTool, GrepTool]),
    ExitPlanModeV2Tool,
    FileReadTool, FileEditTool, FileWriteTool, NotebookEditTool,
    WebFetchTool, TodoWriteTool, WebSearchTool,
    TaskStopTool, AskUserQuestionTool, SkillTool, EnterPlanModeTool,
    ListMcpResourcesTool, ReadMcpResourceTool,
    ...(isToolSearchEnabledOptimistic() ? [ToolSearchTool] : []),
  ]
}
```

Deny filtering happens before the model sees anything:

```ts
export function filterToolsByDenyRules<
  T extends { name: string; mcpInfo?: { serverName: string; toolName: string } },
>(tools: readonly T[], permissionContext: ToolPermissionContext): T[] {
  return tools.filter(tool => !getDenyRuleForTool(permissionContext, tool))
}
```

And the pool is a deterministic merge:

```ts
export function assembleToolPool(
  permissionContext: ToolPermissionContext,
  mcpTools: Tools,
): Tools {
  const builtInTools = getTools(permissionContext)
  const allowedMcpTools = filterToolsByDenyRules(mcpTools, permissionContext)
  const byName = (a: Tool, b: Tool) => a.name.localeCompare(b.name)
  return uniqBy(
    [...builtInTools].sort(byName).concat(allowedMcpTools.sort(byName)),
    'name',
  )
}
```

Four properties to reproduce: (a) enablement (`isEnabled`) is *not* the same
axis as denial (permission rules); (b) built-ins win a name collision because
they are concatenated first and `uniqBy` keeps the first; (c) each partition is
sorted independently so the built-in block is byte-stable across sessions; (d)
`assembleToolPool` is called per *worker*, not once per process — a child agent
gets its own pool from its own permission context (section 5.3).

## 3. Message-row rendering (`components/MessageRow.tsx`)

The row coordinator decides which of the render paths above applies, and — more
importantly — decides whether to re-render at all.

```ts
export type Props = {
  message: RenderableMessage
  isUserContinuation: boolean
  hasContentAfter: boolean
  tools: Tools
  commands: Command[]
  verbose: boolean
  inProgressToolUseIDs: Set<string>
  streamingToolUseIDs: Set<string>
  screen: Screen
  canAnimate: boolean
  lastThinkingBlockId: string | null
  latestBashOutputUUID: string | null
  columns: number
  isLoading: boolean
  lookups: ReturnType<typeof buildMessageLookups>
}
```

Message kinds it dispatches on: ordinary tool use, `grouped_tool_use`,
`collapsed_read_search`, thinking, plain assistant/user text. Transcript mode is
`screen === 'transcript'`, and it is passed down as `isTranscriptMode` to the
tool's own renderers rather than changing which renderer runs.

Two predicates define liveness:

```ts
export function isMessageStreaming(
  msg: RenderableMessage,
  streamingToolUseIDs: Set<string>,
): boolean            // any member tool id for grouped/collapsed, else the one id

export function allToolsResolved(
  msg: RenderableMessage,
  resolvedToolUseIDs: Set<string>,
): boolean            // grouped rows resolve only when every member resolves
```

And the memo comparator is the frame budget:

```ts
export function areMessageRowPropsEqual(prev: Props, next: Props): boolean {
  if (prev.message !== next.message) return false
  if (prev.screen !== next.screen) return false
  if (prev.verbose !== next.verbose) return false
  if (prev.message.type === 'collapsed_read_search' && next.screen !== 'transcript') return false
  if (prev.columns !== next.columns) return false
  const prevIsLatestBash = prev.latestBashOutputUUID === prev.message.uuid
  const nextIsLatestBash = next.latestBashOutputUUID === next.message.uuid
  if (prevIsLatestBash !== nextIsLatestBash) return false
  if (prev.lastThinkingBlockId !== next.lastThinkingBlockId && hasThinkingContent(next.message)) return false
  const isStreaming = isMessageStreaming(prev.message, prev.streamingToolUseIDs)
  const isResolved = allToolsResolved(prev.message, prev.lookups.resolvedToolUseIDs)
  if (isStreaming || !isResolved) return false
  return true
}
```

The invalidation set is exactly: identity, screen mode, verbosity, width,
"is the newest shell output", "is the newest thinking block", streaming, and
resolution. Nothing else. Animation is likewise gated on unresolved content, so
a settled transcript costs no spinner ticks.

## 4. Grouping many calls into one row (`utils/groupToolUses.ts`)

```ts
const GROUPING_CACHE = new WeakMap<Tools, Set<string>>()

function getToolsWithGrouping(tools: Tools): Set<string> {
  let cached = GROUPING_CACHE.get(tools)
  if (!cached) {
    cached = new Set(tools.filter(t => t.renderGroupedToolUse).map(t => t.name))
    GROUPING_CACHE.set(tools, cached)
  }
  return cached
}
```

Rules, in order:

1. Grouping is disabled entirely in verbose mode.
2. Only tools implementing `renderGroupedToolUse` participate.
3. A group needs **two or more calls of the same tool from the same API
   message** — i.e. one assistant turn that fanned out.
4. Results are collected by `tool_use_id`.
5. The grouped row replaces the individual rows at the position of the first
   member; user messages that contain only grouped results are dropped from the
   stream.
6. The group retains its children for detail rendering:

```ts
const groupedMessage: GroupedToolUseMessage = {
  type: 'grouped_tool_use',
  toolName: info.toolName,
  messages: group,
  results,
  displayMessage: firstMsg,
  uuid: `grouped-${firstMsg.uuid}`,
  timestamp: firstMsg.timestamp,
  messageId: info.messageId,
}
```

The `WeakMap` keyed on the `Tools` array is why point 4 of section 2 matters: a
stable tool array is also a stable grouping cache.

## 5. Delegation (`tools/AgentTool/AgentTool.tsx`)

### 5.1 Input contract

```ts
const baseInputSchema = lazySchema(() => z.object({
  description: z.string().describe('A short (3-5 word) description of the task'),
  prompt: z.string().describe('The task for the agent to perform'),
  subagent_type: z.string().optional(),
  model: z.enum(['sonnet', 'opus', 'haiku']).optional(),
  run_in_background: z.boolean().optional(),
}))
```

plus, when multi-agent is on:

```ts
name: z.string().optional()        // makes the child addressable: SendMessage({to: name})
team_name: z.string().optional()
mode: permissionModeSchema().optional()
```

plus isolation:

```ts
isolation: z.enum(['worktree']).optional()
cwd: z.string().optional()
```

`description` exists **only** for display. It is the string every compact
surface shows, which is why the schema demands 3-5 words.

### 5.2 Output contract

```ts
const syncOutputSchema = agentToolResultSchema().extend({
  status: z.literal('completed'),
  prompt: z.string(),
})

const asyncOutputSchema = z.object({
  status: z.literal('async_launched'),
  agentId: z.string(),
  description: z.string(),
  prompt: z.string(),
  outputFile: z.string(),          // where the parent can watch progress
  canReadOutputFile: z.boolean().optional(),
})
```

Background launch returns immediately:

```ts
return {
  data: {
    isAsync: true as const,
    status: 'async_launched' as const,
    agentId: agentBackgroundTask.agentId,
    description,
    prompt,
    outputFile: getTaskOutputPath(agentBackgroundTask.agentId),
    canReadOutputFile,
  },
}
```

`canReadOutputFile` is computed from whether the *calling* agent has Read/Bash
in its own pool — the result text changes depending on what the caller can do
with it. A teammate spawn is a third, private shape:

```ts
type TeammateSpawnedOutput = {
  status: 'teammate_spawned'
  prompt: string
  teammate_id: string
  agent_id: string
  agent_type?: string
  model?: string
  name: string
  color?: string
  tmux_session_name: string
  tmux_window_name: string
  tmux_pane_id: string
  team_name?: string
  is_splitpane?: boolean
  plan_mode_required?: boolean
}
```

### 5.3 Execution order

Reproducible sequence, in the order the file performs it:

1. Read the current permission context and app state.
2. Filter visible agent definitions by MCP requirements and agent permission
   rules; resolve `subagent_type` against that filtered set.
3. Mint a **stable agent id before anything runs**. The same id becomes the
   worktree slug, the task id, the progress `agentId`, the transcript path, and
   the notification target.
4. Build the worker's pool from the worker's own context:

```ts
const workerPermissionContext = {
  ...appState.toolPermissionContext,
  mode: selectedAgent.permissionMode ?? 'acceptEdits',
}
const workerTools = assembleToolPool(workerPermissionContext, appState.mcp.tools)
```

5. Enforce the recursion guards:

```ts
if (toolUseContext.options.querySource === `agent:builtin:${FORK_AGENT.agentType}`
    || isInForkChild(toolUseContext.messages)) {
  throw new Error('Fork is not available inside a forked worker. Complete your task directly using your tools.')
}
if (isTeammate() && teamName && name) {
  throw new Error('Teammates cannot spawn other teammates — the team roster is flat. To spawn a subagent instead, omit the `name` parameter.')
}
if (isInProcessTeammate() && teamName && run_in_background === true) {
  throw new Error('In-process teammates cannot spawn background agents. Use run_in_background=false for synchronous subagents.')
}
```

6. Register the task **before** execution starts (`registerAsyncAgent` for
   background, `registerAgentForeground` for synchronous). A foreground task is
   registered too, so it can be promoted to background later.
7. Run the child. A fork reuses the parent's system prompt, message list, and
   tool array verbatim (prompt-cache prefix); an ordinary child gets its own
   system prompt and a single user message.
8. Stream progress up (section 6).
9. Transition task state **before** slow cleanup — completion is reported, then
   worktrees and notifications are handled.
10. Worktrees are removed when clean and retained when they contain changes.
11. Emit the completion/failure/kill notification.

## 6. Progress aggregation (`tasks/LocalAgentTask/LocalAgentTask.tsx`)

### 6.1 Shapes

```ts
export type ToolActivity = {
  toolName: string
  input: Record<string, unknown>
  activityDescription?: string
  isSearch?: boolean
  isRead?: boolean
}

export type AgentProgress = {
  toolUseCount: number
  tokenCount: number
  lastActivity?: ToolActivity
  recentActivities?: ToolActivity[]
  summary?: string
}

export type ProgressTracker = {
  toolUseCount: number
  latestInputTokens: number
  cumulativeOutputTokens: number
  recentActivities: ToolActivity[]
}

export function getTokenCountFromTracker(tracker: ProgressTracker): number {
  return tracker.latestInputTokens + tracker.cumulativeOutputTokens
}
```

`recentActivities` is a ring bounded by `MAX_RECENT_ACTIVITIES = 5`: activities
are pushed on tool use, then `while (length > 5) shift()`, and
`lastActivity` is simply its final element. The token rule is the non-obvious
part: provider input-token usage is already
cumulative for the child's context, so the tracker keeps the **latest** input
count and **accumulates** output. Summing both would multiply-count the prompt.

### 6.2 Task state

```ts
export type LocalAgentTaskState = TaskStateBase & {
  type: 'local_agent'
  agentId: string
  prompt: string
  selectedAgent?: AgentDefinition
  agentType: string
  model?: string
  abortController?: AbortController
  unregisterCleanup?: () => void
  error?: string
  result?: AgentToolResult
  progress?: AgentProgress
  retrieved: boolean
  messages?: Message[]
  lastReportedToolCount: number
  lastReportedTokenCount: number
  isBackgrounded: boolean
  pendingMessages: string[]
  retain: boolean
  diskLoaded: boolean
  evictAfter?: number
}
```

with the shared base (`Task.ts`):

```ts
export type TaskStateBase = {
  id: string
  type: TaskType
  status: TaskStatus
  description: string
  toolUseId?: string
  startTime: number
  endTime?: number
  totalPausedMs?: number
  outputFile: string
  outputOffset: number
  notified: boolean
}

export type TaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'killed'
export function isTerminalTaskStatus(status: TaskStatus): boolean {
  return status === 'completed' || status === 'failed' || status === 'killed'
}
```

### 6.3 Transitions

| function | effect |
| --- | --- |
| `registerAsyncAgent` | create `status: 'running'`, init disk output, install cleanup, register |
| `registerAgentForeground` | same, `isBackgrounded: false` |
| `backgroundAgentTask` | flip to background, resolve the background signal so execution transfers |
| `unregisterAgentForeground` | drop a foreground task that finished without backgrounding |
| `updateAgentProgress` | write counters + activities, preserving any existing `summary` |
| `updateAgentSummary` | write the short background summary |
| `completeAgentTask` | `completed`, store result, schedule eviction |
| `failAgentTask` | `failed`, store error, schedule eviction |
| `killAsyncAgent` | abort, `killed`, evict task output |
| `enqueueAgentNotification` | build the XML `<task-notification>` for the parent |

The registry itself (`utils/task/framework.ts`) is deliberately boring and has
two properties worth copying:

```ts
const updated = updater(task)
if (updated === task) {
  // Updater returned the same reference (early-return no-op). Skip the
  // spread so s.tasks subscribers don't re-render on unchanged state.
  return prev
}
```

and re-registration (resume) carries forward UI-held state — `retain`,
`startTime`, `messages`, `diskLoaded`, `pendingMessages` — so a resumed agent
does not jump position in the panel or lose the transcript already on screen.
Timing constants: `POLL_INTERVAL_MS = 1000`, `STOPPED_DISPLAY_MS = 3_000` (a
killed task stays visible for three seconds), `PANEL_GRACE_MS = 30_000`.

### 6.4 Notification

```ts
const summary =
  status === 'completed'
    ? `Agent "${description}" completed`
    : status === 'failed'
      ? `Agent "${description}" failed: ${error || 'Unknown error'}`
      : `Agent "${description}" was stopped`
```

The enqueued notification carries: task id, parent tool-use id, output file
path, status, summary, final result, total tokens, tool uses, duration, and
worktree path/branch. A background agent's completion is therefore a *message
into the parent's queue*, not a UI event.

### 6.5 Progress wire format

Events are emitted through the tool's `onProgress` with a synthetic tool-use id
derived from the child's first assistant message:

```ts
onProgress({
  toolUseID: `agent_${assistantMessage.message.id}`,
  data: { message: normalizedFirstMessage, type: 'agent_progress', prompt, agentId: syncAgentId },
})
// subsequent
onProgress({
  toolUseID: `agent_${assistantMessage.message.id}`,
  data: { message: m, type: 'agent_progress', prompt: '', agentId: syncAgentId },
})
```

`Progress` for this tool is a union: `type Progress = AgentToolProgress | ShellProgress`
— a child's shell output can be forwarded to the parent's row directly.

## 7. Result retrieval (`tools/TaskOutputTool/TaskOutputTool.tsx`)

```ts
const inputSchema = lazySchema(() => z.strictObject({
  task_id: z.string(),
  block: semanticBoolean(z.boolean().default(true)),
  timeout: z.number().min(0).max(600000).default(30000),
}))

type TaskOutputToolOutput = {
  retrieval_status: 'success' | 'timeout' | 'not_ready'
  task: TaskOutput | null
}
```

Behavior worth reproducing:

- Blocking mode polls the registry every `100ms` until the status leaves
  `running`/`pending`, honoring the abort signal, then returns the task; a
  timeout returns `retrieval_status: 'timeout'` **with the current state**
  rather than an error.
- Waiting emits its own progress event: `{ type: 'waiting_for_task', taskDescription, taskType }`.
- Retrieval marks `notified: true`.
- For a `local_agent`, the in-memory result beats the on-disk transcript:

> Prefer the clean final answer from the in-memory result over the raw JSONL
> transcript on disk. The disk output is a symlink to the full session
> transcript (every message, tool use, etc.), not just the subagent's answer.

- The tool is now documented as deprecated in favor of `Read` on the returned
  `outputFile` path. That is the direction of travel: **the transcript path is
  the API**, and the ad-hoc retrieval tool is scaffolding.
- Result serialization is tagged text, not JSON: `<retrieval_status>`,
  `<task_id>`, `<task_type>`, `<status>`, `<exit_code>`, `<output>`, `<error>`.

## 8. The fleet surfaces

Five distinct densities exist for the same underlying registry. Reproducing the
*set* matters more than reproducing any one of them.

### 8.1 One line per child (`components/AgentProgressLine.tsx`)

```ts
type Props = {
  agentType: string
  description?: string
  name?: string
  descriptionColor?: keyof Theme
  taskDescription?: string
  toolUseCount: number
  tokens: number | null
  color?: keyof Theme
  isLast: boolean
  isResolved: boolean
  isError: boolean
  isAsync?: boolean
  shouldAnimate: boolean
  lastToolInfo?: string | null
  hideType?: boolean
}
```

Status text is three cases and nothing more:

```ts
const isBackgrounded = isAsync && isResolved
if (!isResolved) return lastToolInfo || "Initializing…"
if (isBackgrounded) return taskDescription ?? "Running in the background"
return "Done"
```

Layout: `├─` for members, `└─` for the last; agent type or `@name`; description;
tool-use and token counts **while running only** (a resolved background agent
hides them); last tool activity as the live line. `hideType` suppresses the
repeated type label when every child shares a type.

### 8.2 Last activity, rendered by the tool that produced it (`components/tasks/renderToolActivity.tsx`)

```ts
export function renderToolActivity(activity: ToolActivity, tools: Tools, theme: ThemeName) {
  const tool = findToolByName(tools, activity.toolName)
  if (!tool) return activity.toolName
  try {
    const parsed = tool.inputSchema.safeParse(activity.input)
    const parsedInput = parsed.success ? parsed.data : {}
    const userFacingName = tool.userFacingName(parsedInput)
    if (!userFacingName) return activity.toolName
    const toolArgs = tool.renderToolUseMessage(parsedInput, { theme, verbose: false })
    if (toolArgs) return <Text>{userFacingName}({toolArgs})</Text>
    return userFacingName
  } catch {
    return activity.toolName
  }
}
```

This is the whole point of section 1 in 25 lines: a *child agent's* activity
line inside a *parent's* progress panel is rendered by the child's tool's own
renderer, at `verbose: false`, with `safeParse` and a `try/catch` so a
malformed or unknown tool degrades to its raw name instead of breaking the
frame.

### 8.3 Compact in-progress panel (`tools/AgentTool/UI.tsx`)

Progress events are validated, then consecutive read/search/REPL events are
collapsed into a synthetic summary row:

```ts
type SummaryMessage = {
  type: 'summary'
  searchCount: number
  readCount: number
  replCount: number
  uuid: string
  isActive: boolean
}
```

Counts increment on tool *results* only, so a use/result pair is not counted
twice. Then:

- no progress yet → `Initializing…`
- terminal too small → one row:

```tsx
<Text dimColor>
  In progress… · <Text bold>{toolUseCount}</Text> tool {toolUseCount === 1 ? 'use' : 'uses'}
  {tokens && ` · ${formatNumber(tokens)} tokens`} ·{' '}
  <ConfigurableShortcutHint action="app:toggleTranscript" context="Global" fallback="ctrl+o" description="expand" parens />
</Text>
```

- normal → last few processed messages, plus `+N more tool uses`
- transcript mode → all processed messages, plus the child's prompt and final
  response, rendered with the same static nested message renderers as the main
  transcript.

Result states get distinct renderings: `remote_launched` (remote task id +
session URL), `async_launched` (backgrounded line; prompt in transcript mode),
`completed` (usage + duration + optional prompt/transcript/response), rejected,
and error — the last two rendering the progress they had before the fallback
message. Completion summary:

```ts
const result = [
  totalToolUseCount === 1 ? '1 tool use' : `${totalToolUseCount} tool uses`,
  formatNumber(totalTokens) + ' tokens',
  formatDuration(totalDurationMs),
]
const completionMessage = `Done (${result.join(' · ')})`
```

### 8.4 Footer pill (`tasks/pillLabel.ts`, `components/tasks/BackgroundTaskStatus.tsx`)

The always-visible one-liner. `getPillLabel` reduces N tasks to a phrase, and
the same function feeds the turn-duration transcript line so the two surfaces
cannot disagree:

```ts
case 'local_agent':
  return n === 1 ? '1 local agent' : `${n} local agents`
case 'in_process_teammate': { /* count distinct teamName */ return teamCount === 1 ? '1 team' : `${teamCount} teams` }
case 'local_bash': /* "2 shells, 1 monitor" */
case 'remote_agent': return n === 1 ? `${DIAMOND_OPEN} 1 cloud session` : `${DIAMOND_OPEN} ${n} cloud sessions`
...
// mixed types
return `${n} background ${n === 1 ? 'task' : 'tasks'}`
```

`pillNeedsCta` restricts the dimmed `· ↓ to view` call-to-action to genuine
attention states only, so a merely-running fleet does not nag.

Which tasks count is a single predicate (`tasks/types.ts`):

```ts
export type TaskState =
  | LocalShellTaskState | LocalAgentTaskState | RemoteAgentTaskState
  | InProcessTeammateTaskState | LocalWorkflowTaskState | MonitorMcpTaskState | DreamTaskState

export function isBackgroundTask(task: TaskState): task is BackgroundTaskState {
  if (task.status !== 'running' && task.status !== 'pending') return false
  if ('isBackgrounded' in task && task.isBackgrounded === false) return false
  return true
}
```

When every running task is an addressable teammate, the pill becomes a
horizontally scrolled row of `@name` pills instead — `main` first, then
teammates sorted by name (idle last when not selected), with
`calculateHorizontalScrollWindow(pillWidths, availableWidth, 2, selectedIdx)`
producing `startIndex/endIndex/showLeftArrow/showRightArrow` and `←`/`→`
markers. Selection, hover, "currently viewed" (bold), and idle (dim) are four
separate visual states of the same pill.

### 8.5 Per-task row and detail dialog

`components/tasks/BackgroundTask.tsx` renders one row per task type, each
truncated to `maxActivityWidth` (default 40) with
`truncate(text, activityLimit, true)`, then a status fragment:
`TaskStatusText status label="done" suffix=", unread"` where the `, unread`
suffix comes from `status === 'completed' && !task.notified`. A teammate row is
`@name: activity`; a shell row is `command <ShellProgress>`; a workflow row
shows `N agents` while running.

`components/tasks/AsyncAgentDetailDialog.tsx` is the expansion: title
`agentType › description`, subtitle `status icon + elapsed + tokens + tools`
(from `agent.result?.totalTokens ?? agent.progress?.tokenCount`, likewise tool
count), then a **Progress** block listing `recentActivities` through
`renderToolActivity` with the last one marked `›` and undimmed, then the prompt
(clipped to 300 chars, or the extracted `<plan>` block if present), then the
error. Keys: `←` back, `Esc/Enter/Space` close, `x` stop while running.

## 9. Inferred mapping to OpenAgents Coder (proposal)

This section is inference. It names what to adopt, what to reject, and what has
no counterpart yet. The implementation sequencing lives in the companion
[port plan](./2026-08-23-openagents-coder-tui-agent-fleet-port-plan.md).

Adopt:

- **Per-tool renderer registry** keyed by tool name, over partial arguments,
  with a `verbose`/`expanded`/`width` option bag — the `renderToolActivity`
  degradation path (`safeParse`, `try/catch`, fall back to the raw name) is the
  contract that makes an open tool set safe to render.
- **A task registry separate from the transcript.** Fleet state is not derivable
  from a message list, and `cc` proves the two surfaces need separate stores
  with one shared id.
- **Stable child id minted before launch**, reused as task id, transcript path,
  and progress key.
- **`description` as a display-only, 3-5-word field** on the delegation
  contract.
- **Aggregate-then-render progress**: counters plus a bounded recent-activity
  ring, with the latest-input/cumulative-output token rule.
- **One collapse per density**: N children → one phrase; one child → one line;
  expanded → nested transcript.
- **Terminal-size-aware self-collapse** inside the renderer, not truncation
  imposed from outside.
- **Explicit background/foreground promotion** and `, unread` until retrieved.
- **Recursion guards as errors returned to the model**, with remediation in the
  message text.

Reject or defer:

- React/Ink component composition. The Coder interface is an ANSI row painter
  by necessity (OpenTUI's FFI is Bun-only), so renderers must return
  `ReadonlyArray<string>` of styled rows, not nodes. This is a real constraint,
  not a preference, and it changes the render contract's return type
  everywhere.
- `tmux`-backed teammates and panes.
- The eleven-method tool interface in full. Five methods carry most of the
  value: activity label, use row, progress rows, result rows, grouped rows.
- Ambient app state (`getAppState`/`setAppState` inside tools). OpenAgents
  should thread an explicit store handle or Effect service instead.

No counterpart yet, and therefore new work:

- A durable per-child transcript path the parent can `Read`.
- A completion notification that enters the parent's message queue.
- Concurrency and quota accounting, which `cc` mostly does not do and which
  transcript 275 requires (per-chat active computers, budgeted fan-out).

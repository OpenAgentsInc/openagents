# 3. The Command & Message Contracts: User Input Meets the Model

> **Status: written.** Reverse-engineered from the Claude Code 2.1.88 source snapshot (see the Prologue (`prologue-how-to-read-a-1900-file-agent.md`) for snapshot caveats).
>
> **Layer:** Layer 1 (Core Capability & Input Contracts)
> **Prerequisites:** [Chapter 00](prologue-how-to-read-a-1900-file-agent.md), [Chapter 01](ch001-atoms-types-stores-and-primitives.md), [Chapter 02](ch002-the-tool-contract.md)
> **Primary Source Files:**
> - `types/command.ts` — three-variant `Command` union, `LocalJSXCommandContext`, `CommandAvailability`, and leaf helpers (216 lines)
> - `commands.ts` — `COMMANDS` thunk, memoized `loadAllCommands`, live `getCommands` filtering, skill projections, and remote/bridge allowlists (754 lines)
> - `utils/slashCommandParsing.ts` — zero-import slash parser with two-token `(MCP)` glue (60 lines)
> - `utils/processUserInput/processUserInput.ts` — multi-modal input router, image resizing, `bridgeOrigin` gate, `ULTRAPLAN` rewrite, attachments, and `UserPromptSubmit` hooks (605 lines)
> - `utils/processUserInput/processTextPrompt.ts` — plain-text prompt assembly, `promptId`, interaction spans, and OTel logging (100 lines)
> - `utils/messages.ts` — dual UI/API projections, Stage 1 `normalizeMessagesForAPI`, thinking-block passes, `stripSignatureBlocks`, and `ensureToolResultPairing` (5,512 lines)

---

## Why This Layer Exists

Chapter 02 built the model-facing half of Layer 1 (`Tool`, `ToolUseContext`, `assembleToolPool`). Before Layer 2 can build a system prompt and call the LLM, Claude Code needs the human-facing half:

1. **One contract for user-invocable commands:** `/compact`, `/model`, `/commit`, `/my-skill`, and `/server:prompt (MCP)` share one `/` prefix across host actions, Ink dialogs, inline prompt expansions, and forked subagents.
2. **One multi-modal input funnel:** Terminal strings, SDK/VS Code `ContentBlockParam[]` arrays, pasted images, `!` shell escapes, IDE selections, attachments, Remote Control bridge messages, and `UserPromptSubmit` hooks must collapse into one `shouldQuery` envelope.
3. **Two projections over one transcript:** Ink wants 1-block rows, spinners, retry rows, and compaction dividers; the Anthropic API requires strictly alternating `user`/`assistant` turns, valid thinking signatures, and paired `tool_use`/`tool_result` IDs.

---

## 1. The Three-Variant Command Contract & Registry

### The Problem

Separate registries for CLI commands, Ink pickers, disk skills, plugins, workflows, and MCP prompts would fracture autocomplete, `SkillTool`, and remote-safety checks. Eagerly importing ~100 command modules or reading auth state at module load would also hurt startup latency and break mid-session `/login` switches.

### The Real Implementation

`types/command.ts` unifies every command into `Command = CommandBase & (PromptCommand | LocalCommand | LocalJSXCommand)` (`types/command.ts:205-206`). Every import at `types/command.ts:1-14` is `import type`; only `getCommandName` and `isCommandEnabled` (`types/command.ts:209-216`) emit runtime code.

1. **`PromptCommand` (`type: 'prompt'`, `types/command.ts:25-57`)** unifies built-in prompt commands (`/init`, `/review`, `/insights`), bundled skills (`skills/bundledSkills.ts:75-95`), disk skills and `.claude/commands/` (`skills/loadSkillsDir.ts:467`, `skills/loadSkillsDir.ts:608`), plugins (`utils/plugins/loadPluginCommands.ts:300-325`), workflows (`feature('WORKFLOW_SCRIPTS')`), and MCP prompts (`services/mcp/client.ts:2056-2095`), carrying `hooks`, `skillRoot`, `allowedTools`, `model`, `effort`, `paths`, and inline/fork controls:

```ts
// types/command.ts:42-56
// Execution context: 'inline' (default) or 'fork' (run as sub-agent)
// 'inline' = skill content expands into the current conversation
// 'fork' = skill runs in a sub-agent with separate context and token budget
context?: 'inline' | 'fork'
// Agent type to use when forked (e.g., 'Bash', 'general-purpose')
// Only applicable when context is 'fork'
agent?: string
effort?: EffortValue
// Glob patterns for file paths this skill applies to
// When set, the skill is only visible after the model touches matching files
paths?: string[]
getPromptForCommand(
  args: string,
  context: ToolUseContext,
): Promise<ContentBlockParam[]>
```

2. **`LocalCommand` (`type: 'local'`, `types/command.ts:74-78`)** lazy-loads host code via `load: () => Promise<LocalCommandModule>` and returns `LocalCommandResult` (`types/command.ts:16-23`: `'text'`, `'compact'`, or `'skip'`).
3. **`LocalJSXCommand` (`type: 'local-jsx'`, `types/command.ts:144-152`)** lazy-loads an Ink dialog mounted via `setToolJSX`, completing through `LocalJSXCommandOnDone` (`types/command.ts:117-126`: `display?: 'skip' | 'system' | 'user'`, `shouldQuery?`, `metaMessages?`, `nextInput?`, `submitNextInput?`).

**Ch 02 → Ch 03 seam:** `Tool.ts:11` imports `type { Command }` into `ToolUseContext.options.commands` (`Tool.ts:158-160`), while `types/command.ts:6` extends `ToolUseContext` into `LocalJSXCommandContext` (`types/command.ts:80-98`, adding `setMessages`, `canUseTool`, `theme`, `onChangeAPIKey`, `resume`). Both `local` and `local-jsx` receive `LocalJSXCommandContext`; `import type` breaks the bidirectional cycle. On `CommandBase` (`types/command.ts:175-203`), static auth/provider `availability?: ('claude-ai' | 'console')[]` (`types/command.ts:169-173`) is kept strictly separate from dynamic `isEnabled?: () => boolean`.

`commands.ts` splits discovery across three volatility tiers:
- **Deferred built-ins (`COMMANDS`, `commands.ts:258-346`):** A private `lodash-es/memoize` thunk because `isUsing3PServices()` and `login()` read config and auth unavailable at module load. Heavy `/insights` (`commands/insights.ts`, ~113 KB) uses a lazy shim (`usageReport`, `commands.ts:190-202`), and `INTERNAL_ONLY_COMMANDS` (`commands.ts:225-254`) spreads only when `process.env.USER_TYPE === 'ant' && !process.env.IS_DEMO` (`commands.ts:343-345`).
- **Memoized `loadAllCommands(cwd)` vs. live `getCommands(cwd)` (`commands.ts:449-517`):**

```ts
// commands.ts:449-485
const loadAllCommands = memoize(async (cwd: string): Promise<Command[]> => {
  const [
    { skillDirCommands, pluginSkills, bundledSkills, builtinPluginSkills },
    pluginCommands,
    workflowCommands,
  ] = await Promise.all([
    getSkills(cwd),
    getPluginCommands(),
    getWorkflowCommands ? getWorkflowCommands(cwd) : Promise.resolve([]),
  ])

  return [
    ...bundledSkills,
    ...builtinPluginSkills,
    ...skillDirCommands,
    ...workflowCommands,
    ...pluginCommands,
    ...pluginSkills,
    ...COMMANDS(),
  ]
})
// ...
export async function getCommands(cwd: string): Promise<Command[]> {
  const allCommands = await loadAllCommands(cwd)

  // Get dynamic skills discovered during file operations
  const dynamicSkills = getDynamicSkills()

  // Build base commands without dynamic skills
  const baseCommands = allCommands.filter(
    _ => meetsAvailabilityRequirement(_) && isCommandEnabled(_),
  )
```

Running `meetsAvailabilityRequirement` (`commands.ts:417-443`; `'console'` requires `!isClaudeAISubscriber() && !isUsing3PServices() && isFirstPartyAnthropicBaseUrl()`, excluding Bedrock/Vertex/Foundry and gateways) and `isCommandEnabled` outside `loadAllCommands` lets `/login` take effect immediately. `getCommands` then deduplicates `getDynamicSkills()` and splices them right before built-in `COMMANDS()` (`commands.ts:505-516`), preserving `findCommand` order (`commands.ts:688-698`).

- **MCP & skill projections:** Reactive MCP commands live in `AppState.mcp.commands` (`services/mcp/client.ts:2054-2096`) and merge via `useMergedCommands` (`hooks/useMergedCommands.ts:5-15`); behind `feature('MCP_SKILLS')`, `getMcpSkillCommands` (`commands.ts:547-559`) filters `loadedFrom === 'mcp'` (its loader `skills/mcpSkills.js` is absent from this snapshot). Two memoized selectors project distinct skill slices: `getSkillToolCommands` (`commands.ts:563-581`) returns strictly model-invocable prompt commands (`!cmd.disableModelInvocation` at `commands.ts:569`), whereas `getSlashCommandToolSkills` (`commands.ts:586-608`) selects skill-shaped prompt commands and explicitly *includes* `cmd.disableModelInvocation` (`commands.ts:598`). Human-facing labels (`formatDescriptionWithSource`, `commands.ts:728-754`) stay strictly in the UI layer.
- **Remote & bridge allowlists:** `REMOTE_SAFE_COMMANDS` (`commands.ts:619-637`, 17 local-TUI built-ins) feeds `filterCommandsForRemoteMode` (`commands.ts:684-686`). For Remote Control mobile/web input, **`BRIDGE_SAFE_COMMANDS` (`commands.ts:651-660`) and `isBridgeSafeCommand` (`commands.ts:672-676`)** relax **PR #19134**'s blanket bridge block (originally added after iOS `/model` popped the local terminal Ink picker, `commands.ts:666-670`): `'local-jsx'` stays blocked (`false`), `'prompt'` is safe by construction (`true`), and `'local'` fails closed against six allowlisted commands (`compact`, `clear`, `cost`, `summary`, `releaseNotes`, `files`).

### Hidden Invariants & Failure Modes

- **Split cache invalidation (`commands.ts:523-539`):** `clearCommandMemoizationCaches()` clears `loadAllCommands`, `getSkillToolCommands`, `getSlashCommandToolSkills`, and—behind `feature('EXPERIMENTAL_SKILL_SEARCH')` (`commands.ts:96-100`)—`clearSkillIndexCache?.()` (outer memoization in absent `services/skillSearch/localSearch.js`) *without* calling `clearSkillCaches()`, because `utils/skills/skillChangeDetector.ts:92-97` invokes it right after discovering dynamic skills. Full `clearCommandsCache()` wipes everything on `/reload-plugins`. Neither clears `COMMANDS.cache`, and `commands.ts:1` disables import sorting to protect ANT-ONLY strip markers.

> [!NOTE]
> **Gates & Snapshot Absences in `commands.ts`:** Sixteen compile-time `feature('...')` flags (`commands.ts:63-122`, `commands.ts:401`) and `process.env.USER_TYPE === 'ant'` (`commands.ts:49`) gate conditional `require()` imports, while a seventeenth flag (`feature('MCP_SKILLS')`, `commands.ts:550`) gates `getMcpSkillCommands()` runtime filtering and `process.env.USER_TYPE === 'ant' && !process.env.IS_DEMO` (`commands.ts:343`) gates spreading `INTERNAL_ONLY_COMMANDS` into `COMMANDS()`. Thirteen conditional `require()` targets (`commands/agents-platform/index.js`, `commands/proactive.js`, `commands/assistant/index.js`, `commands/remoteControlServer/index.js`, `commands/force-snip.js`, `commands/workflows/index.js`, `services/skillSearch/localSearch.js`, `commands/subscribe-pr.js`, `commands/torch.js`, `commands/peers/index.js`, `commands/fork/index.js`, `commands/buddy/index.js`, `tools/WorkflowTool/createWorkflowCommand.js`) are **absent from this snapshot** (call-site evidence only); 5 `feature()`-gated commands (`commands/brief.ts`, `commands/bridge/index.ts`, `commands/voice/index.ts`, `commands/remote-setup/index.ts`, `commands/ultraplan.tsx`) are present; and 18 inert `.js` stub files back 19 of the 28 `INTERNAL_ONLY_COMMANDS` entries (17 one-line `{ isEnabled: () => false, isHidden: true, name: 'stub' }` default stubs plus 4-line `commands/reset-limits/index.js` exporting both `resetLimits` and `resetLimitsNonInteractive`).

---

## 2. Slash Parsing & Runtime Dispatch

### The Problem

A leading `/` could be a built-in command, an MCP prompt with a space before `(MCP)`, a mistyped skill with long arguments, or a POSIX path like `/etc/hosts`. Once resolved, each variant must protect UI focus, redact secrets, preserve compaction timestamps, and prevent local output from baiting the model into replying next turn.

### The Real Implementation

`parseSlashCommand` (`utils/slashCommandParsing.ts:25-60`) is a zero-import parser returning `ParsedSlashCommand | null` (`{ commandName, args, isMcp }`, `utils/slashCommandParsing.ts:5-9`). Non-`/` input or a bare `'/'` returns `null`; otherwise it splits strictly on `' '` and glues `words[1] === '(MCP)'` into `commandName + ' (MCP)'` to match `services/mcp/client.ts:2066-2070` (`"{client.name}:{prompt.name} (MCP)"`).

Because POSIX paths like `/usr/bin/env` also start with `/`, semantic disambiguation happens inside `utils/processUserInput/processSlashCommand.tsx` (922 lines: 921 pre-compiled `.tsx` lines + line 922 inline sourcemap, with build-time folded `"external" === 'ant'` checks at `utils/processUserInput/processSlashCommand.tsx:276`, `utils/processUserInput/processSlashCommand.tsx:430`, and `utils/processUserInput/processSlashCommand.tsx:498`). When `!hasCommand()` (`utils/processUserInput/processSlashCommand.tsx:333-381`), it probes `stat('/' + commandName)` and tests `looksLikeCommand(commandName)` (`!/[^a-zA-Z0-9:\-_]/.test(commandName)`, `utils/processUserInput/processSlashCommand.tsx:304-308`). Only `looksLikeCommand && !isFilePath` rejects with `Unknown skill: ...` (`shouldQuery: false`), emitting a UI-only warning `Args from unknown skill: ...` (**`gh-32591`**, `utils/processUserInput/processSlashCommand.tsx:355-357`) so typed arguments survive; real filesystem paths fall through as plain prompts (`shouldQuery: true`).

For known commands, `getMessagesForSlashCommand` (`utils/processUserInput/processSlashCommand.tsx:525-777`) blocks `userInvocable === false` and switches on `command.type`:
- **`local-jsx` (`utils/processUserInput/processSlashCommand.tsx:551-656`):** Settles asynchronously via `onDone`. On the `options?.display === 'system'` branch (`utils/processUserInput/processSlashCommand.tsx:591-593`), it skips `' dismissed'` scrollback entries when fullscreen mode is enabled (`isFullscreenEnvEnabled()`, gated by `CLAUDE_CODE_NO_FLICKER` / `process.env.USER_TYPE === 'ant'` outside `tmux -CC` at `utils/fullscreen.ts:112-129`). Two explicit deadlock guards protect the input queue (`utils/processUserInput/processSlashCommand.tsx:622-655`): `doneWasCalled` stops an early-exit `onDone()` plus returned JSX from re-arming `isLocalJSXCommand`, and `.catch()` clears JSX and resolves so thrown errors never wedge `queryGuard`.
- **`local` (`utils/processUserInput/processSlashCommand.tsx:657-722`):** Redacts arguments to `'***'` when `isSensitive` is set and wraps `'text'` results in `createCommandInputMessage` (`system/local_command`). For `'compact'` results, it bumps `displayText`'s timestamp by `Date.now() + 100` (`utils/processUserInput/processSlashCommand.tsx:689`) so `--resume` selects it as the latest leaf, then calls `resetMicrocompactState()` (`utils/processUserInput/processSlashCommand.tsx:699`).
- **`prompt` (`utils/processUserInput/processSlashCommand.tsx:723-921`):**
  - *Forked (`context === 'fork'`, `utils/processUserInput/processSlashCommand.tsx:62-295`):* The synchronous path streams `runAgent({ isAsync: false, querySource: 'agent:custom', ... })` and returns `shouldQuery: false`. Behind `feature('KAIROS') && (await context.getAppState()).kairosEnabled` (`utils/processUserInput/processSlashCommand.tsx:102-183`), it instead launches an async `runAgent` with a standalone `bgAbortController` and captured `spawnTimeWorkload = getWorkload()`. Before running, the background task polls every `200` ms up to `MCP_SETTLE_TIMEOUT_MS` (`10_000` ms, `utils/processUserInput/processSlashCommand.tsx:56-57`) until no MCP client is `'pending'`, calls `refreshTools?.()`, and re-enqueues `<scheduled-task-result>` (`isMeta: true, skipSlashCommands: true`).
  - *Inline (`getMessagesForPromptSlashCommand`, `utils/processUserInput/processSlashCommand.tsx:827-921`, shared by `SkillTool` via `processPromptSlashCommand` at `utils/processUserInput/processSlashCommand.tsx:817-826`):* Behind `feature('COORDINATOR_MODE') && isEnvTruthy(process.env.CLAUDE_CODE_COORDINATOR_MODE) && !context.agentId` (`utils/processUserInput/processSlashCommand.tsx:837-868`), main-thread coordinators receive a delegation stub while workers (`agentId` set) expand the prompt normally:

```tsx
// utils/processUserInput/processSlashCommand.tsx:874-912
const hooksAllowedForThisSkill = !isRestrictedToPluginOnly('hooks') || isSourceAdminTrusted(command.source);
if (command.hooks && hooksAllowedForThisSkill) {
  const sessionId = getSessionId();
  registerSkillHooks(context.setAppState, sessionId, command.hooks, command.name, command.type === 'prompt' ? command.skillRoot : undefined);
}
// ...
const skillPath = command.source ? `${command.source}:${command.name}` : command.name;
const skillContent = result.filter((b): b is TextBlockParam => b.type === 'text').map(b => b.text).join('\n\n');
addInvokedSkill(command.name, skillPath, skillContent, getAgentContext()?.agentId ?? null);
const metadata = formatCommandLoadingMetadata(command, args);
const additionalAllowedTools = parseToolListFromCLI(command.allowedTools ?? []);
// ...
const messages = [createUserMessage({
  content: metadata,
  uuid
}), createUserMessage({
  content: mainMessageContent,
  isMeta: true
}), ...attachmentMessages, createAttachmentMessage({
  type: 'command_permissions',
  allowedTools: additionalAllowedTools,
  model: command.model
})];
```

### Hidden Invariants & Failure Modes

- **Skill hooks & compaction isolation:** `hooksAllowedForThisSkill` enforces `strictPluginOnlyCustomization: ["hooks"]`, while `addInvokedSkill` keys `bootstrap/state.ts:1510-1524` by `"{agentId ?? ''}:{skillName}"` so invoked skills never leak across subagents during compaction.
- **Metadata & 110 KB latency guard:** Expanded-body attachment extraction passes `{ skipSkillDiscovery: true }` (`utils/processUserInput/processSlashCommand.tsx:897-901`) to avoid stalls on 110 KB `SKILL.md` files. Meanwhile, `formatCommandLoadingMetadata` (`utils/processUserInput/processSlashCommand.tsx:803-816`) emits `<command-name>` and `<command-message>` using qualified `command.name` (plus `<skill-format>true</skill-format>` for model-only skills).
- **Synthetic caveat shield (`utils/processUserInput/processSlashCommand.tsx:512-515`):** Unless `messageShouldQuery`, all messages are `system/local_command`, or the result is a compaction boundary, `processSlashCommand` prepends `createSyntheticUserCaveatMessage()` (`utils/messages.ts:566-571`, `isMeta: true` `<local-command-caveat>`) so the model ignores local output next turn.

---

## 3. Multi-Modal Input Routing & `UserPromptSubmit` Hooks

### The Problem

Input arrives from terminals (strings, pasted images, `!` shell escapes, IDE selections), SDK/VS Code callers (`ContentBlockParam[]` arrays), and Remote Control bridges—while `UserPromptSubmit` hooks can augment, stop, or erase model-bound prompts.

### The Real Implementation

`ProcessUserInputContext = ToolUseContext & LocalJSXCommandContext` and `ProcessUserInputBaseResult` (`utils/processUserInput/processUserInput.ts:62-83`) unify REPL (`utils/handlePromptSubmit.ts:476-496`) and SDK (`QueryEngine.ts:416-428`) callers behind `processUserInput()` (`utils/processUserInput/processUserInput.ts:85-270`). The outer shell echoes non-`isMeta` prompt strings (`setUserInputOnProcessing`), profiles with `queryCheckpoint()` (`CLAUDE_CODE_PROFILE_QUERY`, `utils/queryProfiler.ts:36`), and returns early when `!result.shouldQuery` so local turns bypass hooks.

Inside `processUserInputBase()` (`utils/processUserInput/processUserInput.ts:281-589`):
1. **String vs. `ContentBlockParam[]` (`utils/processUserInput/processUserInput.ts:300-349`):** Resizes inline `image` blocks into `normalizedInput` (fixing dropped resizes and iOS `mediaType` vs. `media_type` per `mobile-apps#5825`), collects `imageMetadataTexts`, splits a trailing `text` block into `inputString` + `precedingInputBlocks`, and throws if `inputString === null && mode !== 'prompt'`.
2. **Terminal `pastedContents` (`utils/processUserInput/processUserInput.ts:351-420`):** Filters pastes via `isValidImagePaste` (`types/textInputTypes.ts:367-369`, dropping 0-byte images that 400), persists via `storeImages()` (`utils/imageStore.ts:84-99`), resizes in parallel (`Promise.all`), and appends `imageMetadataTexts` as an `isMeta: true` `UserMessage` via `addImageMetadataMessage()` (`utils/processUserInput/processUserInput.ts:592-605`) on every exit except `bridgeOrigin` rejection.
3. **`bridgeOrigin` gate (`utils/processUserInput/processUserInput.ts:422-453`):** Mobile/web input arrives with `bridgeOrigin: true` and `skipSlashCommands: true`. For `/...`, `isBridgeSafeCommand` (`commands.ts:672-676`) clears `effectiveSkipSlash` for safe commands, short-circuits known unsafe commands (`shouldQuery: false`), and lets unknown `/shrug` fall through as plain text.
4. **Gated `ULTRAPLAN` rewrite (`utils/processUserInput/processUserInput.ts:467-493`):** Behind `feature('ULTRAPLAN')` plus 8 runtime guards (checking `hasUltraplanKeyword(preExpansionInput ?? inputString)` so pasted text cannot trigger), rewrites to `/ultraplan ${rewritten}` before attachment I/O so React batches UI transitions without flicker.
5. **Attachments & IDE selection (`utils/processUserInput/processUserInput.ts:495-514`):** Skipped for `/slash` commands in `prompt` mode; otherwise drains `getAttachmentMessages` (`utils/attachments.ts:2937-2970`), which wraps `getAttachments` (`utils/attachments.ts:743-1003`). Inside `getAttachments`, collection runs under a 1,000 ms timeout (`utils/attachments.ts:767`) and short-circuits to queued commands only when `CLAUDE_CODE_DISABLE_ATTACHMENTS` or `CLAUDE_CODE_SIMPLE` is set (`utils/attachments.ts:752-760`). On the main thread, `ideSelection` emits `selected_lines_in_ide` or `opened_file_in_ide` while honoring `isFileReadDenied()`.
6. **Three dispatch exits:**
   - **`mode === 'bash'` (`utils/processUserInput/processUserInput.ts:517-529`):** Dynamically imports `utils/processUserInput/processBashCommand.tsx` (140 lines: 139 code + line 140 sourcemap). It selects PowerShell only when `isPowerShellToolEnabled() && resolveDefaultShell() === 'powershell'` (`utils/processUserInput/processBashCommand.tsx:26`), lazy-requiring ~300 KB `PowerShellTool` behind a Windows-only gate (`CLAUDE_CODE_USE_POWERSHELL_TOOL` / `process.env.USER_TYPE === 'ant'`, `utils/shell/shellToolUtils.ts:17-22`). Execution runs with `dangerouslyDisableSandbox: true`, formats stdout via `processToolResultBlock(shellTool, { ...data, stderr: '' }, randomUUID())` (`utils/processUserInput/processBashCommand.tsx:98-106`), and returns `shouldQuery: false`.
   - **`/slash` (`utils/processUserInput/processUserInput.ts:533-551`):** Dynamically imports `utils/processUserInput/processSlashCommand.tsx`.
   - **Plain text (`utils/processUserInput/processUserInput.ts:553-588`):** Logs `tengu_subagent_at_mention` when an `@agent-` attachment is present and delegates assembly to `processTextPrompt()`.

**Plain-text assembly (`utils/processUserInput/processTextPrompt.ts:19-100`):** Every non-slash, non-bash prompt funnels through `processTextPrompt()`. It mints a fresh `promptId` via `setPromptId(promptId)` (`bootstrap/state.ts`) and opens the telemetry interaction span using the *first* text block (`.find(block => block.type === 'text')`).

By contrast, OTel `user_prompt` logging (redacted unless `OTEL_LOG_USER_PROMPTS` is set, `utils/telemetry/events.ts:14`) reads **`.findLast(block => block.type === 'text')`** (`utils/processUserInput/processTextPrompt.ts:47-57`)—a production scar from **`anthropics/claude-code#33301`**, because VS Code submits `ContentBlockParam[]` arrays with `<ide_selection>` context prepended ahead of the user's actual message. After logging `tengu_input_prompt` keyword analytics, `processTextPrompt()` drops whitespace-only text when pasted images are attached (`utils/processUserInput/processTextPrompt.ts:67-76`) so the model sees clean image blocks, then returns `[userMessage, ...attachmentMessages]` with `shouldQuery: true`.

When `result.shouldQuery` is `true`, `executeUserPromptSubmitHooks` (`utils/processUserInput/processUserInput.ts:182-263`) distinguishes blocking from stopping and truncates `additionalContexts` / non-empty `hook_success` strings at `MAX_HOOK_OUTPUT_LENGTH = 10000` (`applyTruncation`, `utils/processUserInput/processUserInput.ts:272-279`):

```ts
// utils/processUserInput/processUserInput.ts:193-224
// Return only a system-level error message, erasing the original user input
if (hookResult.blockingError) {
  const blockingMessage = getUserPromptSubmitHookBlockingMessage(
    hookResult.blockingError,
  )
  return {
    messages: [
      // TODO: Make this an attachment message
      createSystemMessage(
        `${blockingMessage}\n\nOriginal prompt: ${input}`,
        'warning',
      ),
    ],
    shouldQuery: false,
    allowedTools: result.allowedTools,
  }
}

// If preventContinuation is set, stop processing but keep the original
// prompt in context.
if (hookResult.preventContinuation) {
  const message = hookResult.stopReason
    ? `Operation stopped by hook: ${hookResult.stopReason}`
    : 'Operation stopped by hook'
  result.messages.push(
    createUserMessage({
      content: message,
    }),
  )
  result.shouldQuery = false
  return result
}
```

### Hidden Invariants & Failure Modes

> [!WARNING]
> **Three Traps in `utils/processUserInput/`:**
> 1. **`blockingError` erases vs. `preventContinuation` preserves:** `blockingError` wipes `result.messages` into a UI-only `warning` `SystemMessage` (note `${input}` at `utils/processUserInput/processUserInput.ts:202` stringifies arrays as `[object Object]`); `preventContinuation` retains the prompt and appends a stop message.
> 2. **Unescaped `<persisted-output>`:** `utils/processUserInput/processBashCommand.tsx:102-106` skips `escapeXml()` on `mapped.content` (escaping only `stderr` and raw fallback `data.stdout`) so disk-spilled `<persisted-output>` tags aren't broken into `&lt;persisted-output&gt;`.
> 3. **Snapshot absences:** Type-only imports `constants/querySource.js` (`utils/processUserInput/processUserInput.ts:8`), `types/message.js` (`utils/processUserInput/processUserInput.ts:20-27`), and `types/tools.js` (`utils/processUserInput/processBashCommand.tsx:8`, `ShellProgress` observed in `components/BashModeProgress.tsx:34`) are **absent from this checkout**.

---

## 4. Dual Message Views & The Two-Stage API Boundary

### The Problem

One `Message[]` array must serve Ink (1-block rows, spinners, retry rows, compaction dividers) and the Anthropic API (alternating `user`/`assistant` turns, zero UI chrome), while ~10 internal callers—compaction (`services/compact/compact.ts:1293`), VCR (`services/vcr.ts:96`), analytics, and `query.ts:855`—need normalized messages without model, beta, or cache context.

> [!NOTE]
> **Snapshot Absence of `types/message.ts` & `types/utils.ts`:** Both `types/message.ts` (`utils/messages.ts:41-73`, 31 types) and `types/utils.ts` (`utils/messages.ts:121`, `DeepImmutable`) are **absent from this snapshot**. Observed via `utils/messages.ts` factories: 5 core union variants (`user`, `assistant`, `attachment`, `progress`, and `system` with **14 constructed subtypes** at `utils/messages.ts:4335-4603`) plus 4 stream/SDK envelopes (`tombstone`, `tool_use_summary`, `stream_event`, `stream_request_start` at `utils/messages.ts:2951-2962`).

### The Real Implementation

`utils/messages.ts` projects the transcript in opposite directions:

| Dimension | UI View (`components/Messages.tsx`) | API View (`normalizeMessagesForAPI` → Stage 2) |
|---|---|---|
| **Granularity** | **Splits 1 → N** via `normalizeMessages` (`utils/messages.ts:741-823`) with deterministic `deriveUUID` (`utils/messages.ts:725-728`) so Ink never remounts rows. | **Merges N → 1** via `mergeUserMessages` (`utils/messages.ts:2411-2449`) and `mergeAssistantMessages` (`utils/messages.ts:2389-2400`). |
| **Non-chat records** | Keeps `attachment`, `progress`, `system`; `reorderMessagesInUI` (`utils/messages.ts:855-1026`) groups tools/hooks; `shouldShowUserMessage` (`utils/messages.ts:4658-4677`) hides `isMeta` except `origin.kind === 'channel'` (`feature('KAIROS') \|\| feature('KAIROS_CHANNELS')`). | Drops `isVirtual`, `progress`, `isSyntheticApiErrorMessage`, and 13 of 14 `system` subtypes; promotes `system/local_command` to `UserMessage`; bubbles and expands `attachment` into `isMeta` `UserMessage[]`. |
| **Scale / Compaction** | `buildMessageLookups` (`utils/messages.ts:1170-1340`) precomputes O(1) Maps; `components/Messages.tsx:496-498` passes `{ includeSnipped: true }` to `getMessagesAfterCompactBoundary`. | `query.ts:365` slices via `getMessagesAfterCompactBoundary` (`utils/messages.ts:4643-4656`, absent `services/compact/snipProjection.js` under `feature('HISTORY_SNIP')`). |

*(Notably, `filterUnresolvedToolUses` at `utils/messages.ts:2796-2800` skips `normalizeMessages` because derived UUIDs would defeat JSONL resume deduplication.)*

> [!IMPORTANT]
> **The Two-Stage Normalization Boundary:**
> **Stage 1** (`normalizeMessagesForAPI`, `utils/messages.ts:1989-2370`) is model-agnostic and returns **internal `(UserMessage | AssistantMessage)[]`** retaining `uuid`, `isMeta`, `timestamp`, and `message.id`—**not** wire `MessageParam[]`.
> **Stage 2** lives in `services/api/claude.ts` ([Chapter 05](ch005-talking-to-the-model.md)): right after calling `normalizeMessagesForAPI` (`services/api/claude.ts:1266-1315`), it runs model-aware `!useToolSearch` re-stripping, calls **`ensureToolResultPairing`** (`services/api/claude.ts:1301` — *outside* `normalizeMessagesForAPI`), strips advisor blocks without `ADVISOR_BETA_HEADER`, and caps media at `100` (`API_MAX_MEDIA_PER_REQUEST`). Finally, `addCacheBreakpoints` (`services/api/claude.ts:3063-3211`) emits wire `MessageParam[]` via `userMessageToMessageParam` and `assistantMessageToMessageParam` (`services/api/claude.ts:588-674`).

**Why UI-only `system` messages vanish here:** First, `reorderAttachmentsForAPI` (`utils/messages.ts:1481-1527`) bubbles attachments upward in O(N) until hitting an `assistant` or `tool_result` user message, and `isVirtual` records are dropped (`utils/messages.ts:1999-2001`). Second, `stripTargets` (`utils/messages.ts:2004-2054`) scans `isSyntheticApiErrorMessage` (`utils/messages.ts:321-329`, `model === '<synthetic>'`) for 5 PDF/image errors, tagging the preceding `isMeta` user message to strip poisoned blocks. Only *after* `compact_boundary` has been sliced (`query.ts:365`) and `stripTargets` has read synthetic errors—yet *before* role merging—does the filter run:

```ts
// utils/messages.ts:2056-2075
const result: (UserMessage | AssistantMessage)[] = []
reorderedMessages
  .filter(
    (
      _,
    ): _ is
      | UserMessage
      | AssistantMessage
      | AttachmentMessage
      | SystemLocalCommandMessage => {
      if (
        _.type === 'progress' ||
        (_.type === 'system' && !isSystemLocalCommandMessage(_)) ||
        isSyntheticApiErrorMessage(_)
      ) {
        return false
      }
      return true
    },
  )
```

Thirteen of fourteen `system` subtypes (`compact_boundary`, `microcompact_boundary`, `api_error`, `turn_duration`, `stop_hook_summary`, `api_metrics`, `informational`, etc.) are dropped; only `subtype === 'local_command'` (`utils/messages.ts:1529-1533`) promotes to a `UserMessage` (`utils/messages.ts:2078-2093`). Stripping earlier breaks compaction and error recovery; stripping later corrupts role merging.

Inside the main `switch` (`utils/messages.ts:2076-2293`) and post-passes (`utils/messages.ts:2301-2367`):
- **User merging (`mergeUserMessages`, `utils/messages.ts:2411-2449`):** Strips `tool_reference` blocks via `isToolSearchEnabledOptimistic()`, applies `stripTargets`, and merges consecutive user turns (required by Bedrock). It hoists `tool_result` blocks first (`hoistToolResults`, `utils/messages.ts:2470-2483`), appends `\n` onto **`a`'s tail** (`joinTextAtSeam`, `utils/messages.ts:2505-2515`) so `b`'s leading `<system-reminder>` survives, and preserves the non-meta `uuid` (`a.isMeta ? b.uuid : a.uuid`, `utils/messages.ts:2443`). Inside `if (feature('HISTORY_SNIP'))` when `isSnipRuntimeEnabled()` is active (`utils/messages.ts:2414-2438`), an earlier branch preserves the same non-meta `uuid` (`utils/messages.ts:2429`) while narrowing `isMeta` to `a.isMeta && b.isMeta ? true : undefined` so mixed real-user turns still receive `[id:]` tags.
- **Assistant reassembly:** Canonicalizes tool names, calls `normalizeToolInputForAPI` (`utils/api.ts:685-718`, stripping injected `ExitPlanModeV2` and legacy `FileEditTool` fields), drops `caller` when tool search is off, and walks *backward* across interleaved `tool_result`s and different-`id` teammate assistants (`utils/messages.ts:2250-2264`) to merge split streaming chunks sharing `message.id`.
- **Attachment expansion (`normalizeAttachmentForAPI`, `utils/messages.ts:3453-4286`):** Expands runtime attachments into `isMeta` `UserMessage[]`—with gated branches for `feature('EXPERIMENTAL_SKILL_SEARCH')` (`utils/messages.ts:3506`), `feature('HISTORY_SNIP')` (`context_efficiency` at `utils/messages.ts:4149`, requiring absent `services/compact/snipCompact.js`), and `process.env.CLAUDE_CODE_VERIFY_PLAN === 'true'` (`utils/messages.ts:4244`)—then folds them into the preceding user turn via `mergeUserMessagesAndToolResults` (`utils/messages.ts:2372-2387`).
- **The `#21049` smooshing scar:** Leaving `<system-reminder>` text as siblings after a `tool_result` taught models premature 3-token `end_turn` stops (`#21049`, `utils/messages.ts:2605-2609`). When Statsig/GrowthBook gate `tengu_chair_sermon` is off, a legacy ungated path still smooshes all-text siblings into string-content `tool_result`s (`utils/messages.ts:2615-2628`). Enabling `tengu_chair_sermon` turns on *universal* smooshing (images, documents, and search results), `ensureSystemReminderWrap` (`utils/messages.ts:2273-2277`), and post-pass `smooshSystemReminderSiblings` (`utils/messages.ts:2334-2338`). Separately, `tengu_toolref_defer_j8m` (`utils/messages.ts:2160`, `utils/messages.ts:2301-2305`) relocates siblings off `tool_reference` messages (`relocateToolReferenceSiblings`) instead of injecting the fallback `'Tool loaded.'` turn boundary.
- **Final Stage 1 guards:** `sanitizeErrorToolResultContent` (`utils/messages.ts:2343`, unconditional) strips non-text blocks from `is_error: true` tool results; `feature('HISTORY_SNIP') && process.env.NODE_ENV !== 'test'` appends `[id:...]` tags via `isSnipRuntimeEnabled()` from absent `services/compact/snipCompact.js` (`utils/messages.ts:2351-2364`); and `validateImagesForAPI` (`utils/messages.ts:2367`) enforces the 5 MB base64 cap.

### Hidden Invariants & Failure Modes

- **Cache & VCR protection:** Inbound `normalizeContentFromAPI` (`utils/messages.ts:2721-2730`) leaves whitespace text unmodified so prompt-cache hashes hold (logging unparseable JSON only when `process.env.USER_TYPE === 'ant'` at `utils/messages.ts:2687`). Outbound Stage 2 `userMessageToMessageParam` (`services/api/claude.ts:622-630`) clones array `content` so `cache_edits` splicing (`feature('CACHED_MICROCOMPACT')`) never mutates caller arrays. Both `tengu_chair_sermon` and `HISTORY_SNIP` guard against changing VCR fixture hashes.

---

## 5. Wire-Safety Invariants: Thinking Blocks & Tool Pairing

### The Problem

Interrupted streams, compaction slices, `/resume`, `/login`, and fallback retries can strand thinking shards, leave trailing thinking atop whitespace text, invalidate signatures, duplicate `tool_use` IDs across turns, or orphan `tool_result`s—all fatal 400 errors.

### The Real Implementation

#### A. Thinking-Block Pass Order & Signature Stripping

Stage 1 runs four assistant-cleanup passes in a **strict, non-commutable order**:

```ts
// utils/messages.ts:2307-2325
// Filter orphaned thinking-only assistant messages (likely introduced by
// compaction slicing away intervening messages between a failed streaming
// response and its retry). Without this, consecutive assistant messages with
// mismatched thinking block signatures cause API 400 errors.
const withFilteredOrphans = filterOrphanedThinkingOnlyMessages(relocated)

// Order matters: strip trailing thinking first, THEN filter whitespace-only
// messages. The reverse order has a bug: a message like [text("\n\n"), thinking("...")]
// survives the whitespace filter (has a non-text block), then thinking stripping
// removes the thinking block, leaving [text("\n\n")] — which the API rejects.
//
// These multi-pass normalizations are inherently fragile — each pass can create
// conditions a prior pass was meant to handle. Consider unifying into a single
// pass that cleans content, then validates in one shot.
const withFilteredThinking =
  filterTrailingThinkingFromLastAssistant(withFilteredOrphans)
const withFilteredWhitespace =
  filterWhitespaceOnlyAssistantMessages(withFilteredThinking)
const withNonEmpty = ensureNonEmptyAssistantContent(withFilteredWhitespace)
```

1. `filterOrphanedThinkingOnlyMessages` (`utils/messages.ts:4991-5058`) drops thinking-only shards whose `message.id` has no non-thinking sibling.
2. `filterTrailingThinkingFromLastAssistant` (`utils/messages.ts:4781-4828`) strips trailing thinking blocks from `messages.at(-1)` (substituting `'[No message content]'` if all blocks were thinking).
3. `filterWhitespaceOnlyAssistantMessages` (`utils/messages.ts:4869-4919`) drops whitespace-only assistants and re-merges newly adjacent `user` turns—catching `[text("\n\n")]` exposed by step 2.
4. `ensureNonEmptyAssistantContent` (`utils/messages.ts:4933-4977`) patches non-final `content: []` to `NO_CONTENT_MESSAGE` (`'(no content)'`), leaving a final empty assistant legal for prefill.

Across credential or model changes, `stripSignatureBlocks` (`utils/messages.ts:5066-5099`) strips `thinking`, `redacted_thinking`, and—behind `feature('CONNECTOR_TEXT')`—`isConnectorTextBlock` (from absent `types/connectorText.js`), stripping thinking-only shards to `content: []` so `mergeAssistantMessages` absorbs them. It runs unconditionally on `/login` (`commands/login/login.tsx:24`) and behind `process.env.USER_TYPE === 'ant'` on `FallbackTriggeredError` (`query.ts:927-929`). In Stage 2, `assistantMessageToMessageParam` (`services/api/claude.ts:658-661`) also refuses to attach `cache_control` to trailing `thinking`, `redacted_thinking`, or `CONNECTOR_TEXT` blocks.

#### B. Bidirectional `ensureToolResultPairing` (`utils/messages.ts:5133-5460`)

Because 1-to-1 `(assistant, user)` adjacency exists only after Stage 1 merges streaming chunks and consecutive user turns, `ensureToolResultPairing` runs in **Stage 2 at `services/api/claude.ts:1301`**:

```ts
// utils/messages.ts:5301-5443
// Find missing tool_result IDs (forward direction: tool_use without tool_result)
const toolUseIdSet = new Set(toolUseIds)
const missingIds = toolUseIds.filter(id => !existingToolResultIds.has(id))

// Find orphaned tool_result IDs (reverse direction: tool_result without tool_use)
const orphanedIds = [...existingToolResultIds].filter(
  id => !toolUseIdSet.has(id),
)
// ...
// Build synthetic error tool_result blocks for missing IDs
const syntheticBlocks: ToolResultBlockParam[] = missingIds.map(id => ({
  type: 'tool_result' as const,
  tool_use_id: id,
  content: SYNTHETIC_TOOL_RESULT_PLACEHOLDER,
  is_error: true,
}))
// ...
if (getStrictToolResultPairing()) {
  throw new Error(
    `ensureToolResultPairing: tool_use/tool_result pairing mismatch detected (strict mode). ` +
      `Refusing to repair — would inject synthetic placeholders into model context. ` +
      `Message structure: ${messageTypes.join('; ')}. See inc-4977.`,
  )
}
```

Six invariants are enforced in one forward pass:
1. **No leading `tool_result` (`utils/messages.ts:5152-5203`):** Strips `tool_result` blocks from any `user` message not preceded by an `assistant`, substituting `'[Orphaned tool result removed due to conversation resume]'` at index 0 so requests open with `user`.
2. **Cross-message `tool_use` dedup (`utils/messages.ts:5147`, `utils/messages.ts:5225-5234`):** Tracks `allSeenToolUseIds` globally across turns (`CC-1212` `"tool_use ids must be unique"`).
3. **Same-message server/MCP pairing (`utils/messages.ts:5205-5211`, `utils/messages.ts:5235-5243`):** Strips `server_tool_use`/`mcp_tool_use` lacking a matching `*_tool_result` in the same assistant message.
4. **Non-empty assistant fallback (`utils/messages.ts:5248-5256`):** Inserts `'[Tool use interrupted]'` if stripping empties content.
5. **Bidirectional repair + role alternation (`utils/messages.ts:5267-5399`):** Prepends `is_error: true` `SYNTHETIC_TOOL_RESULT_PLACEHOLDER` (`'[Tool result missing due to internal error]'`, `utils/messages.ts:246-247`) for forward `missingIds`, strips reverse `orphanedIds` and duplicate `tool_result`s, and replaces an emptied `nextMsg` with `createUserMessage({ content: NO_CONTENT_MESSAGE, isMeta: true })` (`utils/messages.ts:5382-5387`) to preserve role alternation.
6. **HFI strict mode (`utils/messages.ts:5437-5456`):** Interactive turns repair and log `tengu_tool_result_pairing_repaired`; HFI enables `getStrictToolResultPairing()` (`bootstrap/state.ts:1093-1095`, default `false`) to throw citing `inc-4977` instead of tainting trajectories.

### Hidden Invariants & Failure Modes

- Because `ensureToolResultPairing` runs after Stage 1, prepending `syntheticBlocks` can create a `[tool_result, <system-reminder>]` sibling Stage 1 never saw—so `utils/messages.ts:5370-5374` re-runs `smooshSystemReminderSiblings` on `patchedNext` when `tengu_chair_sermon` is enabled.

---

## Putting Layer 1 Together

```text
Keyboard / SDK ContentBlockParam[] / Pasted Images / IDE / Bridge
   │
   ▼
processUserInput()  (utils/processUserInput/processUserInput.ts)
   ├─ resize inline & pasted images + storeImages() + imageMetadataTexts
   ├─ bridgeOrigin (isBridgeSafeCommand) + ULTRAPLAN rewrite (gated)
   ├─ getAttachmentMessages() (1s timeout; skipped for /slash in prompt)
   ├─ mode==='bash' -> processBashCommand.tsx          (shouldQuery: false)
   ├─ '/slash'      -> parseSlashCommand + processSlashCommand.tsx
   │                  [local/forked: false | local-jsx: opt-in | prompt: true]
   ├─ plain text    -> processTextPrompt.ts            (shouldQuery: true)
   └─ if (shouldQuery) -> executeUserPromptSubmitHooks() (10k cap)
   │
   ▼
Shared Message[] Transcript (user, assistant, attachment, progress, system)
   ├──> UI View   : normalizeMessages (1->N, deriveUUID) + reorderMessagesInUI
   └──> Model View:
          [Pre]  getMessagesAfterCompactBoundary + prependUserContext (Ch 04)
          [Stg1] normalizeMessagesForAPI() (utils/messages.ts:1989-2370)
                   ├─ reorderAttachmentsForAPI + drop isVirtual
                   ├─ stripTargets + drop progress/UI system/synth errors
                   ├─ promote local_command + merge user & split assistants
                   ├─ filterOrphaned -> filterTrailing -> filterWhitespace
                   │  -> ensureNonEmpty + smoosh + sanitizeErrorToolResult
                   └─ returns internal (UserMessage | AssistantMessage)[]
          [Stg2] services/api/claude.ts (Ch 05 seam)
                   ├─ !useToolSearch strip + ensureToolResultPairing()
                   └─ stripAdvisor/media(100) + addCacheBreakpoints -> wire
```

---

## How This Snaps Into the Next Layer

Layer 1 (`Tool`, `Command`, `processUserInput`, Stage 1 normalization) is complete and snaps directly into **Layer 2**, starting with [Chapter 04](ch004-prompt-engineering-and-cache-boundaries.md):

- **Skill inventory → dynamic prompt:** `constants/prompts.ts:458` awaits `getSkillToolCommands(cwd)` (`commands.ts:563`) inside `getSystemPrompt()` to populate the post-boundary `session_guidance` section (`constants/prompts.ts:491-494`).
- **Split context snapshots → `isMeta` user message & system prompt:** `context.ts` splits cached snapshots across `getUserContext()` (`CLAUDE.md` + `currentDate`, `context.ts:155-189`) and `getSystemContext()` (`git status`, `context.ts:116-150`, appended to the system prompt via `appendSystemContext` at `utils/api.ts:437-447`). `prependUserContext` (`utils/api.ts:449-474`, invoked at `query.ts:660`, no-op when `process.env.NODE_ENV === 'test'` at `utils/api.ts:453`) wraps `getUserContext()` via `createUserMessage({ ..., isMeta: true })` so Stage 1 merges it with the first user turn.
- **Reciprocal tool-input cleanup:** `utils/messages.ts:2215` calls back into `utils/api.ts:685-718` (`normalizeToolInputForAPI`) to strip runtime-injected fields before API serialization.
- **Stage 2 forward seam ([Chapter 05](ch005-talking-to-the-model.md)):** `services/api/claude.ts:588-674`, `services/api/claude.ts:1266-1315`, `services/api/claude.ts:1701-1709`, and `services/api/claude.ts:3063-3211` run `ensureToolResultPairing`, `addCacheBreakpoints`, `userMessageToMessageParam`, and `assistantMessageToMessageParam` to emit cached wire `MessageParam[]` payloads.

---

## Reading Exercises & Code Checkpoints

1. **Trace Remote Control bridge safety from registry to input router:**
   ```sh
   rg -n "isBridgeSafeCommand|BRIDGE_SAFE_COMMANDS|REMOTE_SAFE_COMMANDS" commands.ts utils/processUserInput/processUserInput.ts
   ```
   Why does `isBridgeSafeCommand()` automatically allow every `prompt` command while requiring an explicit allowlist for `local` commands and blocking every `local-jsx` command?

2. **Follow the Stage 1 normalization and Stage 2 tool-pairing boundary:**
   ```sh
   rg -n "normalizeMessagesForAPI|reorderAttachmentsForAPI|ensureToolResultPairing|filterTrailingThinkingFromLastAssistant" utils/messages.ts services/api/claude.ts
   ```
   Verify that `ensureToolResultPairing()` runs at `services/api/claude.ts:1301` rather than inside `normalizeMessagesForAPI()`. Why must same-`id` assistant merging and model-aware tool-search stripping finish before pairing runs?

3. **Audit input-pipeline gates, hook truncation, and `isMeta` image metadata:**
   ```sh
   rg -n "MAX_HOOK_OUTPUT_LENGTH|executeUserPromptSubmitHooks|ULTRAPLAN|addImageMetadataMessage" utils/processUserInput/
   ```
   Which early-return branch inside `processUserInputBase()` bypasses `addImageMetadataMessage()`, and why do `UserPromptSubmit` hooks never execute for `!` bash commands?

4. **Inspect the Chapter 03 → Chapter 04 and Stage 2 (Chapter 05) seams:**
   ```sh
   rg -n "getSkillToolCommands|prependUserContext|userMessageToMessageParam|assistantMessageToMessageParam" constants/prompts.ts utils/api.ts services/api/claude.ts
   ```
   Notice that `prependUserContext()` (`utils/api.ts:449-474`) constructs a `UserMessage` with `isMeta: true` while `userMessageToMessageParam()` (`services/api/claude.ts:588-631`) strips internal metadata into an Anthropic SDK `MessageParam`. Where in the pipeline does `isMeta` stop mattering?

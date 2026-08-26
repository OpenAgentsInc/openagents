# Claude Code Teardown 02: Tool Surface, Schemas & Sandboxing

Scope: `~/work/projects/repos/cc` (`Tool.ts`, `tools.ts`, `tools/`, `native-ts/`, `schemas/`, `utils/sandbox/`) versus `packages/openagents-cli/src/` (`coder-tools.ts`, `coder-tool-families.ts`, `coder-tool-budget.ts`, `coder-shell.ts`, `coder-plugins.ts`, `coder-plugin-engine.ts`, `coder-capability.ts`) and `plugins/` (Rust PDK + guest crates).

## Component Breakdown

| Subsystem | Claude Code | OpenAgents Coder |
|---|---|---|
| Tool contract | `Tool.ts` (792 ln), `buildTool()` | `CoderTool` interface in `coder-tools.ts` |
| Tool registry | `tools.ts`: presets, deny filters, pool assembly | `cli.ts` `declareTools()` closure |
| Built-in tools | 40+ under `tools/` | 4 factories + `capability` |
| Shell safety | tree-sitter parse, sandbox runtime, permissions | static regex refusal table |
| Extensibility | MCP client + plugin scaffolding | WASM plugin host + Rust PDK |
| Result sizing | per-tool `maxResultSizeChars` | per-model-family token budgets |

## Claude Code Implementation

**The Tool contract** (`Tool.ts`) is a ~40-member type parameterized over `<Input, Output, Progress>`:

- *Semantics predicates*: `isConcurrencySafe(input)`, `isReadOnly(input)`, `isDestructive(input)`, `isEnabled()`, `isOpenWorld(input)`, `interruptBehavior(): 'cancel' | 'block'`, `isSearchOrReadCommand(input)` — the query loop uses these to decide parallel dispatch, permission prompts, and interruption behavior.
- *Schemas*: `inputSchema` (zod), optional hand-written `inputJSONSchema` override, `outputSchema` (zod), `strict?: boolean` (structured-output mode), `inputsEquivalent(a,b)` for transcript dedup.
- *Permissions*: `checkPermissions(input, ctx): Promise<PermissionResult>` where `PermissionResult` is a discriminated union — `behavior: 'allow' | 'ask' | 'deny' | 'passthrough'`, carrying `updatedInput` (permission may rewrite arguments) and reasons. `getPath(input)` and `preparePermissionMatcher(input)` let hook `if` patterns like `"Bash(git *)"` match against parsed commands rather than bare names.
- *Presentation*: `prompt({getToolPermissionContext, tools, agents})`, `userFacingName`, background color, plus JSX renderers (`renderToolUse*`) — tools own their Ink UI.
- *Economy*: `maxResultSizeChars`, `shouldDefer`, `alwaysLoad`, `searchHint`, `aliases`.
- *Context*: `ToolUseContext` carries `abortController`, `readFileState: FileStateCache`, app-state accessors, MCP clients/resources, `requestPrompt`, denial-tracking state, and `contentReplacementState` (aggregate tool-result budgeting across a thread).

`buildTool(def)` fills fail-closed defaults: `isConcurrencySafe -> false` (assume unsafe), `isReadOnly -> false` (assume writes), `checkPermissions -> allow` (defer to the general system), `toAutoClassifierInput -> ''` (skip the security classifier unless overridden).

**Registry** (`tools.ts`): `getAllBaseTools()` composes ~45 tools with environment-conditioned inclusion (`USER_TYPE === 'ant'`, embedded bfs/ugrep replacing Glob/Grep, worktree mode, todo v2, swarms). `assembleToolPool(permissionContext, mcpTools)` merges MCP tools and **sorts with built-ins as a contiguous prefix** because the server's cache policy places a breakpoint after the last prefix-matched built-in — interleaving would bust downstream prompt caches. `filterToolsByDenyRules` strips denied MCP servers entirely.

**Per-tool depth**: BashTool alone spans 18 files (1,143-ln core): `bashSecurity.ts` blocks command-substitution vectors ($(), <(), Zsh `=cmd` equals-expansion that hides the real binary from deny rules, glob qualifiers, heredoc-in-substitution), `sedEditParser`/`sedValidation` catch edits smuggled through sed, `bashPermissions.ts` strips env-var prefixes and safe wrappers before rule matching, `shouldUseSandbox.ts` consults settings, GrowthBook flags and `excludedCommands`. FileEdit/Write enforce read-before-write through the shared `FileStateCache` (LRU, 100 entries / 25 MB, an `isPartialView` flag forcing explicit Read after auto-injection). AgentTool is 2,370 lines (fork/resume/memory/subagent contexts). AskUserQuestionTool structures mid-turn interaction.

**Deferral**: MCP tools are always deferred; `ToolSearchTool` (471 ln) matches queries against name+`searchHint` and injects full schemas inside a `<functions>` block, making them callable thereafter. This keeps hundreds of MCP tools out of the initial prompt.

**Sandboxing**: `utils/sandbox/sandbox-adapter.ts` bridges `@anthropic-ai/sandbox-runtime`: filesystem read/write restriction configs, network host patterns, a violation store, per-platform dependency checks, and settings integration. The comment on `excludedCommands` is doctrine: *"a user-facing convenience feature, not a security boundary… the sandbox permission system is the actual control."*

**native-ts/**: pure-TS reimplementation of three native modules (nucleo-based `file-index`, `color-diff`, `yoga-layout`) to drop compiled dependencies.

**Extensibility**: `services/mcp/` (~20 files: auth, channel allowlists, elicitation handlers) plus `plugins/bundled/index.ts` — which registers *nothing*; it is scaffolding awaiting migration of bundled skills.

## OpenAgents Coder State

**Contract** (`coder-tools.ts`): `CoderTool = { name, description, parameters: Record<string, unknown>, run(args, signal): Promise<string> }`. Raw JSON Schema, no zod, no output schema, no permission method, no predicates. Documented decision: refusals return text the model can act on; throwing kills the turn. Four factories: `delegate` (fan-out N children across lanes, abort wires to `registry.stopAll()`, per-child numbering), `skill` (enum-constrained catalog reader), `openagents` (CLI passthrough taking an `args` string vector), `shell`.

**Shell** (`coder-shell.ts`, 175 ln): `/bin/sh -c`, stdin closed, stdout/stderr merged in arrival order, timeout SIGKILL (120 s default, 600 s ceiling), abort wired to SIGKILL, a `dropped` character counter past `OUTPUT_LIMIT`, and a cut notice naming how much is missing. Safety is `REFUSED`: nine regexes (deleting a root or home, formatting a filesystem, writing raw devices, repartitioning, stopping the machine, fork bombs, recursive mode changes on roots, direct device redirects) each paired with a human reason. No parser, no sandbox, no permission rules.

**Family-aware declarations** (`coder-tool-families.ts`): `toolFamilyOf(model)` returns `default | gemini | local`; measured emphasis sentences are appended per tool per family (Gemini gets batching/token-economy instructions backed by Terminal Bench data). **Per-family result budgets** (`coder-tool-budget.ts`): exhaustive `FamilyBudget` records (`contextWindowTokens`, `resultTokens`, `charactersPerToken`, `because`); unknown families fall back to the *smallest* row with a surfaced `substituted` flag; `budgetedResult()` cuts the middle with an explicit omission notice. Applied in every harness (`coder-thread.ts:783` et al.).

**WASM plugin host** (`coder-plugins.ts` + `coder-plugin-engine.ts`): manifest-first loading; SHA-256 digest pin verified *before compile*; `inspect()` proves the module's import list is covered by declared capabilities (pure compute means zero imports; mounts mean exactly `openagents.read_file` + `openagents.list_dir`, plus a bounded `read_file_range`); read-only mounts canonicalize paths, refuse absolute paths/`..`/symlinks, bound bytes-per-file and entries-per-listing; `${workspace}` mount expansion; `packet-v0` ABI; typed `{code, reason}` refusals both directions. The engine seam mandates engines enforce their own limits ("an engine that cannot kill a runaway guest is not enforcing anything"); the Node default runs one `worker_threads` worker per invocation, terminated at timeout. The Rust PDK (`plugins/pdk`) generates the whole ABI from `plugin_entry!(handle)` over serde types. Eleven guest crates ship prebuilt `.wasm` artifacts with checked-in digests.

**Capability discovery** (`coder-capability.ts`): one standing `capability` tool; no embeddings — returns the whole catalog and the model picks by exact name; unmatched requests are recorded to `~/.openagents/capability-gaps.jsonl` for a future registry loop.

**Session wiring** (`cli.ts` ~2515–2600): `declareTools()` rebuilds `[shell, skill?, openagents, delegate?, capability, ...visiblePlugins()]` each turn. Plugins are **turn-scoped**: reader-pinned (`/plugin load`) or *warm* (matched by retrieval or invoked within two turns), instances cached so revival is free. `PluginApproval` is currently `ask: () => "allow"` — an auto-consent placeholder. `summarizeToolCall` renders calls as typed command lines rather than JSON.

## Gap Analysis

1. **No file primitives as model tools.** CC's Read/Edit/Write/Glob/Grep/NotebookEdit (~4,000 ln) enforce read-before-write via shared cache state, handle images, and give ripgrep-backed search. OA's lane edits files through `shell` and delegates file-heavy work to child harnesses (the Claude/Codex lanes bring their own tools). Consequence: the OA-native lane has no structural protection against blind writes, and no fast indexed search.
2. **No permission architecture.** CC's `PermissionResult` union, rule sources (allow/deny/ask x origin), additional working directories, modes, and denial tracking have no OA counterpart — only the shell blocklist and an auto-approving `PluginApproval`. Nothing stands between the model and `run()`.
3. **No OS sandbox for shell.** CC wraps Bash in `sandbox-runtime` (filesystem/network restrictions, violation auditing). OA's regex table is a blocklist against named footguns, not confinement; a destructive command outside the nine patterns executes unrestricted.
4. **No hooks/interceptors.** CC's PreToolUse hooks can rewrite inputs or deny calls; `schemas/hooks.ts` types the whole event surface. OA has no seam between declaration and execution.
5. **No concurrency metadata.** Without `isConcurrencySafe`, OA cannot parallelize independent tool calls within a turn (it fans out via `delegate` instead — a deliberate but coarse substitute).
6. **No deferral protocol for third-party catalogs.** CC defers MCP tools behind ToolSearch. OA's warm-window achieves similar token economy for plugins, and `capability` covers discovery — arguably cleaner — but there is no path for hundreds of external tools.
7. **Schema rigor.** No zod validation layer, no `strict` structured outputs, no output schemas; plugin manifests do carry typed I/O schemas, so the concept exists at the WASM boundary only.

Counter-gaps (where OA leads): import-inspection-before-instantiate is stronger than CC's empty plugin scaffolding; per-family token budgets with honest cut notices exceed CC's flat `maxResultSizeChars`; family-tuned descriptions are measurement-driven; digest pinning gives provenance receipts CC does not offer for MCP.

## Actionable Recommendations

1. **Port `PermissionResult` as a seam now** (before UI exists): add optional `checkPermissions?(args): Promise<{behavior:'allow'|'deny'|'ask', updatedInput?}>` to `CoderTool`, defaulting to allow; route `shell` and `pluginTool` through it. Cheap today, expensive after call sites multiply.
2. **Add a read-before-write guard**: track `path -> last-read` in the session (the `FileStateCache` idea minus the LRU), and have the `shell` tool warn when a write-target file was never read this session. One map buys most of the invariant.
3. **Encode fail-closed defaults in a `makeTool` factory** once the count passes ~10: `readOnly = false`, `concurrencySafe = false` — mirroring `TOOL_DEFAULTS` so omissions fail safe.
4. **Add `isReadOnly`/`isConcurrencySafe` predicates** and let the runner issue concurrent calls over safe tools; the delegate fan-out pattern generalizes.
5. **Optional seatbelt for shell**: wrap `/bin/sh` in `sandbox-exec` (macOS) with a write-confined profile when available, keeping the regex table as a fast pre-filter — adopting CC's doctrine that convenience lists are not boundaries.
6. **PreToolUse interceptor seam**: `(args) => args | refusal` invoked by the harness before `run`, hosting future permission checks and audit logging without touching each tool.
7. **Keep warm-window deferral**, but extend `capability`'s gap log consumer so unmatched requests can trigger registry publication — closing CC's MCP reach without MCP's prompt-token cost.
8. **Preserve the inspection-first plugin model** as the documented differentiator; port CC's `maxResultSizeChars` only as a per-plugin manifest field (plugins already declare limits).

# Chapter 00 — How to Read a 1,900-File Agent

> **Layer:** Prologue (Orientation)
> **Prerequisites:** None
> **Primary Source Files:**
> - `entrypoints/cli.tsx` — zero-import CLI bootstrapper
> - `main.tsx` — full CLI wiring and dispatch (~804 KB)
> - `screens/REPL.tsx` — interactive terminal conversation UI (~896 KB)
> - `docs/REPO_AUDIT.md` — existing source-tree audit index

---

## Why Reading Top-Down Fails

When most engineers open an unfamiliar CLI repository, they reach straight for `main.tsx`.

In Claude Code, opening `main.tsx` first is the fastest way to get lost. At over 800 KB, `main.tsx` sits at the very *top* of the dependency pyramid: it imports startup profilers, telemetry, OAuth, remote managed settings, migrations, MCP servers, plugins, skills, permission policies, structured I/O, Ink renderers, and the REPL launcher all at once. Reading `main.tsx` before understanding the primitives underneath is like trying to learn how an internal combustion engine works by staring at a fully assembled Formula 1 car from the outside.

This book takes the exact opposite route: **bottom-up assembly**. We will start at the zero-dependency leaves—files that are 8, 34, 44, and 88 lines long—and stack one layer onto the next until `main.tsx` and `screens/REPL.tsx` read like straightforward wiring diagrams.

Before we touch Layer 0, however, let's get a quick satellite view of the repository and learn the **five codebase idioms** you will see in almost every file you open.

---

## 1. The Lay of the Land: Where the 1,910 Files Live

This checkout contains **1,910 tracked files**, of which **1,884 are `.ts` and `.tsx` source files**. Roughly 85% of the codebase lives in just seven directories:

| Directory | Files | What Lives Here |
| --- | ---: | --- |
| `utils/` | 564 | Shared infrastructure, permissions, hooks, plugins, settings, swarm, messages, shell, git, telemetry |
| `components/` | 389 | Reusable React/Ink terminal UI views, message renderers, permission prompts, dialogs, design system |
| `commands/` | 207 | Built-in slash commands (`/compact`, `/resume`, `/model`, `/mcp`, `/plugin`, `/permissions`, etc.) |
| `tools/` | 184 | Model-callable tools (`BashTool`, `FileReadTool`, `FileEditTool`, `GlobTool`, `GrepTool`, `AgentTool`, etc.) |
| `services/` | 130 | Domain services: Anthropic API client (`api/`), compaction (`compact/`), MCP (`mcp/`), LSP (`lsp/`), analytics, memory |
| `hooks/` | 104 | React hooks bridging terminal input, permissions, tasks, IDE, voice, and live settings into components |
| `ink/` | 96 | A full repository-local React terminal renderer (custom DOM, reconciler, Yoga layout, screen diffing, ANSI/mouse/focus) |

The remaining ~200 files form the **architectural spine** of the application—and they are remarkably compact:

- **Root orchestrators & contracts:** `Tool.ts`, `tools.ts`, `Task.ts`, `tasks.ts`, `commands.ts`, `context.ts`, `query.ts`, `QueryEngine.ts`, `setup.ts`, `replLauncher.tsx`, `cost-tracker.ts`, `main.tsx`
- **Focused subsystems:** `bootstrap/` (1), `state/` (6), `query/` (4), `types/` (11), `entrypoints/` (8), `cli/` (19), `tasks/` (12), `memdir/` (8), `skills/` (20), `keybindings/` (14), `vim/` (5), `bridge/` (31), `remote/` (4), `server/` (3), `screens/` (3)

> [!TIP]
> Notice the asymmetry: **surface area** (hundreds of UI components, slash commands, and utilities) is huge, while the **architectural core** (`Tool.ts`, `state/store.ts`, `query.ts`, `QueryEngine.ts`, `services/tools/*`, `Task.ts`) is only a couple dozen files. Master that core and the rest of the 1,884 files snap into obvious drawers.

---

## 2. Five Recurring Idioms You Will See Everywhere

Open almost any file in this repo and you will immediately notice five patterns that look unusual if you haven't seen them explained before. Learning them right now will save you hours of confusion.

### Idiom 1: Build-Time Dead-Code Elimination with `feature('...')`

You will see `import { feature } from 'bun:bundle'` at the top of dozens of files, paired with inline ternaries and `require(...)` calls like this one in `tools.ts:25-28`:

```ts
const SleepTool =
  feature('PROACTIVE') || feature('KAIROS')
    ? require('./tools/SleepTool/SleepTool.js').SleepTool
    : null
```

And comments like:

```ts
// Intentionally excludes feature() gates — those are tree-shaking boundaries
// and must stay inline at the guarded blocks for dead-code elimination.
```

*(from `query/config.ts:13-14`)*

**Why it looks like this:** Claude Code ships multiple build flavors from a single TypeScript tree (including internal Anthropic builds identified by `process.env.USER_TYPE === 'ant'` and external customer builds). Bun's bundler evaluates `feature('FLAG')` at **compile time**. When a flag is off, the entire false branch—including its `require(...)` target and all transitive strings/imports—is completely tree-shaken out of the output binary. That is why you see `require()` inside ternaries instead of static top-level ES `import` statements, and why `feature()` checks are never wrapped in a helper function.

### Idiom 2: ESM `.js` Specifiers + `src/` Path Aliases

Even though every source file is `.ts` or `.tsx`, local imports use `.js` extensions (`import { createStore } from './store.js'`) and occasionally use a `src/` root alias (`import { logEvent } from 'src/services/analytics/index.js'`). This is standard TypeScript ESM resolution combined with a bundler/tsconfig alias from the upstream build environment.

### Idiom 3: Startup-Deferred Work (`lazySchema`, Lazy `require`, Dynamic `import()`)

A CLI tool that takes 800 ms just to print `--version` feels broken. Throughout the codebase, module evaluation time is treated as a first-class performance budget:

- `entrypoints/cli.tsx` intercepts `--version` with **zero imports** before touching `main.tsx`.
- `utils/lazySchema.ts` wraps Zod schemas in memoized thunks behind JS getters (`get inputSchema()`) so 60+ tool schemas are never constructed at import time.
- Heavy optional subsystems (OpenTelemetry ~400 KB, gRPC ~700 KB, `App.tsx`/`REPL.tsx`, MCP servers, `local-jsx` slash commands) load behind dynamic `await import(...)`.
- Some `require()` calls are wrapped in thunks specifically to break circular dependencies—for example, `tools.ts:63-71` wraps `TeamCreateTool`, `TeamDeleteTool`, and `SendMessageTool` in `getTeamCreateTool()` / `getSendMessageTool()` because team tools transitively import `tools.ts`.

### Idiom 4: React Compiler Output in `.tsx` Files

If you open `state/AppState.tsx` or `replLauncher.tsx`, you will see lines like:

```ts
import { c as _c } from "react/compiler-runtime";
export function AppStateProvider(t0) {
  const $ = _c(13);
  ...
```

And at the very bottom of the file, a long `//# sourceMappingURL=data:application/json;charset=utf-8;base64,...` comment.

**What you are looking at:** The `.tsx` files in this snapshot were captured **after React Compiler transformation** (automatic fine-grained memoization via `_c(slotCount)`). Even better: the base64 inline source map at the bottom of each `.tsx` file embeds `sourcesContent`—the original, uncompiled TSX source code! When studying `.tsx` files, focus on the semantic React hooks and data flow rather than the generated `$[0]` cache-slot boilerplate.

### Idiom 5: Snapshot Completeness Boundaries

Because this repository is a source snapshot of **Claude Code 2.1.88** (dated against the changelog; see the [Introduction](README.md#repository-snapshot-caveats)) rather than the complete upstream monorepo, keep two factual boundaries in mind:

1. **No build/test harness is checked in:** There is no `package.json`, `tsconfig.json`, or test config in the tracked tree. We treat this repo as a read-only specimen to dissect, verify by reading source and running `rg`, and learn from.
2. **A handful of imported modules are absent from the snapshot:** Notably:
   - `types/message.ts` (widely imported for `Message`, `UserMessage`, `AssistantMessage`, `SystemMessage`, `AttachmentMessage`, `ProgressMessage`, `StreamEvent`, `TombstoneMessage`, etc.—whose exact shapes can still be observed through every constructor in `utils/messages.ts`)
   - `query/transitions.ts` (imported by `query.ts` for `Terminal` and `Continue`, whose exact discriminated union variants are directly visible at every `return { reason: ... }` and `transition: { reason: ... }` site in `query.ts`)
   - Feature-gated `services/contextCollapse/` and `services/skillSearch/` (guarded behind `feature('CONTEXT_COLLAPSE')` and `feature('EXPERIMENTAL_SKILL_SEARCH')`).

Whenever a chapter crosses one of these boundaries, it will tell you explicitly what is verified directly in source and what is absent.

---

## 3. How We Will Build Up the Codebase

Instead of wandering directory by directory, we will assemble the machine in strict dependency order:

1. **Part I (Chapters 1–3):** Atoms (`types/ids.ts`, `state/store.ts`, `bootstrap/state.ts`, `lazySchema`, `generators`, `FileStateCache`) → `Tool` contract → `Command` & `Message` contracts.
2. **Part II (Chapters 4–8):** Prompt construction & cache boundaries → Anthropic streaming API client → tool execution & `StreamingToolExecutor` → the `query.ts` turn state machine → the `QueryEngine.ts` session envelope.
3. **Part III (Chapters 9–10):** Five-stage context compaction and multi-tier memory (`CLAUDE.md`, `memdir/`, `SessionMemory`, `autoDream`).
4. **Part IV (Chapters 11–14):** Permission modes/rules/classifiers, shell/sandbox execution, lifecycle hooks, and settings/policy precedence.
5. **Part V (Chapters 15–16):** Dynamic extensions—MCP, `ToolSearch`, skills, plugins, output styles, and LSP.
6. **Part VI (Chapters 17–19):** Background tasks (`Task.ts`), subagents/forking/worktrees, and multi-agent swarms.
7. **Part VII (Chapters 20–21):** The custom `ink/` React terminal renderer and the interactive `REPL.tsx` UI.
8. **Part VIII (Chapters 22–24):** Finally, `entrypoints/cli.tsx` → `init.ts` → `main.tsx`, headless/SDK structured I/O, and remote bridges.

---

## Reading Checkpoints

Before moving to Chapter 1, run these two read-only inspections in the repo root to see the idioms live:

1. **See the zero-import fast paths in `entrypoints/cli.tsx`:**
   Read lines 1–60 of `entrypoints/cli.tsx` and notice how `CLAUDE_CODE_ABLATION_BASELINE` sets env vars *before* dynamic imports so module-level constants in `BashTool`/`AgentTool` see them.
2. **Count compile-time feature gates across the codebase:**
   ```sh
   rg "feature\('" --no-heading | head -n 30
   ```
   Notice how every `feature('...')` sits inline at a branch rather than behind an abstraction.

Now let's drop all the way down to **Layer 0** and examine the six foundational atoms in [Chapter 1](ch001-atoms-types-stores-and-primitives.md).

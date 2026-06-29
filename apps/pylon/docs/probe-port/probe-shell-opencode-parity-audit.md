# Probe Shell Tool — Opencode BashTool Parity Audit

**Date:** 2026-06-08
**Context:** Probe has no real shell/bash command execution tool. The name `"shell"` is reserved in the Apple FM enum (`packages/runtime/src/backends/apple-fm/tools.ts:10`) but the executor is a noop mock. Opencode has a mature two-tier implementation. This doc explains how opencode implements its shell tool and what probe should build.

---

## How Opencode Implements Bash Execution

### Two Tiers

**V2 Core `BashTool`** (`packages/core/src/tool/bash.ts`) — lightweight, SDK-consumable:

| Aspect | Detail |
|---|---|
| Parameters | `command` (string, required), `workdir` (optional), `timeout` (optional, default 120s, max 600s), `description` (optional) |
| Schema | Effect Schema `Schema.Struct` with `PositiveInt` for timeout, explicit annotations |
| Process spawn | `ChildProcess.make()` from `effect/unstable/process` with `shell` option |
| Workdir | Resolved via `LocationMutation.resolve`; external-directory permission check for paths outside active Location |
| Output capture | `AppProcess.run()` with `maxOutputBytes` (1MB) and `maxErrorBytes` (1MB); compacted via `compactOutput` |
| Timeout | `Effect.timeoutOrElse` with `Duration.millis`; caught as "Timed out" → returns timed-out response (not a failure) |
| Exit code | Non-zero exit is **not** an error; returned in `{ exitCode, output }` success schema |
| Permissions | `assertPermission({ action: "bash", resources: [command] })` + `external_directory` permission |
| Advisory warnings | Scans command tokens for absolute paths outside workdir (advisory only, not enforced) |
| Shell selection | Config entry or platform default: `/bin/sh` on macOS/Linux |
| Security | `stdin: "ignore"`, `detached: true` (process group), `forceKillAfter: 3s` |
| Process group kill | macOS/Linux: `process.kill(-pid)` |

**V1 `ShellTool`** (`packages/opencode/src/tool/shell.ts`) — richer, app-facing:

| Aspect | Detail |
|---|---|
| Parameters | Same four as V2 but `description` is required |
| Shell selection | `Shell.acceptable()` — prefers config, then `$SHELL`, then system default; **denies** fish and nu |
| Shell invocation | bash: `-l -c "source ~/.bashrc; cd -- \"$1\"; eval <cmd>"`; zsh: `-l -c "source ~/.zshenv; cd; eval <cmd>"`; PowerShell: `-NoProfile -Command <cmd>` |
| Tree-sitter parsing | Lazily loads WASM parsers for bash and PowerShell; walks AST to identify commands, args, redirects, subexpressions |
| Permission scanning | `collect()` walks AST: file arguments to `rm`/`cp`/`mv`/etc. → `external_directory` check; command source text → `bash` permission; arity-based always-allow prefixes via `BashArity.prefix()` |
| Streaming output | `Stream.decodeText(handle.all)` with sliding window; when exceeding `limits.maxBytes`, writes to truncation file |
| Abort handling | `ctx.abort` signal → kills process |
| Truncation | Configurable `max_lines` (2000) and `max_bytes` (50KB); overflow written to `tool_`-prefixed files; 7-day cleanup |
| Output finalization | `tail(raw, maxLines, maxBytes)` keeps last N lines/bytes; prepends truncation notice |
| PTY terminal | Separate system with WebSocket-connected persistent sessions; HTTP API (`create`/`list`/`get`/`update`/`remove`/`connect`) |

### AppProcess (`packages/core/src/process.ts`)

The execution primitive:

```
run(command, opts):
  ├── spawn via spawner.spawn(command)
  ├── collect stdout/stderr concurrently (Effect.all, concurrency: unbounded)
  ├── apply byte limits via collectStream (tracks truncation)
  ├── timeout via Effect.timeoutOrElse
  ├── abort signal via Effect.raceFirst(waitForAbort)
  └── return { command, exitCode, stdout, stderr, stdoutTruncated, stderrTruncated }
```

Errors are typed `AppProcessError` with `command`, optional `exitCode`, optional `stderr`, optional `cause`.

### CrossSpawnSpawner (`packages/core/src/cross-spawn-spawner.ts`)

Low-level: uses `cross-spawn` library. Supports piped commands (stream stdout of one into stdin of next). Force-kill escalation: SIGTERM → (3s wait) → SIGKILL.

---

## Probe's Current State

### What Exists

| Item | Status | Location |
|---|---|---|
| Shell tool name | Reserved in enum | `packages/runtime/src/backends/apple-fm/tools.ts:10` |
| Shell tool definition | Noop mock | `packages/runtime/tests/apple-fm-tools.test.ts:22-33` |
| Blueprint ref | `tool.probe.shell_command` | `packages/runtime/src/benchmark/fixtures.ts:83` |
| Process spawning | Only ripgrep for `search_code` | `packages/runtime/src/cli.ts:1347` |
| Tool dispatch | `dispatchProbeLlmTool` in | `packages/runtime/src/llm/tool-runtime.ts:17` |
| Permission system | Default handler always allows | `packages/runtime/src/permission.ts:29-31` |

### Gaps vs Opencode

| Capability | Opencode | Probe |
|---|---|---|
| Bash execution | Full | **None** |
| Shell selection | Configurable + denied shells | N/A |
| Process group kill | macOS/Linux | N/A |
| Timeout with force-kill | Yes (configurable + 3s grace) | N/A |
| Output bounds/truncation | Configurable lines/bytes | Only on file reads (100KB) |
| Streaming output | Yes (V1 ShellTool) | N/A |
| Tree-sitter AST scanning | Yes (V1) | N/A |
| Permission gates | `bash` + `external_directory` | Always-allow default |
| Abort signal | `ctx.abort` → kill tree | N/A |
| Workspace path protection | `LocationMutation` + revalidation | Path validation (no null bytes, no `..`, no `.git`) |
| Exit code handling | Non-zero in success schema | N/A |
| Advisory path warnings | Yes | N/A |
| Cross-platform shell | zsh/bash/sh/cmd/pwsh/git bash | N/A |

---

## What Probe Should Build

### Minimal Viable Shell Tool

Based on opencode's V2 `BashTool` as the minimal contract, probe needs:

```
Parameters:
  command:     string (required)
  description: string (optional)
  workdir:     string (optional, defaults to workspace root)
  timeout:     number (optional, default 120000, max 600000)

Execution:
  1. Resolve workdir (relative paths from workspace root; reject outside)
  2. Select shell (Bun auto-detects; probe could prefer $SHELL or /bin/sh)
  3. Spawn: Bun.spawn([command], { shell: true, cwd, stdio: ["ignore", "pipe", "pipe"] })
  4. Collect stdout/stderr with byte limit (start at 1MB)
  5. Apply timeout via Promise.race + kill timer
  6. On timeout/abort: kill process tree (Bun.spawn supports signal + kill)
  7. Return { output, exitCode, timedOut, truncated }
  8. Non-zero exit → return in success (not an error)
```

### Parameter Schema (Effect Schema)

Probe uses Effect Schema already. Follow opencode's `Schema.Struct` pattern:

```typescript
const ShellParameters = S.Struct({
  command: S.String.annotations({ description: "Shell command string to execute" }),
  description: S.String.annotations({ description: "Concise description of the command's purpose" }).pipe(S.optional),
  workdir: S.String.annotations({ description: "Working directory. Defaults to workspace root." }).pipe(S.optional),
  timeout: S.Number.pipe(S.positive(), S.filter(n => n <= 600_000)).pipe(S.optional)
    .annotations({ description: "Timeout in milliseconds (max 600000, default 120000)" }),
});
```

### Permission Gating

Probe already has a `PermissionHandler` interface (`packages/runtime/src/permission.ts`). For the shell tool:

1. **Workspace confinement**: Reject `workdir` or command paths that escape the workspace root (or use an explicit external-directory allowlist).
2. **Command permission**: Require explicit `"allow"` policy or user approval for shell execution. The current always-allow default is insufficient.
3. **Denied commands**: Consider a blocklist for destructive operations (rm -rf /, dd, etc.) or at minimum advisory warnings.

### Output Bounds & Truncation

Adapt opencode's truncation pattern (`packages/opencode/src/tool/truncate.ts`):
- Default: 2000 lines / 50KB returned inline
- Overflow: write full output to a file, return preview + file path
- Cleanup: stale truncation files after N days

Probe already truncates file reads at 100KB. Reuse that constant or make it configurable.

### PTY Terminal

Probe should **not** build a PTY terminal system initially. The opencode PTY system is a separate HTTP/WebSocket feature for interactive sessions. The shell tool is one-shot per invocation. If persistent terminal sessions are needed later, they are a separate concern.

### Shell Selection

Bun's `spawn` handles shell selection naturally when `shell: true` is passed — it uses the system shell. For probe's use case (coding-agent tool execution on macOS primarily), `/bin/zsh` is the default. No need to replicate the full `Shell.acceptable()` logic initially, but **do deny `fish` and `nu`** like opencode does (they are interactive-first shells).

---

## Implementation Sequence

1. **Phase 1** — Basic shell execution with workspace confinement, output capture, and timeout. Follow the `read_file` / `search_code` pattern in `cli.ts:makeGeminiChatTools`. Wire into `ProbeToolName` enum in `tool-menu.ts`.

2. **Phase 2** — Truncation system and output bounds. Reuse the 100KB constant from file reads; add truncation file storage.

3. **Phase 3** — Permission gating (approval flow). Plumb through the `PermissionHandler` so the main loop can prompt between turns.

4. **Phase 4** — Streaming output, abort signal integration, advisory path scanning.

5. **Phase 5** — Tree-sitter AST-walking for granular permission scanning (opencode V1 parity).

---

## Key Files to Modify

| File | Change |
|---|---|
| `packages/runtime/src/cli.ts` | Add shell tool to `makeGeminiChatTools()` |
| `packages/runtime/src/llm/tool.ts` | Possibly extend `ProbeLlmTool` for output bounds config |
| `packages/runtime/src/llm/tool-menu.ts` | Add `"shell"` to `ProbeToolName` enum + `TOOL_CATALOG` entry |
| `packages/runtime/src/permission.ts` | Replace always-allow default with real gating for shell |
| `packages/runtime/src/backends/apple-fm/tools.ts` | Wire real executor (remove noop mock) |
| `packages/runtime/src/backends/apple-fm/blueprint-tools.ts` | Add `shell` to `appleFmToolNameFromProbe` mapping |

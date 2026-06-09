# Probe Write/Edit Tool Parity With OpenCode

Reference: `projects/repos/opencode` (the upstream opencode repo).
Audience: Probe implementors bringing probe's `write_file` and future `edit` tool
to 100% feature parity with opencode V2 (core) and V1 (opencode package) layers.

---

## Current Probe State

Probe now has three file-mutation tools: `write_file`, `edit_file`, and `apply_patch`.

Tool definitions are in `packages/runtime/src/cli.ts` (`makeGeminiChatTools`).
Handler implementations live in `packages/runtime/src/file-mutation.ts`.
Shared workspace path resolution lives in `packages/runtime/src/workspace.ts`.
Permission/approval system lives in `packages/runtime/src/permission.ts`.

### Tools Available

| Tool          | What it does                                          |
|---------------|-------------------------------------------------------|
| `write_file`  | Full-file overwrite with BOM preservation + locking   |
| `edit_file`   | Partial replace via `oldString`/`newString`/`replaceAll?` with BOM, line-ending, stale-content guard, and locking |
| `apply_patch` | Multi-operation patch (add/update/delete) across files |

### What's implemented (P0 + P1)

| Feature | Status |
|---------|--------|
| `edit_file` tool with exact replace | ✅ |
| `apply_patch` tool (add/update/delete) | ✅ |
| Permission/approval gating with diff preview | ✅ |
| BOM detection and preservation | ✅ |
| Line-ending normalization (CRLF/LF) | ✅ |
| Stale-content guard (race detection) | ✅ |
| Per-file locking via Semaphore | ✅ |

### What's still missing (P2 + P3)

- Revalidation after plan resolution (P2)
- LSP diagnostics after write (P2, depends on LSP service)
- Auto-format after write (P2, depends on format service)
- Event publishing for watchers (P3)
- Snapshot/undo metadata (P3)
- Fuzzy correction strategies (P3)
- Structured error and success types (P3)

---

## Target: OpenCode V2 Tools

OpenCode V2 lives in `packages/core/src/tool/`. It has two file-mutation tools:

### V2 `write` (`packages/core/src/tool/write.ts`)

Full-file overwrite with:

- **Location-scoped path resolution** (`LocationMutation.resolve`) —
  relative paths resolve within the active Location; external absolute paths
  require `external_directory` approval. Rejects escape attempts and symlink
  traversal.
- **Permission gating** (`assertPermission`) — user must approve the edit
  action before write executes. External paths get a separate
  `externalDirectoryPermission` approval first.
- **BOM-preserving write** (`files.writeTextPreservingBom`) — reads existing
  file BOM bytes, preserves them on write, emits at most one BOM.
- **Keyed mutex** — `KeyedMutex` per canonical target serializes writes.
- **Revalidation** — immediately before filesystem write, re-verifies the plan
  (identity, canonical path, resource) to detect mid-flight races.
- **Structured success** — returns `{ operation, target, resource, existed }`.

### V2 `edit` (`packages/core/src/tool/edit.ts`)

Partial file edit via exact string replacement with:

- All of the above path resolution, permission gating, and locking.
- **Exact oldString/newString/replaceAll** parameters.
- **Line-ending normalization** — detects the target file's existing line ending
  style (`\n` vs `\r\n`), normalizes both `oldString` and `newString` to match
  before matching.
- **BOM detection and preservation** — splits BOM from content before matching,
  re-joins on write.
- **Occurrence counting** — rejects if `oldString` not found; rejects if
  multiple matches found and `replaceAll !== true`.
- **Stale-content guard** (`writeIfUnchanged`) — under lock, re-reads the file
  and only writes if the bytes are unchanged since initial read. Returns
  `StaleContentError` on mismatch.
- **Diff preview in model output** — returns a unified-diff snippet showing
  removed and added lines.
- **Structured success** — includes a `replacements` count.

### V2 `apply_patch` (`packages/core/src/tool/apply-patch.ts`)

Multi-operation patch (add, update, delete) with upfront plan resolution and
approval, then uninterruptible sequential application.

---

## Target: OpenCode V1 Tools

OpenCode V1 lives in `packages/opencode/src/tool/`. These have richer
integrations that V2 has deferred.

### V1 `write` (`packages/opencode/src/tool/write.ts`)

Full-file overwrite with all V2 features plus:

- **Diff preview before permission approval** — generates a unified diff via
  `createTwoFilesPatch` and shows it in the permission prompt.
- **Auto-format after write** — runs `format.file(filepath)` after writing;
  re-syncs BOM if format changes it.
- **LSP diagnostics after write** — calls `lsp.touchFile(filepath)`, collects
  diagnostics, and reports them in the output (up to 5 files).
- **Event publishing** — publishes `FileSystem.Event.Edited` and
  `Watcher.Event.Updated`.

### V1 `edit` (`packages/opencode/src/tool/edit.ts`)

Partial edit with all V2 features plus:

- **Fuzzy correction strategies** (8 replacers tried in order):
  1. `SimpleReplacer` — exact match.
  2. `LineTrimmedReplacer` — trimmed line matching.
  3. `BlockAnchorReplacer` — first/last line anchor matching with Levenshtein
     similarity threshold.
  4. `WhitespaceNormalizedReplacer` — normalized whitespace.
  5. `IndentationFlexibleReplacer` — ignores indentation.
  6. `EscapeNormalizedReplacer` — unescaped string matching.
  7. `TrimmedBoundaryReplacer` — trimmed boundary matching.
  8. `ContextAwareReplacer` — first/last line context anchors with 50%
     middle-line similarity.
  9. `MultiOccurrenceReplacer` — finds all exact occurrences.
- **Levenshtein distance** for similarity scoring.
- **Disproportionate match detection** — rejects matches that are too large
  relative to oldString.
- **Per-file semaphore** (not `KeyedMutex`) for serialization.
- **Snapshot / undo metadata** — captures `filediff` metadata for undo support.
- **Diff preview in permission** and **diff preview in model output**.
- **Auto-format**, **LSP diagnostics**, **event publishing** (same as V1 write).

---

## Parity Gap Analysis

The gap can be grouped into layers of increasing sophistication.

### Layer 1: `edit` Tool (✅ Implemented)

`edit_file` tool at `packages/runtime/src/file-mutation.ts:editAnyWorkspaceFile`.
Registered in `cli.ts:makeGeminiChatTools`.

Parameters: `path`, `oldString`, `newString`, `replaceAll?` (optional).

Implementation (in `file-mutation.ts`):
1. Resolves path via `resolveWorkspacePath` (from `workspace.ts`).
2. Rejects empty `oldString`, identical strings, zero matches, or multiple
   matches when `replaceAll !== true`.
3. Reads file with BOM detection, normalizes line endings, matches/replaces
   oldString, restores line endings, re-joins BOM, writes back.
4. Wrapped in per-file lock; stale-content guard re-reads and compares bytes
   before final write.
5. Checks permission handler before proceeding.

### Layer 1b: `apply_patch` Tool (✅ Implemented)

`apply_patch` tool at `packages/runtime/src/file-mutation.ts:applyAnyWorkspaceFilePatch`.
Registered in `cli.ts:makeGeminiChatTools`.

Parameters: `patchText` containing structured operations
(`+ADD <path>`, `+UPDATE <path>`, `+DELETE <path>`).

Single permission approval for the entire patch upfront, then applies
operations sequentially. Reports per-operation status.

### Layer 2: Permission / Approval Gating (✅ Implemented)

Module: `packages/runtime/src/permission.ts`.

- `PermissionHandler` interface with `ask(request)` returning
  `"allow" | "deny" | "always"`.
- Module-level handler set via `setPermissionHandler()`.
- Default is `always allow` (for non-interactive mode).
- In interactive chat (`runGeminiInteractiveChat`), a
  `makeInteractivePermissionHandler()` is installed that shows a diff preview
  and prompts the user with `(y=yes, n=no, a=always)`.
- All mutation tools (`write_file`, `edit_file`, `apply_patch`) check
  permission before executing.

### Layer 3: BOM Handling (✅ Implemented)

Functions in `packages/runtime/src/file-mutation.ts`:
- `splitBom(text)` — strips leading BOM bytes, returns `{ bom, text }`.
- `hasUtf8Bom(content)` — checks Uint8Array for BOM signature.
- `joinBom(text, bom)` — re-joins BOM if needed.
- `readFileWithBom(content)` — decodes with BOM awareness.

`write_file`: reads existing file's BOM, preserves it on write.
`edit_file`: reads existing BOM, splits before matching, re-joins after.

### Layer 4: Line-Ending Normalization (✅ Implemented)

Functions in `packages/runtime/src/file-mutation.ts`:
- `normalizeLineEndings(text)` — strips `\r`.
- `detectLineEnding(text)` — detects `\n` vs `\r\n`.
- `convertToLineEnding(text, ending)` — converts to target line ending.

Applied in `editAnyWorkspaceFile`: content and old/new strings are normalized
to `\n` for matching, then result is converted back to the file's detected
line ending before writing.

### Layer 5: Stale-Content Guard (✅ Implemented)

In `editAnyWorkspaceFile`:
1. On read, captures the raw `Uint8Array` of the file content.
2. Before write (under lock), re-reads the file and byte-compares.
3. If bytes differ, fails with "File changed after read. Read it again before editing."

Also in `writeAnyWorkspaceFile`: re-reads the file for BOM detection before
write, so the stale window is under lock.

### Layer 6: Per-File Locking (✅ Implemented)

In `packages/runtime/src/file-mutation.ts`:
- `Map<string, Semaphore.Semaphore>` keyed by canonical absolute path.
- `getFileLock(filePath)` returns or creates a `Semaphore.makeUnsafe(1)`.
- Both `writeAnyWorkspaceFile` and `editAnyWorkspaceFile` wrap the critical
  read-modify-write section in `lock.withLock(...)`.

### Layer 7: Revalidation After Plan Resolution

OpenCode V2 resolves a "plan" (target identity, canonical path, resource) early,
then revalidates it immediately before the filesystem write. This catches
mid-flight races where the file was moved, deleted, or replaced with a symlink.

**What to build:**

1. Resolve the plan early (for permission approval).
2. After approval but before write, re-read the filesystem identity at the same
   path and verify it matches the plan (same dev/ino or canonical realpath).
3. Fail with a clear error if it changed.

Probe's simpler `resolveWorkspacePath` already resolves to an absolute path.
Revalidation means re-doing the resolution + stat check just before writing.

### Layer 8: LSP Diagnostics After Write

After writing or editing a file, OpenCode V1 touches the file with LSP
(`lsp.touchFile(filepath, "document")`), collects diagnostics, and reports
them in the tool output. This gives the model immediate feedback about errors
it introduced.

**What to build:**

1. After a successful write/edit, send a notification to the language server
   about the changed file.
2. Collect diagnostics for that file (and up to N other files that got new
   diagnostics).
3. Include the diagnostics in the tool output so the model sees them.

This depends on Probe having an LSP service. If one does not exist yet, this is
a larger prerequisite.

### Layer 9: Auto-Format After Write

OpenCode V1 runs the project's formatter on the written file (e.g., Prettier,
dprint) and re-syncs the BOM if the formatter changed it.

**What to build:**

1. After writing, detect the applicable formatter for the file type.
2. Run it.
3. Re-check BOM after formatting (formatters may strip it).

Depends on Probe having a format service.

### Layer 10: Event Publishing For Watchers

OpenCode publishes filesystem events so that file watchers, the TUI file tree,
and other UI elements know about changes.

**What to build:**

Publish a `FileSystem.Event.Edited` and `Watcher.Event.Updated` event after
each mutation. Depends on Probe having an event system.

### Layer 11: Snapshot / Undo Metadata

OpenCode V1 captures `filediff` metadata in its snapshot system after each
edit, enabling undo.

**What to build:**

1. Before writing, capture a copy of the old file content and a diff.
2. Store it in a snapshot/undo service keyed by file path.
3. The UI can then offer an undo command that reverts the last edit.

### Layer 12: Fuzzy Correction Strategies

When the model's `oldString` does not match the file exactly (wrong indentation,
extra whitespace, different quoting), OpenCode V1 tries 8 increasingly relaxed
replacer strategies before giving up. This reduces model errors.

**What to build:**

A replacer pipeline:

```typescript
type Replacer = {
  name: string;
  try: (content: string, oldString: string, newString: string) =>
    { replaced: string; count: number } | undefined;
};
```

Try replacers in order:

1. Exact match (simple string replace).
2. Line-trimmed: trim each line before matching.
3. Block-anchor: require first and last lines to match with high Levenshtein
   similarity; inner lines can vary more.
4. Whitespace-normalized: collapse all whitespace runs to single spaces.
5. Indentation-flexible: ignore leading whitespace differences.
6. Escape-normalized: unescape both strings before matching.
7. Trimmed-boundary: trim whitespace from oldString boundaries.
8. Context-aware: require anchor lines at top and bottom with 50% similarity
   on middle lines.
9. Multi-occurrence: if `replaceAll` is false but a unique replacement can be
   inferred from context.

Each replacer must return a similarity score so the system can detect
disproportionate matches (e.g., matching 500 chars when oldString was 10).

### Layer 13: Structured Error Types

OpenCode uses `Schema.TaggedErrorClass` for typed errors like
`StaleContentError`, `TargetExistsError`, `LocationMutation.RevalidationError`,
and `FSUtil.Error`. These are caught by the tool framework and projected into
the `ToolFailure` response with clear messages.

Probe has `ProbeLlmToolFailure` (one class with a `message` string).
Adding typed error variants lets the tool handler give more specific error
messages and lets callers distinguish error types.

### Layer 14: Structured Success Types

OpenCode V2 returns a typed success object (`{ operation, target, resource, existed }`)
rather than a natural-language string. The `toModelOutput` function projects this
into model-readable text. Probe returns `{ path, content: "written to ..." }`.

Moving to structured success types makes the tool output parseable by other
tools and improves the model's understanding of what happened.

### Layer 15: `apply_patch` Multi-Operation Tool

OpenCode V2 has an `apply_patch` tool that accepts a complete patch with add,
update, and delete operations targeting multiple files. It resolves all plans
up front, approves all mutations once, then applies them sequentially without
interruption.

This is a separate tool, not a replacement for `write`/`edit`. Building it
would let the model express multi-file changes in one turn.

---

## Priority Order

| Priority | Layer | Effort | Value | Status |
|----------|-------|--------|-------|--------|
| P0       | 1  — `edit` tool          | small  | high  | ✅ |
| P0       | 1b — `apply_patch` tool   | medium | med   | ✅ |
| P1       | 2  — permission/approval  | medium | high  | ✅ |
| P1       | 3  — BOM handling         | small  | low   | ✅ |
| P1       | 4  — line-ending norm     | small  | med   | ✅ |
| P1       | 5  — stale-content guard  | small  | med   | ✅ |
| P1       | 6  — per-file locking     | small  | med   | ✅ |
| P2       | 7  — revalidation         | small  | med   | ❌ |
| P2       | 8  — LSP diagnostics      | large  | high  | ❌ (depends on LSP service) |
| P2       | 9  — auto-format          | medium | med   | ❌ (depends on format service) |
| P3       | 10 — event publishing     | medium | low   | ❌ (depends on event system) |
| P3       | 11 — snapshot/undo        | medium | low   | ❌ |
| P3       | 12 — fuzzy correction     | large  | med   | ❌ |
| P3       | 13 — structured errors   | small  | low   | ❌ |
| P3       | 14 — structured success  | small  | low   | ❌ |

**P0+P1 completed.** Total effort was about 1 focused session.
Remaining layers (P2+P3) need about 1-2 weeks depending on LSP/format service readiness.

---

## File-Level Checklist

The following checklist enumerates every specific change needed. Check off items
as they are completed.

### New Tool: `edit` (`packages/runtime/src/file-mutation.ts:editAnyWorkspaceFile`)

- [x] Define tool parameters: `path`, `oldString`, `newString`, `replaceAll?`
- [x] Register in the tool list alongside `write_file`
- [x] Resolve path via existing `resolveWorkspacePath`
- [x] Reject empty `oldString` (use `write_file` instead)
- [x] Reject `oldString === newString`
- [x] Reject if `oldString` not found in file
- [x] Reject if multiple matches found and `replaceAll !== true`
- [x] Perform exact text replacement
- [x] Write result back to disk (with BOM + line endings + locking + stale-content guard)
- [x] Return structured result with replacement count

### New Tool: `apply_patch` (`packages/runtime/src/file-mutation.ts:applyAnyWorkspaceFilePatch`)

- [x] Define patch format (add/update/delete operations)
- [x] Resolve all target plans up front
- [x] Single permission approval for all operations
- [x] Sequential uninterruptible application
- [x] Report partial application on failure

### Existing `write_file` Upgrades

- [x] Add BOM preservation (read existing BOM, preserve on write)
- [ ] Add line-ending normalization (low value for write; defer)
- [x] Add per-file locking
- [ ] Add revalidation before write (P2)

### All Mutation Tools (shared infra)

- [x] Build permission/approval service with diff preview (`permission.ts`)
- [x] Build semaphore per canonical target (`file-mutation.ts:getFileLock`)
- [x] Build stale-content guard (`file-mutation.ts:editAnyWorkspaceFile`)
- [ ] Add structured error types (`StaleContentError`, etc.)
- [ ] Add structured success types
- [ ] Build revalidation step (re-check path identity pre-write)
- [ ] Build replacer pipeline with fuzzy correction strategies (P3)

### Integration (depends on other services)

- [ ] LSP service: `touchFile` + collect diagnostics (P2, deps on LSP)
- [ ] Format service: run formatter after write (P2, deps on format)
- [ ] Event system: publish file-change events (P3, deps on events)
- [ ] Snapshot/undo: capture pre-write state (P3)

---

## Appendix: Key Files in Each Repo

### OpenCode V2 Core (`packages/core`)

| File | Purpose |
|------|---------|
| `src/tool/write.ts` | V2 write tool definition + handler |
| `src/tool/edit.ts` | V2 edit tool definition + handler |
| `src/tool/apply-patch.ts` | V2 multi-op patch tool |
| `src/file-mutation.ts` | File mutation service: create/write/writeTextPreservingBom/writeIfUnchanged/remove with locking + revalidation |
| `src/location-mutation.ts` | Plan resolution: path → canonical target + resource + revalidation |
| `src/fs-util.ts` | Filesystem utility service (readFile, writeFile, writeWithDirs, etc.) |
| `src/effect/keyed-mutex.ts` | Async mutex keyed by string |

### OpenCode V1 Opencode (`packages/opencode`)

| File | Purpose |
|------|---------|
| `src/tool/write.ts` | V1 write tool with LSP, format, events, diff preview in permission |
| `src/tool/edit.ts` | V1 edit tool with 8 fuzzy replacers, Levenshtein, snapshots, semaphore locks |
| `src/util/bom.ts` | BOM split/join/read-file/sync-file utilities |
| `src/snapshot/index.ts` | Snapshot/undo metadata storage |

### Probe (`packages/runtime`)

| File | Purpose |
|------|---------|
| `src/cli.ts` | Tool registration in `makeGeminiChatTools` (write_file, edit_file, apply_patch, read_file, list_files, search_code, current_time); interactive chat with permission handler |
| `src/llm/tool.ts` | `ProbeLlmTool` type + `defineProbeLlmTool` helper |
| `src/llm/tool-runtime.ts` | `dispatchProbeLlmTool` — tool execution dispatch |
| `src/file-mutation.ts` | All file mutation logic: `writeAnyWorkspaceFile` (BOM+locking), `editAnyWorkspaceFile` (partial replace with BOM+line endings+stale-content guard+locking), `applyAnyWorkspaceFilePatch`, BOM utils, line-ending utils, diff preview, per-file locking, stale-content guard |
| `src/workspace.ts` | Workspace path resolution: `resolveProbeWorkspaceRoot`, `resolveProbeChatWorkspaceRoot`, `resolveWorkspacePath` |
| `src/permission.ts` | Permission/approval system: `PermissionHandler`, `setPermissionHandler`, `makeInteractivePermissionHandler`, module-level default auto-allow |

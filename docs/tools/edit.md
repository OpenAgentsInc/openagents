# Edit Tool (`edit.patch`)

## Purpose

Apply structured, workspace‑scoped file edits using a compact patch format. Designed for reliability, reviewability, and safe automation in both model‑initiated and user‑initiated flows.

## Name

`edit.patch`

## Summary

- Accepts a single patch payload that can add, update, delete, and rename files.
- Emits an ACP `tool_call` and a result entry; UI shows a summary of changed paths.
- Enforces workspace boundaries and relative paths.

## Arguments

- `patch` (string, required)
  - Patch text in the format described below (Begin/End Patch envelope with file hunks).
- `workspace_root` (string, optional)
  - Absolute path; sets the working directory for resolving relative file paths in the patch.
- `dry_run` (bool, optional; default: false)
  - If true, validate and summarize changes without writing to disk.
- `allow_delete` (bool, optional; default: false)
  - If false, any `Delete File` hunks are rejected.
- `allow_move` (bool, optional; default: true)
  - If false, `Move to` operations are rejected.

## Result

- On success: `{"
ok": true, "summary": "A x, M y, D z, R w"}` with additional metadata reserved for future use.
- On dry run: same structure with `"ok": true` and no disk changes.
- On failure: `{"
ok": false, "error": "..."}` with a concise message.

## Patch Format

A stripped‑down, file‑oriented diff designed to be easy to parse and safe to apply.

Envelope
```
*** Begin Patch
[ one or more file sections ]
*** End Patch
```

File operations (each section starts with one header):
- `*** Add File: <path>` — create a new file; every following line is prefixed with `+`.
- `*** Delete File: <path>` — delete an existing file; no additional lines follow.
- `*** Update File: <path>` — patch an existing file in place; optionally followed by a move.
  - Optional rename: `*** Move to: <new path>` on the next line.
  - Then one or more hunks.

Hunks
- Start with `@@` or `@@ <context>` (context is a free‑form hint like a function name or class header).
- Then one or more hunk lines, each prefixed by one of:
  - ` ` (space) — context line
  - `-` — line to remove
  - `+` — line to add
- Optional terminator for end‑of‑file edits: `*** End of File`

Grammar
```
Patch := Begin { FileOp } End
Begin := "*** Begin Patch" NEWLINE
End   := "*** End Patch" NEWLINE
FileOp := AddFile | DeleteFile | UpdateFile
AddFile := "*** Add File: " path NEWLINE { "+" line NEWLINE }
DeleteFile := "*** Delete File: " path NEWLINE
UpdateFile := "*** Update File: " path NEWLINE [ MoveTo ] { Hunk }
MoveTo := "*** Move to: " newPath NEWLINE
Hunk := "@@" [ header ] NEWLINE { HunkLine } [ "*** End of File" NEWLINE ]
HunkLine := (" " | "-" | "+") text NEWLINE
```

Constraints
- Paths must be relative to the workspace; absolute paths are rejected.
- All edits must resolve within `workspace_root` (or the session workspace).
- Files are written with UTF‑8 encoding and a trailing newline where appropriate.

## Safety & Guardrails

- Workspace boundary enforcement: no path may escape the workspace after resolution.
- Deletes require `allow_delete: true`.
- Moves can be disabled via `allow_move: false`.
- Reasonable caps on total changed files/bytes may be applied by the runtime.
- Clear, single‑line error messages for invalid patches or path violations.

## Examples

1) Add and update a file
```json
{
  "tool": "edit.patch",
  "arguments": {
    "patch": "*** Begin Patch\n*** Add File: docs/notes.txt\n+First note\n*** Update File: README.md\n@@\n-Old heading\n+New heading\n*** End Patch\n"
  }
}
```

2) Rename a file and change content
```json
{
  "tool": "edit.patch",
  "arguments": {
    "patch": "*** Begin Patch\n*** Update File: src/old.swift\n*** Move to: src/new.swift\n@@ func greet()\n-print(\"Hi\")\n+print(\"Hello\")\n*** End Patch\n"
  }
}
```

3) Deletion with explicit permission
```json
{
  "tool": "edit.patch",
  "arguments": {
    "allow_delete": true,
    "patch": "*** Begin Patch\n*** Delete File: tmp/obsolete.txt\n*** End Patch\n"
  }
}
```

## UI Integration

- The chat timeline shows an `edit.patch` tool call with a compact summary (e.g., changed file count).
- Tapping opens a detail sheet with the original patch and the result summary.

## Implementation Notes

- Under the hood, the runtime validates the envelope and hunks, resolves paths against the workspace, and applies changes atomically per file.
- When running on desktop, the system may use a hardened apply‑patch routine optimized for common cases and tolerant of trailing newline differences.
- On `dry_run`, the runtime parses and validates the patch and returns what would change without writing.


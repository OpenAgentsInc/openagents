# File Tool System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #4 from the Bun/Effect terminal-agent systems list. It covers
file read, search, write, patch/edit, notebook edit, large-file handling,
binary/media handling, path normalization, freshness checks, and workspace
boundary enforcement.

## Target

The file tool system should let an agent inspect and modify a workspace while
making every file operation explicit, bounded, reversible where possible, and
auditable.

The system should treat file reads, searches, writes, and edits as separate
capabilities because they carry different authority and state requirements.

## User-Visible Capability

The user should see:

- File reads with line numbers, range controls, and clear truncation.
- Searches with bounded result counts and pagination.
- Glob/find results that default to the current workspace and respect ignore
  policy.
- Edits that show a structured diff before or after application.
- Writes that distinguish create from update.
- Errors that explain stale reads, missing files, denied paths, large files,
  binary files, notebooks, or unsafe paths.
- Receipts for changed files, line counts, diffs, and artifact refs.

## Core Design

Define a `FileToolService` and separate tools for read, search, glob, write,
edit, notebook edit, and binary/media reads.

Suggested service boundary:

```ts
interface FileToolService {
  read(request: FileReadRequest): Effect.Effect<FileReadResult, FileToolError>
  search(request: FileSearchRequest): Effect.Effect<FileSearchResult, FileToolError>
  write(request: FileWriteRequest): Effect.Effect<FileWriteReceipt, FileToolError>
  edit(request: FileEditRequest): Effect.Effect<FileEditReceipt, FileToolError>
  snapshot(path: WorkspacePath): Effect.Effect<FileSnapshot, FileToolError>
}
```

The model-facing tools should call this service. Direct file-system access
should stay inside the service and its scoped dependencies.

## Durable Shape

Persist file operations as receipts:

- `FileReadReceipt`: path, range, content hash, mtime, partial/full flag,
  token estimate, redaction class.
- `FileSearchReceipt`: query, root, include/exclude rules, result count,
  truncation, offset, files.
- `FileWriteReceipt`: path, create/update, previous hash/mtime, new hash/mtime,
  structured patch, git diff ref.
- `FileEditReceipt`: path, old string or patch intent, match count, structured
  patch, line endings, encoding, previous/new hash, git diff ref.
- `FileArtifactRef`: screenshot, image, PDF page extraction, raw output file,
  or large content preview.

Do not treat raw file content as the durable public record. Public records
should use hashes, diffs, summaries, and redacted artifact refs.

## Freshness Model

Mutating file tools should require a current read snapshot for existing files.

The snapshot should record:

- Canonical path.
- Content hash or mtime.
- Whether the read was partial.
- Range read, if any.
- Encoding and line ending metadata when needed.

Writes or edits should fail or ask when:

- The file has not been read.
- The file was only partially read.
- The file changed since read.
- The target is a notebook but a text editor was requested.
- The target is too large for safe in-memory edit.
- The target path is denied or outside the allowed workspace.

New file creation can be allowed without a prior read when the parent path and
permission policy allow creation.

## Path And Workspace Rules

All file tools should normalize paths before validation:

- Expand tilde only for the current user.
- Resolve relative paths against the active workspace.
- Canonicalize case where the platform is case-insensitive.
- Resolve symlinks for permission checks.
- Validate all path representations that may reach the same target.
- Reject path traversal and ambiguous UNC/network paths unless a policy
  explicitly allows them.
- Treat deny rules as higher priority than allow rules.
- Keep read and edit permissions separate.

Dangerous files and directories should always require explicit approval or be
denied, even inside the workspace. Examples include VCS internals, editor
settings that execute commands, shell startup files, agent config, and MCP
config.

## Read/Search Behavior

Read tools:

- Bound output by line range and token estimate.
- Refuse device paths that can block or produce infinite data.
- Detect binary files and use specialized media handling.
- Support image metadata and bounded image payloads where the model can use
  them.
- Support PDFs through page ranges and extraction limits.
- Record read freshness.

Search tools:

- Prefer structured search APIs over shelling out through generic bash.
- Exclude VCS directories by default.
- Respect ignore patterns and permission deny rules.
- Default to bounded results with explicit pagination.
- Return relative paths under the workspace to save tokens.

## Write/Edit Behavior

Write tools:

- Create missing files when parent policy allows it.
- Overwrite existing files only after freshness validation.
- Record a structured diff and git diff ref.
- Preserve line endings and encoding when updating.
- Notify editor/diagnostic systems after changes.

Edit tools:

- Match old content exactly unless a structured patch mode is explicitly
  supported.
- Support replace-one and replace-all as separate intents.
- Reject no-op edits.
- Reject ambiguous matches unless the request explicitly says replace-all.
- Require notebook-specific tooling for notebook formats.
- Track file history before mutation when undo/rewind is supported.

## Bun/Effect Boundary

Use:

- `Schema` for read/search/write/edit inputs and receipts.
- `Effect.Service` for `FileToolService`.
- `Layer` for real filesystem, in-memory filesystem, git diff, editor notify,
  and diagnostics.
- `Scope` for temporary files, large-output artifacts, and media extraction.
- `Stream` for large reads, search output, and progress.
- `Ref` for per-session read freshness cache.
- `Cause` for typed filesystem, permission, stale-read, binary, and size
  failures.

## Safety Rules

- Reads are not automatically safe: secrets and denied paths still apply.
- Writes are not allowed just because a path is under the workspace.
- Deny rules beat allow rules.
- Existing-file writes require a current full read snapshot.
- Never follow a symlink into a denied or outside-workspace target silently.
- Do not probe UNC/network paths during validation if probing can leak
  credentials.
- Never index or persist raw content that the public projection cannot show.
- Large outputs should become artifact refs with previews and size metadata.
- Secret scanning should run before writing team/shared memory or config files.

## Tests

Minimum tests:

- Read range records a partial snapshot and blocks subsequent full-file write.
- Full read followed by unchanged write succeeds and records a diff.
- External modification after read blocks write/edit.
- New file creation succeeds when parent path is allowed.
- Deny rule blocks read and write even under allowed workspace.
- Symlink pointing outside workspace is denied.
- Device path read is refused without I/O.
- Binary file uses binary/media handling rather than text decode.
- Search results are bounded and paginated.
- Edit rejects no-op, missing old string, ambiguous match, and notebook misuse.
- Large file edit fails before allocating unsafe memory.
- Public receipt excludes raw private content.

## Decision

The file tool system should be a typed filesystem authority layer with explicit
read freshness, path policy, bounded search/read output, structured mutation
receipts, and artifact refs. Generic shell execution should not be the primary
file editing API.

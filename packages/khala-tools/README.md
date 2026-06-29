# @openagentsinc/khala-tools

Shared native Khala tool runtime contracts.

This package is the provider-neutral layer for Khala Code desktop, Khala CLI,
and later Pylon native-tool fallback execution. It owns typed tool definitions,
registries, invocation/result/event contracts, permission requests, output lanes,
and model-provider adapter helpers.

The local tool runtime does not choose model authority. Hosted OpenAgents cloud
is the default backend, OpenRouter BYOK is opt-in through `OPENROUTER_API_KEY`
or the existing Khala provider-key store, and tests use the mock backend with no
network or spend.

Tool results keep four lanes separate:

- bounded model-visible output
- structured UI payloads
- private local artifact refs
- public-safe summaries

Blueprint policy and Pylon/Probe runtime pieces can materialize scoped tool
menus into this package, but local inspect/coding presets do not require a
Blueprint registry to exist.

## Built-In Tools

### `read`

`read` reads text files only. It accepts a workspace-relative or explicitly
approved absolute/external path plus optional 1-indexed `offset` and `limit`
arguments. It returns numbered, bounded model output and structured UI line
metadata. Image-like files are not read as text; the tool returns a `view_image`
hint instead. Credential-shaped paths, device files, sockets, pipes, directories,
and denied workspace escapes are blocked before bytes are returned.

### `ls`

`ls` lists one directory page. It accepts an optional workspace-relative or
approved absolute/external `path` and optional `limit`. Entries are sorted
case-insensitively, dotfiles are included, directories end in `/`, and empty
directories are successful results. The tool returns bounded model output plus
structured UI entries; denied workspace escapes and credential-shaped directory
paths are blocked before listing.

### `glob`

`glob` finds workspace paths by glob pattern. It accepts `pattern`, optional
search root `path`, and optional `limit`. The default implementation uses
`rg --files --hidden --glob ...` when available so repository ignore rules are
honored, then falls back to a deterministic local walker with simple
`.gitignore` support. Results are POSIX-style workspace-relative paths with
structured match counts and truncation metadata.

### `grep`

`grep` searches text content without spending shell authority. It accepts
`pattern`, optional search root `path`, optional file `glob`, `ignore_case`,
`literal`, `context`, and `limit`. The tool uses ripgrep when available, then
falls back to an ignore-aware local walker. Binary files and credential-shaped
paths are skipped, output is bounded by line, match, byte, and model-text caps,
and no-match searches succeed with structured empty results.

### `edit`

`edit` performs exact text replacements in one file. It accepts `path` plus an
`edits` array of `old_text`/`new_text` pairs, with optional `replace_all`.
Matches are normalized to LF for comparison and written back with the original
line-ending style and BOM preserved. Ambiguous, missing, stale, whole-file, and
credential-path edits fail closed; successful edits produce a bounded diff
preview and request scoped write approval before bytes are changed.

### `write`

`write` creates or intentionally rewrites an entire UTF-8 text file. It accepts
`path`, `content`, and optional overwrite guards `expected_sha256` or
`expected_content`. New files may create parent directories under the workspace.
Existing files require a fresh guard before overwrite, preserve an existing
UTF-8 BOM, produce a structured diff receipt, and request scoped write approval
before bytes are changed.

### `apply_patch`

`apply_patch` applies a constrained freeform patch grammar with `*** Begin
Patch` / `*** End Patch` markers and add, update, or delete file operations.
The whole patch is parsed, paths are resolved, and update hunks are matched
before any side effect. One scoped patch approval covers all touched resources.
V1 application is sequential and explicitly non-atomic; partial failures return
a structured receipt with the number of applied operations.

### `exec_command`

`exec_command` runs a bounded local process through `KhalaProcessService`.
It accepts command text or argv, optional `workdir`, `timeout_ms`,
`cancel_after_ms`, `yield_time_ms`, and `max_output_tokens`. Commands default to
the workspace cwd, require scoped shell approval, request extra approval for
external cwd or network-looking commands, stream stdout/stderr events for
terminal renderers, and return a tail-oriented model preview. The default local
service honestly reports that it does not enforce a sandbox.

### `write_stdin`

`write_stdin` writes to or polls an interactive process session created by
`exec_command` with `tty: true`. It accepts `session_id`, optional `chars`,
`yield_time_ms`, and `max_output_tokens`. Empty input polls recent captured
output. Session ownership is bound to the active Khala session, stdin/stdout/
stderr chunks remain ordered for terminal renderers, and oversized previews use
private artifact refs.

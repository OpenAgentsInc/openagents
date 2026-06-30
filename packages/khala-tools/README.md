# @openagentsinc/khala-tools

Shared native Khala tool runtime contracts.

This package is the provider-neutral layer for Khala Code desktop, Khala CLI,
and later Pylon native-tool fallback execution. It owns typed tool definitions,
registries, invocation/result/event contracts, permission requests, output lanes,
and model-provider adapter helpers.

The local tool runtime does not choose model authority. Hosted OpenAgents cloud
is the only real model backend for Khala Code desktop; request-specific
OpenRouter BYOK metadata may be forwarded to hosted Khala, but the desktop
runtime must not call OpenRouter directly. Tests use the mock backend with no
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
terminal renderers, and return a tail-oriented model preview. On macOS, the
default local service runs commands through a Seatbelt profile that limits writes
to the workspace and reports `sandbox.enforced=true`; other platforms continue
to honestly report that no process sandbox is enforced.

### `write_stdin`

`write_stdin` writes to or polls an interactive process session created by
`exec_command` with `tty: true`. It accepts `session_id`, optional `chars`,
`yield_time_ms`, and `max_output_tokens`. Empty input polls recent captured
output. Session ownership is bound to the active Khala session, stdin/stdout/
stderr chunks remain ordered for terminal renderers, and oversized previews use
private artifact refs.

### `ask_user`

`ask_user` asks the local operator a short question for missing information or
preference input. It accepts `prompt`, optional `choices`, `allow_freeform`,
`non_blocking`, `timeout_ms`, `default_answer`, and `public_safe`. It is
separate from permission prompts and cannot grant filesystem, shell, network, or
credential authority. Non-interactive hosts return typed unavailable results;
hosts that support pending prompts emit `user_input_*` events for desktop and
CLI renderers.

### `todo_write`

`todo_write` replaces the current session-local todo list with ordered items
containing stable `id`, `content`, and one of `pending`, `in_progress`,
`blocked`, `completed`, or `cancelled`. Blocked items require
`blocker_reason`, and only one item may be `in_progress`. Todo state is
in-memory session planning state only: it is not persistent memory, accepted
work, payout authority, a product promise, or a public receipt. Hosts render the
latest list from the structured `todo_list_updated` payload instead of scraping
assistant prose.

### `view_image`

`view_image` reads local PNG, JPEG, GIF, and WebP files through the same read
path and external-directory approval boundary as text reads. It validates image
magic bytes, dimensions, file type, and size before returning a structured UI
preview with dimensions, media type, private artifact ref, and redaction
classification. When the active backend supports vision, hosts can pass through
the private image content part; non-vision backends receive the same safe
preview metadata without image bytes in public summaries.

### `web_fetch`

`web_fetch` is part of the optional `network` preset, not the offline coding
core. It fetches one HTTP(S) URL only after explicit `network` permission, with
bounded redirects, timeout, and response bytes. It rejects caller-supplied
headers, credentials, tokens, and authority flags; use local `read`, `glob`, and
`grep` for repository navigation. Results include source URL, final URL, status,
content type, fetched-at timestamp, redirect metadata, bounded private
model-visible text, and private artifacts for truncated or binary bodies.

### `web_search`

`web_search` is also part of the optional `network` preset. It requires explicit
network permission and a configured host search provider; the default service
fails closed as unconfigured. It accepts `query`, optional `domains`,
`recency_days`, and `limit`, but never provider credentials or request headers.
Results are bounded, source-attributed, timestamped, and kept out of public
summaries except for provider/result-count metadata. Use repository-local search
tools for code navigation.

### Browser Preset

`browser_navigate`, `browser_click`, `browser_type`, `browser_read_text`,
`browser_read_dom`, `browser_wait_for`, and `browser_screenshot` are part of the
optional `browser` preset, not the default coding or inspect profiles. They use
the distinct `browser` authority and require explicit browser-surface approval;
network, shell, filesystem, credential, and owner-local danger authority do not
flow through these tool arguments.

The package owns only the provider-neutral `KhalaBrowserService` contract and
structured result shape. Desktop, CLI, or test hosts may back it with Playwright,
Electrobun WebView control, or another local browser driver without coupling
this package to UI code. If no browser service is configured, the default service
fails closed with `browser_unavailable`.

Browser text, URLs, titles, DOM, typed text, selectors, and screenshots are
treated as private local state. Visible text and DOM outputs are bounded for
model context; truncated visible text spills to a private artifact, raw DOM is
always stored as a private `text/html` artifact, and screenshots are returned as
private image artifacts. Public summaries contain only action/result metadata.

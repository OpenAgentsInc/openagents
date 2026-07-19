# Cursor Agent Client Protocol peer runtime

Status: accepted for implementation. Release claims gated by #8897.

## Decision

OpenAgents controls Cursor Agent CLI as an Agent Client Protocol client by
launching the official `cursor-agent` installation with argument `acp`. This is
not Linux Foundation Agent Communication Protocol and not A2A.

The Cursor peer is composed in `packages/cursor-agent-runtime` over the shared
protocol, stdio transport, and session runtime. Discovery scans PATH entries
named `agent` but accepts only a target whose real basename is `cursor-agent`.
Admission pins the real path, launcher SHA-256, a deterministic digest of every
regular file in the Cursor installation directory, normalized date version,
profile revision, stable schema release, platform, and launch arguments. The
installation-closure digest is recomputed immediately before every spawn, so a
changed sibling Node binary, JavaScript chunk, or native module fails closed. The
child receives only the authorized workspace, `HOME`, and a fixed system PATH
needed by Cursor's launcher. Caller PATH entries are never forwarded.

Authentication selects `cursor_login` only when initialize advertises it and
only after the owner accepts a typed external-browser interaction. No ambient
API key or unconditional Cursor call is inferred. A peer with no advertised
auth method proceeds without authentication. Cancellation returns
`auth_required`, while transport/auth failures remain typed `auth_lost`.

Stable `session/set_mode` and `session/set_config_option` operate only on the
session's advertised IDs and values with local no-op suppression. Cursor model
discovery is the directional `cursor/list_available_models` request. Its
versioned decoder bounds the model/config counts, rejects duplicate IDs, and
recursively validates config options against the stable ACP schema. Results
retain Cursor extension provenance and never enter the global stable protocol.

Inbound `cursor/ask_question`, `cursor/create_plan`, and
`cursor/update_todos` handlers must all be installed before the runtime reports
the extension surface. Reverse filesystem/terminal capabilities similarly
remain false unless the complete broker handler set is installed. Both groups,
model discovery, and Cursor's parameterized-model-picker metadata require
fresh live evidence matching the admitted version and executable digest. The
observed build remains `experimental`. #8897 Must explicitly promote a version
only after the complete named matrix passes.

## Evidence boundary

The observed Darwin arm64 CLI reports
`2026.06.24-00-45-58-9f61de7`. Admission classifies it as `2026.6.24` while
retaining the full report in probe evidence. An initialize-only diagnostic
confirmed wire version 1, `cursor_login`, load support, session listing, and
the official `agent acp` command. It does not prove prompt, load repair,
extensions, cancellation, or cross-platform compatibility. Those claims remain
disabled until the pinned matrix in #8897 records successful artifacts. A
separate checked candidate runner now reproduces authentication, sequential
prompts, session listing, mode change, and stream cancellation through this
production composer, but deliberately has no matrix-promotion authority.

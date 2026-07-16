# Cursor Agent Runtime

This package composes the shared Agent Client Protocol transport and session
runtime into the pinned Cursor Agent CLI peer launched as `agent acp`.

It admits only probe-verified executables, rechecks a deterministic digest of
the complete Cursor installation directory before spawn, negotiates only the advertised
`cursor_login` method through an explicit external-browser interaction, binds
sessions to one real workspace root, and gates Cursor extensions on fresh,
version-and-digest-bound live evidence. Cursor-specific payloads remain in the
versioned `@openagentsinc/agent-client-protocol/extensions/cursor` module.

The runtime does not implement the Linux Foundation Agent Communication
Protocol or A2A.

The opt-in `live-smoke` command is diagnostic only and requires both
`CURSOR_ACP_LIVE_RUNTIME=1` and an explicit absolute disposable workspace in
`CURSOR_ACP_LIVE_WORKSPACE`. It cannot mint compatibility evidence or unlock
feature gates.

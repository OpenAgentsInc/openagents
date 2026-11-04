# Aethyrforge microphone capture IPC v1

The [schema](../../proto/coder/microphone-capture-v1.schema.json) defines the
private page-to-native `forgeVoiceCapture` request/reply bridge. Only the current
loopback main frame may use it. `capture` identifies one browser hold generation.
It is separate from the existing finalized-transcript ticket protocol.

The browser opens microphone tracks through WebKit's permission delegate. That
delegate grants only microphone access to the current local main frame with
macOS authorization. Neither key-window changes nor model startup decide the
site's permission. The initial macOS dialog may take key focus without revoking
the grant. A real macOS refusal still denies access.

After obtaining tracks, every hold sends `prepare`, including holds for which
WebKit reuses a cached site grant. Native preparation requires a ready world,
hidden setup, visible window and foreground application. It starts the local
model service if needed, then arms a fresh one-shot final-transcript ticket.
The reply resolves `true` when ready, or rejects with a local error. Startup
failure cannot become a cached site-permission denial. No samples enter the
voice socket before preparation succeeds.

`cancel` invalidates the matching preparation and its ticket, resolves `true`,
and cannot cancel a newer hold. Leaving the application, hiding/minimizing,
opening setup, reloading or quitting also cancels native preparation. Late
preparation cannot arm a canceled capture. The browser closes all tracks on
release/cancel, including tracks returned after cancellation. Native focus
checks use application activity; AppKit temporarily taking key focus within
the same app does not revoke an authorized hold.

# D1-C: visible Effect Native Desktop conversation over Sync

- Issue: #8670
- Parent track: #8574
- Status: implemented; close after the main receipt is posted
- Depends on: closed #8668 and #8669

## Landed boundary

One renderer adapter maps the Runtime Gateway confirmed thread/message results
(introduced in v2 and preserved unchanged in the v3 superset)
into the existing Effect Native `DesktopThread`/transcript model. No component,
preload method, or IPC channel was added.

At boot, the renderer queries the confirmed catalog exactly once to select its
chat authority for that lifetime:

- live confirmed catalog → authoritative Sync mode;
- not-live/unavailable → explicit existing local-only mode.

The modes never merge. In Sync mode, New chat and composer submit generate
stable public refs, enqueue the canonical mutations, and poll the exact ref.
Only confirmation updates the visible authoritative thread. Timeout reports
that reconciliation is still pending; enqueue is never presented as complete.

## Explicit residual

Canonical `chat_message` currently represents owner messages and has no
assistant role, so this leaf does not invent one. Provider-neutral runtime
events/replies, interrupt/resume, rich composer context, live-account GUI
acceptance, and the equivalent mobile Home switch remain later D1 work.

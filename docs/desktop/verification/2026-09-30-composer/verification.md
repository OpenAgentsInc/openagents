# On-screen desktop composer

Issue #10004 mounts the editing foundation from #10002 in the desktop chat
screen. The local draft survives presentation updates and conversation changes.
A successful acknowledgement clears only the submitted editing revision; a
refusal preserves the draft. The offline fixture answers the same shared typed
chat commands, without files, credentials, or a relay.

The field grows from 56 to 196 points as text wraps, then scrolls internally.
Only visible lines shape and paint. It supports selection, pointer drag capture,
word and line movement, undo and redo, trailing spaces, multiline paste,
Shift+Enter, and IME marked text. Enter does not submit while marked text is
active. The platform IME position includes window zoom. Clipboard subprocesses
run off the UI thread; completion cannot change a newer editing revision.

The composer reimplements Zeron's UI design in Rust Native. No Zeron code or
agent harness was copied. Shared chat commands and state remain in
`openagents-chat`. The live host binding lands in #10006 and #10007; this slice's
live client reports an unsupported host until that binding is available.

## Checks

On pinned Rust 1.97.1:

- Adapter tests: 38 passed. Scripted inputs cover Japanese preedit and commit,
  Enter suppression during composition, focus loss, multiline growth, selection,
  undo and redo, trailing spaces, final newlines, and stale clipboard completion.
- Desktop library, executable, and version tests: 104 passed, with three timing
  tests run separately or retained as ignored measurement tools. The composer
  integration submits through the fixture and preserves text on host refusal.
- Shared chat: 22 passed; the public relay smoke remains opt-in.
- Targeted formatting and Clippy with warnings denied passed.
- The separate phone workspace passes `cargo check --locked`.
- The optimized 1,000-line draft benchmark took 0.003 ms per idle tick and changed
  no presentation revision over 1,000 ticks. At one tick per second, callback
  work accounts for 0.0003% of a CPU. This is callback cost, not a process-wide
  idle CPU measurement.
- The native offline window received text and streamed a reply at 3,110 × 1,960
  pixels. Phase timings and the 3,300-row benchmark are recorded in
  [the latency note](../2026-09-30-chat-latency/verification.md).

## Owner checks

The macOS system Japanese IME check is recorded in the workspace
`NEEDS_OWNER.md`. Scripted Japanese callbacks pass; changing the owner's active
input-source configuration is left to the owner. Process-wide idle CPU with the long draft is recorded in
[the native transcript matrix](../2026-09-30-transcript/verification.md).

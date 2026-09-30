# Saved sessions verification

Code: `4b72d72fd4`. Implements [#10017](https://github.com/OpenAgentsInc/openagents/issues/10017),
part of [#10003](https://github.com/OpenAgentsInc/openagents/issues/10003).

The saved-session reader and continuation design reimplement the public MIT
[Zeron reference](https://github.com/zeronsh/zeron) in Rust Native. Both local
harnesses use the existing `coder-history` adapters. Portable read state,
context selection, prompt bounds, and mutation retries live in
`openagents-chat-app`, which the desktop and phone library compile.

## Checks

Rust 1.97.1 on an M5 Max Mac:

- Shared application: 89 tests passed. Checks cover stale reads, source
  replacement, invalid record boundaries, bounded catalog navigation, UTF-8
  context limits, incorrect acknowledgments, exact uncertain retries, and a
  verified refusal followed by a new project selection.
- Desktop library: 83 tests passed. Desktop binary: 35 passed, four opt-in
  checks ignored. Native reader fixtures open both harnesses at default and
  minimum sizes, show titles and times, preserve an unsent trailing-space
  draft, reject editing in the reader, and leave the original files unchanged.
- Real scratch-host integration: all three checks passed. Saved Codex and
  Claude records become two admitted Coder tasks with exact context prompts.
  Lost acknowledgments and host restarts return the same receipts and binding
  timestamps; changed prompt bytes under the same identity are refused.
  Existing auto-start policy remains off in this fixture. Both tasks are
  cancelled and archived through the broker after verification.
- Host control: five focused tests passed. Connect: 51 unit and integration
  checks passed, two opt-in checks ignored. Phone Rust library: 142 passed,
  19 opt-in checks ignored. Its run preceded the latest main phone build.
- Targeted formatting, strict all-target Clippy for the shared application,
  desktop, host, and connect, strict Clippy for the Coder integration target,
  and the optimized desktop build passed. Desktop checks passed again after
  rebasing the code on `origin/main`.

The reader is captured for Codex at [default](codex-1200.png) and
[minimum](codex-760.png) sizes, and Claude Code at [default](claude-1200.png)
and [minimum](claude-760.png) sizes. The fixture uses temporary sources,
identities, roots, a relay, and a resident host. No owner host, keychain item,
pairing, original session, or persistent chat is modified.

## Performance

The optimized fixture keeps 3,300 rows, 500 chats, the Grid backdrop, a
1,200 × 840 point window at 2×, and 115 samples per active phase. The
[repeat](native-repeat.json) records p50/p99 application work plus CPU frame
submission: scroll 3.324/5.142 ms, streaming 7.602/8.726 ms, and sidebar
5.315/7.453 ms. Idle CPU is 4.44% of one core; peak RSS is 230.5 MiB.
Scroll remains below the 8.3 ms target. Streaming exceeds it by 0.426 ms.

The [first run](native.json) had higher streaming and sidebar times. A process
sample during the repeat showed concurrent Rust compilation using over
11 cores. That is evidence of machine contention, not proof that all of the
variation comes from it. The upcoming visual fidelity pass must repeat this
measurement and preserve the performance budgets. These measurements cover
CPU work and submission, not GPU completion or scanout.

The broader Rust Native shaping suite's pre-existing CoreText corpus digest
mismatch remains recorded in the preceding controls receipt. This issue does
not change shaping or corpus fixtures. Installed-source and live-engine
checks are in [NEEDS_OWNER.md](../../../../NEEDS_OWNER.md); they do not hold
this code-complete issue open.

# Desktop chat controls verification

Code: `f51acd722e`. Related: [#10003](https://github.com/OpenAgentsInc/openagents/issues/10003)
and [#9996](https://github.com/OpenAgentsInc/openagents/issues/9996).

The composer follows the public MIT [Zeron reference](https://github.com/zeronsh/zeron):
`crates/ui/src/composer.rs`, `render_send_button` and the pinned composer toolbar.
This is a Rust Native reimplementation. Attachment and clipboard tools sit on
the left of one composer card. A circular arrow submits; a square stops a
hosted reply. Coder retains Queue, Answer, Stop, and its phase-specific steering
choice. Chat management moves to the header's overflow menu. Commands and
secondary actions use quiet surfaces instead of white rectangles. Icon labels
remain in the semantic tree, and hover tooltips name the utility controls.

## Checks

Rust 1.97.1 on an M5 Max Mac:

- Desktop library: 83 checks passed. Desktop binary: 33 passed, four opt-in
  checks ignored; the additional header-menu check passed. It resolves the
  visible button's current activation, exposes all three management actions at
  minimum size, pins through the existing host path, and preserves an unsent
  draft with trailing spaces.
- Shared chat application: 84 checks passed. Native adapter: 47 passed,
  including immediate trailing-space movement and retained painting.
  Semantic view contract: 18 checks passed.
- Targeted formatting, strict all-target Clippy, and the optimized build passed.
  The broader Rust Native shaping suite reports a pre-existing stale CoreText
  corpus digest (`a993b0190687602a` versus `1f4d7f19f6172e12`). This change does
  not alter that corpus, its layout, or its fixtures; that gate remains a
  separate verification limitation.

Painter captures cover [default size](list-1200.png),
[minimum size](list-760.png), [running task controls](running-760.png), and
[the header menu at minimum size](header-menu-minimum.png). The rebuilt ordinary
fixture was opened and its header menu clicked in the native Mac window. The
empty preview remains open for testing. No owner chat, host, keychain item,
pairing, or persistent session was created.

The optimized foreground fixture uses 3,300 rows, 500 chats, a Grid backdrop,
1,200 × 840 points at 2×, and 115 samples per active phase. Retained
[measurements](native.json) show p50/p99 CPU work plus submission:
scroll 2.509/3.368 ms, streaming 6.716/7.902 ms, and sidebar 5.002/5.450 ms.
Idle CPU was 3.65% of one core; peak RSS was 226.9 MiB. These measure CPU work
and submission, not GPU execution completion.

# Desktop command verification

Issue: [#10013](https://github.com/OpenAgentsInc/openagents/issues/10013).

The command registry, key scopes, and bounded overlay navigation reimplement
Zeron’s command and menu designs in shared Rust. No Zeron source or GPUI runtime
is copied. The shared application crate serves desktop and phone consumers.

## Behavior

Cmd on macOS and Ctrl elsewhere map N to new chat, F to search, K to the
palette, comma to Settings, period to stop receiving a reply, and Tab to the
next chat (Shift reverses it). Shift+F10 and the context-menu key open the chat
menu. Right-clicking a sidebar conversation opens the same menu for that chat.
The palette also lists saved chats. Commands use the existing typed chat
requests; opening an overlay preserves the message draft.

Palette navigation skips disabled entries and wraps within the overlay.
The adapter confines focus to the active panel, consumes outside clicks when
it dismisses the panel, and restores composer focus on Escape. Rename has
separate title, Save, and Cancel focus positions. Archive from the menu asks
for confirmation and retains the conversation. Global commands stay inactive
during IME composition in either editor. Enter during palette composition
cannot activate a command, and the first Escape cancels composition without
closing the panel. Tooltips show the registry’s command descriptions and keys.

## Checks

Rust 1.97.1 on an M5 Max Mac:

- Shared application: 76 tests passed.
- Desktop library: 81 passed; binary: 30 passed, 4 opt-in checks ignored;
  pairing: 3 passed.
- Native adapter: 47 passed, including retained-frame equivalence.
- Phone Rust consumer: 142 passed, 19 opt-in checks ignored.
- Targeted formatting, strict all-target Clippy, and release build passed.

The scripted native fixture checks keyboard actions, IME admission, preserved
drafts, context menus, archive confirmation, outside dismissal, and rename
focus. The [palette](palette.png), [minimum window](palette-minimum.png),
[context menu](context-menu.png), and [archive dialog](archive-dialog.png)
captures were inspected. The minimum window limits the palette to three rows;
larger windows show at most five.

A separate in-memory macOS fixture verified Cmd+K, typing Settings and Enter,
Cmd+N from Settings, right-click menus, and outside-click dismissal through
the actual native window. The fixture was closed. No owner host, real home,
keychain, saved conversation, or clipboard was used. Native Linux and Windows
checks remain in the platform issues and NEEDS_OWNER.md.

## Timing

[The native run](native.json) uses 3,300 transcript rows, 500 chats,
1200 × 840 points, 2× rendering, and the Grid backdrop. Each active phase has
115 samples. These timings include application work and CPU frame submission,
not GPU completion or display latency.

| Phase | p50 | p99 |
| --- | ---: | ---: |
| Scroll | 2.502 ms | 3.824 ms |
| Streaming | 6.572 ms | 7.902 ms |
| Sidebar | 4.990 ms | 5.633 ms |

Idle CPU was 3.55%; peak RSS was 228.5 MiB. The earlier eight-case matrix
remains in the transcript verification.

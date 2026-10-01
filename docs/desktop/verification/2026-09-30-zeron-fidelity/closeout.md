# Zeron chat fidelity closeout

Closeout record for [#10029](https://github.com/OpenAgentsInc/openagents/issues/10029).
The slice-by-slice history is in [verification.md](verification.md). This record
maps each acceptance item to evidence on `main`, compares the finished screen
with the reference, and lists the differences that remain. It does not claim
pixel-for-pixel or 100% fidelity.

Reference: public MIT [zeronsh/zeron at `50cf9e97a32e54a8ea7e1174b80b5adc3b1d2ef4`](https://github.com/zeronsh/zeron/tree/50cf9e97a32e54a8ea7e1174b80b5adc3b1d2ef4),
read from a local read-only checkout. "Reference" values below come from that
revision's `crates/ui/src` (`theme.rs`, `settings.rs`, `popover.rs`,
`shell.rs`, `shell/tabs.rs`, `shell/command_palette.rs`) and its
`docs/screenshot.png`. "Measured" values come from the native layout of the
OpenAgents desktop at the default 1,200 × 840 and minimum 760 × 540 point
windows, asserted by the named checks.

## Acceptance → evidence

| Acceptance item | Evidence on `main` | Status |
| --- | --- | --- |
| Comparable native captures at default and minimum sizes | [Captures](#captures) below: titlebar, transcript, palette scroll, rename, profile, empty chat, menus, at 1,200 × 840 and 760 × 540, several at 1× and 2× | Met |
| Compare geometry, typography, and color to the reference | [Comparison](#geometry-typography-and-color) below; each measured value is asserted by a test | Met |
| Document remaining mismatches honestly | [Remaining mismatches](#remaining-mismatches) below | Met |
| Typing: immediate spaces, multiline, IME | [Typing checks](#typing-checks) below: 12 targeted checks pass | Met |
| Keep the 3,300-row / 500-chat benchmark and behavior checks green | [Native benchmark](#native-benchmark): four complete runs, every steady phase under 8.3 ms at p99; full suites pass | Met, with the cold palette-open frame recorded as a remaining latency miss |
| Commit and push `main` in logical slices | Slices from `4be3f56c39` through `ad40a50bef`, listed in [verification.md](verification.md) and the issue | Met |

## Captures

All captures use temporary offline fixtures; none reach an owner host.

| Surface | Default 1,200 × 840 | Minimum 760 × 540 |
| --- | --- | --- |
| Titlebar and empty conversation | [1×](titlebar-1200x840-1x.png) | [2×](titlebar-760x540-2x.png) |
| Transcript: Markdown, inline code, lists, code blocks | [1×](transcript-1200x840-1x.png), [2×](transcript-1200x840-2x.png) | [1×](transcript-760x540-1x.png), [2×](transcript-760x540-2x.png) |
| Command palette, scrolled 40 points | [1×](palette-scroll-1200x840-1x.png), [2×](palette-scroll-1200x840-2x.png) | [1×](palette-scroll-760x540-1x.png), [2×](palette-scroll-760x540-2x.png) |
| Rename dialog over the chat | [1×](rename-1200x840-1x.png), [2×](rename-1200x840-2x.png) | [1×](rename-760x540-1x.png), [2×](rename-760x540-2x.png) |
| Profile footer and menu | [1×](profile-1200x840-1x.png), [2×](profile-1200x840-2x.png) | [1×](profile-760x540-1x.png), [2×](profile-760x540-2x.png) |
| Centered empty composer | [default](empty-chat-1200.png) | [minimum](empty-chat-760.png) |
| Palette history rows | [default](palette-history-1200.png) | [minimum](palette-history-760.png) |
| Chat menu, context menu, jump pill | [jump pill](jump-pill-1200.png) | [chat menu](authored-inks-chat-menu-760.png), [jump pill](jump-pill-760.png), [context menu](solar-context-menu.png) |

Captures from earlier slices (for example `list-1200.png`, `header-list-*.png`)
remain as history and predate the final titlebar.

## Geometry, typography, and color

| Element | Reference | Measured | Check | Result |
| --- | --- | --- | --- | --- |
| Titlebar height; control center | 38; 21 (`TITLEBAR_HEIGHT`, `TITLEBAR_TOP_PAD` 4) | 38; 21 | `native_titlebar_preserves_controls_and_docked_composer_in_both_modes` | Match |
| Cluster start (macOS windowed / fullscreen / other) | 88 / 12 / 10 | 88 / 12 / 10 | `titlebar_matches_zeron_controls_and_steps_through_visited_chats` | Match |
| Sidebar toggle, Back, Forward, plus | 24-point controls; groups 8 apart, history 2 apart | x = start, +32, +58, +90; 24 × 24 | same | Match |
| Control icons | Solar 16 points, muted; disabled history 35% | Solar 16, muted; disabled 35% (alpha 89) | `cluster_button` in `chrome.rs` | Match |
| Title position | max(sidebar + 16, controls + 12) | 272 at the 256-point sidebar | same | Match |
| Title type | 12 medium, text at 85% | 12 medium, alpha 217 | same | Match |
| Target tag | 12, muted at 50%, "project @ device" | 12, muted at 50%, project only | — | Partial |
| Trailing header button | 28, radius 6, 16-point glyph, 11% hover | 28, radius 6, 16, 11% | same | Match |
| Sidebar width min / default / max | 224 / 256 / 400 (`settings.rs`) | 224 / 256 / 400 | `chrome.rs` constants | Match |
| Sidebar row | 45, context 11/16, title 13/17 | 45, 11/16, 13/17 | `sidebar_context_precedes_the_title_without_wrapping_the_row` | Match |
| Canvas / shell / dialog fill | #060606 / #0d0d0d / #101010 | #060606 / #0d0d0d / #101010 | `visual.rs`; palette panel | Match |
| Text / muted | neutral 0.922 / 0.708 (#e5e5e5 / #a3a3a3) | #e5e5e5 / #a3a3a3 | earlier slice checks | Match |
| Hover and selection wash | white 92% L at 11% | rgba(235, 235, 235, 28) | palette and titlebar checks | Match |
| Hairlines | 8% borders, 6% palette rules | 8%, 6% | keycap slice checks | Match |
| Body and reading band | 14/22 Geist, 736-point band | 14/22 Geist, 736 | shared visual geometry tests | Match |
| User bubble | radius 16, max 80% width | 16, 80% | shared visual geometry tests | Match |
| Composer | compact 49, expanded min 120, 14/22.75, Send 28, radius 26 | same | `composer_wraps_and_collapses_without_losing_text_or_clipping_controls` | Match |
| Markdown headings / code | 19/27 … 14/22 semibold; code 12.5/18, 28 header | same | Markdown dimension checks | Match |
| Palette card | 560 wide, radius 16, #101010, 1-point border | 560, 16, #101010, border | `keys_palette_context_menu_and_modal_admission_share_one_registry` | Match |
| Palette scrim | black at 35% | alpha 89 (34.9%) | overlay layout | Match |
| Palette header / footer | 44 minimum, 16 insets, 14-point search; 7/16 footer, 12 gap | same | keycap slice checks | Match |
| Palette results height | clamp(height − 180, 100, 360) | 360 at both windows | `palette_scrolls_its_reference_viewport_without_moving_rows_on_hover` | Match |
| Palette list | 8 insets in the scroll, 2 gap, 30 action / 45 history rows, radius 10 / 8 | same; content height asserted exactly | same | Match |
| Palette section rule | 8 + 1 + 8 inside the first history row | 2 + 15 + 2 between rows, rule 10 below | same | Match |
| Palette history limit | 30 after filtering | 30 | same | Match |
| Palette scroll | pixel scroll; keyboard reveals the row; hover never scrolls | wheel by points; least-movement reveal; hover never scrolls | same | Match |
| Palette edge fade | 18-point shader alpha ramp | 18 one-point bands of the card color | `a_viewport_clips_scrolls_fades_and_bounds_its_rows` | Approximate |
| Rename dialog | 360 card, 20 insets, 16 radius, 15 semibold title | same | `rename_dialog_keeps_the_chat_mounted_and_preserves_the_draft` | Match |
| Menu rows | 13 regular, 16 icons, 10 gap, radius 7 | same | menu fixtures | Match |

## Remaining mismatches

These are known differences, kept intentionally or left for later work:

1. **Frost and shadows.** Zeron blurs the window and draws frosted palette,
   menu, and composer cards with shadows. OpenAgents paints opaque #101010
   cards with no blur or shadow.
2. **Palette edge fade** is quantized into one-point bands of the card color,
   not a per-pixel shader ramp; at 2× each band covers two pixels.
3. **Palette contents.** OpenAgents keeps its own command registry (Search
   chats, Settings, Stop, Rename, Pin, Archive, Restore, conversations) rather
   than Zeron's New chat / New project / Open settings / theme actions. The empty
   state still reads "No matching commands." instead of Zeron's "No results"
   with a hint, because user-facing copy was not changed.
4. **Titlebar.** The plus stays visible on the blank canvas and on non-chat
   pages, so New chat remains one visible, accessible control everywhere;
   Zeron hides it there. The target tag shows the project only, without
   "@ device", and nothing for chats outside a project. There are no
   side-chat, fork, project-actions, files, or changes-pane controls; the
   trailing slot holds OpenAgents' chat menu. The harness brand icon before
   the title is absent. Back and Forward reuse visited pages within this
   window session; there is no keyboard shortcut for them yet.
5. **Composer chrome.** The model badge ("Fable 5 High"), the "Local checkout"
   project footer, the branch selector, and the "Do anything…" placeholder are
   not reproduced; OpenAgents keeps its Coder policy, attachments, and copy.
6. **Activity rows.** Zeron's collapsed activity summary ("Ran 4 commands ·
   read 1 file …") and its transcript outline rail are not reproduced;
   OpenAgents keeps its own tool rows.
7. **Syntax colors in captures.** Highlighting is applied by a background
   worker, so fixture captures taken immediately show code before colors
   arrive. The palette and UTF-8 ranges are checked separately.
8. **Motion.** Zeron's popup, hover-fade, and titlebar tweens are not animated.
9. **Native latency.** The cold first palette frame repaints and uploads the
   whole window to apply the scrim: 13.380, 7.927, and 10.994 ms at 1,200 ×
   840 at 2×, so two of three runs miss 8.3 ms. Moving the scrim to the GPU
   compositor would remove the full repaint; it is not done here.

## Typing checks

All pass on this Mac with the merged code:

- Immediate and trailing spaces: `each_trailing_space_moves_the_caret_and_remains_selectable`
  (adapter), `a_delayed_task_acknowledgment_preserves_edits_and_a_refusal_keeps_the_draft`
  (a leading space appended immediately), `a_running_task_offers_queue_stop_and_steer_and_retries_clear_only_the_acknowledged_draft`,
  `empty_chat_centers_the_draft_and_a_real_reply_docks_it`.
- Multiline: `scripted_editing_preserves_composition_and_grows_multiline_input`,
  `multiline_paste_remains_a_single_undo_step_through_the_adapter`,
  `composer_wraps_and_collapses_without_losing_text_or_clipping_controls`.
- IME and marked text: `rerenders_preserve_selection_undo_and_marked_input_without_refocusing`,
  `transcript_click_cancels_marked_composer_text`,
  `menu_rows_stay_visible_through_text_ime_and_idle_ticks`,
  `rename_dialog_keeps_the_chat_mounted_and_preserves_the_draft`,
  `the_header_menu_keeps_management_actions_reachable_and_preserves_the_draft`.

## Native benchmark

Optimized build of `ad40a50bef`: 3,300 rows, 500 chats, the Grid backdrop, at 2×,
with no other OpenAgents preview window open. Times are CPU work plus frame
submission, including surface acquisition; they do not measure GPU completion
or display scanout. Each active phase keeps 115 samples.

| Phase p50 / p99 (ms) | [Run 1](native-closeout.json) | [Run 2](native-closeout-repeat1.json) | [Run 3](native-closeout-repeat2.json) | [Minimum window](native-closeout-minimum.json) |
| --- | --- | --- | --- | --- |
| Scroll | 4.755 / 6.137 | 4.378 / 4.964 | 4.713 / 4.972 | 2.424 / 2.742 |
| Streaming | 4.261 / 4.897 | 4.166 / 4.841 | 4.571 / 4.885 | 2.908 / 3.098 |
| Sidebar | 3.626 / 4.387 | 3.251 / 4.623 | 2.898 / 3.526 | 3.426 / 3.622 |
| Composer | 2.898 / 6.255 | 2.443 / 5.730 | 1.826 / 3.808 | 2.630 / 5.239 |
| Commands (filtering) | 5.070 / 5.803 | 2.689 / 3.096 | 3.174 / 4.574 | 5.215 / 6.064 |
| Chat-menu selection | 1.963 / 2.461 | 1.900 / 2.351 | 1.884 / 2.738 | 2.415 / 2.839 |
| Acquisition p99, every phase | ≤ 0.065 | ≤ 0.067 | ≤ 0.065 | ≤ 0.071 |
| First palette frame | 13.380 | 7.927 | 10.994 | 7.875 |
| First chat-menu frame | 8.362 | 5.434 | 5.587 | 5.286 |
| Idle CPU, one core | 2.49% | 3.78% | 2.67% | 3.77% |
| Peak RSS | 215.3 MiB | 207.9 MiB | 209.8 MiB | 186.3 MiB |

Every steady phase is below 8.3 ms at p99 in all four runs. The acquisition
stalls of 10–24 ms recorded in earlier runs (`native-authored-inks.json`,
`native-popup-pointer.json`, `native-paint-before-acquire*.json`) do not recur:
acquisition never exceeds 0.071 ms here. Those runs had scratch preview windows
open or other worktrees compiling; that circumstance is the likely cause, but
these runs do not prove it. The remaining miss is the cold first palette frame:
full damage (4,032,000 pixels) costs 4.4–7.6 ms of painting and 1.9–2.9 ms of
upload. The chat-menu opening also shows full damage because the benchmark
opens it as the palette's scrim disappears.

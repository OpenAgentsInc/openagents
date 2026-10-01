# Desktop chat polish: Verse page, filter, placeholder, engines (2026-10-01)

Issues: [#10071](https://github.com/OpenAgentsInc/openagents/issues/10071)
(no Verse behind chat; Verse is its own page) and
[#10072](https://github.com/OpenAgentsInc/openagents/issues/10072) (filter
threshold, empty-chat placeholder, engines in the sidebar). The owner
reported both from the Mac build at `1ebd5e20f7`.

## What changed

| Request | Now | Test |
| --- | --- | --- |
| No Grid behind chat | `grid::Layer` loads, connects, and draws nothing unless the Verse page is open. On every other page it reports the plain background (`dim 1`, smallest texture) and asks for no frames. | `the_world_loads_only_on_the_verse_page_and_is_released_when_left`, `without_a_world_the_verse_page_stays_plain` (`src/grid.rs`) |
| Verse is its own page | A **Verse** button sits in the sidebar footer between the Local profile and Settings. It uses the same 28-point icon-button style as Settings. Opening the page creates the spectator; leaving it drops the spectator, its relay connection, and its GPU world, and stops Play. Watch, Play, and Reduce motion behave as before on that page. The page title and profile-menu entry say "Verse". | `the_verse_opens_from_the_footer_beside_settings` (`src/chrome.rs`); the footer geometry test still passes unchanged, with Settings flush right |
| Filter sessions… with fewer than 5 chats | Hidden until there are 5 saved chats. It also stays visible while a filter is typed. With fewer chats, Cmd/Ctrl+F opens the palette, which searches chats. | `the_filter_shows_once_there_are_enough_chats`; `keys_palette_context_menu_and_modal_admission_share_one_registry` |
| Empty-chat placeholder | The footer now always creates the selected chat's composer field. Without a field, the composer surface painted nothing, not even the placeholder. | `the_placeholder_paints_in_the_centered_and_the_docked_composer` checks the placeholder pixels in both layouts at both sizes and scales, and that typing hides it |
| Engine block out of the transcript | The content header is empty. Each engine is one row above the sidebar footer: name, model (shortened to fit), a 32-point usage bar for the tightest window, and its percent. Hovering a row shows every window with its reset time. Clicking opens Settings > Coder, which now shows the full #10018 report. | `the_sidebar_shows_each_engine_in_one_row_and_the_transcript_none`, `engines_sit_above_the_footer_and_the_filter_waits_for_five_chats` |

Copy that described the Grid as "behind the window" has been updated in
Settings' Reduce motion line, `--help`, the README, `docs/desktop/release.md`,
and module docs. INVARIANTS rows 108 and 504 are updated to match.

## Captures (headless, real shell)

The captures were written by the tests with `OPENAGENTS_POLISH_EVIDENCE` set:

- Composer placeholder: `composer-centered-*` and `composer-docked-*`, at
  1200×840 and 760×540, 1x and 2x.
- Sidebar engines with 3 chats and no filter: `sidebar-few-chats-*`.
- With 5 chats the filter appears: `sidebar-five-chats-*`.
- Verse page selected in the footer: `verse-page-*`. This fixture installs no
  GPU world, so the page shows its plain state. The world itself is covered by
  the GPU fixture `playable_grid_gpu_and_native_views_at_both_sizes_and_scales`.
- Full engine report on Settings > Coder: `settings-coder-engines-*`.

## Idle CPU on the chat page

I ran `scripts/benchmark-desktop-chat.py` against an optimized build of this
change on the same Mac. The "verse" cases install the app's real
`grid::Layer`, with its watcher and a refused loopback relay. They stay on the
chat page, as the app does until Verse is opened. The full output is in
[`matrix-summary.json`](matrix-summary.json).

| Case | Idle CPU now | Idle CPU in the [closeout](../2026-09-30-basic-chat-closeout/verification.md) |
| --- | --- | --- |
| default-1x plain / verse | 0.45% / 0.22% | 0.44% / 2.79% |
| default-2x plain / verse | 0.39% / 0.51% | 0.42% / 4.11% |
| minimum-1x plain / verse | 0.22% / 0.30% | 0.32% / 3.00% |
| minimum-2x plain / verse | 0.45% / 0.40% | 0.38% / 2.98% |

With the Verse layer installed, idle CPU on the chat page now matches the
no-Verse numbers. Peak RSS is within about 5 MiB of plain in every case;
before, Verse added 15–22 MiB. Every scroll p99 is below 8.3 ms. Measuring the
Verse page itself on hardware is still an owner check.

## Checks

- `cargo test --locked -p openagents-desktop -p openagents-chat-app`: passes
- `cargo clippy --locked -p openagents-desktop -p openagents-chat-app --all-targets -- -D warnings`: passes
- `cargo fmt --check`: passes

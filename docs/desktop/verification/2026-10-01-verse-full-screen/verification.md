# The Verse page fills the window (#10116)

The Verse page's world fills the content area (right of the sidebar, title
bar to bottom) and resizes with the window. Watch, Play, the status line, and
Full screen lie over it at the top left and right; an open board is a card in
the middle; the key hint is dim at the bottom. The stray "OpenAgents" label
under the page is gone. Full screen (the button, Ctrl+Cmd+F, F11, or the
window's own full-screen control on this page) hides the sidebar and title
bar and lets the world cover the window; Esc closes a board, then releases a
held mouse, then leaves full screen.

## Captures

The real GPU layer (the Verse renderer on a `wgpu` device) composited under
the window's views, offline:

```sh
OPENAGENTS_GRID_EVIDENCE=/tmp/verse cargo test -p openagents-desktop \
  --bin openagents-desktop -- --ignored playable_grid_gpu
```

| Size | Watch | Play |
| --- | --- | --- |
| 1280×800 | [watch](watch-1280x800-1x.png) | [play](play-1280x800-1x.png) |
| 1920×1080 | [watch](watch-1920x1080-1x.png) | [play](play-1920x1080-1x.png) |
| 760×540 (narrow) | [watch](watch-760x540-1x.png) | [play](play-760x540-1x.png) |
| Full screen | [1280×800 watch](full-watch-1280x800-1x.png) | [1920×1080 play](full-play-1920x1080-1x.png) |

Boards over the world: [Gym, 1280×800](gym-1280x800-1x.png),
[Results, narrow](results-760x540-1x.png).

## Tests

- `the_verse_fills_its_pane_and_goes_full_screen` (shell): the page's bounds
  at 1280×800, 1920×1080 and 760×540; the controls' places; the toggle,
  Ctrl+Cmd+F, F11, Esc, the window's own full screen, and leaving the page;
  Esc order in Play (mouse first, then full screen).
- `grid_play_owns_input_and_releases_it_on_escape_focus_and_departure`:
  capture and release on Esc, focus loss, modals, and departure, unchanged.
- `the_world_loads_only_on_the_verse_page_and_is_released_when_left` (grid):
  the layer draws into the page's node, or the whole window in full screen.
- `a_gpu_surface_stays_inside_nested_scroll_clips` (rust-native-desktop):
  `Scene::backdrop_rect` names a surface first, then a laid-out node.

Not checked here: the native macOS full-screen animation on a real window
(the window's own transition is reported through `fullscreen_changed`).

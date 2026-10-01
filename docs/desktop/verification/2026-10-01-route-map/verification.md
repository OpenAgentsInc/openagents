# Verification: the route map (#10085)

Checked on `main` on 2026-10-01 on an Apple M5 Max (macOS), for
[#10085](https://github.com/OpenAgentsInc/openagents/issues/10085). Design:
[docs/desktop/route-map.md](../../route-map.md).

## Captures

Written by `openagents-desktop --capture DIR` (`capture_map` in
`crates/openagents-desktop/src/main.rs`), headless through
`rust_native_desktop::capture`, against the in-process fake host, from the
committed sources and records:

| File | What it shows |
| --- | --- |
| [map-default.png](map-default.png), [map-default-2x.png](map-default-2x.png) | The Map page at the default 1200×840 points, 1x and 2x, fitted: families, routes, members, the legend, and the Gaps panel |
| [map-minimum.png](map-minimum.png), [map-minimum-2x.png](map-minimum-2x.png) | The minimum 760×540 window, 1x and 2x: the toolbar wraps, the whole map still fits |
| [map-zoomed-in-work-dispatch.png](map-zoomed-in-work-dispatch.png) | Zoomed into `work.dispatch`: its answers, Coder, its numbers (P 95 % · R 100 % · 120 rows), and its details |
| [map-inspector-coder.png](map-inspector-coder.png), [map-inspector-coder-2x.png](map-inspector-coder-2x.png) | Coder selected: its engines and plugins named around it, and the inspector (engines, plugins it can admit, Project map adopted for everyone, members) |
| [map-gaps-filtered.png](map-gaps-filtered.png) | **Gaps only**: nodes without a gap dimmed in place, the Gaps panel first page with "Requests nothing serves" and **Draft a plugin in chat** first |
| [map-zoomed-out.png](map-zoomed-out.png) | Zoomed out past the fit: only the families and the router are named |
| [map-outline.png](map-outline.png) | The outline: every node by kind, name, state, and gaps, for keyboards and screen readers |
| [gate-chat-offer.png](gate-chat-offer.png) | The release gate, live: "show me how you route things" on the desktop answered with **Open the map** (`meta.map.desktop@1`) |
| [gate-gaps.png](gate-gaps.png) | The release gate's Gaps panel after inspecting Coder |

## The release gate

`scripts/release/acceptance.sh --bin-dir … --only route-map,route-map-chat
--no-engines` against debug binaries of `a4ca70b097` and the live chat worker
(`f04b34bad8`), reading no engine login
([summary](gate-summary.md)):

```text
PASS route-map: opened from the footer; zoomed into work.dispatch; inspected Coder; capability.missing is a gap with "Draft a plugin in chat"; released on leaving
PASS route-map-chat: typed routes.map offer on Some("meta.map.desktop@1"); Open the map opened the Map page
```

## Tests

```sh
cargo test -p coder --test route_map_sources          # sources pinned to the router
cargo test -p openagents-chat-app --lib route_map     # graph, gaps, inspector, layout, camera, hit-testing
cargo test -p openagents-desktop route_map            # the page, the window, the offer, AccessKit
cargo test -p openagents-desktop the_map_and_the_verse_open_from_the_footer_beside_settings
cargo test -p openagents-chat route_counts            # local counts, read without keeping turns
cargo test -p coder --lib router::bank                # meta.map only offers the map on the desktop
```

What they cover, against the issue's acceptance:

- **Graph from fixtures**: every router route from `RouteId::ALL` in order
  under its family; answers, knowledge, Coder, engines, plugins, decks, and
  screens from the sources; zero, one, and many plugins with the showcase
  (`every_route_of_the_router_is_on_the_map`,
  `members_and_edges_follow_a_request`, `zero_one_or_many_plugins`).
- **Statuses from records**: Project map Adopted, Code finder and Test
  reader Reproduced, Outline Not packaged; the ladder from records, and a
  foreign publisher's result never counts
  (`the_catalog_plugins_have_their_real_statuses`,
  `the_plugin_ladder_reads_the_records`).
- **Gap rules**: every route and plugin gap kind and engine gaps, each with
  its next step (`route_gaps_from_the_sources`,
  `plugin_gaps_and_their_next_steps`, `a_catalog_plugin_is_tested_in_chat`,
  `engine_readiness_from_this_computer`); the steps become a chat draft, a
  copied command, a GitHub issue, or Settings
  (`next_steps_become_effects`, `a_chat_step_drafts_a_message_unsent`).
- **Inspector content**:
  `the_inspector_shows_why_numbers_and_records`,
  `the_page_views_carry_the_inspector_gaps_and_outline`.
- **Hit-testing and transform math**: `camera_transform_math`,
  `hit_testing`, `a_click_selects_the_node_under_it`,
  `drag_wheel_and_pinch_move_the_camera`,
  `a_double_click_zooms_into_a_node`, `arrow_keys_step_between_nodes`,
  `keyboard_navigation_between_nodes`,
  `positions_hold_across_refreshes_and_rings_do_not_overlap`,
  `level_of_detail_labels`.
- **AccessKit names**: `accessible_names_say_kind_state_and_gaps`,
  `the_surface_is_described_to_screen_readers`, and
  `a_screen_reader_reads_the_map_and_selects_a_node` (the AccessKit tree the
  window hands VoiceOver names every node, and a screen reader's click
  selects Coder).
- **Reduce motion**: `reduce_motion_moves_the_camera_at_once`,
  `keys_reach_the_map_and_reduce_motion_moves_at_once`.
- **Resources**: `the_footer_opens_the_map_and_leaving_releases_it` (built
  on open, dropped on leaving; no surface version on other pages).

## Timing

`cargo test --release -p openagents-desktop --lib paint_timing -- --ignored
--nocapture`, 30 panned frames each, the whole graph:

| Window | Map surface | 1x fitted | 1x zoom 1.6 | 2x fitted | 2x zoom 1.6 |
| --- | --- | --- | --- | --- | --- |
| 1200×840 | 600×700 pt | 0.35 ms | 0.38 ms | 0.96 ms | 0.75 ms |
| 760×540 | 304×400 pt | 0.15 ms | 0.11 ms | 0.40 ms | 0.26 ms |

That is the map surface's paint alone, well inside a 16.7 ms frame; the
window's own layout and upload are on top. The page asks for frames only
while the camera eases (260 ms), and none when it is still or closed.

## Not verified here

- A person's pass at 60 fps in the real window with a trackpad pinch, and a
  live VoiceOver pass over the map: owner checks (`NEEDS_OWNER.md`).
- Linux and Windows windows: the page is the same Rust Native views and
  surface, built and tested on macOS only.
- `records.json` is a committed snapshot of the relay (37 verified results,
  18 releases, 1 adoption); the window doesn't refresh it while it runs.

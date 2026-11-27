# Components

OpenAgents Web's Coax component library, led by the dark-mode **Apps SDK UI** basic kit (`components/skin/apps-sdk.coax` and `components/kit/apps-sdk.coax`) alongside the legacy Mono Frames showcase. Everything renders directly to real browser DOM nodes from compiled Coax WebAssembly (`components/core/tree.coax` + `components/backend/dom.coax`), with 755 bundled SVG icons (`components/core/icons.coax`) and zero WebGPU requirement.

## Apps SDK UI (complete dark-mode catalog)

- **Actions & display**: `as_button`, `as_icon_button`, `as_button_link`, `as_copy_button`, `as_link`, `as_badge`, `as_spinner`, `as_loading_dots`, `as_progress_ring` (`CircularProgress`), `as_progress_bar`, `as_avatar`, `as_avatar_group_open/end`, `as_alert_open/end`, `as_empty_open/end`, `as_image`, `as_shimmer`, `as_kbd`, `as_breadcrumb_*`.
- **Forms**: `as_field_open/end`, `as_input`, `as_textarea`, `as_checkbox`, `as_switch`, `as_segmented` (`as_seg_*`), `as_radio_group_*`, `as_slider`, `as_select_*`, `as_tag_input_*`, and `as_date_input` (DatePicker & DateRangePicker + calendar grid).
- **Overlays**: `as_popover_*`, `as_menu_*`, `as_tooltip_*`, and native modal `as_dialog_*` (`<dialog data-modal="true">`).
- **Content & shell**: `as_card_*`, `as_divider`, `as_tabs_*`, `as_code`, `as_source`, `as_link_card`, `as_step_row`, data tables, prose, `as_sidebar_item`, `as_suggestion_chip`, thread bubbles, and composer.

## Legacy Mono Frames set

Below the Apps SDK UI showcase on `/components`, the monochrome framed chat, settings, listing, work log, and dock components (`components/kit/frames.coax`, `components/skin/mono.coax`) remain live and interactive.

## How it is built

The components are written in Coax (`components/kit/frames.coax`). A component builds nodes in a tree (`components/core/tree.coax`): each node has a stable id, a tag, its text or children, and the frame behind it. The browser backend (`components/backend/dom.coax`) compares each tree with the last one it sent and writes only the changes. A small host (`coax/web/dom-host.js`) applies them and holds no logic. A page that is still sends nothing and does no work.

The look is one skin, `mono` (`components/skin/mono.coax`). It writes the page's whole stylesheet from Coax. There is no colour in it: state is shown by brightness, fill, line style and words.

## The frames

The frames and their motion are [ARWES](https://github.com/arwes/arwes/tree/bdbaa0324900ee978d42036d1304a053c1fe54b5), copyright 2014–2024 Romel Perez, under the [MIT license](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/LICENSE), ported to Coax (`components/core/arwes/`). A frame is an SVG behind its content, with three layers: the ground, the lines and the decorations. The page reports each framed node's size, Coax computes the frame's points for that size, and the browser draws them as paths. A frame assembles when its node first appears: the ground fades in, the lines draw, the decorations arrive.

## The earlier components

The components drawn on the GPU, in four skins, are at [/components-gpu](/components-gpu/index.md). OpenAgents for desktop still uses them.

The source is in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents) under `components/`. The reasons for this set are in `docs/audits/2026-11-11-chat-zero-base-audit.md`, sections 10 and 11.

# Components

OpenAgents Web's Coax component library, led by the dark-mode **Apps SDK UI** basic kit (`components/skin/apps-sdk.coax` and `components/kit/apps-sdk.coax`) alongside the legacy Mono Frames showcase. Everything renders directly to real browser DOM nodes from compiled Coax WebAssembly (`components/core/tree.coax` + `components/backend/dom.coax`), with 755 bundled SVG icons (`components/core/icons.coax`) and zero WebGPU requirement.

## Apps SDK UI basics (dark mode)

- **Buttons & icon buttons**: `as_button` and `as_icon_button` across `AS_SOLID`, `AS_SOFT`, `AS_OUTLINE`, and `AS_GHOST` variants; semantic colors (`AS_PRIMARY`, `AS_SECONDARY`, `AS_INFO`, `AS_SUCCESS`, `AS_WARNING`, `AS_DANGER`, `AS_DISCOVERY`); sizes `sm`/`md`/`lg`/`xl`; optional pill radius, leading icon, disabled, and busy spinner states.
- **Badges & avatars**: `as_badge` (soft, solid, outline, pill, status dot), `as_avatar` (sizes, square, online indicator), and `as_avatar_group_open/end`.
- **Alerts**: `as_alert_open/end` across semantic colors and variants, automatically emitting `role="alert"` on danger banners.
- **Forms**: `as_field_open/end`, `as_input` (outline/soft, pill, leading icon, validation `aria-invalid`, disabled), and `as_textarea`.
- **Selection controls**: `as_switch` (`role="switch"`), `as_checkbox` (`role="checkbox"`), and `as_seg_open/item/end` (`role="radiogroup"`).
- **Surfaces & feedback**: elevated `as_card_open/end`, `as_spinner`, `as_shimmer`, `as_link`, `as_divider`, `as_code` with copy button, `as_empty_open/end`, and dark-mode surface/color token swatches.

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

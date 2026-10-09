# Adopting the Apps SDK UI design language on the web

Written October 8, 2026. This plan moves the whole OpenAgents web UI
(`crates/openagents-web`, served at openagents.com) onto the design language of
OpenAI's [Apps SDK UI](https://github.com/openai/apps-sdk-ui), reimplemented
in Rust. Pages stay server-rendered: Axum and Maud for HTML, HTMX for requests
and server-sent events, Tailwind for utilities, and Alpine.js only for small
browser-side interactions. No React, and no Node in the build.

It also adds a light theme, **Coder Light**, based on Apps SDK UI's light
mode. Coder Noir stays as the dark theme.

## Sources

| Source | What it is | Status |
| --- | --- | --- |
| `~/work/projects/repos/apps-sdk-ui` (`@openai/apps-sdk-ui` 0.2.2, commit `0f00143`, MIT) | The reference design system: React components over CSS modules, Tailwind 4 tokens, 755 icons | Read-only reference |
| `~/work/autopilot4-deprecated/src/apps_sdk_icons.rs` (commit `b94fe7a`, "Add Apps SDK icon component gallery") | All 755 icons already converted to a dependency-free Rust and Maud module (3,043 lines; `AppsSdkIcon { name, svg }`, `apps_sdk_icon_named`) | Pull in (phase 1) |
| `~/work/autopilot4-deprecated/src/ui_components/` and `static/styles/foundations/` | Earlier Maud components and tokens | **Do not reuse.** They follow shadcn and an older `--oa-*` palette, not Apps SDK UI |

Apps SDK UI is MIT licensed. Ported token values, component CSS and icon paths
carry its copyright notice in a `NOTICE` file in the crate that holds them,
as `crates/coder-ui/NOTICE` already does for its sources.

## What Apps SDK UI is made of

- **Three token layers**, all CSS custom properties:
  - `variables-primitive.css` (284 lines): gray, color and alpha ramps. Each
    value is `light-dark(<light>, <dark>)`, for example
    `--gray-0: light-dark(#ffffff, #0d0d0d)`.
  - `variables-semantic.css` (566 lines): about 259 role colors (text,
    surface, border, and background per intent and variant, such as
    `--color-background-danger-soft-hover`), plus type scale, radius, spacing
    and motion. It is declared through Tailwind 4 `@theme`, so the roles are
    also utilities.
  - `variables-components.css` (215 lines): per-component sizes and spacing
    (`--button-size`, `--alert-gutter`, `--avatar-size`, and so on).
- **Theme switching**: a `data-theme="light|dark"` attribute on `<html>` or on
  any element sets `color-scheme`, and `light-dark()` resolves every
  primitive. Tailwind `dark:` and `light:` variants key off the same
  attribute. Themes can nest.
- **Components** (29): Alert, Avatar, Badge, Button and ButtonLink,
  CopyButton, Checkbox, CodeBlock, DatePicker, DateRangePicker, EmptyMessage,
  Icon, Image, Indicator, Input, Markdown, Menu, Popover, RadioGroup,
  SegmentedControl, Select, SelectControl, ShimmerText, Slider, Switch,
  TagInput, Textarea, TextLink, Tooltip, Transition.
- **Styling** lives in plain CSS modules over the tokens, for example
  `Button.module.css` drawing the shape in `::before` and the focus ring in
  `::after`. React only switches class names and wires behavior. This is what
  makes a faithful Rust port practical: the CSS carries over almost unchanged,
  and the React part is replaced by HTML attributes.
- **Behavior from Radix**: Checkbox, Menu, Popover, RadioGroup,
  SegmentedControl, Select, Slider, Switch and Tooltip use Radix for focus,
  keyboard and positioning. Those are the components that need care without
  React (see "Behavior without React").

## Where our web UI is today

- **Rendering**: 136 route registrations in `crates/openagents-web`. Most
  pages build HTML with `format!` and `push_str`, escaping by hand with
  `layout::escape`. Maud is used in only 6 files (`composer.rs`, `demo.rs`,
  `chat_html.rs`, `pages/chat.rs`, `pages/home.rs`, `cloud/work.rs`). The
  Cloud app (`src/cloud/`, roughly 40 modules) is almost all `format!`.
- **Styles**: about 46 KB across 11 files in `static/`: `site.css`,
  `components.css`, `cloud.css`, `composer.css`, `chat-html.css`,
  `demo*.css`, `fonts.css`, and the generated `tailwind.css`.
- **Tailwind**: Tailwind 4 through the pinned standalone CLI
  (`scripts/build-web-tailwind.sh`, no Node). Utilities carry a `tw:` prefix,
  there is no preflight, and colors map to Coder Noir's `--noir-*` variables
  (`static/tailwind.input.css`). The output `static/tailwind.css` is checked
  in.
- **Theme**: Coder Noir, dark only. It is defined twice: as `u32` constants
  in `crates/coder-ui/src/coder_noir.rs` for native and Wasm surfaces, and as
  `--noir-*` CSS variables for the site (`src/palette.rs` maps the white
  ladder). There is no light theme and no `data-theme` switching.
- **Fonts**: Geist for body text and Paper Mono for code (`static/fonts.css`).
- **Scripts**: HTMX and the HTMX SSE extension are vendored under
  `static/vendor`, plus small page scripts (`chat.js`, `ask.js`, `flow.js`,
  and others). No Alpine.
- **Content Security Policy**: strict, with `script-src 'self'` (plus
  `'wasm-unsafe-eval'` on the Wasm pages) and no `'unsafe-eval'`. This matters
  for Alpine.
- **Rust Native components**: `crates/coder-ui` (about 15,500 lines) holds the
  shared component library from #10943. It renders `rust_native::view`
  element trees for native apps and the `/components` Wasm catalog
  (`coder-components-web`). It is a different rendering path from the
  server-rendered site, and it uses Coder Noir.

## Decisions

1. **One Rust component crate for server HTML**: `crates/openagents-ui`. It
   holds the tokens, component CSS, icons and Maud builders. `openagents-web`
   depends on it, and nothing in it depends on Axum. Keeping it separate from
   `coder-ui` keeps the server-HTML path free of the Rust Native view types.
   Both share the same token values (see 5).
2. **Port the CSS, not the React.** The token files and each component's
   `.module.css` are carried over nearly verbatim into
   `crates/openagents-ui/static/` as plain CSS with an `oa-` class prefix
   (`.oa-button`, `.oa-button[data-variant=soft]`). The Maud builders emit
   the same structure and data attributes the React components put on the
   DOM.
3. **Typed builders, not class strings.** Each component is a Rust builder
   that produces Maud `Markup`, for example
   `Button::new("Save").variant(Soft).color(Primary).size(Md).icon(Icon::Check)`.
   Builders own their markup, ARIA attributes and escaping. Pages never write
   component class names by hand. This also removes the `format!` escaping
   risk as pages move over.
4. **Tailwind for layout, tokens for look.** Keep the standalone CLI, the
   `tw:` prefix and no preflight. Point `@theme` at the Apps SDK UI semantic
   tokens instead of `--noir-*`, so `tw:bg-surface`, `tw:text-secondary` and
   similar resolve to the active theme. Components never depend on Tailwind
   utilities, so a page can mix both safely.
5. **Themes**:
   - **Coder Light**: the Apps SDK UI light values, unchanged at first.
   - **Coder Noir**: our current dark theme, expressed in Apps SDK UI's token
     structure. Where Noir defines a role (canvas, surfaces, strokes, content
     ladder, accent), its value wins. Where it defines nothing (intents such as
     danger, caution and discovery, with their soft, outline and ghost states),
     Apps SDK UI's dark value is used.
   - Each primitive becomes `light-dark(<Coder Light>, <Coder Noir>)`, so one
     token file serves both themes, switched by `data-theme`.
   - The token values are generated from one Rust table (extending
     `coder-ui`'s `coder_noir.rs` with a sibling `coder_light.rs`) by a small
     generator binary that writes the CSS. Native and web then cannot drift.
6. **Theme selection**: `data-theme` on `<html>`, defaulting to
   `prefers-color-scheme`. The choice is stored in a first-party cookie read on
   the server, so the first paint has the right theme and there is no flash.
   A small Alpine toggle sets it. Nested `data-theme` lets a single panel, such
   as a terminal, stay dark inside a light page.
7. **Fonts**: keep Geist for body text and Paper Mono for code instead of Apps
   SDK UI's system sans. The type scale (sizes, line heights, weights,
   tracking) comes from Apps SDK UI. Revisit after Coder Light is on screen.
8. **Icons**: copy `apps_sdk_icons.rs` into `openagents-ui` as
   `icons.rs` with its generator. Instead of a runtime name lookup, generate a
   Rust `enum Icon` with one variant per icon, so a misspelled icon fails to
   compile. Render inline SVG with `aria-hidden` and `currentColor`. The
   multi-color brand icons (for example Zendesk) keep their own fills.
   Unused icons are compiled out, since only referenced variants are linked.

## Behavior without React

| Need | Approach |
| --- | --- |
| Requests, partial updates, live updates | HTMX and the vendored SSE extension, as today |
| Small local state: open/closed, selected tab, copy feedback, theme toggle, dismissible alerts | Alpine.js, the **CSP build** (`@alpinejs/csp`) vendored under `static/vendor` with a pinned version and SHA-256, so `script-src 'self'` keeps working with no `'unsafe-eval'`. Components are registered with `Alpine.data(...)` in one checked-in script, not inline expressions |
| Popover, Menu, Select dropdown, Tooltip | The native Popover API (`popover`, `popovertarget`) for show, hide and light-dismiss. Position with CSS anchor positioning where supported, and a small Alpine fallback that places the panel elsewhere. Roving focus and arrow keys from one shared Alpine `menu` component |
| Dialogs | Native `<dialog>` with `showModal()` |
| Checkbox, RadioGroup, Switch | Native `<input>` elements styled to match. A Switch is `<input type="checkbox" role="switch">`. Native inputs give keyboard and form behavior for free |
| SegmentedControl | A radio group styled as segments |
| Slider | `<input type="range">` styled to match |
| Select | Native `<select>` for simple choices. The richer SelectControl (search, multiple selection) becomes a Popover plus Alpine listbox in a later phase |
| DatePicker, DateRangePicker | Native `<input type="date">` first. A custom calendar only if a page needs it |
| Transition, ShimmerText | CSS transitions and keyframes from the source CSS; Alpine `x-transition` where an element enters or leaves |
| Markdown, CodeBlock | Keep our existing Rust Markdown renderer and syntax highlighter, restyled with the tokens |

Each of these works without JavaScript in its basic form: links navigate,
forms submit, and details stay readable. Alpine adds polish rather than being
required to read or act.

## Phased plan

Each phase lands on `main` with `cargo fmt` and the touched crates' tests.

### Phase 0: groundwork

- Create `crates/openagents-ui` with the Apps SDK UI `NOTICE`.
- Port the three token files. Generate them from the Rust table (decision 5),
  with Coder Noir's existing values mapped onto the dark side.
- Add `data-theme` to the document shell (`layout.rs`), the cookie-backed
  theme choice, and `prefers-color-scheme` as the default.
- Point `static/tailwind.input.css` `@theme` at the semantic tokens. Keep the
  `--noir-*` names as aliases until no page uses them.
- Vendor the Alpine CSP build with its checksum. Add a CSP test asserting no
  `'unsafe-eval'`.

### Phase 1: icons and primitives

- Bring in `apps_sdk_icons.rs` as a generated `Icon` enum.
- Build the components with no Radix behavior: Button and ButtonLink,
  CopyButton, TextLink, Badge, Indicator, Avatar, Alert, EmptyMessage, Input,
  Textarea, Checkbox, RadioGroup, Switch, SegmentedControl, Slider, Image,
  CodeBlock, Markdown styling, ShimmerText.
- Add a catalog route, `/ui`, that renders every component in every variant,
  in both themes side by side. It doubles as the visual test surface.

### Phase 2: overlays

- Popover, Menu, Tooltip, Select and SelectControl, and dialogs, on the
  native Popover API, `<dialog>`, and shared Alpine components for keyboard
  focus.
- Tests for focus order, Escape, light-dismiss and screen-reader roles.

### Phase 3: page migration

Move pages from `format!` to Maud with `openagents-ui` components, in this
order. Each step deletes the CSS it replaces from the old `static/*.css`
files.

1. The document shell, header and navigation (`layout.rs`), so every page
   gets the theme and the toggle.
2. Home and the chat composer (`pages/home.rs`, `composer.rs`,
   `chat_html.rs`, `pages/chat.rs`), the most-used surfaces.
3. The Cloud app (`src/cloud/`): workbench, projects, environment panel,
   agents, Verse, billing, retail, team and sales. These are the most
   `format!`-heavy, and they share forms, tables, status badges and
   "Outcome unknown"/retry patterns that should become shared components.
4. Content and remaining pages (`pages/content.rs`, `download.rs`,
   `stats.rs`, `live.rs`, `profile.rs`, and others).
5. The demo pages (`demo.rs`), last, because they reproduce an earlier design
   on purpose.

### Phase 4: native alignment

- Point `coder-ui` (Rust Native, Wasm catalog, desktop and mobile) at the same
  token table so native surfaces also get Coder Light. That is a separate,
  larger change. The web migration does not depend on it.

## Checks

- **Visual**: the `/ui` catalog in both themes, compared with Apps SDK UI's
  Storybook for each component.
- **Contrast**: a test that every text role meets WCAG AA (4.5:1) against its
  surface in both themes, like `palette.rs` already checks for Coder Noir.
- **Behavior**: focus, keyboard and ARIA tests for every interactive
  component, plus a check that each component works without JavaScript in its
  basic form.
- **Security**: CSP tests (no `'unsafe-eval'`, scripts only from `'self'`) and
  escaping tests for every builder that takes user text.
- **Size**: keep the shipped CSS under the current 46 KB plus the token
  files, by deleting old CSS as pages move.

## Open questions

- **Accent color**: Coder Noir's accent is a near-white, neutral `#ededed`.
  Coder Light can use Apps SDK UI's light neutral (black) or a brand accent.
  The default in this plan is neutral black.
- **Fonts**: Geist and Paper Mono, or Apps SDK UI's system sans, for Coder
  Light.
- **Default theme**: follow the system setting (as planned), or always start
  in Coder Noir.

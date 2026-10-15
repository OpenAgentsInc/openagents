# A shadcn converter, and what the OpenAgents UI system is

Status: plan, 2026-10-15. Nothing here is built yet.

The owner asked for two things:

1. A converter that can pull any shadcn/ui component, block or page from
   the shadcn ecosystem and port it to our surfaces, deterministically and
   agentically. It should keep shadcn's component names and hierarchy and
   fit our Apps SDK UI design language as far as it can.
2. A brainstorm of what an overall OpenAgents UI system is, with the
   converter as one part of it.

This doc builds on two existing docs and does not repeat them:

- [`inventory.md`](inventory.md) lists every UI system and platform we
  have today, with a component matrix.
- [`../web/apps-sdk-ui-adoption-plan.md`](../web/apps-sdk-ui-adoption-plan.md)
  explains how `openagents-ui` ports Apps SDK UI to Maud, and covers
  "Behavior without React".

Read those first.

## Summary

- shadcn is a **source-distribution system**, not a package. Every item is a
  JSON document, `registry-item.json`, that inlines its TSX source and
  declares its npm dependencies, its registry dependencies, its CSS
  variables and its CSS. Because of that, a deterministic pull is possible.
- The converter is a Rust tool (`crates/ui-port`, a dev-time binary). It
  works in five steps:
  1. Fetch and resolve an item graph the same way `shadcn add` does.
  2. Parse the TSX with `oxc`.
  3. Lower it to a typed component IR that keeps the shadcn names, slots
     and `cva` variants.
  4. Map Tailwind classes onto our tokens.
  5. Emit Maud builders for `openagents-ui` and `rust_native::Element`
     trees.

  Every item gets a fidelity report.
- An **agentic layer** sits on top. It chooses items through an embedding
  index (no keyword routing), repairs whatever the deterministic pass could
  not map, and has to pass golden screenshots, `copy_guard::assert_plain`
  and accessibility checks.
- **Tokens:** shadcn's roughly 30 semantic variables map onto our
  Apps SDK UI roles in `oa-tokens`. We also publish the mapping back as a
  shadcn theme, so the two systems can talk in both directions.
- **The UI system** has four layers: tokens, primitives, components, and
  blocks/pages. It has three renderer families: web/Maud, Rust Native
  (desktop and mobile), and terminal. We also run our own registry: our
  components published as a shadcn-compatible registry.
- **M0** recreates the section structure of shadcnblocks' Landing Page 5 on
  `/landing`, using MIT shadcn/ui primitives and our own copy. We do not
  copy the paid block. See "Licensing", which is the largest risk.

## 1. The shadcn ecosystem, mechanically

### Items

Every installable unit is a **registry item**, defined by the schema
`https://ui.shadcn.com/schema/registry-item.json`. The fields that matter
for a converter:

| Field | Meaning | What the converter does with it |
| --- | --- | --- |
| `name`, `title`, `description`, `categories`, `meta` | Identity and tags | Stores them as IR metadata and adds them to the selection index |
| `type` | One of `registry:ui`, `component`, `block`, `page`, `hook`, `lib`, `file`, `style`, `theme`, `font`, `base`, `item` | Chooses the lowering path (table below) |
| `dependencies`, `devDependencies` | npm packages, optionally pinned with `@version` | Each package must map to a known **behaviour adapter** (for example `@radix-ui/react-dialog` maps to our Dialog behaviour). An unknown package is a fidelity fallback. |
| `registryDependencies` | Other items. The address can be a bare name (`button`, from the built-in registry), a namespace (`@acme/x`), GitHub (`owner/repo/item#ref`), a URL, or a local path | Resolved recursively into a graph |
| `files[]` | `{ path, type, target?, content }`. Published JSON inlines `content`, the TSX source | Input to the parser |
| `cssVars` | `{ theme, light, dark }` maps of CSS custom properties | Token harmonizer (section 3) |
| `css` | At-rules and selectors (`@layer base`, `@utility`, `@keyframes`) | Lowered into a component stylesheet, or rejected |
| `tailwind` | Deprecated (Tailwind v3 config) | Read only for old items |
| `envVars`, `docs` | `.env` additions and install notes | Recorded in the report. We never write them. |
| `font` | Google font descriptor | Refused unless it names an allowed font (see Constraints) |

Example: `https://ui.shadcn.com/r/styles/new-york-v4/card.json` has
`type: "registry:ui"` and one file with inline TSX. That file exports
`Card`, `CardHeader`, `CardTitle`, `CardDescription`, `CardAction`,
`CardContent` and `CardFooter`. Each one renders a `<div data-slot="card-…">`
whose classes go through `cn(...)`. Those `data-slot` attributes are the
hierarchy we keep.

How each item type is lowered:

| `type` | Lowering |
| --- | --- |
| `registry:ui` | Primitive or component in `openagents-ui` (plus a Rust Native mapping when one exists) |
| `registry:component`, `registry:block` | Composition of existing components, emitted as a Maud function in a page module |
| `registry:page` | A page skeleton for `openagents-web` (route not wired automatically) |
| `registry:hook`, `registry:lib` | Never transpiled. Mapped to a known behaviour adapter or listed as unmapped. `lib/utils` (`cn`) is built in. |
| `registry:theme`, `registry:style`, `registry:base` | Token harmonizer input only |
| `registry:font` | Refused by default |

### Registries and resolution

- A registry is a `registry.json` file with `name`, `homepage` and
  `items[]`, optionally composed with `include`. `shadcn build` flattens it
  into one JSON file per item.
- A project's `components.json` declares `style` (`new-york`), Tailwind
  settings (`css`, `baseColor`, `cssVariables`, `prefix`), `aliases`, and
  `registries`. Each entry under `registries` maps `@namespace` to a URL
  template containing `{name}` and optionally `{style}`. It can also carry
  `headers` and `params`, which may reference `${ENV}` values (this is how
  paid registries authenticate).
- `shadcn add` resolves in five steps:
  1. Fetch the root item.
  2. Recursively fetch its `registryDependencies` from their own registries.
  3. Sort them topologically.
  4. Deduplicate files by target; the last one wins.
  5. Deep-merge `cssVars`, `css`, `tailwind` and `envVars`. Cycles are
     refused.
- `shadcn view` prints an item, and `shadcn search` / `list` query
  registries.
- An official MCP server (`shadcn mcp`) lets agents list, search and
  install from the registries configured in `components.json`.

### Blocks ecosystem

shadcnblocks.com is a large third-party registry under `@shadcnblocks`.
Landing Page 5, for example, installs with
`npx shadcn add @shadcnblocks/page/landing-page5`. It is an "elite"
(paid) page made of these sections, in order:

1. Sticky mega-menu navbar
2. Centred hero with a promo pill and a framed product image
3. Three image tiles with dual actions
4. Split headline with an image tray
5. Testimonials
6. Four-tier pricing
7. Pattern-panel CTA
8. Footer

Other public registries (Origin UI, Magic UI, Aceternity and others) use the
same item format, and shadcn links them from its registry directory. Some
items are built on Base UI or React Aria instead of Radix. The converter
handles that the same way: the npm dependency picks a behaviour adapter.

## 2. The converter

### Constraints it must respect

These are the rules the converter has to keep:

- **No Node and no React in our build.** The converter runs at **dev time**
  and writes Rust source that we review and commit. The production build
  never fetches a registry.
- **Fonts.** Berkeley Mono must never be in the repo or on the web. The
  converter refuses `registry:font` items and `font-*` families that are not
  on our allowed list (our `--font-sans` / `--font-mono` system stacks and
  the vendored Geist under `crates/openagents-web/static/fonts/geist`).
- **Copy.** Placeholder copy from blocks ("Acme Inc", "Lorem ipsum",
  invented testimonials) is replaced, never shipped. Every emitted page has
  to pass `copy_guard::assert_plain`
  (`crates/openagents-web/src/copy_guard.rs`). The voice is "we" for
  OpenAgents.
- **No dead controls.** A shadcn block's `href="#"` buttons and fake forms
  get either a real target or removal. The fidelity report lists every one
  of them.
- **No keyword routing.** Choosing items from a description goes through
  the typed selector in section 2.7, never through string matching.

### 2.1 Pipeline

```
registry item(s) ─fetch/resolve─▶ ItemGraph ─oxc parse─▶ TSX AST
   ─lower─▶ ComponentIR ─class map─▶ StyledIR ─emit─▶ { Maud | rust_native | registry JSON }
                                          └──────▶ FidelityReport
```

| Stage | Module (`crates/ui-port/src/…`) | Deterministic? |
| --- | --- | --- |
| Fetch and resolve | `registry/` reads `components.json`-style config (`ui-port.toml`), uses the same URL templates and `${ENV}` headers, caches by SHA-256 under `target/ui-port/cache/`, and orders the graph topologically | Yes, given a lockfile (`ui-port.lock`: item address to content hash) |
| Parse | `parse/` uses `oxc_parser` and `oxc_ast` (pure Rust, fast, handles TSX). swc would also work; oxc is the smaller dependency. | Yes |
| Lower to IR | `lower/` recognises `function X(props)` and `forwardRef` components, `cva(...)` calls, `cn(...)`, `data-slot`, `asChild`/`Slot`, Radix `*.Root`/`*.Trigger`/`*.Content` parts, `lucide-react` icons, and conditional JSX | Yes. Anything it does not recognise becomes an `Opaque` node, never a guess. |
| Class map | `tw/` tokenises the class string, handles variant prefixes (`hover:`, `md:`, `dark:`, `data-[state=open]:`, `group-*`), and looks up a static table that maps utilities to tokens | Yes |
| Emit | `emit/maud.rs`, `emit/native.rs`, `emit/registry.rs` | Yes. Output goes through `rustfmt` and is byte-stable. |
| Report | `report.rs` writes `FidelityReport` as JSON plus a Markdown summary | Yes |

The command line, kept small:

```sh
ui-port pull button card dialog            # built-in registry
ui-port pull @shadcnblocks/hero45 --plan    # resolve + report only, no emit
ui-port pull ./my-item.json --emit maud,native
ui-port report button                       # print the fidelity report
```

### 2.2 The component IR

The IR is what keeps shadcn's names and hierarchy. A sketch:

```rust
/// One registry item after lowering.
pub struct ItemIr {
    pub address: ItemAddress,          // "@shadcn/card", content hash
    pub kind: ItemKind,                // Ui | Component | Block | Page
    pub components: Vec<ComponentIr>,  // Card, CardHeader, … in source order
    pub css_vars: CssVarSet,           // theme/light/dark, pre-harmonisation
    pub behaviours: Vec<BehaviourRef>, // from npm deps (radix dialog, …)
    pub notes: Vec<FidelityNote>,
}

pub struct ComponentIr {
    pub name: Ident,                   // "CardHeader": kept verbatim
    pub slot: Option<String>,          // data-slot="card-header"
    pub element: HtmlTag,              // div, button, a, …
    pub props: Vec<PropIr>,            // typed: bool, string, enum, children, ReactNode slot
    pub variants: Option<VariantSet>,  // from cva
    pub base_classes: ClassList,
    pub children: Vec<NodeIr>,         // JSX tree with Slot/children placeholders
    pub as_child: bool,                // Radix Slot pattern
    pub behaviour: Option<BehaviourRef>,
}

pub struct VariantSet {                // cva(base, { variants, defaultVariants, compoundVariants })
    pub axes: Vec<VariantAxis>,        // variant: default|destructive|outline|…, size: sm|default|lg|icon
    pub defaults: Vec<(Ident, Ident)>,
    pub compound: Vec<CompoundVariant>,
}

pub enum NodeIr {
    Element { tag: HtmlTag, slot: Option<String>, classes: ClassList, attrs: Vec<AttrIr>, children: Vec<NodeIr> },
    Component { name: Ident, props: Vec<(Ident, ExprIr)>, children: Vec<NodeIr> },
    Children,                          // {children} / {props.children}
    Text(String),                      // literal copy: flagged for replacement in blocks
    Icon(IconRef),                     // lucide name -> our icon set or fallback
    Map { over: ExprIr, item: Box<NodeIr> },   // array.map in blocks
    Cond { test: ExprIr, then: Box<NodeIr>, other: Option<Box<NodeIr>> },
    Opaque { source: String, reason: String }, // anything not understood
}
```

### 2.3 Emitters

**Maud / `openagents-ui` (web).** For each `ComponentIr` the emitter
produces:

- a builder struct with the same name, in the style of the existing
  `Button` (`crates/openagents-ui/src/actions/button.rs`): a typed
  constructor, `cva` axes as Rust enums (`ButtonVariant`, `ButtonSize`), and
  `impl Render`;
- a stylesheet at `static/components/<item>.css`. The bundle picks it up
  with no other edit (see the `lib.rs` docs).

The `data-slot` attributes stay in the HTML, so the DOM hierarchy matches
shadcn's.

Classes are emitted as **component CSS over our tokens**, not as Tailwind
utilities. We have no Tailwind build; `openagents-ui` ships plain CSS in
`@layer components`. So `bg-primary text-primary-foreground hover:bg-primary/90`
lowers to:

```css
[data-slot="button"][data-variant="default"] {
  background: var(--color-background-primary-solid);
  color: var(--color-text-inverse);
}
```

and a matching `:hover` rule that uses the `-hover` role.

**Rust Native.** Only components that have a semantic equivalent map to
`rust_native::Element`:

- Button maps to `Button`.
- Input and Textarea map to `Field`.
- Dialog maps to `Dialog`.
- RadioGroup and ToggleGroup map to `Choice`.

Layout-only wrappers (Card, CardHeader) become `Stack` with a named style.
Everything else is a fidelity gap, and these gaps feed phase 2 of the
inventory doc (growing the `Element` vocabulary). The emitter writes a view
builder function per component in the consuming app crate. It never puts
web-only details into `rust-native`.

**Terminal.** Out of scope. Terminal stays Noir and ratatui. The IR could
target it later.

**Effect Native.** `effect-native` lives in its own repo, and this monorepo
has no Effect Native code (see the inventory). The IR is
renderer-neutral, so an Effect Native emitter (TSX in, Effect Native
component set out) is an optional later target in that repo. It is not
part of this plan's milestones.

### 2.4 Behaviour: Radix to our behaviour layer

shadcn components get focus, keyboard, aria and positioning from Radix (or
Base UI or React Aria). We do not ship React. The adoption plan already
chose native HTML first: `<dialog>`, the Popover API, native inputs, and
small Alpine CSP components. The converter maps npm dependencies to those
adapters:

| shadcn uses | Our behaviour | Status |
| --- | --- | --- |
| `@radix-ui/react-dialog`, `alert-dialog`, `sheet` (`vaul` drawer) | Native `<dialog>` + `showModal()`; sheet = positioned dialog | Dialog exists (`overlays/dialog.rs`); Sheet and Drawer need styling |
| `popover`, `hover-card`, `tooltip` | Popover API + anchor positioning + Alpine fallback | Exists (`overlays/popover.rs`, `tooltip.rs`) |
| `dropdown-menu`, `context-menu`, `menubar` | Shared Alpine `menu` (roving focus, arrows, typeahead) | Menu exists; context menu and menubar are hand-written |
| `select` | Native `<select>`; rich `SelectControl` for search | Exists |
| `checkbox`, `switch`, `radio-group`, `slider`, `toggle` | Native inputs | Exists |
| `tabs`, `accordion`, `collapsible` | `<details>`/`<summary>` for accordion and collapsible; Tabs as an Alpine `tabs` component with `role=tablist` | **Hand-write Tabs** |
| `navigation-menu` (mega menu) | `<nav>` + Popover API per item | **Hand-write** |
| `scroll-area` | Native overflow + styled scrollbars | Map to CSS |
| `cmdk` (Command) | Alpine listbox + filter | **Hand-write** |
| `embla-carousel` | CSS scroll-snap (what `/landing` already does) | Map to CSS |
| `recharts` (Chart) | Server SVG | Out of scope for M0–M2 |
| `react-hook-form`, `zod` (Form) | Server-validated forms + HTMX | Structure only; validation is ours |
| `sonner` (toast) | HTMX out-of-band swap into a live region | **Hand-write** |
| `lucide-react` icons | Our icon set (755 Apps SDK icons); a static name table, with unmapped names in the report | Table to write |

Every behaviour adapter carries a keyboard and aria contract test (role,
`aria-expanded`, focus return, Escape). Those tests are hand-written once,
and every emitted component that uses the adapter inherits them.

### 2.5 Fidelity report

Every pull writes `docs/ui/ports/<item>.md`, generated from
`target/ui-port/<item>.json`:

```rust
pub struct FidelityReport {
    pub item: ItemAddress,
    pub hash: String,
    pub license: LicenseClass,          // Mit | FreeTerms | Paid | Unknown (section 6)
    pub components: Vec<ComponentFidelity>,
    pub score: f32,                     // exact / total mapped units
}
pub struct ComponentFidelity {
    pub name: Ident,
    pub classes: Tally,                 // exact, approximated (nearest token), dropped
    pub behaviours: Vec<(BehaviourRef, Mapping)>, // Exact | Adapter | HandWritten | Missing
    pub icons: Tally,
    pub opaque_nodes: Vec<String>,      // JSX we could not lower
    pub copy_to_replace: Vec<String>,   // literal text in blocks/pages
    pub dead_controls: Vec<String>,     // href="#", onClick with no target
    pub refused: Vec<String>,           // fonts, env vars, remote images
}
```

A port can merge only when two conditions hold:

- `opaque_nodes`, `copy_to_replace` and `dead_controls` are all empty.
- Every `Missing` behaviour has a hand-written adapter or an explicit
  decision.

### 2.6 Keeping shadcn names inside our kit

- **Names are verbatim.** `Card`, `CardHeader`, `CardTitle`,
  `CardDescription`, `CardAction`, `CardContent`, `CardFooter`. Variant
  enums keep shadcn's value names (`default`, `destructive`, `outline`,
  `secondary`, `ghost`, `link`).
- **Where we already have the component**, the shadcn name becomes the
  canonical name or an alias, never a second implementation:

  | shadcn | `openagents-ui` today |
  | --- | --- |
  | Button | `actions::Button` (map the variants onto `ButtonVariant`) |
  | Badge | `actions::Badge` |
  | Alert | `actions::Alert` |
  | Avatar | `actions::Avatar` |
  | Input, Textarea | `forms::Input`, `forms::Textarea` |
  | Dialog, Popover, Tooltip, DropdownMenu | `overlays::Dialog`, `overlays::Popover`, `overlays::Tooltip`, `overlays::Menu` |
  | Table | `content::Table` |

  The alias table lives at `crates/ui-port/src/names.rs`, and it is the
  only place where names are mapped.
- **New shadcn-only components** live in a new `openagents-ui` module,
  `crates/openagents-ui/src/kit/`, one file per shadcn item (`kit/card.rs`,
  `kit/tabs.rs`, `kit/navigation_menu.rs`), with
  `pub use kit::card::{Card, CardHeader, …}`.
- **Regeneration is safe.** Generated files carry a header with the item
  address and hash. Hand fixes go in a sibling `*_fix.rs` or in the CSS
  override layer, so a re-pull does not overwrite them.

### 2.7 The agentic layer

The deterministic pass does most of the work. An agent handles the rest:

1. **Choose.** A request ("a pricing section with four tiers and a monthly
   toggle") is embedded and searched by cosine similarity against an index
   of registry items. Each item is indexed by its `title`, `description`,
   `categories` and the IR's component names, with a structural summary. We
   use our Google-first embeddings, and ranked results go through the
   Decisions API (`/v1/decisions`) when a choice is unclear. A typed
   `UiItemQuery { kind, sections, required_behaviours, license_ok }` filters
   on exact enum fields only after semantic retrieval. There is no keyword
   routing anywhere.
2. **Pull.** It runs `ui-port pull` and reads the fidelity report.
3. **Fix leftovers.** It writes hand-written adapters, real copy, real link
   targets, and nearest-token decisions for approximated classes. All fixes
   go in `*_fix.rs` and the override CSS.
4. **Prove.** Before the agent may open a commit, four checks must pass:
   - Golden screenshots of the emitted specimen on `/ui`, in Light and
     Dark, at three widths. These are compared with a reference render of
     the upstream item built from its own JSON. The reference is rendered
     locally and never committed for paid items.
   - `copy_guard::assert_plain`.
   - The behaviour contract tests.
   - An axe-style accessibility check (role, name, contrast against our
     tokens).

The shadcn MCP server is useful for browsing, but its "install" writes
React files. We use only its list and search output, as one more input to
the index.

## 3. Harmonising tokens

There are three vocabularies:

- **shadcn:** about 30 variables. They are surface/foreground pairs plus
  `border`, `input`, `ring`, `chart-1..5`, `sidebar-*` and `radius`, all in
  oklch, mapped to Tailwind through `@theme inline`.
- **Apps SDK UI:** about 259 semantic roles (`--color-background-primary-solid`,
  `--color-text`, `--color-border`, …). Each is a `light-dark()` value,
  stored in `crates/oa-tokens/src/apps_sdk.rs`, with Noir overrides in
  `noir.rs` and our own roles in `product.rs`.
- **Native:** the 22 roles in `oa_tokens::Palette` (`palette.rs`).

`oa-tokens` stays the single source. We add `crates/oa-tokens/src/shadcn.rs`,
a static table from each shadcn variable to an `oa-tokens` role:

| shadcn | Our role (web) | Notes |
| --- | --- | --- |
| `background` / `foreground` | `--color-surface` / `--color-text` | |
| `card` / `card-foreground` | `--color-surface-elevated` / `--color-text` | |
| `popover` / `popover-foreground` | `--color-surface-elevated-secondary` / `--color-text` | |
| `primary` / `primary-foreground` | `--color-background-primary-solid` / `--color-text-inverse` | |
| `secondary` / `secondary-foreground` | `--color-background-primary-soft` / `--color-text` | |
| `muted` / `muted-foreground` | `--color-background-primary-soft-alpha` / `--color-text-secondary` | |
| `accent` / `accent-foreground` | `-soft-hover` / `--color-text` | |
| `destructive` | `--color-background-danger-solid` | shadcn has no `destructive-foreground` |
| `border`, `input` | `--color-border`, `--color-border-strong` | |
| `ring` | `--color-ring` | |
| `chart-1..5` | Product data-viz roles, new in `product.rs` | |
| `sidebar-*` | Shell sidebar roles from `shell.css` | |
| `radius` | `--radius-lg` (0.625rem) | shadcn's `sm` to `4xl` scale is derived from it, and ours already matches |

Hover and active states come from our `-hover` and `-active` roles instead
of shadcn's `/90` opacity tricks. That is where we look better than a
literal port.

The mapping runs in three directions:

- **In.** The class mapper resolves `bg-primary` through this table.
  Arbitrary values (`bg-[#123456]`, `text-[13px]`) snap to the nearest token
  and are reported as "approximated"; they never pass through as raw values.
  An item's own `cssVars` are matched to an existing role. A new variable is
  refused and reported, never added silently. This is the inventory doc's
  rule that only `oa-tokens` may hold a color value.
- **Out.** `oa-tokens` generates a `registry:theme` item whose
  `cssVars.light` and `cssVars.dark` hold our values in shadcn names. Any
  shadcn project can then wear Coder Light / Coder Noir. This also gives us
  a check: the generated theme must round-trip through the table.
- **Native.** The `Palette` roles take the same table's targets, so a card
  ported to desktop paints with the same roles as on the web.

## 4. The OpenAgents UI system

### Layers

| Layer | What it is | Where it lives |
| --- | --- | --- |
| Tokens | Color roles, type, radius, spacing, motion; Light and Noir | `crates/oa-tokens` (single source), generated into `openagents-ui/static/tokens-*.css` and `Palette` |
| Primitives | Single-purpose controls with a behaviour contract: Button, Input, Checkbox, Dialog, Popover, Menu, Tabs… | `openagents-ui` (web), `rust_native::Element` (native) |
| Components | Compositions with names: Card, PlanCard, PluginCard, Composer, Thread, ToolCall | `openagents-ui::{content, shell, kit}` and app view builders |
| Blocks and pages | Sections and full pages: hero, pricing, footer, chat page | `openagents-web/src/pages/*`, new `openagents-ui::blocks` for reusable sections |

### Renderers

| Family | Paints with | Gets from the converter |
| --- | --- | --- |
| Web | Maud + `openagents-ui` CSS, HTMX, Alpine CSP | Full emitter |
| Rust Native | `rust-native-desktop` (wgpu), SwiftUI/UIKit and Android Views hosts, `rust-native-web` | Semantic emitter. Gaps grow `Element`. |
| Terminal | ratatui, Noir only | Nothing for now |
| Effect Native | Separate `effect-native` repo | IR is reusable there later |

The rule from the inventory doc still holds: we share **specs and tokens**
across platforms, not painters. The converter's IR is the natural home for
that spec. We add a `docs/ui/components.md` table (inventory phase 2) that
names each component once, with its shadcn name, its `openagents-ui` type,
its `Element`, and its status per platform.

### Our own registry

We publish our components as a shadcn-compatible registry, so other people
(and our own agents) can pull them in the same format:

- **Index.** `https://openagents.com/r/registry.json` is served by
  `openagents-web`, with one route per item at `/r/{name}.json`.
- **Generation.** Items are generated by `ui-port` (`emit/registry.rs`)
  from the IR of our own components. The file content is a small React/TSX
  rendering of the same structure and token classes, for shadcn users. The
  same JSON also carries a `meta.openagents` block: Maud type path,
  `Element` mapping, and fidelity.
- **Namespace.** Users add `"@openagents": "https://openagents.com/r/{name}.json"`
  to their `components.json`.
- **Theme.** The theme item is section 3's output.
- **Content.** Only our own work and MIT-derived components (with a
  `NOTICE`) are published, never ported third-party blocks. See section 6.

### Docs and catalog

`/ui` stays the single catalog, as the inventory recommends. Every ported
item gets a specimen there in Light and Dark, a link to its fidelity
report, and its shadcn name. We do not need Storybook: `/ui` is already our
Storybook, built from the real components.

### Testing

| Check | Applies to |
| --- | --- |
| Byte-stable emit (re-pull with the same lock gives the same output) | Converter |
| Golden HTML snapshots per specimen; golden PNGs at three widths, Light and Dark | Web components and blocks |
| Desktop captures through `rust_native_desktop::capture_views` | Native mappings |
| Behaviour contract tests (keyboard, focus, aria) | Each behaviour adapter |
| `copy_guard::assert_plain` on every page and specimen | All web pages |
| Color-literal ban outside `oa-tokens` | All crates (inventory phase 1d) |
| Stylesheet budget (290 KB) | `openagents-ui` bundle |
| Registry round-trip: our published item, lowered again, gives the same IR | Our registry |

## 5. Roadmap

| Phase | Goal | Acceptance |
| --- | --- | --- |
| **M0: Landing Page 5 structure on `/landing`** | First proof, done mostly by hand with the IR sketched. Rebuild `/landing` (`crates/openagents-web/src/pages/landing.rs`, `static/landing/landing.css`) as the Landing Page 5 *section order*: navbar, hero with pill and framed product shot, three tiles, split headline with image tray, testimonials (real quotes only, or the section is omitted), four-tier pricing from our real plans (`pricing.rs`), CTA, footer. Built from MIT shadcn/ui primitives (Card, Button, Badge, NavigationMenu) ported by hand into `openagents-ui::kit`, with our copy and our screenshots. No paid block source is read into the repo. | `/landing` renders every section in Light and Dark at 375, 768 and 1280 px. `copy_guard::assert_plain` passes. Every link and button goes somewhere real. The existing landing test is updated. Card, CardHeader, … are visible on `/ui` under their shadcn names. |
| **M1: `ui-port` for `registry:ui`** | `crates/ui-port`: fetch, resolve and lock; oxc parse; IR; class map; Maud emitter; fidelity report. Target the built-in shadcn registry (MIT). | Card, Badge, Separator, Tabs, Accordion, Skeleton, Breadcrumb, Table and Pagination pull with score ≥ 0.9. Re-pull is byte-identical. Reports are written to `docs/ui/ports/`. Each item has a `/ui` specimen. |
| **M2: Tokens and behaviours** | `oa-tokens/src/shadcn.rs`; the behaviour adapter table; hand-written Tabs, NavigationMenu, Command, Toast | Our theme emitted as a `registry:theme` round-trips. Contract tests pass for each adapter. The color-literal ban covers generated CSS. |
| **M3: Blocks and agentic layer** | `registry:block`/`page` lowering, the embedding index over registries we are licensed for, the agent loop with golden screenshots | An agent turns a one-line request into a merged section on a real page, with a passing report and screenshots and no human edits beyond review. Choices go through the index and Decisions API (test: no string matching in the selection path). |
| **M4: Native emitter** | `rust_native::Element` emission; Badge, StatusChip, Card-as-Stack; desktop captures | The same Card and Button specs render on `/ui` and in a desktop capture with the same token roles. Gaps are filed against `Element`. |
| **M5: Our registry** | `/r/registry.json` and `/r/{name}.json`, `@openagents` namespace, theme item | `npx shadcn add @openagents/button` works in a fresh Next.js app. Our IR round-trip test passes. |

## 6. Licensing, open questions and risks

### Licensing (the biggest risk)

| Source | License | What we may do |
| --- | --- | --- |
| shadcn/ui built-in registry | MIT | Port freely. Keep a `NOTICE` in `openagents-ui`, as we do for Apps SDK UI. |
| Free third-party registries | Varies (often MIT) | `ui-port` reads the license and refuses an `Unknown` license for anything that gets committed |
| **shadcnblocks paid ("Pro"/"elite") blocks, including Landing Page 5** | Commercial license | **Not in this repo.** The license forbids publishing in a public repository, distributing as a UI library, kit or template, and "Derivatives" (which includes AI-assisted restyling, renaming or reformatting). It also forbids AI generators trained on their components. `openagents` is public, and our registry would be redistribution. |

What this means in practice:

- M0 reproduces Landing Page 5's **section order and layout idea** (which
  is not their code). It is built from MIT primitives and our own content.
  Nobody runs `shadcn add @shadcnblocks/...` inside the repo.
- `ui-port` has a `LicenseClass::Paid` path that runs only in an out-of-repo
  scratch directory, for local study. The emitted output is marked
  "do not commit", and a pre-commit check refuses files whose generated
  header carries a paid hash.
- The selection index includes only items we may commit.
- If we want paid blocks in a product, that needs an owner decision (a
  licence plus a private repo), not an agent decision.

### Open questions

1. Do we want a Tailwind-utility path at all (for example
   `openagents-ui/static/utilities.css`, generated from the same table), or
   only component CSS? This plan assumes component CSS only, which keeps the
   290 KB budget.
2. Should `kit/` components be generated once and then owned by hand
   (shadcn's own "copy and own" philosophy), or regenerated on each
   upstream change? This plan proposes: own the generated file, keep fixes
   separate, and re-pull on purpose.
3. Should Effect Native consume the IR directly (JSON), or should it get its
   own emitter in its repo?
4. Our published registry items need TSX content. Should they be
   hand-written per component, or emitted from the IR?

### Risks

- **Class-map breadth.** Tailwind v4 has a large surface (arbitrary values,
  container queries, `group-*` and `peer-*`). Mitigation: a static table
  grown by fidelity reports, with snap-and-report as the default, never
  passthrough.
- **Behaviour drift.** Radix semantics are subtle (focus return, nested
  dismiss). Mitigation: contract tests per adapter, and native elements
  first.
- **Visual drift from upstream.** Mitigation: golden screenshots against a
  locally rendered reference.
- **Two names for one thing.** Mitigation: one alias table (`names.rs`)
  and one components table (`docs/ui/components.md`).
- **Placeholder copy leaking.** Mitigation: the `copy_to_replace` gate plus
  `copy_guard`.
- **Disk and build cost.** `oxc` is a dev-only dependency of a dev-only
  binary. It is not linked into `openagents-web`.

## Sources

- shadcn docs: [components](https://ui.shadcn.com/docs/components),
  [docs](https://ui.shadcn.com/docs),
  [theming](https://ui.shadcn.com/docs/theming),
  [components.json](https://ui.shadcn.com/docs/components-json),
  [registry-item.json](https://ui.shadcn.com/docs/registry/registry-item-json),
  [registry.json](https://ui.shadcn.com/docs/registry/registry-json),
  [namespaces](https://ui.shadcn.com/docs/registry/namespace),
  [MCP](https://ui.shadcn.com/docs/mcp),
  and the published item `https://ui.shadcn.com/r/styles/new-york-v4/card.json`.
- shadcnblocks: [Landing Page 5](https://www.shadcnblocks.com/page/landing-page5),
  [license](https://www.shadcnblocks.com/license).
- Ours: [`inventory.md`](inventory.md),
  [`../web/apps-sdk-ui-adoption-plan.md`](../web/apps-sdk-ui-adoption-plan.md),
  [`../coder/rust-native/README.md`](../coder/rust-native/README.md),
  [`../research/2026-10-09-openui-adaptation.md`](../research/2026-10-09-openui-adaptation.md)
  (catalog-validated generative UI, a neighbour of the agentic layer),
  `crates/openagents-ui`, `crates/oa-tokens`, `crates/rust-native`,
  `crates/openagents-web/src/copy_guard.rs`, and the read-only reference
  `~/work/projects/repos/apps-sdk-ui`.

# Desktop UI × Apps SDK UI Harmonization Audit

Date: 2026-07-14
Status: audit / implementation plan. Owner directive (verbatim): "We need to
improve the openagents desktop UI. still a lot of weird one-off styles. We
need ALL styles harmonized with the projects/repos/apps-sdk-ui while
preserving our starcraft design. all of the relevant apps sdk ui components
need parallels in our Effect Native, And our UI should use those conventions."

Grounded in full reads of `~/work/projects/repos/apps-sdk-ui` (OpenAI
`@openai/apps-sdk-ui` v0.2.2, read-only reference) and the current OpenAgents
surfaces (`apps/openagents-desktop`, the vendored
`apps/openagents.com/packages/effect-native-*` at catalog v29/v30 pin,
upstream `OpenAgentsInc/effect-native` at catalog v31, `packages/ui`,
`packages/design-tokens`) on 2026-07-14.

Companions: `docs/effect-native/2026-07-08-styling-tailwind-stylex-effect-native.md`
(the styling-contract decision this audit must not violate),
`docs/effect-native/DEMAND_REGISTER.md`, the upstream `effect-native/GAPS.md`
growth rule, and `apps/openagents-desktop/src/renderer/design-conformance.test.ts`
(the existing design-language oracle).

---

## 1. Executive summary

Apps SDK UI is the best-organized small design system we track: a strict
three-tier token architecture (primitive → semantic → component), a fully
symmetric semantic color matrix (8 tones × solid/soft/outline/ghost variants ×
hover/active states), one uniform variant mechanism across every component,
an 8-step control-size lattice with paired gutter/radius/icon sub-tokens, and
a 745-icon `currentColor` icon system. Its value to us is **the architecture
and the conventions, not the code**: it is React + CSS Modules + class
strings, which our styling decision explicitly rejects.

Our side splits cleanly into three very different conditions:

1. **The desktop renderer is already token-pure.** Every visual value in
   `apps/openagents-desktop/src/renderer` resolves through `@effect-native/tokens`
   (`--en-*`); a conformance test bans raw hex/rgb/spacing literals. The
   "weird one-off styles" in desktop are **not color literals** — they are
   **structural one-offs**: per-call-site style-object recipes re-deciding
   padding/border/surface per view, a 1,044-line `app.css` host stylesheet, a
   magic-number dimension allowlist, and missing component variants that force
   ad hoc composition (see §4).
2. **Effect Native's catalog (~70 tags at v31) covers most of Apps SDK UI's
   30 components structurally but lacks its variant system.** Our Button has
   `primary|secondary|ghost` and nothing else — no tone axis, no
   solid/soft/outline/ghost axis, no size lattice on the public prop. That
   variant poverty is the single biggest cause of desktop one-offs: when a
   component can't express "soft danger, small", the call site hand-rolls it
   as a style object.
3. **The repo still carries a second, divergent token identity**:
   `packages/design-tokens` (`--oa-*`, amber `#f5b73a` accent, ~250 named
   colors) backing `packages/ui` (Foldkit web, **947 raw hex literals**).
   That is the actual largest one-off pool in the monorepo. It is on the
   EN-4/CV2 retirement path; the plan below deletes it rather than
   harmonizing it.

**The plan in one sentence:** adopt Apps SDK UI's token architecture, variant
matrix, control lattice, and icon/breakpoint/motion conventions **translated
into the Effect Native typed-object world** (typed variants + theme tiers +
`data-*` lowering in the DOM renderer — never class strings), land the
missing components/variants upstream in `OpenAgentsInc/effect-native` through
the GAPS growth rule with khalaTheme (Protoss blue, dark-only) as the shipped
theme, bump the vendor pin, then sweep the desktop renderer to replace every
per-call-site style recipe with a catalog variant and extend the conformance
oracle so the one-offs cannot come back.

---

## 2. What Apps SDK UI actually is (reference inventory)

Package `@openai/apps-sdk-ui`: React 18/19 + Tailwind 4 peers; the library
itself styles via CSS Modules + CSS custom properties; `clsx` only (no CVA,
no tailwind-merge); Radix for menu/popover/tooltip primitives.

### 2.1 Three-tier token architecture

`src/styles/index.css` imports in strict order — this ordering IS the system:

- **Tier 1 — Primitives** (`variables-primitive.css`): raw ramps with no
  meaning. `--gray-0…1000`; `green/red/pink/orange/yellow/purple/blue` each at
  `-25…-1000` plus alpha steps `-a25…-a300`; `--alpha-0…70`; shadow geometry.
- **Tier 2 — Semantic** (`variables-semantic.css`): meaning mapped onto
  primitives. Text (`--color-text`, `-secondary`, `-tertiary`, `-inverse`),
  surfaces (`--color-surface`, `-secondary`, `-tertiary`, `-elevated`,
  `-elevated-secondary`), borders (`-subtle`/`-`/`-strong`), the **semantic
  color matrix** `--color-{background|border|text|ring}-{tone}-{variant}[-state]`
  for 8 tones (primary, secondary, danger, success, warning, caution,
  discovery, info) × variants (solid/soft/soft-alpha/surface/outline/ghost) ×
  states (hover/active), a dual typography scale
  (`--font-heading-5xl…xs-*` and `--font-text-lg…3xs-*`, each a
  size/line-height/weight/tracking quad), the **control lattice**
  (`--control-size-3xs…3xl` with paired `--control-gutter-*`,
  `--control-radius-*`, `--control-font-size-*`, `--control-icon-size-*`),
  and motion (`--cubic-enter/-exit/-exit-snappy/-move`,
  `--transition-duration-basic: 150ms`).
- **Tier 3 — Component** (`variables-components.css`): per-component tokens
  (`--button-gap-md`, `--input-*`, `--menu-*`, `--codeblock-syntax-1..5`, …)
  that components consume, and that semantic tokens feed.

The critical mechanism is the **token indirection chain**: semantic token →
component-local custom property → CSS property. A Button rule never touches a
primitive; each `data-color`/`data-size` selector only re-points local
`--button-*` vars:

```css
&:where([data-color="primary"]) {
  --button-background-color: var(--color-background-primary-solid);
  --button-background-color-hover: var(--color-background-primary-solid-hover);
  --button-text-color: var(--color-text-primary-solid);
}
&:where([data-size="md"]) { --button-size: var(--control-size-md); … }
```

### 2.2 One uniform variant mechanism

Every component expresses variants the same way: the TSX sets `data-color`,
`data-variant`, `data-size`, `data-pill`, `data-loading`, `data-selected`,
etc.; the CSS Module selects on them. There is no JS variant library. The
shared prop vocabulary lives in one `types.ts`: `ControlSize` (3xs–3xl),
`SemanticColor` (8 tones), `Variant` (solid/soft/outline/ghost), `TextColor`,
`FontWeight`, with narrowing wrappers (`Sizes<T>`, `Variants<T>`,
`SemanticColors<T>`).

### 2.3 The 30 components

Button/ButtonLink/CopyButton, Badge, Alert, Avatar/AvatarGroup, Checkbox,
CodeBlock, DatePicker, DateRangePicker, EmptyMessage, Icon (745 SVGs), Image,
Indicator (LoadingIndicator/LoadingDots/CircularProgress), Input, Markdown,
Menu, Popover, RadioGroup, SegmentedControl, Select, SelectControl,
ShimmerText, Slider, Switch, TagInput, Textarea, TextLink, Tooltip/CopyTooltip,
Transition (Animate/AnimateLayout/TransitionGroup), AppsSDKUIProvider.

### 2.4 Other conventions worth stealing

- **Icons:** 745 PascalCase named components, all `1em × 1em`,
  `viewBox 0 0 24 24`, `fill="currentColor"` — size from font-size/icon
  tokens, color inherited. Sizing utilities `icon-xs…icon-2xl` (14–24px) map
  to `--control-icon-size-*`.
- **Breakpoints:** one config object feeds Tailwind, the CSS mixins, and the
  `useBreakpoint` hook — single source of truth. Values: xs 380, sm 576,
  md 768, lg 1024, xl 1280, 2xl 1536. Mobile-first.
- **Hover discipline:** hover styles are gated behind
  `@media (hover:hover) and (pointer:fine)` everywhere (a mixin), so touch
  devices never get sticky hover.
- **Motion:** named cubic-bezier tokens + one basic duration; enter/exit
  handled by `data-entering`/`data-exiting` attributes with
  `[data-exiting]{pointer-events:none}`.
- **Dark mode:** `light-dark()` on `[data-theme]` — noted for completeness;
  **we do not adopt this** (we are dark-only by owner mandate).
- **Per-subpath exports** (`/components/*`, `/hooks/*`, `/css`) and
  Storybook-first docs (a `.stories.tsx` + `.mdx` per component, foundation
  MDX pages for tokens/colors/typography/dark-mode/responsive/icons).

---

## 3. What we have today

### 3.1 Effect Native (the canonical system)

- Upstream `OpenAgentsInc/effect-native`, catalog `effect-native/v31`,
  ~70 component tags, closed catalog with a GAPS growth rule (real demanding
  screen + typed props + all renderers + conformance in one bump; no plugin
  escape hatch). Vendored into the monorepo at
  `apps/openagents.com/packages/effect-native-*` with a pinned commit
  (currently v29/v30-era — a pin bump is already due).
- **Styling contract:** typed style objects on token literals
  (`backgroundColor: ColorToken`, `padding: SpacingToken`, …) with
  deterministic merge; per-component style key allowlists
  (`buttonStyleKeys`, `cardStyleKeys`, …); `variants.state/platform/breakpoint`.
  Class strings are rejected by decision doc and owner confirmation.
- **Tokens (`@effect-native/tokens`):** a single flat semantic tier —
  39 color roles (background, surface, surfaceRaised, surfaceOverlay,
  textPrimary/Muted/Faint/Inverse/Disabled, accent/accentHover/accentActive,
  border×3, focus, status×4, stateHover/Active/Selected alpha overlays,
  scrim, code/diff/syntax) — plus spacing, radius (0/2/4/6/8/9999),
  a 5-step type scale, breakpoints (sm/md/lg/xl), a 4-step control lattice
  (24/28/32/40px heights), motion, and elevation schemas. Everything Effect
  Schema-validated.
- **khalaTheme** is the one shipped theme: Protoss blue `#3b82f6` accent on
  `#05070d`, dark-only, no runtime switch. The DOM renderer lowers tokens to
  `--en-*` custom properties.

### 3.2 The desktop renderer

`apps/openagents-desktop/src/renderer` (~26.5k LOC) is 100% Effect Native —
every screen is a pure `state → View` projection from catalog constructors;
`theme.ts` is 17 lines re-exporting khalaTheme; `app.css` (1,044 lines) is
host layout physics with every color/space/radius via `var(--en-*)`.
`design-conformance.test.ts` mechanically bans hex/rgb/hsl literals, off-scale
spacing/radius strings, raw font sizes, and un-allowlisted numeric dimensions.
A grep for hex colors returns zero real offenders (all hits are issue refs in
comments, ID selectors, or test assertions of the canonical palette).

### 3.3 The legacy debt (for honesty about "one-off styles")

- `packages/ui` (Foldkit web, class-string driven, its own `basecoat/`
  shadcn-style set): **947 raw hex literals**, amber-era palette
  (`#f1efe8` ×132, `#ffb400` ×73, `#d32f2f` ×38, …).
- `packages/design-tokens` (`--oa-*`): ~250 named colors including a
  near-but-not-equal `khala*` blue family (`khalaEnergyBlue #3a7bff` vs
  canonical `#3b82f6`) coexisting with the amber accent.

These are EN-4/CV2 conversion sources, not harmonization targets. The plan
retires them; it does not port Apps SDK UI conventions into them.

---

## 4. Where desktop's "weird one-off styles" actually are

Since colors are already tokenized, the one-offs are structural. Four classes:

1. **Per-call-site style recipes.** `shell.ts` (3,670 lines) and siblings
   hand-assemble the same micro-decisions repeatedly:
   `style: { width: "full", borderColor: "borderSubtle", borderWidth: 1, surface: "glass" }`,
   `style: { padding: "0", borderWidth: 0, typeScale: "caption", color: "textFaint" }`, …
   Each is token-pure but **each call site re-derives the recipe** — the
   definition of a one-off. Apps SDK UI never lets a call site do this: the
   recipe lives in the component (tier-3 tokens + variants), and the call
   site says `variant="soft" color="secondary" size="sm"`.
2. **`app.css` as a structural escape hatch.** 1,044 lines of host CSS is a
   parallel styling channel outside the typed system. Some of it is genuinely
   host physics (scrollbars, root sizing); much of it is component-shaped
   styling (control lattice application, panel borders) that belongs in
   catalog components / renderer lowering.
3. **The numeric-dimension allowlist.** The conformance oracle allowlists 13
   magic numbers (840, 420, 360, 280, 480, 336, 240, 320, 560, 400, 4, 56,
   64) — each is a one-off dimension a token should own (dimension tokens
   exist: xs/sm/md/lg/xl/full; the lattice needs more steps, not more
   exceptions).
4. **Missing variants/components forcing composition one-offs.** No
   Avatar → hand-rolled initials chips; no SegmentedControl → ad hoc button
   rows; no EmptyMessage → per-view empty states; no LoadingDots/Shimmer →
   per-view "thinking" affordances; Button without tone/size axes → styled
   Stacks pretending to be buttons.

Fixing class 4 (in the catalog) is what makes fixing classes 1–3 (in the
desktop) possible.

---

## 5. Component parity map: Apps SDK UI → Effect Native

Verdicts: **HAVE** (parity exists), **PARTIAL** (tag exists, conventions
missing), **GAP** (needs a new catalog entry via GAPS.md), **REJECT** (do not
adopt), **N/A**.

| Apps SDK UI | Effect Native today | Verdict | Action |
|---|---|---|---|
| Button / ButtonLink | `Button` (`primary\|secondary\|ghost`), `Link` | **PARTIAL** | Adopt the full matrix: `tone` (8 semantic tones mapped to our Tone set), `variant` solid/soft/outline/ghost, `size` on the control lattice, `pill`, `loading`, `block`, `selected`. This is the highest-leverage single change. |
| CopyButton / CopyTooltip | copy intent exists on CodeBlock only | **GAP** | Typed `CopyButton` (content + copied-state + typed onCopy intent). |
| Badge | `Badge`, `Chip` (Tone set) | **PARTIAL** | Add `variant` solid/soft/outline + size to Badge/Chip. |
| Alert | `StatusBanner` (tone+message+retry) | **PARTIAL** | Extend StatusBanner to the tone×variant matrix with icon+title+body slots (or add `Alert` and keep StatusBanner as the persistent case). |
| Avatar / AvatarGroup | none (GAPS row "waiting") | **GAP** | Demanding screens now exist: desktop sidebar accounts, fleet operator rows, forum. Typed image/initials/fallback + size + group overlap. |
| Checkbox | `Checkbox` | **HAVE** | Align sizes to the lattice. |
| CodeBlock | `CodeBlock` (typed pre-tokenized lines) | **HAVE** | Keep our typed model (no runtime highlighter — deliberate). Adopt their 5-slot syntax-token idea only as naming reference; we already ship syntax color tokens. |
| DatePicker / DateRangePicker | none | **GAP (deferred)** | No demanding desktop screen yet. Enter GAPS as `waiting`; do not build ahead of demand. |
| EmptyMessage | none | **GAP** | Desktop history/workspace/fleet panes all hand-roll empty states today. Typed icon+title+description+action. |
| Icon (745, currentColor, 1em) | `Icon` (~31 closed names) | **PARTIAL** | Keep the closed-set discipline; adopt their **conventions** (currentColor, 1em box, 24 viewBox, PascalCase semantic names, size from `--control-icon-size-*`) and batch-expand the set from real desktop demand (~80–120 names, not 745). |
| Image | `Image` | **HAVE** | — |
| Indicator (LoadingIndicator/Dots/CircularProgress) | `Meter`/`Progress` only | **GAP** | Typed `Spinner` + `LoadingDots` (+ determinate circular as a Meter variant). Transcript/tool-card streaming states demand it. |
| Input | `TextField` | **PARTIAL** | Add `variant` outline/soft, lattice `size`, `gutterSize`, invalid/disabled parity. |
| Markdown | `Markdown` (typed pre-parsed) | **HAVE** | Keep typed model; no react-markdown. |
| Menu (Radix) | `DropdownMenu`, `ContextMenu` | **HAVE** | Parity incl. submenu/radio/checkbox items already typed. |
| Popover | `Popover` | **HAVE** | — |
| RadioGroup | `RadioGroup` | **HAVE** | — |
| SegmentedControl | `Tabs` (adjacent, not same) | **GAP** | Distinct component: single-choice control with animated thumb; desktop settings + workbench mode switches demand it. |
| Select / SelectControl | `Select`, `Combobox` | **PARTIAL** | Add multi-select and the SelectControl trigger conventions (variant/size/pill, dropdown-icon type). |
| ShimmerText | none | **GAP** | Streaming transcript demands it (pending text, tool wait states). |
| Slider | `Slider` | **HAVE** | Add marks if a screen demands. |
| Switch | `Toggle` | **HAVE** | — |
| TagInput | none | **GAP (deferred)** | No demanding screen yet; GAPS `waiting`. |
| Textarea | `TextField` (single-line), `Composer` (rich) | **PARTIAL** | Plain multiline TextField mode with autoResize for settings/forms. |
| TextLink | `Link` | **HAVE** | — |
| Tooltip | `Tooltip` | **HAVE** | — |
| Transition (Animate/TransitionGroup) | GAPS row "overlay animation polish: waiting" | **GAP (scoped)** | Adopt the **token side** now (named easing curves, basic duration, `data-entering`/`data-exiting` lowering in render-dom); full Animate primitives stay demand-gated. |
| AppsSDKUIProvider | `khalaThemeLayer` + platform Layers | **N/A** | Effect Layers already own this role. |
| Hooks (useBreakpoint, useEscCloseStack, useAutoGrowTextarea, …) | viewport tokens, `makeKeymap` scope stack, Composer | **HAVE (as runtime services)** | Convention already superior (typed services, not hooks). Adopt only missing behaviors (auto-grow textarea → TextField multiline). |
| Storybook | `@effect-native/gallery` | **PARTIAL** | Gallery should grow a per-component page + foundation pages (tokens/typography/icons) mirroring their MDX docs discipline. |

**REJECT list (explicit):** React/JSX as authoring model; CSS Modules +
class strings + `clsx`; Radix dependency; `light-dark()` dual theming (we are
dark-only, one Protoss-blue theme, no light mode); runtime markdown parsing
and runtime syntax highlighting (our typed pre-parsed models are deliberate);
CVA-style JS variant maps; vendoring any Apps SDK UI code (read-only
reference per workspace rules — we port conventions, never files).

---

## 6. Convention adoption: what "harmonized" means, precisely

Seven conventions to adopt, each translated into the typed-object world:

### C1. Three-tier tokens (primitive → semantic → component)

Today `@effect-native/tokens` is one flat semantic tier with hand-picked hex
values. Adopt the tier model:

- **Tier 1 (new):** typed primitive ramps — a `blue-25…1000` Protoss ramp, a
  cool `gray-0…1000` ramp, status hue ramps, and alpha steps — as schema'd
  values in `@effect-native/tokens`. khalaTheme's 39 semantic roles become
  **derivations from the ramps** instead of free-floating hex, which is what
  keeps future colors on-brand by construction.
- **Tier 2 (existing):** the current semantic roles, extended by the variant
  matrix (C2).
- **Tier 3 (new):** component token structs (`theme.components.button`, etc.)
  that the DOM renderer lowers to `--en-button-*` custom properties, mirroring
  the `--button-*` indirection. Variants re-point component tokens; base
  lowering rules consume them. This single mechanism is what deletes
  per-call-site recipes: the recipe moves into the theme, typed.

### C2. The tone × variant × state matrix

Adopt the symmetric grid as **typed schemas**, not CSS:
`Tone` = our existing tone set aligned to their 8 (primary=accent, secondary,
danger, success, warning, info; add `caution`/`discovery` only if a screen
demands), `Variant` = solid/soft/outline/ghost, states = rest/hover/active/
selected/disabled resolved through the existing alpha-overlay state engine
(`stateHover`/`stateActive`/`stateSelected` — dark themes lighten; already our
model). The matrix lands as theme data (`colorMatrix[tone][variant][state]` →
background/border/text/ring roles), and Button/Badge/Alert/Chip/SelectControl
consume it. Protoss blue stays the primary tone; the matrix is how the other
tones stop being hand-mixed per view.

### C3. The control lattice

Extend `controlTokens` from 4 steps (sm/md/lg/xl = 24/28/32/40) toward their
8-step lattice **as demand requires** (likely add `xs` and `2xs` for dense
desktop chrome), and — the important part — pair every step with typed
`gutter`, `radius`, `fontSize`, and `iconSize` sub-values as their
`--control-*` family does. One `size` prop then coherently sizes a control,
its padding, its text, and its icon. Kill the desktop's numeric-dimension
allowlist by adding the missing lattice/dimension steps instead.

### C4. `data-*` lowering in render-dom

Adopt their variant-expression mechanism **at the renderer layer**: render-dom
emits `data-en-component="button" data-variant="soft" data-tone="danger"
data-size="sm"` and ships component-scoped CSS that re-points `--en-button-*`
vars per attribute — exactly their indirection chain, generated from the typed
theme rather than hand-written. Benefits: debuggable DOM, stable QA/behavior-
contract selectors, and the desktop's `app.css` component-shaped rules migrate
into renderer-owned lowering.

### C5. Icon conventions

Keep the closed registry; adopt `currentColor` + `1em` + 24-viewBox + semantic
PascalCase names + lattice-driven `iconSize`. Expand the set demand-driven
(sidebar, tool cards, git panel, settings, fleet — audit shows ~80–120 needed
names) in one or two catalog bumps.

### C6. Breakpoints, hover, motion

- Align breakpoint **values** to theirs (sm 576 / md 768 / lg 1024 / xl 1280;
  add xs 380 and 2xl 1536 only if a screen demands) so responsive behavior
  matches the best-tested grid in our reference set; keep them typed tokens.
- Gate all hover lowering behind `(hover:hover) and (pointer:fine)` in
  render-dom (their mixin discipline; matters for future touch/mobile DOM).
- Adopt named easing tokens (`enter`, `exit`, `exitSnappy`, `move`) + a
  `basic: 150ms` duration in `MotionThemeSchema`, and `data-entering`/
  `data-exiting` lowering with `pointer-events: none` while exiting.

### C7. Docs/gallery discipline

Mirror their Storybook shape in `@effect-native/gallery`: one page per
component with live variants, plus foundation pages (Design tokens, Colors,
Typography, Icons, Responsive). Agents and humans both need the "what exists,
what are its variants" surface to stop inventing one-offs.

---

## 7. Implementation plan

Routing per repo rules: **all catalog/token/renderer work lands upstream in
`OpenAgentsInc/effect-native`** through the GAPS register (typed props,
all renderers, conformance, catalog bump), then the monorepo bumps the vendor
pin; the desktop sweep is monorepo work. No local one-off primitives — gaps go
upstream (EN-2 / #8572).

### Phase 0 — Token architecture (upstream, one catalog-neutral bump + one bump)

1. Primitive ramps + derived khalaTheme (C1 tier 1). No visual change:
   derivations must reproduce the exact current canonical hex values
   (`#05070d`, `#3b82f6`, `#8fb3ff14`, …) — locked by test.
2. Tone×variant×state matrix schema + khalaTheme values (C2).
3. Control-lattice sub-tokens (C3) and motion/easing tokens (C6).
4. Component-token tier + render-dom `data-*`/`--en-<component>-*` lowering
   (C1 tier 3, C4), hover gating (C6).

### Phase 1 — Control matrix on existing components (upstream)

5. Button: tone/variant/size/pill/loading/block/selected on the matrix.
6. Badge, Chip, StatusBanner→Alert, TextField, Select/SelectControl trigger:
   same axes. Per-component style-key allowlists tighten accordingly.

### Phase 2 — Missing components (upstream, demand-named per GAPS)

7. Avatar + AvatarGroup (sidebar accounts, fleet rows).
8. SegmentedControl (settings, workbench mode switch).
9. EmptyMessage (history/workspace/fleet empty panes).
10. Spinner + LoadingDots + ShimmerText (transcript streaming states).
11. CopyButton (transcript, code, diagnostics).
12. Icon set expansion batch.
13. Deferred, registered `waiting` in GAPS: DatePicker/DateRangePicker,
    TagInput, full Transition primitives.

### Phase 3 — Desktop sweep (monorepo, after vendor pin bump)

14. Bump the vendored `effect-native-*` pin to the new catalog.
15. Replace every per-call-site style recipe in `shell.ts`, `settings.ts`,
    `fleet-workspace.ts`, `git-panel.ts`, etc. with matrix variants; delete
    hand-rolled avatar/segment/empty/loading compositions in favor of the new
    components.
16. Shrink `app.css` to true host physics (target: under ~300 lines); move
    component-shaped rules into renderer lowering.
17. Eliminate the numeric-dimension allowlist (add lattice/dimension steps
    upstream where genuinely needed; the allowlist shrinks to zero or near).
18. **Extend `design-conformance.test.ts`:** ban ad hoc style objects where a
    matrix variant exists (e.g. any `style.borderColor`+`borderWidth` recipe
    on a Stack that duplicates Card/Alert; any `padding`+`typeScale`+`color`
    trio duplicating a Button/Badge variant); ban new `app.css` growth beyond
    the physics allowlist; keep the zero-hex rule.
19. Land the owner directive as a behavior-contract entry (statement
    verbatim, oracle = the extended conformance suite) per the
    behavior-contracts mandate, in the same change as the sweep.

### Phase 4 — Repo-wide closure

20. `packages/ui` + `packages/design-tokens`: confirm no retained surface
    still imports them after the EN-4/CV2 conversions; delete on the
    absorption schedule (`docs/effect-native/2026-07-09-web-absorption-burndown.md`).
    Do not port conventions into them.
21. Gallery/docs pages (C7) as components land.

Sequencing note: Phases 0–1 are the unlock and should run first and fast;
Phase 2 items are independent lanes (good subagent fanout targets); Phase 3
is one coordinated desktop lane after the pin bump.

---

## 8. Invariants and non-negotiables

- **StarCraft design is preserved by construction:** khalaTheme stays the
  only shipped theme; Protoss blue `#3b82f6` stays the primary tone; the
  canonical palette hex values are pinned by test through the primitive-ramp
  refactor; dark-only, no light mode, no `light-dark()`.
- **The styling contract does not move:** typed style objects + deterministic
  merge; no class strings in any public contract (the `data-*` adoption is
  renderer lowering, invisible to the authoring layer).
- **Closed catalog discipline holds:** every new component/variant goes
  through GAPS with a demanding screen, all renderers, and conformance in one
  bump. Apps SDK UI is a conventions reference, never vendored code.
- **No new work in the legacy `--oa-*`/Foldkit stack.**

## 9. Success criteria

1. Desktop renderer contains zero per-call-site style-object recipes that
   duplicate a matrix variant (oracle-enforced, not just swept).
2. `app.css` reduced to host physics; numeric allowlist eliminated or ≤3
   documented entries.
3. Every Apps SDK UI component marked HAVE/PARTIAL/GAP above is either at
   parity, shipped, or registered `waiting` in GAPS with a named reason.
4. One token identity in the repo: `--en-*`/khalaTheme; the `--oa-*` package
   deleted with the legacy web surfaces.
5. Gallery documents every component's variants; an agent can answer "what
   button variants exist" without reading source.

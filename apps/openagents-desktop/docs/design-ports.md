# Design ports

In-repo provenance for presentation-language ports into the OpenAgents
Desktop Effect Native views. Ports translate a reference app's visual
anatomy into typed style objects on the shared `@effect-native/tokens`
vocabulary — never Tailwind/CSS class strings (owner decision 2026-07-08),
never vendored reference code, always our catalog icon set and the one
uniform dark product theme (Autopilot UI since 2026-07-15, #8858, superseding
the historical Protoss-blue theme named in the dated entries below).

## 2026-07-11 — opencode prompt-input → chat composer shape

Owner directive (verbatim): "edit our chat input composer to look exactly like
the opencode desktop, and put our codex/claude toggle in that bar underneath
it. find their css and adapt it to ours."

Source (read-only; nothing vendored):
`projects/repos/opencode/packages/app/src/components/prompt-input.tsx`, the
`newLayoutDesigns` branch (OpenCode's modern prompt-input; it is Tailwind +
`--v2-*` custom properties, not a standalone `.css` file). Extracted spec,
Tailwind/CSS-var values translated to concrete numbers:

| OpenCode prompt-input (newLayoutDesigns) | value | Effect Native (this repo) |
| --- | --- | --- |
| container | `rounded-xl` (12px), `min-h-[96px]`, `bg-v2-background-bg-base` fill, `shadow-[var(--v2-elevation-raised)]`, dashed border only while dragging | `Card` radius `xl` (8, quantized down from their 12 — the 24px composer radius stays excluded), `surface: "glass"`, `border`/1px, `background`/`surface` fill via glass |
| input (editor) | `min-h-[52px]`, `px-4`/`pt-4`/`pb-2` (16/16/8), `text-[13px]`, `leading-5` (20px), `font-[440]`, scroll container `max-h-[180px]` | multiline `TextField` (textarea) on TOP, `minHeight: 64` (documented dimension), body typeScale, app.css `resize:none` + internal scroll |
| bottom action bar | `flex h-11 items-center px-2` (height 44px, px 8px), left group `gap-1` (4px) | `Stack` row `shell-composer-bar`, `gap "1"`, min-height `--en-control-xl-height` (40) |
| attach `+` | `IconButton` `size-7` (28px), `rounded-md` (6px), ghost, `text-v2-icon-icon-muted`, `icon="plus"` | `shell-attach-image` `IconButton` (fixed 44px), `Plus` glyph, `surfaceRaised`/`textMuted`, radius `md` |
| harness/model controls | live in the left area of the bar, after `+` | Fable\|Codex recessed segmented `shell-harness-row`, relocated into the bar after `+` (owner directive) |
| send | `IconButton` `size-7` `rounded-md`, `variant="primary"`, gradient `bg-contrast` fill, `disabled:opacity-50`, `icon="arrow-up"` (or `stop`) | circular `shell-note` (radius `full` on the 44px `IconButton` square), `accent`/`textInverse` when the input has text/images, `surfaceRaised`/`textMuted` ghost when blank; `Stop` shares the circle while streaming |

Layout change: OpenCode already stacks the editor above a bottom action bar
inside one container; the owner target is that exact shape plus the
codex/claude toggle moved into the bar. We flipped our previous
toggle-on-top / inline `[+ input send]` row into `[image thumbnails]` →
`[multiline input]` → `[+  Fable|Codex  ⇢spacer⇢  ●send]`. Every prior feature
(attach picker + drop/paste, harness toggle + Shift+Tab + evidence gating,
image thumbnails, Stop-while-streaming, queue-until-idle, disabled-reason
popovers, `DesktopInputChanged`/`DesktopNoteSubmitted` wiring) is preserved,
re-homed into the bar.

Colors adapted to Protoss-blue tokens (no OpenCode `--v2-*` values used);
styling stays typed token style objects — the only new numeric dimension (the
input `minHeight: 64`) rides the documented `design-conformance.test.ts`
allowlist. Glyph gap: the catalog `IconName` set has no `ArrowUp` (or
paperclip); interim uses `Plane` (send) / `Plus` (attach), recorded as
`D-DESK-09` in `docs/effect-native/DEMAND_REGISTER.md`. Behavior contract:
`openagents_desktop.chat.opencode_composer_shape.v1` in
`apps/openagents-desktop/src/contracts/ux-contracts.ts`.

## 2026-07-11 — opencode tool/message card language → chat transcript cards

Owner directive (verbatim): "Make a design pass through the
projects/repos/opencode desktop app. any of its tool/message card
formatting, we should port its tailwind stuff to our Effect Native, i want
our component slooking just like theirs but adapted to our starcraft blue
etc, and using the openai apps sdk icons we are."

Behavior contract: `openagents_desktop.chat.opencode_card_design_language.v1`
(`src/contracts/ux-contracts.ts`).

Source receipts (read directly from the local reference clone at
`~/work/projects/repos/opencode`; the coordinated extraction spec
`opencode-card-design-spec.md` had not landed when this pass shipped, so the
receipts below are first-hand):

- `packages/session-ui/src/components/basic-tool.css`
  - `[data-component="tool-trigger"]`: dense single-line tool row — 16px
    icon/indicator slot, 8px gaps, title `14px` medium (`--text-strong`),
    inline single-line ellipsized subtitle `14px` regular muted
    (`basic-tool-tool-subtitle`), optional args muted, trailing action slot.
  - `[data-component="task-tool-card"]` (subagent/task tools only): boxed
    row — `padding: 8px 12px`, `border-radius: 6px` (8px in the new layout),
    thin (0.5–1px) weak border, subtle translucent raised background.
  - `BasicTool` (`basic-tool.tsx`): details are a collapsible body,
    **closed by default**; pending status shows an inline indicator in the
    16px slot.
- `packages/session-ui/src/components/message-part.css`
  - assistant column gap 12px; user rows end-aligned `max-width
    min(82%, 64ch)`; `[data-component="diagnostics"]` failure text uses the
    danger foreground as content (not raw payloads); tool output sections
    are separate, selectable, below the trigger.

Translation to Effect Native (this repo, `src/renderer/shell.ts`
`toolCardMessage` + `questionCardMessage`, `src/renderer/tool-cards.ts`):

| opencode | Effect Native (typed tokens) |
| --- | --- |
| tool-trigger row, 8px gap | `Stack` row, `gap: "2"` |
| 16px icon slot | catalog `Icon` `size: "sm"` (16px), our icon set |
| title 14px medium `--text-strong` | `Text` `variant: "label"` (14/500) `weight: "medium"` `color: "textPrimary"` |
| inline muted ellipsized subtitle | `Text` `variant: "body"` `color: "textMuted"` on the same row, pre-bounded by the humanizer (160 chars) |
| pending indicator / completion state | toned catalog `Badge` chip (Running/OK/Failed — neutral/success/danger) |
| collapsible details, closed by default | bounded raw args/result behind the compact `details` toggle (`DesktopToolCardToggled`) |
| task-tool-card box (8px/12px, 6px radius, thin border, translucent bg) | catalog `Card` `padding: "2"` `radius: "md"` `borderWidth: 1` `borderColor: "border"` `surface: "glass"` — applied to agent-class tools only (`Agent`, `mcp__codex__*`) |
| diagnostics danger text | failure text as `Text` `color: "danger"` content line |
| light/dark theming | NOT ported — uniform Protoss-blue dark theme only |

Deliberate deviations:

- opencode shows no explicit "OK" chip on completed tools; our cards keep
  the toned status chip because the EP250 tool-card behavior contract
  requires the started → ok/failed chip on the same updating card.
- opencode's whole trigger row toggles its collapsible; our expand
  affordance is a dedicated compact keyboard-focusable button (the same
  compact details pattern the message rows use).
- The question card (no opencode equivalent) reuses the same family:
  header chip, dense option Buttons with dim caption descriptions.

## 2026-07-11 — apps-sdk-ui chrome language → every non-message surface

Owner directive (verbatim): "do a separate design pass of
projects/repos/apps-sdk-ui and thats what i want to use for the rest of the
app chrome, menus, etc, everything other than messages, but still harmonized
to messages. we want that design language, ported to starcraft kinda,
represented in EVERY other surface of the app"

Behavior contracts:
`openagents_desktop.chrome.apps_sdk_chrome_design_language.v1`,
`openagents_desktop.chrome.disabled_control_reason_popover.v1`,
`openagents_desktop.chat.new_chat_autofocuses_composer.v1`
(`src/contracts/ux-contracts.ts`). Mechanical oracle:
`src/renderer/design-conformance.test.ts`.

Source receipts (read directly from the local reference clone at
`~/work/projects/repos/apps-sdk-ui`; extraction spec
`apps-sdk-chrome-design-spec.md` in the EP250 scratchpad):

- `src/styles/variables-primitive.css` — the alpha ramp (`--alpha-*`: one
  base color at fixed opacities powering ALL hover/active fills and hairline
  borders — state changes are translucent overlays of one base color, never
  new hues), the symmetric 25-step gray ramp, hairline + shadow geometry
  (dark alphas .2/.2/.36/.3).
- `src/styles/variables-semantic.css` — surfaces (elevation = LIGHTER
  surface; elevated surfaces drop their hairline), text ladder
  (text/secondary/tertiary/inverse), translucent borders
  (subtle alpha-06 / border alpha-12 / strong alpha-20), the control metric
  lattice (`--control-size/gutter/font/radius-*`), motion
  (`--cubic-enter (0.19,1,0.22,1)`, exit `(0.8,0,0.4,1)`, 150ms basic),
  radius scale, disabled recipe.
- `src/components/{Button,Menu,Select,Input,Badge,SegmentedControl,Popover,
  Tooltip,Alert,EmptyMessage}.module.css` — component anatomy: ghost/solid/
  soft/outline state stepping, menu 6px gutter + item pad 6×8 + nested inner
  radius (outer − gutter), overlay `shadow + hairline` + 350ms enter / 200ms
  exit scale(.95→1), segmented recessed track + elevated thumb, pill gutter
  ×1.33, press-scale width buckets (`src/lib/helpers.ts:22-35`).

Harmonization rule (chrome × message cards): chrome follows apps-sdk-ui
geometry, cards follow OpenCode geometry, and BOTH are quantized onto the
single `@effect-native/tokens` scale — any off-scale value snaps to the
nearest step; the scale is never widened for one component. One shared
state-fill engine (`stateHover`/`stateActive`/`stateSelected`), one motion
set (150/350/200ms), one focus ring, one dim ladder
(`textPrimary > textMuted > textFaint > textDisabled`).

Translation to Effect Native (this repo):

| apps-sdk-ui | Effect Native (typed tokens) |
| --- | --- |
| alpha-overlay state engine (`--alpha-*` fills) | `stateHover`/`stateActive`/`stateSelected` color tokens (upstream `@effect-native/tokens`), applied by the vendored DOM renderer's chrome base ruleset to buttons + nav items |
| gray-scale "primary" solids (neutral brand) | **Protoss substitution**: solid primaries are `accent` blue with `accentHover`/`accentActive` steps — the one deliberate hue departure |
| surfaces ladder + elevated overlays | `background`/`surface`/`surfaceRaised`/`surfaceOverlay`; overlays carry `--en-elevation-overlay-shadow` + hairline ring; in-flow panels stay border-only (`borderSubtle`) |
| text-tertiary / inverse / disabled | `textFaint` / `textInverse` / `textDisabled` (new roles) |
| translucent borders subtle/base/strong | `borderSubtle` / `border` / `borderStrong` |
| motion (150ms ease, 350ms cubic-enter, 200ms cubic-exit) | theme `motion` group -> `--en-motion-*`/`--en-ease-*`; palette + tooltip enter animations in app.css |
| 9-step control lattice | trimmed 4-step theme `control` group (sm 24/8/14, md 28/10/16, lg 32/12/18, xl 40/14/20) -> `--en-control-*` |
| menu/popover panel recipe (6px gutter, radius 12, item 6×8, nested radius) | command palette Card: `surfaceOverlay` + `borderSubtle` + radius `xl` (8, quantized down from their 12) + padding "1.5"; ghost rows at nested radius `sm` |
| segmented control (recessed track + elevated thumb) | composer harness chips: track `background` + padding "0.5" + radius `lg`; selected chip `surfaceRaised` at nested radius `md` |
| compact chrome tooltip | catalog Tooltip on the overlay recipe (disabled-control reason popover) |
| focus ring 2px, offset 2 | renderer chrome: `outline: 2px solid focus; outline-offset: 2px` on :focus-visible |
| press-scale (width-bucketed 0.96–0.995) | single-bucket `scale(0.98)` on :active (deviation — CSS cannot width-bucket without JS measurement) |
| radius roles | quantized onto khala 2/4/6/8: controls `lg`/`xl`, overlays `xl`, badges `md`, nested `sm`; theme.ts radius drift (8/12) corrected by consuming `khalaTheme` directly |

Deliberately NOT ported (per the spec's exclusion list): the light theme and
`light-dark()` machinery; the `caution` (yellow) intent (collapsed into
`warning`); the `discovery` (purple) intent; the `pink` primitive family;
the 24px composer radius and 16-24px radius steps (cap is `xl` 8);
neutral-inverted white primary solids (Protoss `accent` instead — and solid
accent keeps light text rather than `textInverse`, matching their BLUE
family's white-on-blue treatment; `textInverse` stays reserved);
the translucent backdrop-blur popover variant; the 9-step control lattice;
their icon set (ours stays); per-character shimmer (RN-safe 1200ms opacity
wave instead, timing kept).

Upstream receipts: `OpenAgentsInc/effect-native` — 12 new color roles +
`motion`/`elevation`/`control` ThemeSchema groups + render-dom chrome base
ruleset and `--en-motion/elevation/control-*` lowering (GAPS register row
2026-07-11); vendored snapshot re-pinned in
`apps/openagents.com/packages/effect-native-vendor.json`.

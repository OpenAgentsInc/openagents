# Institute Noir — Default Theme & Zero-Base UI Specification

**Status:** Specification (`2026-11-27`). Ready for implementation.  
**Canonical Name:** **Institute Noir** (`institute-noir` / `UISKIN_INSTITUTE_NOIR`)  
**Default Scope:** Site-wide default across `openagents.com` (`/`, `/login`, `/onboarding`, `/chat`, `/settings/*`, `/r/*`, `/referrals`, `/components`, `/books`), plus first-class `UiSkin` parity for OpenAgents Desktop (`apps/openagents-desktop/`) and OpenAgents iOS (`apps/openagents-ios/`).  
**Visual Reference:** [`kitlangton/effect-institute`](https://github.com/kitlangton/effect-institute) (MIT License; audited at commit `89878a9`).

---

## 1. Purpose & Zero-Base Context

OpenAgents is zero-basing its web and chat UI (see conversation `defd9140-0129-41f0-9e91-30fbd09bc232` and [docs/audits/2026-11-11-chat-zero-base-audit.md](../audits/2026-11-11-chat-zero-base-audit.md)). Today, the web workspace concatenates three competing visual systems inside `apps/openagents-web/chat-web.coax` (`mono_css` ARWES chamfered SVG frames + `apps_sdk_css` rounded pill tokens + `cwc_css` pure-black overrides) alongside disparate inline `<style>` sheets in `apps/openagents-site/index.html`, `apps/openagents-web/chat/index.html`, `apps/openagents-web/login/index.html`, `apps/openagents-web/onboarding/index.html`, and `apps/openagents-web/settings/*/index.html`.

**Institute Noir** replaces that multi-layer stack with one cohesive, precision-instrument aesthetic adapted faithfully from **Effect Institute**:

- Pure OLED `#000000` canvas everywhere — never tinted grey sidebars or grey cards.
- Zero-radius (`0px`) architectural grid geometry separated by crisp `1px` hairlines (`#222222` / `#333333`).
- `Commit Mono` Variable monospace across **all** UI chrome, prose, controls, and code.
- Warm off-white primary ink (`#f1efe8`) paired with calibrated white alpha ladders.
- Emerald-only semantic success/credits (`#228c64` → `#2aaf7e` → `#34d399`), GitHub Dark syntax highlighting, and critically damped (`bounce: 0`) spring physics.
- One single source of truth in Coax (`components/skin/institute-noir.coax`) that emits both the complete web stylesheet (`institute_noir_css`) and the native GPU `Theme` / `UiSkin` (`institute_noir()` / `institute_noir_skin()`).

---

## 2. The Eight Design Invariants of Institute Noir

Every OpenAgents surface wearing **Institute Noir** must obey these eight rules:

1. **True Black (`#000000`) Ground Everywhere.**
   Page background, sidebar, headers, footers, cards, dropdowns, modals, and code blocks rest on `#000000`. Never use `#050505`, `#0a0a0a`, `#0c0c0e`, or `#0e0e0e` as resting chrome shells. Surface separation comes from `1px` borders (`#222222`), subtle hover/active fills (`#080808` / `#141414`), or a `2%` white wash (`rgba(255, 255, 255, 0.02)`) for banner bands.
2. **Sharp Architectural Geometry (`border-radius: 0`).**
   Nav bars, buttons, inputs, cards, menus (`rounded-none`), badges, `<kbd>` keycaps, and step progress squares (`6×6px`) use `0px` corner radius. No rounded `12px–20px` card shells and no `9999px` pill buttons in structural chrome. Only inherently circular primitives (circular progress rings, slider track/thumb, live pulsing status dots) may be round.
3. **100% Monospace Typography (`Commit Mono` Variable).**
   A single variable monospace typeface (`Commit Mono`, weights `200–700`, SIL OFL 1.1) renders headings, labels, navigation, prose, buttons, tabular numbers, and code. Global tracking is `0.02em`; structural micro-headers are uppercase `12px` with `0.3em` tracking (`text-white/40`); menu section labels are uppercase `11px` with `0.18em` tracking (`text-white/50`).
4. **Warm Off-White Primary Ink (`#f1efe8`) & Pure White Alpha Ladder.**
   Primary text uses `#f1efe8` rather than sterile blue-white `#ffffff` or `#ededed`. Secondary/tertiary chrome, hairline washes, and hover states step down a strict `rgba(255, 255, 255, α)` ladder (`0.80` down to `0.02`).
5. **Strict Hue Restraint (No Brand Blue Chrome).**
   Remove blue focus rings (`#0285ff`, `#346bf1`) and accent chrome from buttons, inputs, and links. Keyboard focus rings are monochrome `2px solid rgba(255, 255, 255, 0.35)` (`outline-offset: 2px`, `:focus-visible` only). Color appears exclusively for semantic status (emerald completion/credits, blue active execution, red failure/destructive, orange/amber interruption/draft) and syntax highlighting inside code.
6. **48px Bordered Grid Bars (`NAV_BAR_HEIGHT = 48px`).**
   Top headers, bottom footers, and sticky section bars are strictly `48px` tall, bordered by `1px solid #222222`, and subdivided into full-height action cells via `1px` vertical `border-l` / `border-r` dividers. Hovering a cell shifts fill to `rgba(255, 255, 255, 0.05)`–`0.06` without changing geometry.
7. **Critically Damped Springs & Blur-Fade Entrances.**
   All structural and cursor motion defaults to critically damped springs (`bounce: 0`, `visualDuration: 0.2s–0.5s`) with subtle `blur(4px) → blur(0px)` materializations. Scrubbable or streaming views keep elements mounted and animate targets rather than unmounting/remounting. All motion collapses to instant state changes under `prefers-reduced-motion: reduce`.
8. **Calm Instrument Pointer & Crisp Floating Telemetry.**
   Interactive chrome maintains `cursor: default` (no jarring arrow-to-hand flicker across adjacent grid cells; text inputs retain `cursor: text`). Hover affordances come from instant brightness/fill shifts and optional rAF-throttled floating shortcut tooltips (`#171717`, `1px #222222`, uppercase micro-title + `<kbd>` chip).

---

## 3. Exact Design Token Specification

Every value below is transcribed verbatim from `kitlangton/effect-institute` (`src/styles.css`, `src/components/chapters/constants.ts`, `src/lib/neo-code-block/annotation-colors.ts`, `src/lib/neo-code-block/constants.ts`, `src/lib/visual-effect/`, `src/components/narrated/pixi-effect-row/engine/palette.ts`, and compiled Shiki `github-dark` output).

### 3.1 Core Surfaces, Borders & Text

| CSS Custom Property | Value | Hex / Computed | Role |
| --- | --- | --- | --- |
| `--in-ground` | `#000000` | `#000000` | Page, sidebar, header, footer, card, menu, code block ground |
| `--bg-hover` | `#080808` | `#080808` | Interactive row / card / prompt hover fill |
| `--bg-active` | `#141414` | `#141414` | Active group header, selected row, pressed surface |
| `--in-tooltip-bg` | `#171717` | `#171717` | Floating cursor tooltips (`bg-neutral-900`), elevated badges |
| `--in-banner-wash` | `rgba(255, 255, 255, 0.02)` | — | Subtle band wash (ticker strip, banner header, callout strip) |
| `--border-subtle` | `#222222` | `#222222` | Universal `1px` grid rail, dividers, card/menu/input resting edges |
| `--border-inactive` | `#222222` | `#222222` | Inactive panel / cell hairline |
| `--border-active` | `#333333` | `#333333` | Focused input, active card edge, emphasized divider |
| `--color-text` | `#f1efe8` | `#f1efe8` | Primary body copy, warm off-white ink (`18.3:1` on `#000`) |
| `--color-accent` | `rgba(255, 255, 255, 0.10)` | — | Subtle hover/selection fill on menus and kbd chips |
| `--color-accent-foreground` | `#f1efe8` | `#f1efe8` | Text/icon on `--color-accent` |
| `--color-muted-foreground` | `rgba(255, 255, 255, 0.45)` | `~#737373` | Secondary/muted copy (`4.5:1` WCAG AA on `#000`) |
| `--color-destructive` | `oklch(0.704 0.191 22.216)` | `~#ff6467` | Destructive actions, danger labels |
| `--in-focus-ring` | `rgba(255, 255, 255, 0.35)` | — | `:focus-visible` outline (`2px solid`, `outline-offset: 2px`) |
| `--in-active-glow` | `rgba(255, 255, 100, 0.80)` | — | Active spoken/streaming focus glow (`0 0 9.6px`) |

### 3.2 Pure White Alpha Opacity Ladder

| Token | Value | Contrast on `#000` | Usage |
| --- | --- | --- | --- |
| `--white-90` | `rgba(255, 255, 255, 0.90)` | `16.9:1` (AAA) | High-emphasis node/icon foreground |
| `--white-80` | `rgba(255, 255, 255, 0.80)` | `12.6:1` (AAA) | Active nav links, hovered bar titles, dropdown actions |
| `--white-70` | `rgba(255, 255, 255, 0.70)` | `9.7:1` (AAA) | Banner titles, dropdown items, kbd text |
| `--white-60` | `rgba(255, 255, 255, 0.60)` | `7.4:1` (AAA) | Nav bar titles, toolbar icons, prompt labels |
| `--white-50` | `rgba(255, 255, 255, 0.50)` | `5.3:1` (AA) | Descriptions, secondary metadata, resting header links |
| `--white-45` | `rgba(255, 255, 255, 0.45)` | `4.5:1` (AA) | Minimum AA body muted text (`--color-muted-foreground`) |
| `--white-40` | `rgba(255, 255, 255, 0.40)` | `3.7:1` (large/UI) | Tracked uppercase section headers (`0.3em`), disabled labels |
| `--white-30` | `rgba(255, 255, 255, 0.30)` | — | Inactive/upcoming text, disabled icons, subtle hints |
| `--white-20` | `rgba(255, 255, 255, 0.20)` | — | Progress ring track, slider track, OG structural borders |
| `--white-15` | `rgba(255, 255, 255, 0.15)` | — | Inline code border, `<kbd>` cap fill |
| `--white-12` | `rgba(255, 255, 255, 0.12)` | — | Incomplete `6×6px` progress square |
| `--white-08` | `rgba(255, 255, 255, 0.08)` | — | Inline code wash, dot grid resting opacity, active sub-row |
| `--white-06` | `rgba(255, 255, 255, 0.06)` | — | Nav bar cell hover fill, prompt entrance flash |
| `--white-04` | `rgba(255, 255, 255, 0.04)` | — | Keyboard-selected row wash, scrollbar track |

> **Accessibility Note:** Effect Institute uses `white/40` on decorative uppercase micro-headers (`CHAPTERS`, `HOTKEYS`). On OpenAgents surfaces where muted text conveys actionable or required information (timestamps, credit subtitles, form hints), enforce `--color-muted-foreground` (`rgba(255, 255, 255, 0.45)` or `--white-50`) so WCAG AA `4.5:1` holds across all readable copy.

### 3.3 Semantic Status & Agent Execution Palette

| Semantic Role | Token / Constant | Hex / Value | Glow / Border Accent |
| --- | --- | --- | --- |
| **Success (Rest)** | `--color-success` | `rgb(34, 140, 100)` (`#228c64`) | Completed progress squares, partial ring, credit meter fill |
| **Success (Active)** | `--color-success-mid` | `rgb(42, 175, 126)` (`#2aaf7e`) | Active completed step indicator |
| **Success (Bright)** | `--color-success-bright` | `rgb(52, 211, 153)` (`#34d399`) | Full ring, completion flash, `0 0 6px 1px rgba(52, 211, 153, 0.45)` |
| **Task Completed** | `PALETTE.completed` | `#15803d` (green-700) / `#16a34a` (OG) | `rgba(74, 222, 128, 0.70)` border |
| **Task Running** | `PALETTE.running` | `#3b82f6` (blue-500) | Border `#64c8ff`, shadow `0 0 24px rgba(59, 130, 246, 0.20)` |
| **Task Idle** | `PALETTE.idle` | `#475569` (slate-600) / `#27272a` (OG) | `rgba(255, 255, 255, 0.18)` border |
| **Task Failed / Error** | `PALETTE.failed` | `#ef4444` (red-500) | Bubble `rgba(239, 68, 68, 0.95)`, border `rgba(248, 113, 113, 0.70)` |
| **Task Interrupted / Warn** | `PALETTE.interrupted` | `#f97316` (orange-500) | Draft/warning badge: `rgba(245, 158, 11, 0.20)` bg, `#fbbf24` text, `rgba(245, 158, 11, 0.30)` border |
| **Task Terminated / Death** | `PALETTE.death` | `#0b0b0b` | Border `rgba(220, 38, 38, 0.40)`, glow `rgba(220, 38, 38, 0.80)` |
| **Line Diff / Callout Wash** | `LINE_BG_*` | `green/red/yellow/blue-500` @ `20%` | `rgba(34,197,94,.2)`, `rgba(239,68,68,.2)`, `rgba(234,179,8,.2)`, `rgba(59,130,246,.2)` |

### 3.4 Code Annotation Palette (`annotation-colors.ts`)

Eight base RGB channels for code marks (`rgba(r, g, b, 0.30)` fill + `rgba(r, g, b, 0.80)` `1px` border), glows (`rgba(r, g, b, 1.0)` at `32px` blur, `0.35` opacity), and squiggles:

| Name | RGB | Hex |
| --- | --- | --- |
| `yellow` *(default mark)* | `234, 179, 8` | `#eab308` |
| `green` *(default glow)* | `34, 197, 94` | `#22c55e` |
| `blue` | `59, 130, 246` | `#3b82f6` |
| `red` *(default squiggle)* | `239, 68, 68` | `#ef4444` |
| `purple` | `168, 85, 247` | `#a855f7` |
| `pink` | `236, 72, 153` | `#ec4899` |
| `orange` | `249, 115, 22` | `#f97316` |
| `gray` (`grey`) | `107, 114, 128` | `#6b7280` |

### 3.5 GitHub Dark Syntax Palette → OpenAgents `SYN_*` Roles

Effect Institute highlights code at build time with Shiki `github-dark`. Mapping those exact hex values onto OpenAgents' 12 token roles (`components/core/syntax.coax`):

| `SYN_*` Index & Role | Hex | GitHub Dark Scope |
| --- | --- | --- |
| `0` `SYN_COMMENT` | `#6A737D` | Comments, docblocks |
| `1` `SYN_KEYWORD` | `#F97583` | Keywords, control flow, operators (`=`, `:`, `<`, `*`, `...`) |
| `2` `SYN_STRING` | `#9ECBFF` | String literals, template strings |
| `3` `SYN_NUMBER` | `#79B8FF` | Numbers, constants, booleans, built-in primitives |
| `4` `SYN_TYPE` | `#B392F0` | Types, classes, interfaces, error tags |
| `5` `SYN_FUNCTION` | `#B392F0` | Function names, method calls |
| `6` `SYN_PROPERTY` | `#79B8FF` | Object keys, struct fields, properties |
| `7` `SYN_VARIABLE` | `#FFAB70` | Parameters, local bindings (plain identifiers default to `#E1E4E8`) |
| `8` `SYN_PUNCTUATION` | `#E1E4E8` | Braces, parens, commas, default text |
| `9` `SYN_TAG` | `#85E89D` | HTML / JSX / XML tags |
| `10` `SYN_ATTRIBUTE` | `#B392F0` | Attributes, decorators, directives |
| `11` `SYN_INVALID` | `#EF4444` | Invalid syntax / error tokens |

### 3.6 Typography, Breakpoints & Scrollbars

- **Font Family:** `'Commit Mono', ui-monospace, monospace` (variable WOFF2 `CommitMono-Variable.woff2`, `86.8 KB`, `font-weight: 200 700`, `font-display: swap`, preloaded in `<head>` with `rel="preload" as="font" type="font/woff2" crossorigin`).
- **Base Rendering:** `text-rendering: optimizeLegibility` (prose) / `geometricPrecision` (code blocks), `-webkit-font-smoothing: antialiased`, `-moz-osx-font-smoothing: grayscale`, `letter-spacing: 0.02em`, `color-scheme: dark`.
- **Type Ramp:**
  - Root `html` prose base: `18px` / `line-height: 1.75` (dense chat/workspace surfaces step to `14px`–`15px` / `line-height: 1.55`).
  - Responsive workspace scale (from `responsiveSettings.ts`):
    - `mobile` (`< 768px`): `14px`, `line-height: 1.55` (`21.7px`)
    - `narrow` (`768–1139px`): `15px`, `line-height: 1.55` (`23.25px`), left rail max `460px`
    - `medium` (`1140–1399px`): `17px`, `line-height: 1.55` (`26.35px`), left rail max `560px`
    - `full` (`>= 1400px`): `19px`, `line-height: 1.55` (`29.45px`), left rail max `640px`
  - **Architectural Micro-Headers:** `12px`, `font-weight: 400–500`, `text-transform: uppercase`, `letter-spacing: 0.30em`, `color: rgba(255, 255, 255, 0.40)`.
  - **Menu / Card Section Labels:** `11px`, `text-transform: uppercase`, `letter-spacing: 0.18em`, `color: rgba(255, 255, 255, 0.50)`.
  - **Status Badges / Keycaps:** `10px–11px`, `font-weight: 500`, `text-transform: uppercase`, `letter-spacing: 0.05em`.
  - **Inline Code Span:** `0.9em`, `background: rgba(255, 255, 255, 0.08)`, `border: 1px solid rgba(255, 255, 255, 0.15)`, `padding: 0.15em 0.4em`, `border-radius: 0`.
  - **Inline Output Pill:** `background: rgba(39, 39, 42, 0.50)`, `color: #a1a1aa`, `box-shadow: inset 0 0 0 1px rgba(63, 63, 70, 0.50)`, `padding: 0 8px`, `border-radius: 0`.
- **Scrollbars:** Hidden on clean chrome/code rails (`.hide-scrollbar`); thin dark custom scrollbar on log panes (`width: 6px`, track `rgba(255,255,255,0.04)`, thumb `rgba(64,64,64,0.50)` → `0.70` hover).

### 3.7 Motion & Physics Constants

| Preset / Element | Spring / Curve | Duration / Params | Source |
| --- | --- | --- | --- |
| `fast` | Critically damped spring | `visualDuration: 0.2s`, `bounce: 0` | `animationConfigs.ts` |
| `default` | Critically damped spring | `visualDuration: 0.3s`, `bounce: 0` | `animationConfigs.ts` |
| `smooth` | Critically damped spring | `visualDuration: 0.4s`, `bounce: 0` | `animationConfigs.ts` |
| `bouncy` *(celebration only)* | Underdamped spring | `visualDuration: 0.3s`, `bounce: 0.3` | `animationConfigs.ts` |
| Code line translate & blur | Critically damped spring | `visualDuration: 0.45s`, `bounce: 0`, `blur(4px) → 0` | `neo-code-block/constants.ts` |
| Code part width/opacity | Critically damped spring | `visualDuration: 0.4s`, `bounce: 0`, `restDelta: 0.1` | `neo-code-block/constants.ts` |
| Code cursor movement | Critically damped spring | `visualDuration: 0.5s`, `bounce: 0` | `neo-code-block/constants.ts` |
| Spotlight / bright fade | Linear/ease | `0.3s`, dim opacity `0.4` | `neo-code-block/constants.ts` |
| Rolling header swap | Gentle spring | `visualDuration: 0.35s`, `bounce: 0.15`, vertical mask fade | `RollingGroupHeader.tsx` |
| Button press flash | Two-phase color pulse | `#ffffff` (`50ms`) → `rgba(255,255,255,0.6)` (`300ms`) | `SectionIndicator.tsx` |
| Progress square completion | Gentle spring | `visualDuration: 0.6s`, `bounce: 0.05` + emerald glow `0.4s/1.0s` | `RollingGroupHeader.tsx` |
| Pulsing Dot Grid ripple | `easeOutExpo` Gaussian wave | `900ms`, radius `180px`, `waveWidth: 50px`, damping `e^(-3t)` | `PulsingDotGrid.tsx` |

---

## 4. OpenAgents Surface-by-Surface Adaptation (Site Default)

### 4.1 Public Root (`apps/openagents-site/index.html` & `apps/openagents-web/index.html`)
- Replace `--surface: #0c0c0e`, `--fg: #ffffff`, system sans font, and `border-radius: 18px` `.launch-card` with **Institute Noir**:
  - Critical `<head>` CSS: `html,body{background:#000000;margin:0;color:#f1efe8}` + preload `/fonts/CommitMono-Variable.woff2`.
  - Centered `max-w-2xl` (`672px`) column with full-height `1px solid #222222` left/right borders (`border-x`), `48px` top nav strip (`border-b #222222`), optional `PulsingDotGrid` subtle canvas in the hero cell, sharp `0px` countdown/launch frame (`border-y #222222`, `rgba(255,255,255,0.02)` band wash), `tracking-[0.3em]` uppercase `V1.0.0 COMING SOON`, and `48px` bottom footer strip (`border-t #222222`).
  - Keep the zero-scroll-bounce `100dvh` lock and synchronous `.inside` session gate so logged-in refreshes never flash the landing page.

### 4.2 Auth, Onboarding & Referral Landing (`apps/openagents-web/login-web.coax`, `/login`, `/onboarding`, `/r/<code>`)
- Drop `apps_sdk_css` and `lw_css`'s `#111111` / `8px` rounded buttons and `#3b82f6` input focus rings.
- Render centered `max-w-[380px]` sharp architectural card (`#000000` ground, `1px solid #222222` outer frame, internal `1px` dividers):
  - Header block: OpenAgents mark + `OpenAgents` in `Commit Mono`, subtitle in `rgba(255,255,255,0.50)`.
  - Secondary OAuth buttons (Google / Apple / GitHub): `height: 44px`, `border-radius: 0`, `#000000` fill, `1px solid #222222`, hover `#080808` + `#333333` border, `#f1efe8` text.
  - Primary CTA (`Continue with GitHub` on `/onboarding`, `Continue` on email): `height: 44px`, `border-radius: 0`, `#f1efe8` fill, `#000000` ink, hover `#ffffff`.
  - Email / 6-digit code inputs: `height: 44px`, `border-radius: 0`, `#000000` fill, `1px solid #222222`, focus `#333333` border + `2px solid rgba(255,255,255,0.35)` ring.
  - Fixed `48px` bottom footer bar (`border-t: 1px solid #222222`).

### 4.3 Zero-Based Cloud Chat Workspace (`apps/openagents-web/chat-web.coax` & `chat/chat-cloud.coax`)
- **Single Stylesheet:** Replace the 3-layer `mono_css` + `apps_sdk_css` + `cwc_css` concatenation in `chat-web.coax` with a single `institute_noir_css(out)` call. Remove legacy ARWES SVG frames (`svg.fr`), scanlines, and rounded OpenAI pills completely.
- **Zero-Based Fresh Sidebar (`260px` width, `#000000` ground, `border-right: 1px solid #222222`):**
  - **Top Brand Header (`48px`, `border-bottom: 1px solid #222222`):** Left cell has OpenAgents mark + `OpenAgents` wordmark (`Commit Mono` 500, `#f1efe8`); right cell is a full-height `48×48px` collapse toggle separated by `border-left: 1px solid #222222` (`rgba(255,255,255,0.60)`, hover `#ffffff` on `rgba(255,255,255,0.06)`).
  - **Middle Rail:** Clean `#000000` rail on initial zero-base start. When thread/project rows are enabled, render full-bleed `0px` rows separated by `border-top: 1px solid #222222` (`hover: #080808`, `active: #141414`) with `6×6px` square status indicators (`rgba(255,255,255,0.12)` idle, `#3b82f6` running, `#228c64` completed, `#ef4444` failed).
  - **Pinned Bottom Credits Cell (`border-top: 1px solid #222222`, flush `0px` geometry — USD only, zero sats/Bitcoin UI in v1.0):**
    - Top row: uppercase `CREDITS` micro-label (`11px`, `tracking-[0.18em]`, `white/45`) left-aligned; `tabular-nums` USD balance (`$10.00`, `#f1efe8` or `#34d399` emerald when promo credit is active) right-aligned.
    - Hairline `2px` progress track (`rgba(255,255,255,0.12)` background, `#228c64` emerald fill).
    - Sub-label: `FREE CREDIT` (`11px`, `white/45`) — strictly USD-denominated (never show `sats` or Bitcoin units in v1.0 web chrome).
    - Full-width flush footer button (`height: 40px`, `border-top: 1px solid #222222`, `#000000` → `#080808` hover, uppercase `tracking-[0.18em]`, `ADD CREDITS`).
- **Top Workspace Bar (`48px`, `border-bottom: 1px solid #222222`):** Sharp bordered cells — active conversation title (`#f1efe8`, left), execution target & model selector as a bordered `border-l` cell (not a floating pill) with emerald/blue `6×6px` status square.
- **Empty State & Transcript Canvas:**
  - Empty canvas optionally hosts `PulsingDotGrid` (`3×3px` dots, `18px` spacing, `0.08` opacity) behind a sharp `1px #222222` centered welcome/composer block.
  - Transcript measure: `max-width: 72ch`, `Commit Mono`, `#f1efe8` assistant prose, user turns delineated by a `1px solid #222222` left rail or full-width hairline separator with `#080808` wash (no rounded speech bubbles).
  - Tool call / agent step rows: `0px` bordered blocks (`1px solid #222222`), `6×6px` status square + uppercase tool name + monospace args + GitHub Dark output/diff pane.
  - **Composer:** Sharp `0px` bottom-anchored frame (`#000000` ground, `1px solid #222222`, `#333333` on `:focus-within`), `border-top: 1px solid #222222` utility strip, square `#f1efe8`/`#000000` `SEND` / `#ef4444` `STOP` cell.

### 4.4 Settings, Appearance & Hotkeys (`/settings/*`)
- `/settings/appearance` exposes **Theme** selection with **Institute Noir** (`institute-noir`) as the site default, persisted in `localStorage` (`openagents:theme`).
- Add a clean **Hotkeys** customization view modeled on Effect Institute's `/hotkeys` (`src/routes/hotkeys.tsx`): grouped by category, 2-key chord capture (`600ms` timeout), inline conflict badges, and per-action / global reset.

### 4.5 Cross-Platform Coax `UiSkin`, Desktop & iOS Mapping

New canonical skin file: **`components/skin/institute-noir.coax`**, registered in `components/skin/all.coax` as `UISKIN_INSTITUTE_NOIR` (making `UISKINS :: 5` or setting it as the default pixel/web skin).

Exact `Theme` (`components/skin/zeron.coax`) field mapping for `institute_noir() -> Theme`:

| `Theme` Field | Institute Noir Value | Notes |
| --- | --- | --- |
| `shell`, `sidebar`, `base`, `bg` | `hex(0x000000, 1)` | True OLED black everywhere |
| `surface`, `composer_tint` | `hex(0x000000, 1)` | Composer rests on `#000000` with `1px #222222` frame |
| `raised`, `secondary`, `menu`, `popover` | `hex(0x141414, 1)` *(or `0x000000` w/ `#222222` edge for dropdowns)* | Matches `--bg-active` / `#171717` tooltip elevation |
| `secondary_hover` | `hex(0x080808, 1)` | Matches `--bg-hover` |
| `border`, `line`, `line_subtle` | `hex(0x222222, 1)` | Matches `--border-subtle` / `--border-inactive` |
| `line_strong` | `hex(0x333333, 1)` | Matches `--border-active` |
| `text`, `accent`, `primary` | `hex(0xf1efe8, 1)` | Warm off-white ink & monochrome primary fill |
| `accent_hover`, `primary_hover` | `hex(0xffffff, 1)` | Crisp white on hover |
| `accent_active`, `primary_active` | `hex(0xd6d4cd, 1)` | Subtle press dim |
| `muted` | `rgba(1, 1, 1, 0.50)` | Secondary text (AA compliant) |
| `faint` | `rgba(1, 1, 1, 0.40)` | Micro-headers, tertiary labels |
| `disabled` | `rgba(1, 1, 1, 0.30)` | Disabled controls (`OPACITY_INACTIVE`) |
| `hover`, `menu_selected` | `rgba(1, 1, 1, 0.06)` | Matches `hover:bg-white/6` |
| `pressed`, `selected`, `row_selected` | `rgba(1, 1, 1, 0.10)` | Matches `--color-accent` |
| `selection` | `rgba(1, 1, 1, 0.20)` | Monochrome text selection wash |
| `focus_ring` | `rgba(1, 1, 1, 0.35)` | Matches `:focus-visible` `2px` ring |
| `success` | `hex(0x228c64, 1)` | `--color-success` |
| `done` | `hex(0x34d399, 1)` | `--color-success-bright` |
| `info`, `connecting` | `hex(0x3b82f6, 1)` | `PALETTE.running` |
| `input` | `hex(0x64c8ff, 1)` | `PALETTE.runningBorder` |
| `warning`, `limited`, `approval` | `hex(0xf97316, 1)` | `PALETTE.interrupted` |
| `danger`, `failed` | `hex(0xef4444, 1)` | `PALETTE.failed` |
| `plan` | `hex(0xb392f0, 1)` | GitHub Dark purple (`#B392F0`) |
| `syntax[0..11]` | `INSTITUTE_SYNTAX` | Exact 12-slot GitHub Dark array from §3.5 |
| Radii (`cmp.radius`, `app.row_radius`, `app.sheet_radius`, `set.card_radius`, `set.button_radius`, `md.block_radius`, `md.code_radius`) | `0.0f32` | Zero-radius architectural geometry |

- **Desktop (`apps/openagents-desktop/page/customize.coax` & `page/skin.coax`):** Add `CP_SKIN_INSTITUTE_NOIR` (`"Institute Noir"`: *"Pure-black architectural monospace — sharp hairlines, warm ink, emerald state."*). Optionally bundle `CommitMono-Variable.ttf` alongside Geist/DM Sans in `components/fonts/`.
- **iOS (`apps/openagents-ios/Sources/`):** Align `OATheme` constants with Institute Noir (`background/surface = #000000`, `elevated = #141414`, `border = #222222`, `textPrimary = #F1EFE8`, `textSecondary = white 50%`, `statusReady = #34D399`, `statusError = #EF4444`, `statusRunning = #3B82F6`, monospace font option via `Commit Mono`).

---

## 5. Complete Harvest Inventory from `kitlangton/effect-institute`

OpenAgents is **Coax-first** (no React/Tailwind/TanStack/Effect-TS runtime in shipped web/desktop/iOS apps). We harvest **design tokens, font binaries, algorithms, shaders, interaction patterns, and test methodologies** — ported cleanly into Coax, CSS, WGSL/Metal, and minimal vanilla host bindings.

### 5.1 Harvest Priority Matrix

| Tier | Asset / Subsystem | Source in `effect-institute` | Target in `openagents` | Porting Strategy | License |
| --- | --- | --- | --- | --- | --- |
| **T1 (Immediate)** | CSS tokens, surface/border/emerald scale, focus-ring & scrollbar rules | `src/styles.css`, `src/lib/neo-code-block/styles.css`, `src/components/chapters/constants.ts` | `components/skin/institute-noir.coax` (`institute_noir_css`, `institute_noir_skin`) | Direct transcription into Coax CSS emitter + `Theme`/`UiSkin` | MIT |
| **T1 (Immediate)** | **Commit Mono Variable Font** (`86.8 KB` WOFF2) | `public/fonts/CommitMono-Variable.woff2` | `components/fonts/CommitMono-Variable.woff2` + `CommitMono-OFL.txt` + web `/fonts/` | Ship static WOFF2 + preload link; add OFL license & `components/fonts/README.md` entry | SIL OFL 1.1 |
| **T1 (Immediate)** | **GitHub Dark Syntax Palette & Annotation Colors** | `src/lib/animated-code/animation/compile/highlight.ts` + `src/lib/neo-code-block/annotation-colors.ts` | `components/skin/institute-noir.coax` (`INSTITUTE_SYNTAX`, annotation roles) | Map 12 hex slots to `SYN_*` in `components/core/syntax.coax` | MIT |
| **T1 (Immediate)** | **OG Image Architectural Frame** (Swiss grid + 45° hatch footer + state tiles) | `src/routes/og/-frame.tsx`, `src/routes/og/chapter.tsx` | `apps/openagents-site/og/`, `apps/openagents-web/books-build.coax` | Port SVG/HTML frame for `openagents.com/og/*` & book chapter OG cards | MIT |
| **T2 (Workspace UX)** | **Pulsing Dot Grid** (`3×3px` square dots, `18px` pitch, click-driven Gaussian ripple) | `src/components/chapters/backgrounds/PulsingDotGrid.tsx` | `coax/web/` minimal canvas effect & `components/backend/gpu-shader.coax` | Port 80-line Gaussian `easeOutExpo` ripple math to Coax GPU shader + lightweight DOM canvas | MIT |
| **T2 (Workspace UX)** | **WebGL Running-Energy & Failure-Pulse Shaders** | `src/components/narrated/pixi-effect-row/engine/shaders.ts` (`ENERGY_FRAGMENT`, `FAILURE_PULSE_FRAGMENT`) + `palette.ts` | `components/backend/gpu-shader.coax` | Port both GLSL fragment shaders to WGSL/Metal for live running/failed agent & tool execution cards | MIT |
| **T2 (Workspace UX)** | **Micro-Interaction Kit** (`6×6px` progress squares, square-cap emerald ring, mask-faded rolling headers, 50ms button flash, critically damped springs) | `RollingGroupHeader.tsx`, `ChapterProgressRing.tsx`, `SectionIndicator.tsx`, `src/utils/animationConfigs.ts` | `components/core/spring.coax`, `components/app/base.coax`, `components/kit/` | Align `UiMotion` / `UiSpring` presets to `bounce: 0`, `0.2/0.3/0.4s`; add 6×6px step strip & rolling header mask | MIT |
| **T2 (Workspace UX)** | **Multi-Key Chord Hotkey Engine & Conflict UI** | `src/lib/hotkeys/*` (`registry.ts`, `match.ts`, `conflicts.ts`, `normalize.ts`, `format.ts`) + `src/routes/hotkeys.tsx` | `apps/openagents-web/chat/chat-settings.coax`, `components/app/settings.coax` | Port 2-key sequence matcher (`gg`, `gh`, `j`/`k`) & conflict detector to Coax for web + Desktop parity | MIT |
| **T2 (Workspace UX)** | **Cursor-Following Action/Shortcut Tooltip** | `src/components/chapters/cursor/ChapterCursor.tsx`, `CursorTooltipRenderer.tsx`, `src/atoms/cursor.ts` | `components/kit/` + `coax/web/dom-host.js` | Port sharp `#171717` `1px #222222` rAF-positioned floating tooltip with uppercase title + `<kbd>` chip | MIT |
| **T3 (Interactive & Polish)** | **Morphing Code Renderer & Permanent-Mount Pool** (`useLinePool`, pre-split `allRanges`/`brightBoundaries`, `NeoCodeMeasurementsManager`, `CodeSlideshow` wire format) | `src/lib/neo-code-block/*`, `src/lib/animated-code/animation/compile/*` | `components/chat/markdown.coax`, `components/app/diff.coax`, `apps/openagents-web/` | Harvest the architectural pattern (permanent mount pool + pre-split highlight spans + single-rAF Range measurement) for live streaming code diffs & step-through agent explanations | MIT |
| **T3 (Interactive & Polish)** | **Procedural Web Audio UI SFX Recipes** (`linkHoverSynth` click, `toggleSynth`, `dingBellSynth`, `failBassSynth`, `winSynth`) | `src/sounds/synthDefinitions.ts`, `src/sounds/AppSounds.ts`, `src/sounds/ToneSoundSystem.ts` | Opt-in host Web Audio utility (default muted / Settings → Appearance toggle) | Synthesize with raw Web Audio API `OscillatorNode` + `GainNode` (zero Tone.js bundle cost; ~1.5 KB) | MIT |
| **T3 (Interactive & Polish)** | **Frame-Hash Regression Harness Pattern** | `src/lib/content/lessons.test.ts` | `components/chat/frame-hash-checks.coax` | Already philosophically aligned; extend to pin Institute Noir rendered frames & token splits | MIT |
| **T3 (Infra)** | **Safari-Safe `WebAudioFader` & `206` Byte-Range Asset Pattern** | `src/atoms/services/audio-player/fader.ts`, `src/contentAssets.ts` | `metaverse/voice*` web audio playback & static media handlers | Reference for click-free 15ms gain ramps and Safari WebKit bypass (`bugs 240405/221334`) | MIT |

### 5.2 Explicitly NOT Harvested (Keep OpenAgents Coax-First)

- **Do NOT adopt** React 19, Effect-TS (`effect` / `@effect/atom-react`), TanStack Start/Router, Tailwind CSS, PixiJS runtime, Tone.js library, Shiki JS runtime, or Radix UI primitives into `apps/openagents-web/` or `apps/openagents-desktop/`. OpenAgents ships pure Coax Wasm/Metal/Vulkan + `dom-host.js`.
- **Do NOT adopt** Effect Institute's ElevenLabs narration authoring pipeline (`src/authoring/**`) or Cloudflare Workers deployment scripts.
- **Do NOT adopt** desktop-only mobile blocking (`MobileLayout.tsx` apology screen). OpenAgents is explicitly **web, mobile & desktop** — Institute Noir must degrade cleanly to responsive mobile viewports (`< 768px`) with a `48px` bordered mobile top bar and full-screen drawer.

### 5.3 Attribution & Licensing Requirements

When harvesting any code, shaders, fonts, or patterns into `components/`:
1. Add an entry to **`components/NOTICE`** attributing `kitlangton/effect-institute` (MIT License, Copyright Kit Langton) for the Institute Noir design tokens, shaders, and interaction patterns.
2. Add **`components/fonts/CommitMono-OFL.txt`** (SIL Open Font License 1.1, Copyright Eigil Nikolajsen) and document the font file & SHA-256 checksum in **`components/fonts/README.md`**.
3. If any Phosphor icon SVGs are selectively adopted alongside existing Solar/Apps-SDK icons in `components/icons/`, add Phosphor Icons (MIT License) to `components/icons/NOTICE`.

---

## 6. Phased Implementation Plan

### Phase 0 — Font & Legal Foundation
- Commit `CommitMono-Variable.woff2` + `CommitMono-OFL.txt` under `components/fonts/`, publish `/fonts/CommitMono-Variable.woff2` in both `apps/openagents-site/` and `apps/openagents-web/`, and update `components/fonts/README.md` and `components/NOTICE`.

### Phase 1 — Canonical Coax Skin (`components/skin/institute-noir.coax`)
- Implement `institute_noir() -> Theme`, `institute_noir_skin() -> UiSkin`, `institute_noir_settings() -> UiSettings`, and `institute_noir_css(out: [] u8) -> int` in one clean Coax unit with `#test` checks pinning every hex color, zero radius, `Commit Mono` font stack, and 12-slot GitHub Dark `INSTITUTE_SYNTAX` array.
- Register `UISKIN_INSTITUTE_NOIR` in `components/skin/skin.coax` and `components/skin/all.coax`.

### Phase 2 — Site Default Cutover (`openagents.com`)
- Update `apps/openagents-site/index.html` and `apps/openagents-web/{index,chat/index,login/index,onboarding/index,r/index,settings/*/index}.html` to share the exact Institute Noir critical `<head>` CSS (`#000000` / `#f1efe8` / `Commit Mono` preload) so there is zero FOUC or color shift across routes.
- Switch `apps/openagents-web/login-web.coax` from `apps_sdk_css` + `lw_css` to `institute_noir_css` + compact login layout rules.

### Phase 3 — Zero-Based Chat Workspace (`apps/openagents-web/chat-web.coax`)
- Replace the 3-layer stylesheet (`mono_css` + `apps_sdk_css` + `cwc_css`) in `chat-web.coax` with `institute_noir_css`.
- Implement the zero-based Institute Noir sidebar: `48px` brand/collapse header, pure `#000000` rail, `1px #222222` dividers, and the flush bottom **Credits** cell displaying USD balance only (emerald accent, hairline meter, zero sats/Bitcoin copy).
- Restyle the `48px` chat top bar, sharp `0px` composer frame, `72ch` monospace transcript, inline code boxes, GitHub Dark fenced code blocks, and `6×6px` tool-status squares.
- Wire `/settings/appearance` so **Institute Noir** is the site default.

### Phase 4 — Cross-Platform Parity (Desktop, iOS & `/components`)
- Add `Institute Noir` to `/components` and `/components-gpu` skin gallery (`components/gallery/`).
- Register `CP_SKIN_INSTITUTE_NOIR` in `apps/openagents-desktop/page/customize.coax` and `apps/openagents-desktop/page/skin.coax`.
- Align `apps/openagents-ios/Sources/` `OATheme` tokens (`#000000`, `#222222`, `#f1efe8`, `#34d399`, `#ef4444`, `#3b82f6`) and sharp border rhythm.

### Phase 5 — Harvested Signature Effects
- Port `PulsingDotGrid` (empty states on `/`, `/login`, `/onboarding`, empty `/chat`).
- Port `ENERGY_FRAGMENT` and `FAILURE_PULSE_FRAGMENT` to WGSL/Metal in `components/backend/gpu-shader.coax` for live running/failed agent execution cards.
- Add `RollingGroupHeader` mask-faded status/title roll, `SectionIndicator` 50ms flash-to-white button feedback, multi-key chord hotkeys (`/settings` + `/chat`), and opt-in procedural Web Audio micro-clicks (muted by default).

---

## 7. Verification & Acceptance Gates

Before declaring Institute Noir shipped as the site default:

1. **Coax Unit & Layer Checks:**
   - `./coax/bin/coax test components/skin/institute-noir.coax` (verifies exact hex values, `0px` radii, and all 12 `SYN_*` syntax colors).
   - `coax/web/tool.sh components-check`, `coax/web/tool.sh layer-check`, and `coax/web/tool.sh tools-check` pass without increasing any allowlist count.
2. **Zero Frankenstein CSS:**
   - `chat-web.coax` and `login-web.coax` emit **one** stylesheet (`institute_noir_css`); `apps_sdk_css` and `mono_css` are no longer stacked together on `/chat` or `/login`.
3. **Visual Proof (Headless Chrome):**
   - Capture `coax/web/tool.sh shot` at desktop (`1280×800`) and mobile (`390×844`) across `/`, `/login`, `/onboarding`, `/chat` (logged-in with `$10.00` credits + active thread + code block), and `/settings/appearance`.
   - Confirm `#000000` background, `#f1efe8` body ink, `#222222` `1px` hairlines, `0px` card/button/input radii, `Commit Mono` loaded (`document.fonts`), zero scroll bounce on `/`, `/login`, `/onboarding`, and zero FOUC on reload.
4. **WCAG & Copy Hygiene:**
   - Verify contrast ≥ `4.5:1` on all readable body/secondary text (`#f1efe8` = `18.3:1`, `white/50` = `5.3:1`, `white/45` = `4.5:1`, `#34d399` = `10.9:1`).
   - Verify zero `sats` / Bitcoin labels in v1.0 web Credits UI and zero third-party branding leaks.

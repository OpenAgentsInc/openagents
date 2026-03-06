# OpenAgents UI System Guide

Working system specification for the Autopilot Desktop UI.  
Purpose: keep visual decisions consistent across panes, sidebar controls, and future surfaces.

---

## 1) Scope

- **Product surface:** `apps/autopilot-desktop`
- **Primary layout model:** pane-based workspace + global sidebar + hotbar
- **Source of truth for tokens:** `crates/wgpui/src/theme/mod.rs`
- **Related docs:** `docs/PANES.md`, `docs/brand.md`

---

## 2) Design Principles

- **Clarity over novelty:** UI must communicate state and next action immediately.
- **Consistency over one-offs:** prefer shared tokens and components.
- **Low visual noise:** accents are signals, not decoration.
- **Stable geometry:** foundation elements do not move during resize.
- **Predictable interactions:** same control style implies same behavior everywhere.

---

## 3) Typography System

Typography is a core part of the control surface.  
Hierarchy and role clarity take priority over stylistic expression.

---

### Font Families

Autopilot uses two font families with distinct responsibilities.

#### UI Typeface

- **Font family:** Inter
- **Usage:**
  - Pane titles
  - Buttons and controls
  - Labels and form text
  - Status messages
  - Sidebar navigation
  - Default body text

Inter is chosen for:
- High legibility at small sizes
- Neutral, technical tone
- Excellent weight range for hierarchy
- Long-session readability

#### Monospace Typeface

- **Font family:** JetBrains Mono
- **Usage:**
  - Command execution output
  - Logs and system messages
  - File paths and identifiers
  - Code-like structured responses

JetBrains Mono communicates machine-level precision without visual noise.

---

### Font Tokens

Font families must be referenced via theme tokens.  
Hardcoding font families in components is not permitted.

theme::font::UI   = Inter
theme::font::MONO = JetBrainsMono

### Typography

- `theme::font_size::XS = 12`
- `theme::font_size::SM = 14`
- `theme::font_size::BASE = 16`
- `theme::font_size::LG = 20`

Usage:

- **12px:** metadata, timestamps, secondary labels
- **14px:** primary labels, status captions
- **16px:** default control and body text
- **20px:** headings, key values, major state labels

---

### Spacing

- `theme::spacing::XS = 4`
- `theme::spacing::SM = 8`
- `theme::spacing::MD = 12`
- `theme::spacing::LG = 16`
- `theme::spacing::XL = 24`

Usage:

- **4–8px:** micro spacing, icon gaps
- **12px:** control padding default
- **16–24px:** section and pane spacing

---

### Colors (Semantic Tokens)

Hardcoded hex values are not permitted outside this document.

#### Text
- `theme::text::{PRIMARY, SECONDARY, MUTED, DISABLED}`

#### Backgrounds
- `theme::bg::{APP, SURFACE, ELEVATED, HOVER}`

#### Borders
- `theme::border::{DEFAULT, STRONG, FOCUS, ERROR}`

#### Accent
- `theme::accent::PRIMARY`  
  Reserved for focus, execution, and primary actions.

#### Status
- `theme::status::{SUCCESS, WARNING, ERROR, INFO}`

---

## 4) Dark Mode Theme (Default)

Dark mode is the **canonical** Autopilot appearance.  
Light mode derives from this. Not the other way around.

Pure black and pure white are intentionally avoided.

## DARK MODE

### Backgrounds & Surfaces

theme::bg::APP      = #0B0F14
theme::bg::SURFACE  = #10161D
theme::bg::ELEVATED =rgb(28, 28, 29)
theme::bg::HOVER    = #1A222C


### Borders

- theme::border::DEFAULT = #1F2933
- theme::border::STRONG  = #53696c
- theme::border::FOCUS   = #3c464a
- theme::border::ERROR   = #E06C75
- theme::border::ACTIVE  = #3d494f


### Text

- theme::text::PRIMARY   = #E6EDF3
- theme::text::SECONDARY = #9FB0C3
- theme::text::MUTED     = #6B7C8F
- theme::text::DISABLED  = #4B5A6B

### Accent

- theme::accent::PRIMARY = #4FD1C5   // Command cyan

### Status

- theme::status::SUCCESS = #5EDC9A
- theme::status::WARNING = #E3B341
- theme::status::ERROR   = #E06C75
- theme::status::INFO    = #61AFEF

## 5) Light Mode Theme

### Backgrounds & Surfaces

- theme::bg::APP      = #F6F8FB
- theme::bg::SURFACE  = #FFFFFF
- theme::bg::ELEVATED = #EEF2F6
- theme::bg::HOVER    = #E5EBF2

### Borders 

- theme::border::DEFAULT = #D1DAE3
- theme::border::STRONG  = #B9C6D4
- theme::border::FOCUS   = #03655c
- theme::border::ERROR   = #D65C65


### Text

- theme::text::PRIMARY   = #0E1621
- theme::text::SECONDARY = #3A4A5E
- theme::text::MUTED     = #6B7C8F
- theme::text::DISABLED  = #9AA8B6

### Accent

- theme::accent::PRIMARY = #03655c   // Same as dark mode

### Status

- theme::status::SUCCESS = #3FBF86
- theme::status::WARNING = #D9A441
- theme::status::ERROR   = #D65C65
- theme::status::INFO    = #4A90E2

## 6) Pane Layout Standards

Constants:

- `PANE_TITLE_HEIGHT = 28`
- `PANE_MIN_WIDTH = 220`
- `PANE_MIN_HEIGHT = 140`
- `PANE_MARGIN = 18`

Guidelines:

- Pane titles must remain compact and scannable.
- Internal padding defaults to 12px.
- **Pane container corner radius defaults to `4px`.**
- Each pane must explicitly define:
  - a primary action area,
  - a state area (`loading`, `ready`, `error`),
  - empty state behavior.
- Panes should not rely on color alone to convey state.
- Scroll behavior must feel stable and predictable.

---

## 7) Control Patterns

### Button Styles

All buttons use:

- border width: `1px`
- corner radius: `6px`
- default text size: `theme::font_size::SM`

#### Primary (Highest Visual Weight)

Use for the main action in a section (e.g. `Save`, `Run`, `Confirm`).

- **Standard**
  - background: `theme::accent::PRIMARY`
  - border: `theme::accent::PRIMARY`
  - text: `theme::bg::APP`
- **Hover**
  - background: `theme::accent::PRIMARY` with slightly higher contrast (theme-adjusted)
  - border: `theme::accent::PRIMARY`
  - text: `theme::bg::APP`
- **Focus**
  - background: `theme::accent::PRIMARY`
  - border: `theme::border::FOCUS`
  - text: `theme::bg::APP`
  - focus indicator: visible high-contrast ring or outline using `theme::border::FOCUS`

#### Secondary (Medium Visual Weight)

Use for important but non-primary actions (e.g. `Cancel`, `Back`, `Open Details`).

- **Standard**
  - background: `theme::bg::HOVER`
  - border: `theme::border::DEFAULT`
  - text: `theme::text::PRIMARY`
- **Hover**
  - background: `theme::bg::ELEVATED`
  - border: `theme::border::STRONG`
  - text: `theme::text::PRIMARY`
- **Focus**
  - background: `theme::bg::HOVER`
  - border: `theme::border::FOCUS`
  - text: `theme::text::PRIMARY`
  - focus indicator: visible high-contrast ring or outline using `theme::border::FOCUS`

#### Tertiary (Lowest Visual Weight)

Use for optional or contextual actions (e.g. `Learn more`, `View logs`, icon-adjacent utility actions).

- **Standard**
  - background: transparent (or `theme::bg::APP` with 0 alpha)
  - border: transparent
  - text: `theme::text::SECONDARY`
- **Hover**
  - background: `theme::bg::HOVER`
  - border: transparent
  - text: `theme::text::PRIMARY`
- **Focus**
  - background: transparent (or `theme::bg::APP` with 0 alpha)
  - border: `theme::border::FOCUS`
  - text: `theme::text::PRIMARY`
  - focus indicator: visible high-contrast ring or outline using `theme::border::FOCUS`

Rules:

- One primary button max per local action group.
- Primary and secondary can appear together; tertiary should not compete with primary CTAs.
- Use clear verb labels only (`Retry`, `Refresh`, `Apply`, `Save`).
- Button labels must be horizontally and vertically centered within the button bounds.
- Keep button heights consistent within a section (`34px` default, `26px` compact).
- Hover changes must be visible in both dark and light themes.
- Focus state is required on all interactive button variants.

---

### Toggle Controls

Rules:

- Toggle state must be understandable without relying on color alone.
- Entire control row should be clickable.
- Prefer inline, compact toggles (~20px height).
- Label both states explicitly when possible (e.g. `Offline | Online`).
- Avoid icon-only toggles for stateful controls.

---

### Inputs

Rules:

- Every input must have a visible label.
- Placeholder text is supplementary, never the only explanation.
- Validation should appear before submit when feasible.
- Invalid states must include:
  - semantic color,
  - explanatory text.
- Focus state must use `theme::border::FOCUS`.

---

## 8) Status and Feedback

Required pane states:

- **Loading**
- **Ready**
- **Error**

Rules:

- Always use semantic status colors.
- Status copy should be short, specific, and actionable.
- Recoverable errors must include a retry affordance.
- Loading states must include subtle motion to avoid “frozen” perception.
- Avoid blocking the entire pane for recoverable states.

---

## 9) Sidebar UX Standards

Layout:

- **Top:** global state and quick actions (e.g. agent online/offline)
- **Middle:** contextual controls (future expansion)
- **Bottom-right:** utilities and settings

Rules:

- Sidebar controls should be task-oriented and minimal.
- Avoid duplicating controls that exist elsewhere in the same viewport.
- Resize/collapse handle must remain visible and discoverable at all times.
- Sidebar should feel persistent and stable during interaction.

---

## 10) Motion and Interaction

Rules:

- Motion exists to communicate state, not decorate.
- Transitions should be subtle and fast.
- Tooltips may fade in but must dismiss immediately on mouse-out.
- Cursor must reflect affordance:
  - resize handles: `grab` / `grabbing`
  - clickable controls: `pointer`
- During resize, core canvas surfaces must not jitter or shift.

---

## 11) Component Metrics (Added)

The document referenced semantic patterns but did not yet define concrete control metrics.
These values are now defined so implementations stay consistent:

- **Default button height:** `34px`
- **Compact button height:** `26px`
- **Default corner radius:** `6px`
- **Large surface corner radius:** `6px`
- **Default border width (controls):** `1px`
- **Pane outer border width:** `1px`
- **Primary control text size:** `theme::font_size::SM` (`14px`)
- **Meta/control helper text size:** `theme::font_size::XS` (`12px`)

### Border Radius Standards

- **Window panes:** `6px`
- **Buttons (default + compact):** `6px`
- **Input fields:** `6px`
- **Small badges/chips:** `3px`
- **Tooltips / floating helpers:** `6px`
- **Toggle track:** pill (`height / 2`)
- **Toggle thumb:** full circle (`diameter / 2`)

Rules:

- Keep one primary control size per section.
- Do not mix corner radii in the same component family unless semantically required.
- Use compact controls only when density is required and readability is maintained.
- Pane radius should remain visually consistent across all pane variants unless a specific exception is documented.

---

## 12) Accessibility Checklist

- [ ] Text contrast meets WCAG AA minimums.
- [ ] Status is not conveyed by color alone.
- [ ] Pointer targets are reasonably sized for desktop use.
- [ ] Keyboard interaction paths exist where practical.
- [ ] Error copy is actionable and specific.
- [ ] Focus states are clearly visible on all interactive elements.

---


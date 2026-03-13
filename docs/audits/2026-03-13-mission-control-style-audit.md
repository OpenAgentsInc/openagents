# 2026-03-13 Mission Control Rendering Style Audit

## Scope

Analyze the high-quality rendering style of the Mission Control pane, contrast it with the rendering approach used by every other pane in the application, and propose a comprehensive path forward for applying Mission Control's visual quality to the rest of the app using Rust best practices.

### Files examined

| File | Lines | Role |
|------|-------|------|
| `apps/autopilot-desktop/src/pane_renderer.rs` | 8 464 | Main pane dispatch + MC rendering + shared helpers |
| `apps/autopilot-desktop/src/panes/chat.rs` | 4 192 | Chat pane (extracted module) |
| `apps/autopilot-desktop/src/panes/wallet.rs` | 672 | Wallet pane (extracted module) |
| `apps/autopilot-desktop/src/panes/relay_connections.rs` | 141 | Relay connections pane (extracted module) |
| `apps/autopilot-desktop/src/panes/buy_mode_payments.rs` | 65 | Buy-mode payments pane (extracted module) |
| `crates/wgpui/src/theme/mod.rs` | 424 | Global theme system (color tokens, spacing, typography) |

---

## 1. What Makes Mission Control's Rendering High-Quality

Mission Control (`paint_go_online_pane`, lines 581-1421 of `pane_renderer.rs`) is the most visually refined pane in the application. It achieves this through five distinct techniques that are absent or minimal in every other pane.

### 1.1 Dedicated Color Palette

MC defines **11 named color functions** (lines 2191-2243) that create a cohesive, purpose-built visual identity:

```rust
fn mission_control_background_color() -> Hsla { Hsla::from_hex(0x070C14) }
fn mission_control_panel_color() -> Hsla       { Hsla::from_hex(0x0D121A) }
fn mission_control_panel_header_color() -> Hsla { Hsla::from_hex(0x121924) }
fn mission_control_panel_border_color() -> Hsla { Hsla::from_hex(0x263245) }
fn mission_control_text_color() -> Hsla        { Hsla::from_hex(0xD8DFF0) }
fn mission_control_muted_color() -> Hsla       { Hsla::from_hex(0x8A909E) }
fn mission_control_orange_color() -> Hsla      { Hsla::from_hex(0xFFA122) }
fn mission_control_amber_color() -> Hsla       { Hsla::from_hex(0xF9B84D) }
fn mission_control_green_color() -> Hsla       { Hsla::from_hex(0x52E06D) }
fn mission_control_cyan_color() -> Hsla        { Hsla::from_hex(0x2FB7F2) }
fn mission_control_red_color() -> Hsla         { Hsla::from_hex(0xF46060) }
```

These provide a dark-blue-grey base with warm accents (orange, amber, green, cyan, red) that convey meaning: green = healthy, orange = degraded, red = error, cyan = informational. The palette creates clear visual hierarchy with multiple background layers (`background` -> `panel` -> `panel_header`) that give depth.

Additionally, `mission_control_mode_color` (lines 2245-2252) maps runtime mode to accent color, and dedicated label/summary helpers (`mission_control_buy_mode_result_label`, `mission_control_buy_mode_payment_label`, `mission_control_buy_mode_payment_summary`, lines 2254-2331) compute display strings with contextual formatting.

### 1.2 Animation and Motion

MC uses three distinct animation patterns:

**Pulse effect** (lines 1436-1438 of `paint_mission_control_section_panel`):
```rust
let anim_t = mission_control_anim_seconds_f64();
let pulse_phase = (anim_t * 5.0 + bounds.origin.x as f64 * 0.015).rem_euclid(TAU);
let pulse = ((pulse_phase as f32).sin() * 0.5) + 0.5;
```
This creates a subtle travelling wave across panel headers, with the phase offset by horizontal position so adjacent panels shimmer in sequence rather than in lockstep.

**Blink effect** (lines 2011-2014 of `paint_mission_control_status_cell`):
```rust
let anim_t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f32()).unwrap_or(0.0);
let blink = ((anim_t * 9.6 + bounds.origin.x * 0.04).sin() * 0.5) + 0.5;
```
Status cells use a faster frequency (9.6 Hz vs 5.0 Hz) with position-based offset for a "scanning" visual.

**Alert shimmer** (lines 2076-2079 of `paint_mission_control_alert_band`):
```rust
let anim_t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f32()).unwrap_or(0.0);
```
Alert bands pulse to draw attention to warnings or errors.

**Go-online button glow** (lines 6222-6246 of `paint_mission_control_go_online_button`):
```rust
let pulse = ((now_secs * 2.4).sin() * 0.5) + 0.5;
let glow_alpha = if enabled && is_go_online { 0.14 + pulse * 0.08 } else { ... };
```
A slow breathing glow (2.4 Hz) on the primary call-to-action button, with a semi-transparent outer quad that expands 6px beyond the button bounds to create a halo effect.

### 1.3 Multi-Layer Panel Composition

MC panels are composed of **3+ visual layers** per section (lines 1440-1523):

1. **Background quad** with `mission_control_panel_color()`, 1px border, 6px corner radius
2. **Animated header bar** with gradient overlay and pulse-modulated alpha
3. **Title text** positioned precisely within the header
4. **Shimmer accent line** (1px) at the bottom of the header with position-based alpha offset
5. **Content area** below the header with controlled padding

This creates a "card-in-card" look where each section is visually distinct yet part of a cohesive dashboard. The shimmer accent line particularly adds polish - it uses a secondary sinusoidal function with a different phase to create a subtle "alive" quality.

### 1.4 Structured State Computation Helpers

MC separates **state computation from rendering** through dedicated structs and builder functions:

- `MissionControlActiveJobsPanelState` (struct, line 1612) + `mission_control_active_jobs_panel_state()` (lines 1618-1787): Computes job flow snapshots, settlement status, and display lines before painting.
- `MissionControlBuyModePanelState` (struct, line 1810) + `mission_control_buy_mode_panel_state()` (lines 1823-2001): Pre-computes request snapshots, provider/work/payment status, and summary strings.
- `mission_control_alert_message()` (lines 2136-2189): Computes the current alert text from multiple state sources.
- `active_job_stage_display()` (lines 1789-1808): Maps job stage enum to human-readable display string.

This pattern keeps the `paint_go_online_pane` function focused on layout and drawing rather than business logic.

### 1.5 Sophisticated Button Variants

MC defines **custom button renderers** beyond the shared `paint_button` function:

- `paint_mission_control_go_online_button` (lines 6205-6325): Multi-layer button with outer glow halo, inner gradient split (top highlight + bottom shadow), state-dependent border color (green for GO ONLINE, orange for GO OFFLINE), and pulse animation.
- `paint_mission_control_command_button` (lines 6327-6369): Compact command button with accent-colored top border highlight, state-dependent opacity, and MC-palette colors.

Compare this to the shared `paint_primary_button` which uses a single glow layer and two-tone fill but lacks the state-specific animations and accent variety.

---

## 2. How Other Panes Differ (The Gap)

### 2.1 Color Usage: Generic Theme Constants

Every non-MC pane relies exclusively on the global `theme::` constants:

| Token | Used by | Hex equivalent |
|-------|---------|----------------|
| `theme::text::PRIMARY` | All panes | `#FFFFFF` (pure white) |
| `theme::text::MUTED` | All panes | `#7B7C7F` (neutral grey) |
| `theme::status::SUCCESS` | State indicators | `#5EDC9A` |
| `theme::status::ERROR` | Error text | `#E06C75` |
| `theme::accent::PRIMARY` | Loading/links | `#22D3EE` |
| `theme::bg::APP` | Row backgrounds | `#030303` |
| `theme::border::DEFAULT` | All borders | `#164E63` |

**Evidence** (representative samples):

- `paint_provider_status_pane` (line 2686): `theme::text::MUTED` for section headers
- `paint_earnings_scoreboard_pane` (line 3138): `theme::status::SUCCESS` / `theme::accent::PRIMARY` / `theme::status::ERROR` for load state
- `paint_sync_health_pane` (line 3315): identical tri-state color mapping
- `relay_connections.rs` (line 72): `theme::status::SUCCESS` / `theme::accent::PRIMARY` / `theme::text::MUTED` / `theme::status::ERROR`
- `wallet.rs` (line 26): same tri-state pattern
- `buy_mode_payments.rs` (line 35): `theme::text::PRIMARY` and `theme::text::MUTED` only

These theme constants produce a usable but **flat** appearance. There is no layered background hierarchy (panel -> panel header -> content), no warm accent variety (only cyan-family accents), and no visual depth. All panes look the same regardless of their domain.

### 2.2 Zero Animation

No non-MC pane uses any animation whatsoever. There are no pulse effects, no shimmer, no breathing indicators, no glow halos. Status changes are rendered as static text color changes.

The only animation infrastructure in the shared helpers is the `paint_primary_button` glow (line 6470), but this is a generic static glow, not a time-varying animation.

### 2.3 Flat Layout Structure

Non-MC panes follow a uniform layout pattern:

```
[source badge]                    (top-right corner)
[action buttons]                  (top area, left-aligned)
[state summary text]              (below buttons)
[label: value] lines              (vertical stack, 18px spacing)
[optional: selectable rows]       (table-like, uniform height)
[optional: detail section]        (below rows)
```

This is functional but has no visual grouping. There are no section panels, no header bars, no card-based layout. Content flows as a single column of text lines with no visual hierarchy beyond font size (10-11px) and color (muted vs primary).

**Evidence**:

- `paint_earnings_scoreboard_pane` (lines 3126-3274): 11 consecutive `paint_label_line` calls with no grouping
- `paint_sync_health_pane` (lines 3305-3508): 17 consecutive `paint_label_line` calls
- `paint_reciprocal_loop_pane` (lines 4009-4231): 15 consecutive `paint_label_line` calls
- `wallet.rs` `paint_wallet_pane` (lines 15-257): `paint_label_line` calls interspersed with conditional multiline phrases

### 2.4 No State Computation Separation

Non-MC panes compute display values inline within their paint functions:

```rust
// paint_provider_status_pane, line 2676-2678
let heartbeat_age = provider_runtime
    .heartbeat_age_seconds(now)
    .map_or_else(|| "n/a".to_string(), |age| age.to_string());
```

```rust
// paint_reciprocal_loop_pane, line 4067-4074
let compact_value = |raw: Option<&str>| -> String {
    let value = raw.unwrap_or("missing");
    if value.len() > 24 { format!("{}..{}", &value[..12], &value[value.len()-8..]) }
    else { value.to_string() }
};
```

This mixes business logic with rendering logic, making the paint functions harder to test and reason about.

### 2.5 Chat Pane: A Partial Exception

The Chat pane (`panes/chat.rs`, 4192 lines) is the closest to MC in sophistication:

- **Rich attachment rendering** with domain-specific color tuples (`rich_attachment_colors`, lines 237-277)
- **Custom send button** with circular shape and SVG icon (`paint_chat_send_button`, lines 144-186)
- **Notification badges** with urgent/normal distinction (`paint_notification_badge`, lines 116-142)
- **Markdown rendering** with configurable typography
- **Scroll clipping** and viewport management

However, Chat still lacks: animated indicators, multi-layer panel composition, dedicated color palette (it uses `theme::` constants throughout), and structured state computation (display logic is inline).

---

## 3. Shared Helper Inventory

The following shared helpers exist in `pane_renderer.rs` and are used by non-MC panes:

| Helper | Lines | Purpose |
|--------|-------|---------|
| `paint_source_badge` | 6152-6175 | Corner badge showing data source |
| `paint_state_summary` | 6083-6126 | State + action + error text block |
| `paint_label_line` | 6586-6606 | `label: value` pair at fixed 122px offset |
| `paint_selectable_row_background` | 6128-6150 | Selected/unselected row quad |
| `paint_action_button` | 6177-6179 | Secondary button wrapper |
| `paint_primary_button` | 6181-6183 | Primary button wrapper |
| `paint_tertiary_button` | 6189-6191 | Tertiary button wrapper |
| `paint_disabled_button` | 6201-6203 | Disabled button wrapper |
| `paint_filter_button` | 6193-6199 | Active/inactive filter toggle |
| `paint_multiline_phrase` | (shared) | Wrapped text block |
| `split_text_for_display` | (shared) | Line-wrapping utility |

These helpers are purely functional but visually basic. They use hardcoded offsets (e.g., `x + 122.0` for label-value alignment), fixed font sizes (10-11px), and global theme colors only.

---

## 4. Theme System Assessment

The `wgpui` theme system (`crates/wgpui/src/theme/mod.rs`) provides:

**Strengths:**
- Well-structured `ThemeColors` struct with semantic field names (lines 28-97)
- Design token modules: `font_size`, `spacing`, `line_height`, `shadow`, `duration`, `border_width` (lines 148-270)
- Backwards-compatible color modules: `bg`, `text`, `accent`, `border`, `status` (lines 276-383)
- Global theme switching via `set_theme()` (line 137)

**Gaps:**
- No **pane-level** or **domain-level** color overrides. The theme is global-only.
- No **animation tokens** beyond `duration` constants. No easing curves, no animation state management.
- No **elevation** or **depth** tokens. MC creates depth manually with layered backgrounds; this could be systematized.
- No **component-level** style variants. Buttons, panels, cards, and badges are all ad-hoc.
- The `spacing` module exists but non-MC panes use hardcoded pixel values (`12.0`, `14.0`, `16.0`, `18.0`) rather than tokens.

---

## 5. Comprehensive Path Forward

### Phase 1: Extract MC's Color Palette into the Theme System

**Goal:** Make MC's visual quality available to all panes through the theme rather than private functions.

**Actions:**

1. Add a `PanePalette` struct to `wgpui/src/theme/mod.rs`:

```rust
/// Domain-specific color palette for a pane or visual region.
#[derive(Clone, Copy, Debug)]
pub struct PanePalette {
    pub background: Hsla,
    pub panel: Hsla,
    pub panel_header: Hsla,
    pub panel_border: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub accent_primary: Hsla,    // cyan for MC
    pub accent_success: Hsla,    // green for MC
    pub accent_warning: Hsla,    // orange/amber for MC
    pub accent_error: Hsla,      // red for MC
    pub accent_info: Hsla,       // amber for MC
}
```

2. Define palette presets: `MISSION_CONTROL_PALETTE`, `PROVIDER_PALETTE`, `WALLET_PALETTE`, `CHAT_PALETTE`, etc. MC's current hex values become the `MISSION_CONTROL_PALETTE` preset. Other palettes can start as variations of MC's palette (same structure, different accent hues) or as the current theme constants mapped into the struct.

3. Replace the 11 `mission_control_*_color()` functions with `MISSION_CONTROL_PALETTE` field accesses. This is a pure refactor with no visual change.

4. Migrate non-MC panes from `theme::text::MUTED` / `theme::status::SUCCESS` / etc. to their assigned `PanePalette` fields. Initially these can map to the same values; the structural change enables per-pane customization later.

**Rust best practice:** Use `const` items where possible (`const MISSION_CONTROL_PALETTE: PanePalette = ...`) so palettes are zero-cost. The `Hsla::from_hex` constructor would need to be `const fn` or the values pre-computed as `Hsla::new(h, s, l, a)`.

### Phase 2: Build a Section Panel Component

**Goal:** Replace flat label-line layouts with MC-style section panels across all panes.

**Actions:**

1. Extract `paint_mission_control_section_panel` (lines 1423-1523) into a generic `paint_section_panel` function in a new `components/section_panel.rs` module:

```rust
pub fn paint_section_panel(
    bounds: Bounds,
    title: &str,
    palette: &PanePalette,
    options: SectionPanelOptions,
    paint: &mut PaintContext,
) { ... }

pub struct SectionPanelOptions {
    pub show_animated_header: bool,
    pub corner_radius: f32,
    pub header_height: f32,
}
```

2. The function draws: background quad, optional animated header bar, title text, and returns the content `Bounds` for the caller to draw into.

3. Replace `paint_label_line` chains in non-MC panes with grouped section panels. For example, `paint_earnings_scoreboard_pane` would become:

```
[Section: "Earnings"]
  Today:       1,234 sats
  This month:  45,678 sats
  Lifetime:    1,234,567 sats

[Section: "Performance"]
  Completion ratio:  98.50%
  Payout success:    99.20%
  First job latency: 2.4s
```

4. Create a `paint_section_label_line` helper that uses the palette's `text_muted` for labels and `text` for values, with configurable alignment width.

**Rust best practice:** Use a builder pattern for `SectionPanelOptions` with `Default` impl so callers can customize only what they need: `SectionPanelOptions { show_animated_header: true, ..Default::default() }`.

### Phase 3: Introduce an Animation Framework

**Goal:** Make MC's animation patterns reusable without copy-pasting `SystemTime::now()` + sin() boilerplate.

**Actions:**

1. Add an `animation` module to `wgpui`:

```rust
pub mod animation {
    /// Returns a 0.0..1.0 pulse value based on wall-clock time.
    pub fn pulse(frequency_hz: f32, phase_offset: f32) -> f32 {
        let t = wall_clock_seconds();
        ((t * frequency_hz + phase_offset).sin() * 0.5) + 0.5
    }

    /// Returns a 0.0..1.0 pulse with position-based phase offset.
    pub fn spatial_pulse(frequency_hz: f32, x: f32, spatial_scale: f32) -> f32 {
        pulse(frequency_hz, x * spatial_scale)
    }

    /// Cached wall-clock seconds for the current frame.
    pub fn wall_clock_seconds() -> f32 { ... }
}
```

2. Replace MC's inline animation calculations with calls to these functions. Verify visual equivalence.

3. Document recommended frequencies: ~2.4 Hz for breathing/glow, ~5.0 Hz for shimmer, ~9.6 Hz for fast scan.

4. Add `theme::duration` easing presets for transition animations (future use).

**Rust best practice:** Cache `SystemTime::now()` per frame in a thread-local to avoid multiple syscalls. Provide `#[inline]` hints on hot-path functions.

### Phase 4: Structured State Computation for All Panes

**Goal:** Separate display logic from paint logic across all panes, following MC's `MissionControlActiveJobsPanelState` pattern.

**Actions:**

1. For each pane that currently computes display values inline, define a `*PaneViewState` struct:

```rust
// Example for earnings scoreboard
struct EarningsScoreboardViewState {
    today_display: String,
    month_display: String,
    lifetime_display: String,
    jobs_today_display: String,
    completion_ratio: String,
    // ...
}

fn earnings_scoreboard_view_state(state: &EarningsScoreboardState) -> EarningsScoreboardViewState {
    // All display logic here, testable without PaintContext
}
```

2. Migrate inline computations from paint functions into these builders. The paint function becomes a pure layout-and-draw function.

3. Add unit tests for view state builders (they are pure functions with no graphics dependencies).

**Rust best practice:** Keep view state structs as `Copy` where possible (using `&str` references or `Cow<'_, str>`) to avoid allocation in the render hot path.

### Phase 5: Componentize Button Variants

**Goal:** Replace the current mix of shared buttons + MC-specific buttons with a unified button system.

**Actions:**

1. Define a `ButtonStyle` enum with more variants:

```rust
pub enum ButtonStyle {
    Primary,
    Secondary,
    Tertiary,
    Disabled,
    // New:
    GlowPulse { accent: Hsla, frequency_hz: f32 },
    Command { accent: Hsla },
    CallToAction { accent: Hsla, label_override: Option<&'static str> },
}
```

2. Merge `paint_mission_control_go_online_button` and `paint_mission_control_command_button` into the unified `paint_button` function with the new style variants.

3. All panes can then use `paint_button(bounds, label, ButtonStyle::GlowPulse { accent: palette.accent_success, frequency_hz: 2.4 }, paint)` instead of pane-specific button functions.

**Rust best practice:** Use `#[non_exhaustive]` on the enum to allow adding variants without breaking downstream matches.

### Phase 6: Apply the System Pane-by-Pane

**Goal:** Roll out the new visual quality across all panes in a controlled migration.

**Priority order** (by user visibility and complexity):

| Priority | Pane | Current lines | Key changes |
|----------|------|---------------|-------------|
| 1 | Wallet (`panes/wallet.rs`) | 672 | Section panels for balance/addresses/payments; wallet palette |
| 2 | Earnings Scoreboard | ~150 | Section panels for earnings/performance; animated header |
| 3 | Provider Status | ~460 | Section panels for dependencies/inventory; status animations |
| 4 | Activity Feed | ~155 | Section panels for filters/rows/detail; domain-colored rows |
| 5 | Relay Connections | 141 | Section panels for connection list; status animations |
| 6 | Network Requests | ~380 | Section panels for RFQ/quotes/orders |
| 7 | Sync Health | ~200 | Section panels for health/recovery/replay |
| 8 | Starter Jobs | ~115 | Section panels for budget/jobs |
| 9 | Reciprocal Loop | ~225 | Section panels for health/counters/failures |
| 10 | Alerts Recovery | ~100 | Section panels for alert list/detail |
| 11 | Chat (`panes/chat.rs`) | 4 192 | Palette migration; section panels for thread rail/composer |
| 12 | Settings | variable | Section panels for setting groups |
| 13 | Remaining panes | variable | Apply standard panel treatment |

For each pane migration:
1. Assign a `PanePalette` (can share palettes across related panes)
2. Group related label-lines into section panels
3. Add animation to status indicators
4. Extract inline display logic into a view state struct
5. Replace theme constant references with palette field accesses

### Phase 7: Improve MC Itself

While applying MC's style elsewhere, address these issues in MC's own rendering:

1. **Reduce `paint_go_online_pane` size**: Currently 840 lines. Extract sub-sections (Sell section, Earnings section, Wallet section, Buy section, Active Jobs section, Log section) into separate functions or even separate modules, each receiving the relevant slice of state.

2. **Use spacing tokens**: MC hardcodes pixel offsets (`12.0`, `16.0`, `18.0`, etc.) throughout. Replace with `theme::spacing::MD`, `theme::spacing::LG`, etc.

3. **Use font size tokens**: MC uses raw `11.0`, `10.0`, `9.0` etc. Replace with `theme::font_size::SM` (14px), `theme::font_size::XS` (12px), or add `theme::font_size::XXS` (10px) and `theme::font_size::XXXS` (9px) tokens.

4. **Unify animation time sources**: MC currently uses both `mission_control_anim_seconds_f64()` and inline `SystemTime::now()` calculations. Unify through the Phase 3 animation framework.

5. **Replace `Hsla::from_hex` with `const` palette values**: The MC color functions call `Hsla::from_hex` at runtime. Pre-compute as `const Hsla` values for zero-cost access.

---

## 6. Style Comparison Matrix

| Feature | Mission Control | Other Panes | Chat Pane |
|---------|----------------|-------------|-----------|
| Custom color palette | 11 named functions | None (theme constants only) | None (theme constants only) |
| Background layering | 3 levels (bg/panel/header) | 1 level (APP or SURFACE) | 1 level (APP) |
| Animated indicators | Pulse, blink, shimmer, glow | None | None |
| Section panels | Multi-layer cards with headers | None (flat text flow) | Partial (thread rail, composer) |
| State computation separation | Dedicated structs + builders | Inline in paint functions | Inline in paint functions |
| Custom buttons | Go-online glow, command buttons | Shared button styles only | Custom send button only |
| Corner radius | 6px on panels, 3-4px on buttons | 6px on rows/buttons | 8px on badges |
| Scrollbar management | Per-section scroll with clipping | Present in some panes | Full transcript scroll |
| Typography variety | Multiple sizes + mono/proportional | 10-11px mostly mono | Markdown + mono |
| Spacing system | Hardcoded but precise | Hardcoded, less precise | Hardcoded but structured |

---

## 7. Estimated Effort

| Phase | Description | Estimated size | Dependencies |
|-------|-------------|----------------|--------------|
| 1 | Extract MC palette into theme system | Small (new struct + const items) | None |
| 2 | Build section panel component | Medium (new component + helper) | Phase 1 |
| 3 | Animation framework | Small (utility module) | None |
| 4 | State computation extraction | Medium-Large (per-pane structs) | None |
| 5 | Button variant unification | Small (enum extension) | Phases 1, 3 |
| 6 | Pane-by-pane migration | Large (13 panes) | Phases 1-5 |
| 7 | MC self-improvement | Medium (refactor + token adoption) | Phases 1-3 |

Phases 1, 3, and 4 can proceed in parallel. Phase 2 depends on Phase 1. Phase 5 depends on Phases 1 and 3. Phase 6 depends on all prior phases. Phase 7 can begin alongside Phase 6.

---

## 8. Rust Best Practices Applied

1. **`const` color values**: Pre-compute all palette colors as `const Hsla` to avoid runtime allocation.
2. **Builder pattern**: Use `Default` + field override for options structs (`SectionPanelOptions`, `ButtonStyle` params).
3. **Separation of concerns**: View state structs decouple business logic from rendering, enabling unit testing without a graphics context.
4. **Module organization**: Extract components into `wgpui/src/components/` rather than growing `pane_renderer.rs`.
5. **`#[non_exhaustive]`**: On enums that will gain variants over time (button styles, animation types).
6. **`#[inline]`**: On small hot-path animation helpers to avoid function-call overhead in the render loop.
7. **Thread-local frame cache**: For wall-clock time to avoid repeated `SystemTime::now()` syscalls per frame.
8. **`Cow<'_, str>`**: In view state structs to avoid unnecessary `String` allocation for static labels.
9. **Trait-based rendering**: Consider a `PaneRenderer` trait with `view_state()` and `paint()` methods for future panes, though this is optional given the current architecture.

---

## 9. Recommendations

| Priority | Recommendation |
|----------|----------------|
| P0 | Extract `PanePalette` into `wgpui` theme system (Phase 1) - unblocks all other work |
| P0 | Build `paint_section_panel` component (Phase 2) - highest visual impact |
| P1 | Add animation framework (Phase 3) - enables status animations everywhere |
| P1 | Migrate Wallet pane first (Phase 6, item 1) - user-facing and validates the approach |
| P2 | Extract state computation for all panes (Phase 4) - improves testability |
| P2 | Unify button system (Phase 5) - reduces code duplication |
| P3 | Migrate remaining panes (Phase 6, items 2-13) - systematic rollout |
| P3 | Improve MC itself (Phase 7) - reduce file size and adopt tokens |

---

## 10. Follow-On Issues

- [ ] `wgpui`: Add `PanePalette` struct and `MISSION_CONTROL_PALETTE` const to theme system
- [ ] `wgpui`: Add `animation` module with `pulse()`, `spatial_pulse()`, `wall_clock_seconds()`
- [ ] `wgpui`: Add `SectionPanel` component to `components/`
- [ ] `autopilot-desktop`: Extract MC sub-sections into separate functions
- [ ] `autopilot-desktop`: Migrate Wallet pane to new panel system (pilot migration)
- [ ] `autopilot-desktop`: Define per-pane palettes for all pane domains
- [ ] `autopilot-desktop`: Add view state structs for all non-MC panes
- [ ] `autopilot-desktop`: Systematic pane migration (12 remaining panes after Wallet pilot)
- [ ] `wgpui`: Add `font_size::XXS` (10px) and `font_size::XXXS` (9px) tokens
- [ ] `wgpui`: Make `Hsla::from_hex` a `const fn` (or add `const` constructors)

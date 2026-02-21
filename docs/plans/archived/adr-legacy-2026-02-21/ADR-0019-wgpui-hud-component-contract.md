# ADR-0019: WGPUI HUD Component Contract

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents uses a custom GPU-accelerated UI framework (`wgpui`) for rendering the Autopilot application. This framework provides a component system for building interactive HUD-style interfaces. As we add more inline UI elements (issue selectors, status indicators, interactive panels), we need clear contracts for:

- How components are structured
- How events flow through the system
- How rendering and hit-testing work
- How state is managed across frames

Without clear contracts, ad-hoc UI code accumulates in rendering functions, leading to:
- Duplicated hit-testing logic
- Inconsistent event handling
- Layout calculations scattered across files
- Difficult-to-maintain rendering code

## Decision

**We adopt the WGPUI Component trait as the canonical interface for all interactive HUD elements.**

### Component Trait Contract

All interactive UI elements MUST implement the `Component` trait:

```rust
pub trait Component {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext);
    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult;
    fn size_hint(&self) -> (Option<f32>, Option<f32>) { (None, None) }
}
```

**Invariants:**

1. **Paint is read-only for external state** — `paint()` may mutate internal component state (animation progress, cached measurements) but MUST NOT mutate application state.

2. **Events return propagation intent** — Components MUST return `EventResult::Handled` to stop propagation or `EventResult::Ignored` to allow parent handling.

3. **Bounds are authoritative** — Components MUST NOT render outside their provided bounds (clipping is the caller's responsibility).

4. **State lives in the component** — Interactive state (hover, focus, selection) MUST be stored in the component struct, not in external application state.

### Context System

**PaintContext** provides rendering capabilities:
- `scene: &mut Scene` — Accumulated draw calls
- `text: &mut TextSystem` — Text layout and rendering
- `scale_factor: f32` — DPI scaling (logical → physical)
- `scroll_offset: Point` — Current scroll position

**EventContext** provides interaction capabilities:
- `focused: Option<u64>` — Current keyboard focus
- `hovered: Option<u64>` — Current mouse hover
- Focus chain management for Tab navigation
- Clipboard read/write closures

### Rendering Pipeline

1. Components call `cx.scene.draw_quad()`, `cx.scene.draw_text()`, etc.
2. Primitives accumulate with layer numbers for z-ordering
3. Layers render back-to-front at GPU boundary
4. Physical pixels = logical pixels × scale_factor

**Layer ordering:**
- Lower layer numbers render first (background)
- Higher layer numbers render last (foreground/overlay)
- Components SHOULD use `cx.scene.push_layer()` / `pop_layer()` for overlays

### Hit Testing Contract

Hit testing uses bounding-box AABB:

```rust
impl Bounds {
    pub fn contains(&self, point: Point) -> bool;
}
```

**Rules:**
1. Components store their rendered bounds for later hit-testing
2. Event dispatch respects depth ordering (topmost first)
3. Components MUST check `bounds.contains(point)` before handling mouse events
4. Transparent/decorative areas SHOULD return `EventResult::Ignored`

### Component Categories

**Non-Interactive (decorative):**
- Implement `paint()` only
- Return `EventResult::Ignored` for all events
- Examples: Reticle, Scanlines, Frame, backgrounds

**Interactive:**
- Store internal state: `hovered`, `pressed`, `selected_index`, `value`
- Handle mouse events: move, down, up
- Handle keyboard events: Tab, Enter, Escape, arrows
- Store callbacks: `on_click`, `on_change`, `on_submit`
- Examples: Button, TextInput, CommandPalette, ContextMenu

### Inline Chat Components

For components rendered inline in the chat (like InlineIssueSelector):

1. **Layout calculation** — Compute bounds during the layout phase, store in component
2. **Rendering** — Draw at stored bounds, offset by scroll
3. **Hit testing** — Store button bounds in component for click detection
4. **Event handling** — Check `input_focus` state before capturing keyboard events

**Pattern:**
```rust
// In ChatState
pub inline_issue_selector: Option<InlineIssueSelector>,

// In AppState
pub input_focus: InputFocus,  // ChatInput | IssueSelector | Modal

// In rendering
if let Some(selector) = &mut state.chat.inline_issue_selector {
    // Store bounds during paint for later hit-testing
    selector.suggestion_bounds.clear();
    for (idx, suggestion) in selector.suggestions.iter().enumerate() {
        let bounds = Bounds::new(x, y, width, height);
        selector.suggestion_bounds.push(bounds);
        // render...
    }
}

// In event handling
if state.input_focus == InputFocus::IssueSelector {
    // Handle keyboard shortcuts (1-9, S, Escape)
}
```

## Consequences

**Positive:**
- Clear contract for building interactive UI
- Consistent event handling across components
- Reusable hit-testing and focus management
- Easier to add new inline components

**Negative:**
- Requires refactoring existing ad-hoc UI code
- More boilerplate for simple components
- Learning curve for the component model

**Neutral:**
- Components own their state (different from React/immediate-mode)
- Bounds must be explicitly passed and stored

## Alternatives Considered

1. **Immediate-mode UI (egui-style)** — Rejected because we need persistent state for animations and complex interactions.

2. **Full retained-mode (DOM-like)** — Rejected as too heavy; we don't need full tree diffing.

3. **Continue with ad-hoc rendering** — Rejected because it leads to scattered, hard-to-maintain code.

## References

- `crates/wgpui/src/components/component.rs` — Component trait
- `crates/wgpui/src/components/context.rs` — PaintContext, EventContext
- `crates/wgpui/src/scene.rs` — Scene rendering
- `crates/wgpui/src/components/hud/` — HUD component implementations
- `crates/autopilot/src/app/chat/state.rs` — InlineIssueSelector example

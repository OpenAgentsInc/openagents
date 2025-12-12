# Shadcn Components for Rust/GPUI

## Overview

Port shadcn/ui components to Rust/GPUI. **Primitives first** - all 11 simple components before complex ones. Components live in `crates/ui/src/components/`. Stories are both **individual AND grouped**, plus a UI kitchen sink.

## Architecture

### Component Structure Pattern

```rust
// 1. Variant enums (replaces CVA)
pub enum ButtonVariant { Default, Destructive, Outline, Secondary, Ghost, Link }
pub enum ButtonSize { Default, Sm, Lg, Icon }

// 2. Component struct with builder pattern
pub struct Button {
    label: SharedString,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
    on_click: Option<Box<dyn Fn(&mut Window, &mut App)>>,
}

impl Button {
    pub fn new(label: impl Into<SharedString>) -> Self { ... }
    pub fn variant(mut self, v: ButtonVariant) -> Self { ... }
    pub fn size(mut self, s: ButtonSize) -> Self { ... }
    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self { ... }
}

// 3. RenderOnce for stateless, Render for stateful
impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement { ... }
}
```

### File Organization

```
crates/ui/src/
├── lib.rs                    # Re-exports
├── text_input.rs             # Existing (keep)
├── components/
│   ├── mod.rs                # Component re-exports
│   ├── button.rs
│   ├── label.rs
│   ├── input.rs              # Simplified stateless wrapper
│   ├── textarea.rs
│   ├── separator.rs
│   ├── skeleton.rs
│   ├── spinner.rs
│   ├── progress.rs
│   ├── checkbox.rs
│   ├── switch.rs
│   ├── kbd.rs
│   └── ... (more components)

crates/storybook/src/stories/
├── mod.rs                    # Updated exports
├── ui/
│   ├── mod.rs                # UI story re-exports
│   ├── button_story.rs       # Individual: all button variants
│   ├── label_story.rs
│   ├── input_story.rs
│   ├── ...
│   ├── primitives_story.rs   # Grouped: all primitives together
│   ├── form_controls_story.rs
│   └── ui_kitchen_sink.rs    # All UI components
└── ... (existing stories)
```

---

## Implementation Phases

### Phase 1: All Primitives First (11 components)

Start with every simple component - these have no internal state and minimal logic.

| # | Component | Description | Complexity |
|---|-----------|-------------|------------|
| 1 | `Button` | Primary action, establishes variant pattern | ~100 lines |
| 2 | `Label` | Text wrapper with disabled state | ~30 lines |
| 3 | `Input` | Stateless styled input wrapper | ~40 lines |
| 4 | `Textarea` | Multi-line text input | ~40 lines |
| 5 | `Separator` | Horizontal/vertical divider | ~30 lines |
| 6 | `Skeleton` | Loading placeholder shimmer | ~25 lines |
| 7 | `Spinner` | SVG loading indicator | ~35 lines |
| 8 | `Progress` | Progress bar | ~40 lines |
| 9 | `Checkbox` | Check indicator | ~60 lines |
| 10 | `Switch` | Toggle on/off | ~50 lines |
| 11 | `Kbd` | Keyboard key display | ~35 lines |

**Files to Create:**
```
crates/ui/src/components/mod.rs
crates/ui/src/components/button.rs
crates/ui/src/components/label.rs
crates/ui/src/components/input.rs
crates/ui/src/components/textarea.rs
crates/ui/src/components/separator.rs
crates/ui/src/components/skeleton.rs
crates/ui/src/components/spinner.rs
crates/ui/src/components/progress.rs
crates/ui/src/components/checkbox.rs
crates/ui/src/components/switch.rs
crates/ui/src/components/kbd.rs
```

**Storybook Files:**
```
crates/storybook/src/stories/ui/mod.rs
crates/storybook/src/stories/ui/button_story.rs
crates/storybook/src/stories/ui/primitives_story.rs    # Grouped
crates/storybook/src/stories/ui/ui_kitchen_sink.rs     # All together
```

### Phase 2: Simple Components (8 components)

| Component | Description |
|-----------|-------------|
| `Badge` | Status badges with variants |
| `Avatar` | Image with fallback initials |
| `Alert` | Alert boxes (default/destructive) |
| `Toggle` | Toggle button states |
| `Collapsible` | Expandable section |
| `AspectRatio` | Maintain aspect ratio |
| `RadioGroup` | Radio button group |
| `Slider` | Range slider |

### Phase 3: Medium Components

| Component | Description |
|-----------|-------------|
| `Card` | Container with sub-components |
| `Tabs` | Tab navigation |
| `Accordion` | Collapsible sections |
| `Tooltip` | Hover tooltips |
| `ScrollArea` | Scrollable container |
| `Table` | Data table structure |
| `Breadcrumb` | Navigation breadcrumbs |
| `Pagination` | Page navigation |

### Phase 4: Complex Components

| Component | Description |
|-----------|-------------|
| `Select` | Dropdown selection |
| `Popover` | Click popovers |
| `Dialog` | Modal dialogs |
| `DropdownMenu` | Dropdown menus |
| `Sheet` | Side panels |
| `Command` | Command palette |

---

## Detailed Component Specs

### Button (establishes pattern for all)

```rust
// crates/ui/src/components/button.rs
use gpui::*;
use theme::ui::button;

#[derive(Default, Clone, Copy)]
pub enum ButtonVariant { #[default] Default, Destructive, Outline, Secondary, Ghost, Link }

#[derive(Default, Clone, Copy)]
pub enum ButtonSize { #[default] Default, Sm, Lg, Icon }

pub struct Button {
    label: SharedString,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
    on_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Button {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self { label: label.into(), variant: Default::default(), size: Default::default(), disabled: false, on_click: None }
    }
    pub fn variant(mut self, v: ButtonVariant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: ButtonSize) -> Self { self.size = s; self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }
    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self { self.on_click = Some(Box::new(f)); self }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg, text_color, border) = match self.variant {
            ButtonVariant::Default => (button::DEFAULT_BG, button::DEFAULT_TEXT, None),
            ButtonVariant::Destructive => (button::DESTRUCTIVE_BG, button::DESTRUCTIVE_TEXT, None),
            // ... etc
        };

        div()
            .px(px(16.0)).py(px(8.0))
            .bg(bg)
            .text_color(text_color)
            .rounded(px(6.0))
            .cursor_pointer()
            .when(self.disabled, |d| d.opacity(0.5).cursor_not_allowed())
            .when_some(self.on_click, |d, handler| {
                d.on_click(move |_, window, cx| handler(window, cx))
            })
            .child(self.label)
    }
}
```

### Usage Examples

```rust
// Button
Button::new("Save").variant(ButtonVariant::Default).on_click(|_, _| { })

// Checkbox
Checkbox::new().checked(true).on_change(|checked, _, _| { })

// Progress
Progress::new().value(0.75)  // 75%

// Separator
Separator::horizontal()
Separator::vertical()

// Kbd
Kbd::new("⌘K")
```

---

## Theme Integration

**Add to `crates/theme/src/lib.rs`:**

```rust
pub mod ui {
    use super::*;

    pub mod button {
        use super::*;
        pub const DEFAULT_BG: Hsla = c(0.54, 0.43, 0.67, 1.0);  // accent::PRIMARY
        pub const DEFAULT_TEXT: Hsla = c(0.0, 0.0, 1.0, 1.0);   // white
        pub const DESTRUCTIVE_BG: Hsla = c(0.0, 0.65, 0.55, 1.0);
        pub const DESTRUCTIVE_TEXT: Hsla = c(0.0, 0.0, 1.0, 1.0);
        pub const SECONDARY_BG: Hsla = c(0.0, 0.0, 0.15, 1.0);
        pub const SECONDARY_TEXT: Hsla = c(0.0, 0.0, 0.9, 1.0);
        pub const GHOST_HOVER_BG: Hsla = c(0.0, 0.0, 1.0, 0.1);
        pub const OUTLINE_BORDER: Hsla = c(0.0, 0.0, 0.3, 1.0);
    }

    pub mod checkbox {
        use super::*;
        pub const CHECKED_BG: Hsla = c(0.54, 0.43, 0.67, 1.0);
        pub const UNCHECKED_BG: Hsla = c(0.0, 0.0, 0.1, 1.0);
        pub const CHECK_COLOR: Hsla = c(0.0, 0.0, 1.0, 1.0);
    }

    pub mod progress {
        use super::*;
        pub const TRACK_BG: Hsla = c(0.0, 0.0, 0.15, 1.0);
        pub const INDICATOR_BG: Hsla = c(0.54, 0.43, 0.67, 1.0);
    }
}
```

---

## Storybook Registration

**Update `crates/storybook/src/main.rs`:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString, strum::EnumIter)]
#[strum(serialize_all = "snake_case")]
pub enum ComponentStory {
    // Existing stories...
    PinStates,
    UnitView,
    // ...

    // NEW: UI component stories
    Button,           // Individual
    Primitives,       // Grouped: all 11 primitives
    UiKitchenSink,    // All UI components
}

impl ComponentStory {
    pub fn story(&self, _window: &mut Window, cx: &mut App) -> AnyView {
        match self {
            // ...existing...
            Self::Button => cx.new(|_| ButtonStory).into(),
            Self::Primitives => cx.new(|_| PrimitivesStory).into(),
            Self::UiKitchenSink => cx.new(|_| UiKitchenSinkStory).into(),
        }
    }
}
```

---

## Files to Modify

| File | Change |
|------|--------|
| `crates/ui/src/lib.rs` | Add `pub mod components; pub use components::*;` |
| `crates/ui/Cargo.toml` | Already has gpui, theme deps |
| `crates/theme/src/lib.rs` | Add `pub mod ui { ... }` module |
| `crates/storybook/src/main.rs` | Add new ComponentStory variants |
| `crates/storybook/src/stories/mod.rs` | Add `pub mod ui;` |
| `crates/storybook/Cargo.toml` | Add `ui` dependency |

---

## Execution Order

1. **Setup** - Create `crates/ui/src/components/mod.rs`, add theme colors
2. **Button** - First component, establishes all patterns
3. **Label, Separator, Kbd** - Simplest components
4. **Skeleton, Spinner, Progress** - Loading/status indicators
5. **Input, Textarea** - Text inputs (wrappers)
6. **Checkbox, Switch** - Toggle inputs
7. **Stories** - Individual + Primitives grouped + Kitchen sink

---

## Success Criteria

- [ ] All 11 Phase 1 primitives implemented with builder API
- [ ] Individual story for Button showing all variants/sizes/states
- [ ] Grouped PrimitivesStory showing all 11 primitives
- [ ] UiKitchenSinkStory combining everything
- [ ] Theme colors centralized in `theme::ui::*`
- [ ] `cargo run -p storybook button` works
- [ ] `cargo run -p storybook primitives` works

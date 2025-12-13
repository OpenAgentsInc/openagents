# Layout Engine

## Overview

wgpui uses [Taffy 0.9](https://github.com/DioxusLabs/taffy) for CSS Flexbox layout. Taffy is a high-performance layout library written in Rust.

## Architecture

```rust
struct LayoutEngine {
    taffy: TaffyTree<()>,           // Taffy layout tree
    nodes: SlotMap<LayoutId, NodeId>, // Our ID → Taffy ID mapping
}
```

## Basic Usage

```rust
use wgpui::{LayoutEngine, LayoutStyle, px, pct, auto, length};
use taffy::prelude::*;

let mut engine = LayoutEngine::new();

// Create root container
let root_style = LayoutStyle {
    display: Display::Flex,
    flex_direction: FlexDirection::Column,
    width: px(400.0),
    height: px(300.0),
    padding: Rect {
        left: LengthPercentage::length(16.0),
        right: LengthPercentage::length(16.0),
        top: LengthPercentage::length(16.0),
        bottom: LengthPercentage::length(16.0),
    },
    gap: taffy::Size {
        width: LengthPercentage::length(8.0),
        height: LengthPercentage::length(8.0),
    },
    ..Default::default()
};

// Create children
let child_style = LayoutStyle {
    flex_grow: 1.0,
    ..Default::default()
};

let child1 = engine.request_layout(&child_style, &[]);
let child2 = engine.request_layout(&child_style, &[]);
let root = engine.request_layout(&root_style, &[child1, child2]);

// Compute layout
engine.compute_layout(root, Size::new(800.0, 600.0));

// Get computed bounds
let root_bounds = engine.layout(root);
let child1_bounds = engine.layout(child1);
```

## LayoutStyle

The `LayoutStyle` struct maps to Taffy's `Style`. Note that some fields use Taffy types directly:

```rust
pub struct LayoutStyle {
    pub display: Display,
    pub position: Position,
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub align_self: Option<AlignSelf>,
    pub gap: taffy::Size<LengthPercentage>,        // Note: taffy::Size, not wgpui::Size
    pub width: Dimension,
    pub height: Dimension,
    pub min_width: Dimension,
    pub max_width: Dimension,
    pub min_height: Dimension,
    pub max_height: Dimension,
    pub padding: Rect<LengthPercentage>,
    pub margin: Rect<LengthPercentageAuto>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Dimension,
    pub overflow: taffy::Point<Overflow>,          // Note: taffy::Point
    pub inset: Rect<LengthPercentageAuto>,
}
```

### Display

```rust
display: Display::Flex,  // or Display::None to hide
```

### Flex Direction

```rust
flex_direction: FlexDirection::Column,  // or Row, ColumnReverse, RowReverse
```

### Justify Content (Main Axis)

```rust
justify_content: Some(JustifyContent::Center),
// Options: FlexStart, FlexEnd, Center, SpaceBetween, SpaceAround, SpaceEvenly
```

### Align Items (Cross Axis)

```rust
align_items: Some(AlignItems::Center),
// Options: FlexStart, FlexEnd, Center, Stretch, Baseline
```

### Gap

```rust
// Gap uses taffy::Size, not wgpui::Size
gap: taffy::Size {
    width: LengthPercentage::length(8.0),
    height: LengthPercentage::length(8.0),
},
```

### Sizing

```rust
width: px(200.0),       // Fixed width
height: pct(50.0),      // 50% of parent
min_width: px(100.0),   // Minimum constraint
max_width: px(400.0),   // Maximum constraint
```

### Flex Item Properties

```rust
flex_grow: 1.0,         // Grow factor
flex_shrink: 1.0,       // Shrink factor
flex_basis: auto(),     // Initial size (auto, px, %)
```

### Padding & Margin

```rust
// Padding - build Rect manually
padding: Rect {
    left: LengthPercentage::length(16.0),
    right: LengthPercentage::length(16.0),
    top: LengthPercentage::length(8.0),
    bottom: LengthPercentage::length(8.0),
},

// Margin with auto
margin: Rect {
    left: LengthPercentageAuto::length(8.0),
    right: LengthPercentageAuto::length(8.0),
    top: LengthPercentageAuto::auto(),
    bottom: LengthPercentageAuto::auto(),
},
```

### Positioning

```rust
position: Position::Relative,  // or Absolute

// Inset for absolute positioning
inset: Rect {
    top: LengthPercentageAuto::length(10.0),
    left: LengthPercentageAuto::length(10.0),
    right: LengthPercentageAuto::auto(),
    bottom: LengthPercentageAuto::auto(),
},
```

### Overflow

```rust
// Overflow uses taffy::Point
overflow: taffy::Point {
    x: Overflow::Visible,  // or Scroll, Hidden, Clip
    y: Overflow::Scroll,
},
```

## Dimension Helpers

wgpui provides helper functions that wrap Taffy's associated functions:

```rust
// Pixels
px(100.0) → Dimension::length(100.0)

// Percentage (input is 0-100, converted to 0-1)
pct(50.0) → Dimension::percent(0.5)

// Auto
auto() → Dimension::auto()

// Relative (input is fraction, 1.0 = 100%)
relative(1.0) → Dimension::percent(1.0)

// Length for LengthPercentage
length(8.0) → LengthPercentage::length(8.0)

// Length for LengthPercentageAuto
length_auto(8.0) → LengthPercentageAuto::length(8.0)

// Zero
zero() → LengthPercentage::length(0.0)
```

**Important**: Taffy 0.9 uses associated functions (`Dimension::length()`) not enum variants (`Dimension::Length()`).

## Layout Integration

### With Scene

```rust
let mut engine = LayoutEngine::new();
let mut scene = Scene::new();

// Define layout
let container = engine.request_layout(&container_style, &[child1, child2]);
engine.compute_layout(container, available_size);

// Render with computed bounds
let bounds = engine.layout(container);
scene.draw_quad(Quad::new(bounds).with_background(theme::bg::SURFACE));
```

### Per-Frame

```rust
fn render_frame(&mut self) {
    self.layout_engine.clear();  // Clear previous layout

    // Build layout tree
    let root = self.build_layout();
    self.layout_engine.compute_layout(root, self.viewport_size);

    // Render with computed positions
    self.render_node(root);
}
```

## Measured Nodes

For text or dynamic content, use measured nodes:

```rust
let text_node = engine.request_measured(&style, |known_size, available_space, node_id, context, style| {
    // Measure text with given constraints
    let max_width = match available_space.width {
        AvailableSpace::Definite(w) => Some(w),
        _ => known_size.width,
    };
    let size = text_system.measure_size(text, font_size, max_width);
    taffy::Size { width: size.width, height: size.height }
});
```

**Note**: The current implementation creates a leaf node with context but doesn't wire up the measure function callback. This is a known limitation.

## Common Patterns

### Centered Content

```rust
LayoutStyle {
    display: Display::Flex,
    justify_content: Some(JustifyContent::Center),
    align_items: Some(AlignItems::Center),
    width: relative(1.0),
    height: relative(1.0),
    ..Default::default()
}
```

### Sidebar Layout

```rust
// Container
LayoutStyle {
    display: Display::Flex,
    flex_direction: FlexDirection::Row,
    width: relative(1.0),
    height: relative(1.0),
    ..Default::default()
}

// Sidebar
LayoutStyle {
    width: px(280.0),
    flex_shrink: 0.0,
    ..Default::default()
}

// Main content
LayoutStyle {
    flex_grow: 1.0,
    ..Default::default()
}
```

### Stack with Gap

```rust
LayoutStyle {
    display: Display::Flex,
    flex_direction: FlexDirection::Column,
    gap: taffy::Size {
        width: LengthPercentage::length(8.0),
        height: LengthPercentage::length(8.0),
    },
    padding: Rect {
        left: LengthPercentage::length(16.0),
        right: LengthPercentage::length(16.0),
        top: LengthPercentage::length(16.0),
        bottom: LengthPercentage::length(16.0),
    },
    ..Default::default()
}
```

## Performance

### Caching

Taffy caches layout results. Avoid `clear()` if layout structure is stable:

```rust
// Instead of clear + rebuild, update styles in place
engine.set_style(node_id, new_style);
engine.compute_layout(root, size);
```

### Incremental Layout

For large trees, use dirty tracking:

```rust
// Only mark changed subtrees
engine.mark_dirty(changed_node);
engine.compute_layout(root, size);  // Only recomputes dirty nodes
```

## Type Disambiguation

wgpui has its own `Size` and `Point` types that differ from Taffy's:

| wgpui Type | Taffy Type | Usage |
|------------|------------|-------|
| `wgpui::Size` | Non-generic `{ width: f32, height: f32 }` | Rendering, bounds |
| `taffy::Size<T>` | Generic size with associated types | Gap, available space |
| `wgpui::Point` | Non-generic `{ x: f32, y: f32 }` | Positions |
| `taffy::Point<T>` | Generic point | Overflow settings |

When working with `LayoutStyle`, use Taffy types for `gap` and `overflow` fields.

## Limitations

- No CSS Grid (Taffy supports it, not exposed yet)
- No `aspect-ratio` support
- No `min-content` / `max-content` sizing
- Position: Absolute is relative to nearest positioned ancestor
- `request_measured` doesn't wire up the measure callback (creates static leaf)

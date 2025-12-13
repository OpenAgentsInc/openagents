# Layout Engine

## Overview

wgpui uses [Taffy](https://github.com/DioxusLabs/taffy) for CSS Flexbox layout. Taffy is a high-performance layout library written in Rust.

## Architecture

```rust
struct LayoutEngine {
    taffy: TaffyTree<()>,           // Taffy layout tree
    nodes: SlotMap<LayoutId, NodeId>, // Our ID → Taffy ID mapping
}
```

## Basic Usage

```rust
use wgpui::{LayoutEngine, LayoutStyle, px, pct, auto};

let mut engine = LayoutEngine::new();

// Create root container
let root_style = LayoutStyle {
    display: Display::Flex,
    flex_direction: FlexDirection::Column,
    width: px(400.0),
    height: px(300.0),
    padding: Rect::from_length(length(16.0)),
    gap: Size::from_length(length(8.0)),
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

The `LayoutStyle` struct maps to Taffy's `Style`:

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
gap: Size::from_length(length(8.0)),  // Gap between items
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
padding: Rect {
    top: length(8.0),
    right: length(16.0),
    bottom: length(8.0),
    left: length(16.0),
},
margin: Rect::from_length(length_auto(8.0)),
```

### Positioning

```rust
position: Position::Relative,  // or Absolute
inset: Rect {
    top: length_auto(10.0),
    left: length_auto(10.0),
    ..Default::default()
},
```

## Dimension Helpers

```rust
// Pixels
px(100.0) → Dimension::Length(100.0)

// Percentage
pct(50.0) → Dimension::Percent(0.5)

// Auto
auto() → Dimension::Auto

// Relative (fraction)
relative(1.0) → Dimension::Percent(1.0)

// Length for LengthPercentage
length(8.0) → LengthPercentage::Length(8.0)

// Length for LengthPercentageAuto
length_auto(8.0) → LengthPercentageAuto::Length(8.0)

// Zero
zero() → LengthPercentage::Length(0.0)
```

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
let text_node = engine.request_measured(&style, |known_size, available_space, _, _, _| {
    // Measure text with given constraints
    let max_width = match available_space.width {
        AvailableSpace::Definite(w) => Some(w),
        _ => known_size.width,
    };
    text_system.measure_size(text, font_size, max_width)
});
```

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
    gap: Size::from_length(length(8.0)),
    padding: Rect::from_length(length(16.0)),
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

## Limitations

- No CSS Grid (Taffy supports it, not exposed yet)
- No `aspect-ratio` support
- No `min-content` / `max-content` sizing
- Position: Absolute is relative to nearest positioned ancestor

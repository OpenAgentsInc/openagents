# Text Rendering Coordinate System

This document explains the coordinate system used in WGPUI's text rendering pipeline, the critical bugs that can occur, and how to avoid them.

## Overview

WGPUI uses a **logical pixel** coordinate system for all layout and scene operations. Conversion to physical pixels happens at the **GPU boundary** during rendering. This matches Zed's architecture and ensures consistent behavior across different display scale factors.

## Coordinate Flow

```
1. USER CODE (Logical Pixels)
   ├─ text_system.layout("Hello", Point::new(100, 50), font_size=14.0, ...)
   ├─ scene.draw_quad(Quad::new(Bounds::new(0, 0, 800, 600)))
   └─ All positions, sizes, and font sizes are in LOGICAL pixels

2. TEXT SYSTEM (Internal Processing)
   ├─ shape_text(): Shapes at physical_font_size = font_size * scale_factor
   ├─ cosmic_text returns glyph positions in physical pixels
   ├─ swash rasterizes glyphs at physical_font_size
   └─ layout_styled(): Converts back to LOGICAL by dividing by scale_factor

3. SCENE (Logical Pixels)
   ├─ TextRun stores: origin (logical), glyph offsets (logical), glyph sizes (logical)
   ├─ Quad stores: bounds (logical)
   └─ All scene data is in LOGICAL pixels

4. GPU BOUNDARY (Scale Here!)
   ├─ gpu_quads(scale_factor): Multiplies all quad coords by scale_factor
   ├─ gpu_text_quads(scale_factor): Multiplies all text coords by scale_factor
   └─ This is the ONLY place where logical → physical conversion happens

5. SHADER (Physical Pixels)
   ├─ Receives positions/sizes in physical pixels
   ├─ Viewport is in physical pixels
   └─ Converts to normalized device coordinates (-1 to 1)
```

## Key Principles

### 1. All User-Facing APIs Use Logical Pixels

```rust
// User specifies logical coordinates
let text_run = text_system.layout(
    "Hello",
    Point::new(100.0, 50.0),  // Logical pixels
    14.0,                      // Logical font size
    color,
);

// User specifies logical bounds
scene.draw_quad(Quad::new(
    Bounds::new(0.0, 0.0, 800.0, 600.0)  // Logical pixels
));
```

### 2. TextSystem Shapes at Physical Size But Returns Logical

```rust
fn shape_text(&mut self, text: &str, font_size: f32, style: FontStyle) {
    // Shape at physical size for crisp rendering
    let physical_font_size = font_size * self.scale_factor;
    let metrics = Metrics::new(physical_font_size, physical_font_size * 1.2);

    // cosmic_text returns positions for physical_font_size
    // We convert back to logical by dividing
    let logical_width = line_width / self.scale_factor;
}

fn layout_styled(...) {
    // Glyph offsets and sizes are converted to logical
    let glyph_instance = GlyphInstance {
        offset: Point::new(
            (glyph_x + entry.offset.x) / self.scale_factor,
            (line_y + entry.offset.y) / self.scale_factor,
        ),
        size: Size::new(
            entry.size.width / self.scale_factor,
            entry.size.height / self.scale_factor,
        ),
        ...
    };
}
```

### 3. Scaling Happens ONCE at GPU Boundary

```rust
// In scene.rs
impl GpuQuad {
    pub fn from_quad(quad: &Quad, scale_factor: f32) -> Self {
        Self {
            origin: [
                quad.bounds.origin.x * scale_factor,  // Logical → Physical
                quad.bounds.origin.y * scale_factor,
            ],
            size: [
                quad.bounds.size.width * scale_factor,
                quad.bounds.size.height * scale_factor,
            ],
            ...
        }
    }
}

impl GpuTextQuad {
    pub fn from_glyph(glyph: &GlyphInstance, origin: Point, color: Hsla, scale_factor: f32) -> Self {
        Self {
            position: [
                (origin.x + glyph.offset.x) * scale_factor,  // Logical → Physical
                (origin.y + glyph.offset.y) * scale_factor,
            ],
            size: [
                glyph.size.width * scale_factor,
                glyph.size.height * scale_factor,
            ],
            ...
        }
    }
}
```

## Critical Bug: Double Scaling

### The Bug

The most critical bug in text rendering is **double scaling**, which causes glyphs to render at 2x or more their intended size.

**Symptoms:**
- Text appears too large
- Characters overlap heavily
- Glyph spacing is much smaller than glyph width

**Root Cause:**
Passing `scale_factor` to `glyph.physical()` when already shaping at `physical_font_size`.

```rust
// WRONG - causes double scaling!
let physical_font_size = font_size * self.scale_factor;  // Already scaled
// ...shape at physical_font_size...
let physical_glyph = glyph.physical((0.0, 0.0), self.scale_factor);  // Scales AGAIN!
```

The `physical()` method generates a cache key that includes the scale factor. When swash rasterizes the glyph, it uses this cache key, resulting in:
```
actual_size = physical_font_size * scale_factor
            = font_size * scale_factor * scale_factor
            = font_size * scale_factor²
```

On a 2x display: `14px * 2² = 56px` instead of `14px * 2 = 28px`.

### The Fix

Pass `1.0` to `glyph.physical()` since we already accounted for scaling:

```rust
// CORRECT
let physical_font_size = font_size * self.scale_factor;
// ...shape at physical_font_size...
let physical_glyph = glyph.physical((0.0, 0.0), 1.0);  // Don't scale again!
```

## Testing for Coordinate System Bugs

The following tests in `text.rs` catch coordinate system issues:

### 1. Glyph Size Not Double Scaled

```rust
#[test]
fn test_glyph_size_not_double_scaled() {
    let scale_factor = 2.0;
    let mut system = TextSystem::new(scale_factor);
    let font_size = 14.0;

    let text_run = system.layout("A", Point::ZERO, font_size, Hsla::white());
    let glyph = &text_run.glyphs[0];

    // Glyph width should be ~font_size * 0.6 for monospace, NOT font_size * 1.2
    assert!(glyph.size.width <= font_size * 1.0);
}
```

### 2. Glyph Spacing Matches Width

```rust
#[test]
fn test_glyph_spacing_matches_width() {
    let text_run = system.layout("AA", Point::ZERO, font_size, Hsla::white());

    let spacing = glyph_1.offset.x - glyph_0.offset.x;
    let width = glyph_0.size.width;

    // For monospace, spacing ≈ width. If spacing << width, glyphs overlap!
    assert!(spacing >= width * 0.8);
}
```

### 3. Consistent Sizing Across Scale Factors

```rust
#[test]
fn test_glyph_size_consistent_across_scale_factors() {
    let run_1x = system_1x.layout("A", ...);
    let run_2x = system_2x.layout("A", ...);

    // LOGICAL sizes should be approximately equal
    assert!((size_2x / size_1x - 1.0).abs() < 0.25);
}
```

### 4. Measured Width Contains All Glyphs

```rust
#[test]
fn test_measured_width_contains_all_glyphs() {
    let measured_width = system.measure(text, font_size);
    let rightmost_extent = glyphs.map(|g| g.offset.x + g.size.width).max();

    // Measured width must contain all glyphs
    assert!(measured_width >= rightmost_extent - 1.0);
}
```

## Debugging Coordinate Issues

### Add Debug Logging

```rust
// In shape_text()
log::debug!("shape_text: font_size={}, scale_factor={}, physical_font_size={}",
    font_size, self.scale_factor, physical_font_size);

// In layout_styled()
println!("Glyph[{}]: offset=({:.2}, {:.2}), size=({:.2}, {:.2})",
    i, glyph.offset.x, glyph.offset.y, glyph.size.width, glyph.size.height);
```

### Check Key Ratios

For healthy text rendering at any scale factor:

| Metric | Expected | Bug Indicator |
|--------|----------|---------------|
| glyph.size.width / font_size | ~0.6 for monospace | > 1.0 suggests double scaling |
| glyph spacing / glyph width | ~1.0 for monospace | < 0.5 suggests double scaling |
| measured width / glyph count | ~font_size * 0.6 | 2x expected suggests double scaling |

## Viewport and Resize

The viewport must be in **physical pixels**:

```rust
// In example/application code
let physical_width = config.width as f32;   // From window.inner_size() - physical
let physical_height = config.height as f32;

renderer.resize(&queue, Size::new(physical_width, physical_height), 1.0);
```

But scene layout should use **logical dimensions**:

```rust
let scale_factor = window.scale_factor() as f32;
let logical_width = physical_width / scale_factor;
let logical_height = physical_height / scale_factor;

build_scene(&mut scene, logical_width, logical_height);
```

## Summary

1. **User code**: Always logical pixels
2. **Scene**: Always logical pixels
3. **GPU boundary**: Scale from logical to physical (ONCE!)
4. **Shader**: Physical pixels
5. **Never double-scale**: If you shape at `physical_font_size`, don't pass `scale_factor` to `physical()`
6. **Test thoroughly**: Add tests that catch double-scaling by checking glyph size ratios

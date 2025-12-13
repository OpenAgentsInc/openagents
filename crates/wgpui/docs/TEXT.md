# Text System

## Overview

wgpui uses cosmic-text 0.12 for text shaping and rasterization, with a custom glyph atlas for efficient GPU rendering.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  TextSystem                                                  │
├─────────────────────────────────────────────────────────────┤
│  FontSystem (cosmic-text)                                    │
│  ├── Font database                                          │
│  ├── Font fallback chain                                    │
│  └── Shaping engine                                         │
├─────────────────────────────────────────────────────────────┤
│  SwashCache (cosmic-text)                                    │
│  └── Glyph rasterization                                    │
├─────────────────────────────────────────────────────────────┤
│  GlyphCache (custom)                                         │
│  ├── CacheKey → GlyphEntry mapping                          │
│  └── Atlas UV coordinates                                   │
├─────────────────────────────────────────────────────────────┤
│  AtlasData                                                   │
│  └── R8Unorm texture (1024x1024)                           │
└─────────────────────────────────────────────────────────────┘
```

## Usage

### Measuring Text

```rust
// Measure width without rendering
let width = text_system.measure("Hello, world!", 14.0);

// Measure full size with wrapping
let size = text_system.measure_size(
    "Hello, world!",
    14.0,
    Some(200.0)  // max width
);
```

### Rendering Text

```rust
let text_run = text_system.layout(
    "Hello, world!",
    Point::new(10.0, 20.0),  // origin
    14.0,                     // font size
    theme::text::PRIMARY,     // color
);

scene.draw_text(text_run);
```

## Glyph Atlas

### Texture Format

- Format: `R8Unorm` (single channel, 8-bit grayscale)
- Size: 1024x1024 (constant `ATLAS_SIZE`)
- Filtering: Linear (for smooth scaling)

### Packing Algorithm

Row-based packing with automatic row advancement and 1px padding:

```rust
// Simplified packing logic
if self.atlas_cursor_x + width > self.atlas_size {
    // Move to next row
    self.atlas_cursor_x = 0;
    self.atlas_cursor_y += self.atlas_row_height + 1;  // 1px padding
    self.atlas_row_height = 0;
}

let x = self.atlas_cursor_x;
let y = self.atlas_cursor_y;

self.atlas_cursor_x += width + 1;  // 1px padding
self.atlas_row_height = self.atlas_row_height.max(height);
```

### Cache Key

Glyphs are cached using cosmic-text's `CacheKey` wrapped in a local struct:

```rust
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct GlyphCacheKey {
    cache_key: CacheKey,  // cosmic-text's CacheKey includes font_id, glyph_id, size, subpixel position
}
```

## Text Shaping

### Process

1. **Segmentation**: Break text into Unicode segments
2. **Font Selection**: Choose font for each segment (with fallback)
3. **Shaping**: Apply ligatures, kerning, positioning
4. **Layout**: Compute final glyph positions

### Shaping Mode

```rust
buffer.set_text(
    &mut font_system,
    text,
    Attrs::new(),
    Shaping::Advanced  // Full shaping with ligatures
);
```

### Glyph Rasterization

The `SwashCache::get_image()` returns `&Option<SwashImage>`. To avoid borrow checker issues, we extract data immediately:

```rust
// get_image returns &Option<SwashImage>, requiring .as_ref()
let image_data = self.swash_cache.get_image(&mut self.font_system, cache_key)
    .as_ref()
    .map(|image| {
        (
            image.placement.left,
            image.placement.top,
            image.placement.width,
            image.placement.height,
            image.content,        // SwashContent enum
            image.data.to_vec(),  // Clone the glyph bitmap
        )
    });
```

### SwashContent Types

`SwashContent` is a unit enum (no tuple data) - the bitmap data is in `image.data`:

```rust
match content {
    SwashContent::Mask => {
        // Grayscale mask - use image.data directly (R8 format)
    }
    SwashContent::SubpixelMask => {
        // RGB subpixel data (3 bytes per pixel)
        // Currently skipped - would need conversion to grayscale
    }
    SwashContent::Color => {
        // RGBA color data (4 bytes per pixel, e.g., color emoji)
        // Currently skipped - would need separate RGBA atlas
    }
}
```

## Scale Factor Handling

### High-DPI Support

Text is rasterized at physical resolution for crisp rendering:

```rust
let physical_font_size = font_size * scale_factor;
let metrics = Metrics::new(physical_font_size, physical_font_size * 1.2);
```

### Cache Invalidation

Scale factor changes invalidate the glyph cache:

```rust
fn set_scale_factor(&mut self, scale_factor: f32) {
    if (self.scale_factor - scale_factor).abs() > 0.001 {
        self.scale_factor = scale_factor;
        self.clear_cache();  // Re-rasterize all glyphs
    }
}
```

## TextRun Structure

```rust
pub struct TextRun {
    glyphs: Vec<GlyphInstance>,
    origin: Point,
    color: Hsla,
    font_size: f32,
}

pub struct GlyphInstance {
    pub glyph_id: u16,
    pub offset: Point,      // Relative to origin (logical pixels)
    pub size: Size,         // Glyph dimensions (logical pixels)
    pub uv: [f32; 4],       // Atlas UV (min_u, min_v, max_u, max_v)
}
```

## Font Loading

### System Fonts

cosmic-text loads system fonts automatically:

```rust
let font_system = FontSystem::new();
// Includes all system fonts
```

### Embedded Fonts

To embed custom fonts (not yet implemented):

```rust
font_system.db_mut().load_font_data(
    include_bytes!("../fonts/BerkeleyMono-Regular.ttf").to_vec()
);
```

## Atlas Management

### Dirty Tracking

The text system tracks when the atlas needs GPU upload:

```rust
// Check if atlas changed
if text_system.is_dirty() {
    renderer.update_atlas(
        &queue,
        text_system.atlas_data(),
        text_system.atlas_size(),
    );
    text_system.mark_clean();
}
```

### Atlas Overflow

When the atlas fills up, new glyphs return empty entries:

```rust
if self.atlas_cursor_y + height > self.atlas_size {
    log::warn!("Glyph atlas is full!");
    return GlyphEntry {
        uv: [0.0, 0.0, 0.0, 0.0],
        size: Size::ZERO,
        offset: Point::ZERO,
    };
}
```

## Performance Tips

### Batch Text Draws

Group text with the same style:

```rust
// Good: Single layout call
let run = text_system.layout("Line 1\nLine 2\nLine 3", ...);

// Less efficient: Multiple calls
let run1 = text_system.layout("Line 1", ...);
let run2 = text_system.layout("Line 2", ...);
```

### Pre-warm Cache

For UI with known text, layout once during init:

```rust
// Pre-warm common characters
text_system.layout("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789", ...);
```

### Avoid Per-Frame Measurement

Cache text measurements when content doesn't change:

```rust
struct CachedText {
    content: String,
    measured_size: Size,
}
```

## Implementation Notes

### Borrow Checker Workaround

The layout method collects glyph data in two phases to work around borrow checker constraints:

```rust
// Phase 1: Collect glyph metadata while buffer is borrowed
let mut glyph_data: Vec<(GlyphCacheKey, f32, f32, u16)> = Vec::new();
for run in buffer.layout_runs() {
    for glyph in run.glyphs.iter() {
        // Collect cache key, position, glyph_id
    }
}

// Phase 2: Rasterize glyphs (can now mutably borrow swash_cache)
for (cache_key, glyph_x, line_y, glyph_id) in glyph_data {
    // Check cache or rasterize
}
```

### Coordinate Conversion

All coordinates are converted between physical and logical pixels:

- **Rasterization**: Physical pixels (for sharp rendering)
- **Layout output**: Logical pixels (for consistent positioning)

## Limitations

### Current

- No subpixel anti-aliasing (grayscale only)
- No color emoji support (SwashContent::Color is skipped)
- No RTL layout (cosmic-text supports it, not wired up)
- Single atlas page (no overflow handling beyond warning)
- No text attributes (bold, italic) - uses default attrs

### Planned

- Multi-page atlas
- Color emoji via separate RGBA atlas
- RTL and bidirectional text
- Text selection rendering
- Rich text attributes

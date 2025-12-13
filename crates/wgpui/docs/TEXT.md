# Text System

## Overview

wgpui uses cosmic-text for text shaping and rasterization, with a custom glyph atlas for efficient GPU rendering.

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

- Format: `R8Unorm` (single channel, 8-bit)
- Size: 1024x1024 (configurable)
- Filtering: Linear (for smooth scaling)

### Packing Algorithm

Row-based packing with automatic row advancement:

```rust
fn pack_glyph(&mut self, width: u32, height: u32) -> (u32, u32) {
    // Check if glyph fits in current row
    if self.cursor_x + width > self.atlas_size {
        // Move to next row
        self.cursor_x = 0;
        self.cursor_y += self.row_height + 1;  // 1px padding
        self.row_height = 0;
    }

    let x = self.cursor_x;
    let y = self.cursor_y;

    self.cursor_x += width + 1;  // 1px padding
    self.row_height = self.row_height.max(height);

    (x, y)
}
```

### Cache Key

Glyphs are cached by their cosmic-text `CacheKey`:

```rust
struct CacheKey {
    font_id: fontdb::ID,
    glyph_id: u16,
    font_size_bits: u32,
    x_bin: SubpixelBin,
    y_bin: SubpixelBin,
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
struct TextRun {
    glyphs: Vec<GlyphInstance>,
    origin: Point,
    color: Hsla,
    font_size: f32,
}

struct GlyphInstance {
    glyph_id: u16,
    offset: Point,      // Relative to origin
    size: Size,         // Glyph dimensions
    uv: [f32; 4],       // Atlas UV (min_u, min_v, max_u, max_v)
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

To embed custom fonts:

```rust
font_system.db_mut().load_font_data(
    include_bytes!("../fonts/BerkeleyMono-Regular.ttf").to_vec()
);
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

## Limitations

### Current

- No subpixel anti-aliasing (grayscale only)
- No color emoji support
- No RTL layout (cosmic-text supports it, not wired up)
- Single atlas page (no overflow handling)

### Planned

- Multi-page atlas
- Color emoji via separate RGBA atlas
- RTL and bidirectional text
- Text selection rendering

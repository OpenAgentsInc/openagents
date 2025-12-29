# Text System Upgrade Plan

Port Zed's text system concepts to WGPUI for proper line wrapping, font fallbacks, and advanced text layout.

## Current State (WGPUI)

**Location:** `crates/wgpui/src/text.rs` (331 lines)

**Limitations:**
- Fixed-width measurement: `char_count * font_size * 0.6` (monospace assumption)
- No line wrapping
- Single font family (Vera Mono)
- No font fallbacks
- No text decorations (underline, strikethrough)
- Simple row-based glyph atlas packing
- No layout caching across frames

## Target State (Zed's Architecture)

**Key concepts to port:**
1. `LineWrapper` - Smart text wrapping with word boundary detection
2. `LineLayout` - Shaped line with font runs and positioned glyphs
3. `WrappedLineLayout` - Layout with wrap boundaries
4. `LineLayoutCache` - Per-frame caching with frame swap
5. `DecorationRun` - Text decorations (color, underline, strikethrough, background)
6. `ShapedLine` / `WrappedLine` - Paintable line types
7. `FontRun` - Per-run font specification
8. Font fallback chain

## Implementation Plan

### Phase 1: Core Types (New Module Structure)

Create `crates/wgpui/src/text_system/` directory:

```
text_system/
  mod.rs          - Module exports
  line_layout.rs  - LineLayout, ShapedRun, ShapedGlyph, FontRun
  line_wrapper.rs - LineWrapper, LineFragment, Boundary
  line.rs         - ShapedLine, WrappedLine, DecorationRun
  cache.rs        - LineLayoutCache with frame management
```

#### 1.1 `line_layout.rs` - Core Data Structures

Port from Zed: `/Users/christopherdavid/code/zed/crates/gpui/src/text_system/line_layout.rs`

```rust
// Key types to implement:
pub struct LineLayout {
    pub font_size: f32,
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub runs: Vec<ShapedRun>,
    pub len: usize,
}

pub struct ShapedRun {
    pub font_id: FontId,
    pub glyphs: Vec<ShapedGlyph>,
}

pub struct ShapedGlyph {
    pub id: u16,            // glyph_id
    pub position: Point,    // x, y position
    pub index: usize,       // text byte index
    pub is_emoji: bool,
}

pub struct WrappedLineLayout {
    pub unwrapped_layout: Arc<LineLayout>,
    pub wrap_boundaries: SmallVec<[WrapBoundary; 1]>,
    pub wrap_width: Option<f32>,
}

pub struct WrapBoundary {
    pub run_ix: usize,
    pub glyph_ix: usize,
}

pub struct FontRun {
    pub len: usize,
    pub font_id: FontId,
}
```

**Methods to implement:**
- `LineLayout::index_for_x()` - Get character index at x position
- `LineLayout::closest_index_for_x()` - Get nearest character boundary
- `LineLayout::x_for_index()` - Get x position for character index
- `LineLayout::compute_wrap_boundaries()` - Calculate wrap points
- `WrappedLineLayout::index_for_position()` - Position to index with wrapping
- `WrappedLineLayout::position_for_index()` - Index to position with wrapping

#### 1.2 `line_wrapper.rs` - Text Wrapping

Port from Zed: `/Users/christopherdavid/code/zed/crates/gpui/src/text_system/line_wrapper.rs`

```rust
pub struct LineWrapper {
    font_id: FontId,
    font_size: f32,
    text_system: Arc<TextSystem>,
    cached_ascii_char_widths: [Option<f32>; 128],
    cached_other_char_widths: HashMap<char, f32>,
}

pub enum LineFragment<'a> {
    Text { text: &'a str },
    Element { width: f32, len_utf8: usize },
}

pub struct Boundary {
    pub ix: usize,
    pub next_indent: u32,
}
```

**Key methods:**
- `wrap_line()` - Iterator yielding wrap boundaries
- `is_word_char()` - Word boundary detection (ASCII, Latin, Cyrillic, CJK)
- `width_for_char()` - Cached character width lookup
- `truncate_line()` - Truncation with ellipsis

#### 1.3 `line.rs` - Paintable Lines

Port from Zed: `/Users/christopherdavid/code/zed/crates/gpui/src/text_system/line.rs`

```rust
pub struct DecorationRun {
    pub len: u32,
    pub color: Hsla,
    pub background_color: Option<Hsla>,
    pub underline: Option<UnderlineStyle>,
    pub strikethrough: Option<StrikethroughStyle>,
}

pub struct ShapedLine {
    pub layout: Arc<LineLayout>,
    pub text: String,
    pub decoration_runs: SmallVec<[DecorationRun; 32]>,
}

pub struct WrappedLine {
    pub layout: Arc<WrappedLineLayout>,
    pub text: String,
    pub decoration_runs: SmallVec<[DecorationRun; 32]>,
}

pub struct UnderlineStyle {
    pub color: Option<Hsla>,
    pub thickness: f32,
    pub wavy: bool,
}

pub struct StrikethroughStyle {
    pub color: Option<Hsla>,
    pub thickness: f32,
}

pub enum TextAlign {
    Left,
    Center,
    Right,
}
```

**Key methods:**
- `ShapedLine::paint()` - Render to PaintContext
- `WrappedLine::paint()` - Render wrapped text
- `paint_line()` - Internal painting with decorations
- `aligned_origin_x()` - Text alignment calculation

#### 1.4 `cache.rs` - Layout Caching

Port from Zed: `/Users/christopherdavid/code/zed/crates/gpui/src/text_system/line_layout.rs` (cache section)

```rust
pub struct LineLayoutCache {
    previous_frame: Mutex<FrameCache>,
    current_frame: RwLock<FrameCache>,
}

struct FrameCache {
    lines: FxHashMap<Arc<CacheKey>, Arc<LineLayout>>,
    wrapped_lines: FxHashMap<Arc<CacheKey>, Arc<WrappedLineLayout>>,
    used_lines: Vec<Arc<CacheKey>>,
    used_wrapped_lines: Vec<Arc<CacheKey>>,
}

struct CacheKey {
    text: String,
    font_size: f32,
    runs: SmallVec<[FontRun; 1]>,
    wrap_width: Option<f32>,
}
```

**Key methods:**
- `layout_line()` - Get or compute line layout
- `layout_wrapped_line()` - Get or compute wrapped layout
- `finish_frame()` - Swap previous/current frame caches
- `reuse_layouts()` - Reuse layouts from previous frame

### Phase 2: Font System Enhancement

#### 2.1 Multi-Font Support

Modify `TextSystem` in `text.rs`:

```rust
pub struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
    // NEW: Font ID management
    fonts: Vec<LoadedFont>,
    font_id_counter: FontId,
    fallback_chain: Vec<FontId>,
    // Existing fields...
}

pub struct LoadedFont {
    pub id: FontId,
    pub family: String,
    pub weight: Weight,
    pub style: Style,
}

pub type FontId = u32;
```

**New methods:**
- `add_font(data: &[u8]) -> FontId` - Load font and return ID
- `resolve_font(family, weight, style) -> FontId` - Find best matching font
- `font_metrics(font_id) -> FontMetrics` - Get ascent/descent
- `layout_line(text, font_size, runs) -> LineLayout` - Shape with runs

#### 2.2 Font Fallback Chain

```rust
impl TextSystem {
    pub fn set_fallback_chain(&mut self, fonts: Vec<FontId>) {
        self.fallback_chain = fonts;
    }

    fn resolve_glyph(&self, ch: char, preferred_font: FontId) -> (FontId, GlyphId) {
        // Try preferred font first
        // Walk fallback chain if glyph missing
    }
}
```

### Phase 3: Integration

#### 3.1 Update Text Component

Modify `crates/wgpui/src/components/text.rs`:

```rust
pub struct Text {
    content: String,
    font_size: f32,
    color: Hsla,
    font_style: FontStyle,
    style: StyleRefinement,
    // NEW:
    wrap_width: Option<f32>,
    max_lines: Option<usize>,
    align: TextAlign,
    decoration_runs: Option<Vec<DecorationRun>>,
}
```

**New builder methods:**
- `.wrap(width)` - Enable wrapping at width
- `.max_lines(n)` - Limit to n lines
- `.align(TextAlign)` - Text alignment
- `.underline()`, `.strikethrough()` - Decorations

#### 3.2 Update TextInput Component

Modify `crates/wgpui/src/components/text_input.rs`:

- Replace fixed-width cursor positioning with `LineLayout::x_for_index()`
- Replace fixed-width click detection with `LineLayout::index_for_x()`
- Support variable-width fonts

#### 3.3 Update Scene Integration

Modify `crates/wgpui/src/scene.rs`:

Add support for rendering `ShapedLine` and `WrappedLine` with decorations.

### Phase 4: Glyph Atlas Enhancement

#### 4.1 Better Atlas Packing

Replace simple row packing with shelf allocator or similar:

```rust
// Consider using etagere crate for texture atlas allocation
// Or implement simple shelf allocator

struct GlyphAtlas {
    texture_data: Vec<u8>,
    size: u32,
    shelves: Vec<AtlasShelf>,
    glyph_locations: HashMap<GlyphCacheKey, AtlasTile>,
}

struct AtlasShelf {
    y: u32,
    height: u32,
    cursor_x: u32,
}

struct AtlasTile {
    bounds: Bounds,
    uv: [f32; 4],
}
```

### Phase 5: Testing

Add tests in `crates/wgpui/src/text_system/tests.rs`:

- `test_line_wrapper_basic()` - Simple wrapping
- `test_line_wrapper_cjk()` - CJK word boundaries
- `test_line_layout_index_for_x()` - Position queries
- `test_wrapped_line_multiline()` - Multi-line layout
- `test_cache_frame_reuse()` - Cache efficiency
- `test_font_fallback()` - Fallback resolution

## Files to Create

| File | Description |
|------|-------------|
| `crates/wgpui/src/text_system/mod.rs` | Module exports |
| `crates/wgpui/src/text_system/line_layout.rs` | LineLayout, ShapedRun, etc. |
| `crates/wgpui/src/text_system/line_wrapper.rs` | LineWrapper, wrapping logic |
| `crates/wgpui/src/text_system/line.rs` | ShapedLine, DecorationRun |
| `crates/wgpui/src/text_system/cache.rs` | LineLayoutCache |

## Files to Modify

| File | Changes |
|------|---------|
| `crates/wgpui/src/lib.rs` | Export text_system module |
| `crates/wgpui/src/text.rs` | Add multi-font support, FontId |
| `crates/wgpui/src/components/text.rs` | Use new LineLayout |
| `crates/wgpui/src/components/text_input.rs` | Variable-width support |
| `crates/wgpui/src/scene.rs` | ShapedLine rendering |

## Dependencies

Current cosmic-text usage is sufficient. May add:
- `smallvec` - Already in use
- `parking_lot` - For RwLock (or use std)
- `fxhash` (or `rustc-hash`) - Fast hashing for cache

## Migration Strategy

1. Create new `text_system/` module alongside existing `text.rs`
2. Build and test new types independently
3. Gradually integrate into components
4. Deprecate old fixed-width measurement
5. Update all text consumers

## Success Criteria

- [ ] Line wrapping works with variable-width text
- [ ] Word boundaries respected (Latin, CJK)
- [ ] Multiple fonts can be used in same line
- [ ] Font fallback for missing glyphs
- [ ] Layout cache reduces re-shaping
- [ ] TextInput works with variable-width fonts
- [ ] Text decorations render correctly
- [ ] Tests pass

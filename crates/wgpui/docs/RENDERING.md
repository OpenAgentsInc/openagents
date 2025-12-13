# Rendering Pipeline

## Overview

wgpui uses a two-pipeline rendering approach:
1. **Quad Pipeline**: Renders rectangles with SDF-based rounded corners and borders
2. **Text Pipeline**: Renders text glyphs from a cached atlas texture

Both pipelines use instanced rendering for efficiency - each primitive is a single GPU instance drawn with a 4-vertex triangle strip.

## Quad Pipeline

### Shader Architecture

The quad shader uses Signed Distance Fields (SDF) for smooth, resolution-independent rendering of rounded rectangles.

```wgsl
// Fragment shader core
fn sdf_rounded_rect(p: vec2<f32>, size: vec2<f32>, radii: vec4<f32>) -> f32 {
    // Select corner radius based on quadrant
    var r: f32 = select_corner_radius(p, size, radii);

    // Compute SDF
    let half_size = size * 0.5;
    let center = half_size;
    let q = abs(p - center) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}
```

### Instance Data

Each quad instance contains:

| Field | Type | Size | Description |
|-------|------|------|-------------|
| origin | vec2 | 8B | Top-left position |
| size | vec2 | 8B | Width and height |
| background | vec4 | 16B | RGBA background color |
| border_color | vec4 | 16B | RGBA border color |
| border_width | f32 | 4B | Border thickness |
| corner_radii | vec4 | 16B | TL, TR, BR, BL radii |

### Anti-Aliasing

SDF-based anti-aliasing uses `smoothstep` for crisp edges:

```wgsl
let aa_width = 1.0;  // 1 pixel AA band
let outer_alpha = 1.0 - smoothstep(-aa_width, 0.0, d);
```

### Border Rendering

Borders are rendered by computing two SDFs:
1. Outer SDF for the full shape
2. Inner SDF (inset by border width) for the fill area

```wgsl
let inner_d = d + border_width;
let border_alpha = smoothstep(-aa_width, 0.0, inner_d) * outer_alpha;
let fill_alpha = (1.0 - smoothstep(-aa_width, 0.0, inner_d)) * outer_alpha;
```

## Text Pipeline

### Glyph Atlas

Text rendering uses a glyph atlas texture:
- Format: R8Unorm (single channel, 8-bit grayscale)
- Size: 1024x1024 (configurable)
- Packing: Row-based with 1px padding

### Atlas Management

```rust
struct TextSystem {
    glyph_cache: HashMap<CacheKey, GlyphEntry>,
    atlas_data: Vec<u8>,           // R8 texture data
    atlas_cursor_x: u32,           // Current packing X
    atlas_cursor_y: u32,           // Current packing Y
    atlas_row_height: u32,         // Current row height
}
```

### Text Shaping

cosmic-text handles complex text layout:
1. Unicode segmentation
2. Font selection and fallback
3. Glyph shaping (ligatures, kerning)
4. Layout runs (LTR/RTL support)

### Instance Data

Each text quad instance contains:

| Field | Type | Size | Description |
|-------|------|------|-------------|
| position | vec2 | 8B | Screen position |
| size | vec2 | 8B | Glyph size |
| uv | vec4 | 16B | Atlas UV coords |
| color | vec4 | 16B | RGBA text color |

### Shader

```wgsl
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = textureSample(glyph_atlas, atlas_sampler, in.uv).r;
    return vec4(in.color.rgb * alpha, alpha * in.color.a);
}
```

## Render Pass

### Execution Order

```rust
fn render(&self, encoder: &mut CommandEncoder, view: &TextureView) {
    let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
        color_attachments: &[Some(RenderPassColorAttachment {
            view,
            load: LoadOp::Clear(Color::BLACK),
            store: StoreOp::Store,
        })],
        ..
    });

    // 1. Draw quads (back to front)
    pass.set_pipeline(&self.quad_pipeline);
    pass.draw(0..4, 0..self.quad_count);

    // 2. Draw text (on top)
    pass.set_pipeline(&self.text_pipeline);
    pass.draw(0..4, 0..self.text_count);
}
```

### Blending

Both pipelines use premultiplied alpha blending:

```rust
blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING)
```

## Coordinate Systems

### Logical vs Physical Pixels

- **Logical**: CSS pixels, used for layout and hit testing
- **Physical**: Actual GPU pixels, scaled by device pixel ratio

```rust
let physical_width = logical_width * scale_factor;
```

### NDC Transformation

Vertex shader converts to normalized device coordinates:

```wgsl
let ndc = vec2<f32>(
    (world_pos.x / viewport.x) * 2.0 - 1.0,
    1.0 - (world_pos.y / viewport.y) * 2.0  // Y-flip for WebGPU
);
```

## Performance Optimizations

### Instanced Rendering

All primitives use GPU instancing:
- Quads: 1 draw call for all rectangles
- Text: 1 draw call for all glyphs

### Buffer Management

- Instance buffers created per frame
- Future: Ring buffer for reuse
- Uniform buffer updated on resize only

### Glyph Caching

- Glyphs cached by (glyph_id, font_id, size, subpixel_offset)
- Atlas persists across frames
- Cache cleared on scale factor change

## API Notes

### wgpu 24.0 Deprecations

Some type aliases are deprecated and will be renamed in wgpu 25.0:

```rust
// Current (deprecated)
wgpu::ImageCopyTexture { ... }
wgpu::ImageDataLayout { ... }

// Future (wgpu 25.0)
wgpu::TexelCopyTextureInfo { ... }
wgpu::TexelCopyBufferLayout { ... }
```

### Atlas Upload

The atlas texture is uploaded using `queue.write_texture()`:

```rust
pub fn update_atlas(&self, queue: &wgpu::Queue, data: &[u8], size: u32) {
    queue.write_texture(
        wgpu::ImageCopyTexture {  // Will become TexelCopyTextureInfo
            texture: &self.atlas_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::ImageDataLayout {  // Will become TexelCopyBufferLayout
            offset: 0,
            bytes_per_row: Some(size),
            rows_per_image: Some(size),
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );
}
```

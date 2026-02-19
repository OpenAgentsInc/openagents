# Rendering Pipelines

WGPUI uses a multi-pipeline GPU rendering architecture built on wgpu. This document covers the four rendering pipelines: quads, lines, text, and images.

## Overview

The renderer processes scene primitives through specialized GPU pipelines:

| Pipeline | Purpose | Shader | Primitive |
|----------|---------|--------|-----------|
| Quad | Rectangles, rounded corners, borders | `quad.wgsl` | `GpuQuad` |
| Line | Anti-aliased lines, curve segments | `line.wgsl` | `GpuLine` |
| Text | Glyph rendering from atlas | `text.wgsl` | `GpuTextQuad` |
| Image | Textured quads (SVGs, images) | `image.wgsl` | `GpuImageQuad` |

## Render Order

Within each layer, primitives render in this order:

1. **Quads** - Background rectangles, nodes, panels
2. **Lines** - Connections, curves, edges (render ON TOP of quads)
3. **Text** - Labels, content (render ON TOP of lines)
4. **Images/SVGs** - After all layers complete

This order ensures lines connecting nodes are visible over the node backgrounds, and text is always readable.

## Quad Pipeline

### Purpose

Renders rectangles with optional:
- Background fill color
- Border with configurable width and color
- Rounded corners (via SDF)

### GPU Structure

```rust
#[repr(C)]
pub struct GpuQuad {
    pub origin: [f32; 2],      // Top-left position (physical pixels)
    pub size: [f32; 2],        // Width, height (physical pixels)
    pub background: [f32; 4],  // RGBA color (linear space)
    pub border_color: [f32; 4],// RGBA border color
    pub border_width: f32,     // Border thickness in pixels
    pub corner_radius: f32,    // Radius for rounded corners
    pub _padding: [f32; 2],    // Alignment padding
}
```

### Vertex Attributes

| Location | Format | Offset | Field |
|----------|--------|--------|-------|
| 0 | Float32x2 | 0 | origin |
| 1 | Float32x2 | 8 | size |
| 2 | Float32x4 | 16 | background |
| 3 | Float32x4 | 32 | border_color |
| 4 | Float32 | 48 | border_width |
| 5 | Float32 | 52 | corner_radius |

### Shader Details (`quad.wgsl`)

The vertex shader generates a quad from 4 vertices using `vertex_index`:

```wgsl
let vertex_positions = array<vec2<f32>, 4>(
    vec2<f32>(0.0, 0.0),  // 0: top-left
    vec2<f32>(1.0, 0.0),  // 1: top-right
    vec2<f32>(0.0, 1.0),  // 2: bottom-left
    vec2<f32>(1.0, 1.0),  // 3: bottom-right
);
```

The fragment shader uses a signed distance function (SDF) for rounded corners:

```wgsl
fn rounded_box_sdf(p: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - size + radius;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}
```

Key features:
- Anti-aliased edges via `smoothstep`
- Border rendering by computing inner SDF
- Premultiplied alpha output

### Usage

```rust
let quad = Quad::new(bounds)
    .with_background(Hsla::new(0.6, 0.5, 0.5, 1.0))
    .with_border(Hsla::white(), 2.0)
    .with_corner_radius(8.0);  // For rounded corners

scene.draw_quad(quad);
```

For circles, set `corner_radius` equal to half the smaller dimension:

```rust
let radius = 10.0;
let bounds = Bounds::new(x - radius, y - radius, radius * 2.0, radius * 2.0);
let circle = Quad::new(bounds)
    .with_background(fill_color)
    .with_corner_radius(radius);  // Makes it circular
```

## Line Pipeline

### Purpose

Renders anti-aliased line segments with:
- Configurable width
- Rounded end caps (capsule shape)
- Smooth anti-aliasing via SDF

### GPU Structure

```rust
#[repr(C)]
pub struct GpuLine {
    pub start: [f32; 2],  // Start point (physical pixels)
    pub end: [f32; 2],    // End point (physical pixels)
    pub width: f32,       // Line thickness
    pub _pad: f32,        // Alignment padding
    pub color: [f32; 4],  // RGBA color (linear space)
}
```

### Vertex Attributes

| Location | Format | Offset | Field |
|----------|--------|--------|-------|
| 0 | Float32x2 | 0 | start |
| 1 | Float32x2 | 8 | end |
| 2 | Float32 | 16 | width |
| 3 | Float32x4 | 24 | color |

### Shader Details (`line.wgsl`)

The vertex shader constructs a quad around the line segment:

1. Calculate line direction and perpendicular
2. Expand by half-width plus anti-aliasing margin
3. Generate 4 vertices forming a rotated rectangle

```wgsl
let dir = instance.end - instance.start;
let line_len = length(dir);
let unit_dir = dir / max(line_len, 0.001);
let perp = vec2<f32>(-unit_dir.y, unit_dir.x);

let half_width = instance.width * 0.5 + 1.0;  // +1 for AA

let vertex_positions = array<vec2<f32>, 4>(
    instance.start - perp * half_width,
    instance.start + perp * half_width,
    instance.end - perp * half_width,
    instance.end + perp * half_width,
);
```

The fragment shader computes a capsule SDF:

```wgsl
// Distance to line center
let dist_from_center = abs(in.local_pos.y);

// Distance to endpoints (for rounded caps)
let dist_to_start = length(in.local_pos);
let dist_to_end = length(vec2<f32>(in.local_pos.x - in.line_length, in.local_pos.y));

// SDF based on position along line
var sdf: f32;
if in.local_pos.x < 0.0 {
    sdf = dist_to_start - half_width;  // Start cap
} else if in.local_pos.x > in.line_length {
    sdf = dist_to_end - half_width;    // End cap
} else {
    sdf = dist_from_center - half_width;  // Line body
}
```

**Important**: The variable `line_len` must not shadow the built-in `length()` function.

### Usage

Lines are typically created from curve tessellation:

```rust
let lines = scene.curve_lines_for_layer(layer, scale_factor);
```

## Curve System

### Overview

Bezier curves are rendered by tessellating into line segments, which are then rendered via the line pipeline.

### CurvePrimitive

```rust
pub struct CurvePrimitive {
    pub points: [Point; 4],    // Cubic bezier: start, ctrl1, ctrl2, end
    pub stroke_width: f32,
    pub color: Hsla,
}
```

### Tessellation

Two tessellation methods are available:

1. **Fixed segments**: `tessellate(segments: usize)` - Divides curve into N equal segments
2. **Adaptive**: `tessellate_adaptive(tolerance: f32)` - Subdivides based on flatness

Adaptive tessellation recursively subdivides until the curve midpoint is within `tolerance` of the line midpoint:

```rust
let mid_curve = self.evaluate(mid_t);
let mid_line = Point::new((p0.x + p1.x) / 2.0, (p0.y + p1.y) / 2.0);
let dist_sq = (mid_curve.x - mid_line.x).powi(2) + (mid_curve.y - mid_line.y).powi(2);

if dist_sq <= tolerance * tolerance {
    // Flat enough, emit single segment
} else {
    // Subdivide further
}
```

### Usage

```rust
// Create a cubic bezier curve
let curve = CurvePrimitive::new(start, control1, control2, end)
    .with_stroke_width(2.0)
    .with_color(Hsla::new(0.0, 0.0, 0.5, 1.0));

scene.draw_curve(curve);
```

For graph edges with a slight arc:

```rust
impl BootEdge {
    pub fn to_curve(&self, stroke_width: f32) -> CurvePrimitive {
        let dx = self.to.x - self.from.x;
        let dy = self.to.y - self.from.y;
        let len = (dx * dx + dy * dy).sqrt();

        // Perpendicular offset for curve bow
        let bow_factor = len * 0.15;
        let perp_x = -dy / len * bow_factor;
        let perp_y = dx / len * bow_factor;

        let control1 = Point::new(
            self.from.x + dx * 0.25 + perp_x,
            self.from.y + dy * 0.25 + perp_y,
        );
        let control2 = Point::new(
            self.from.x + dx * 0.75 + perp_x,
            self.from.y + dy * 0.75 + perp_y,
        );

        CurvePrimitive::new(self.from, control1, control2, self.to)
            .with_stroke_width(stroke_width)
            .with_color(self.edge_color())
    }
}
```

## Text Pipeline

### Purpose

Renders text glyphs from a texture atlas with:
- Subpixel positioning
- Color tinting
- Alpha blending

### GPU Structure

```rust
#[repr(C)]
pub struct GpuTextQuad {
    pub position: [f32; 2],  // Top-left (physical pixels)
    pub size: [f32; 2],      // Glyph dimensions
    pub uv: [f32; 4],        // Atlas coordinates [u0, v0, u1, v1]
    pub color: [f32; 4],     // Text color
}
```

## Image Pipeline

### Purpose

Renders textured quads for:
- SVG images (rasterized to texture)
- Bitmap images
- Icons

### GPU Structure

```rust
#[repr(C)]
pub struct GpuImageQuad {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub uv: [f32; 4],
    pub tint: [f32; 4],
}
```

## Uniforms

All pipelines share a common uniform buffer:

```wgsl
struct Uniforms {
    viewport: vec2<f32>,  // Physical viewport size
    scale: f32,           // Display scale factor
    _padding: f32,
}
```

The viewport is in **physical pixels** (logical size * scale factor).

## Coordinate Systems

### Logical vs Physical Pixels

- **Logical pixels**: Used by application code, independent of display scale
- **Physical pixels**: Used by GPU, accounts for retina/HiDPI displays

Conversion happens in `Scene::gpu_quads_for_layer()` etc., which multiply positions and sizes by `scale_factor`.

### NDC Transformation

Shaders convert physical pixel coordinates to Normalized Device Coordinates:

```wgsl
let ndc = vec2<f32>(
    (world_pos.x / uniforms.viewport.x) * 2.0 - 1.0,
    1.0 - (world_pos.y / uniforms.viewport.y) * 2.0
);
```

Note: Y is flipped because screen coordinates have Y increasing downward, while NDC has Y increasing upward.

## Blending

All pipelines use premultiplied alpha blending:

```rust
blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING)
```

Colors must be premultiplied before output:

```wgsl
let final_color = vec4<f32>(color.rgb * color.a, color.a);
```

## Performance Considerations

1. **Instancing**: All pipelines use instanced rendering - one draw call per primitive type per layer
2. **Buffer uploads**: Each layer creates separate GPU buffers during `prepare()`
3. **Texture atlas**: Text glyphs are cached in a shared atlas texture
4. **SVG caching**: Rasterized SVGs are cached by content hash and size

## Debugging Tips

1. **No primitives visible**: Check if `prepare()` is called before `render()`
2. **Primitives behind others**: Check render order (quads -> lines -> text)
3. **Wrong positions**: Verify scale factor and viewport size match
4. **Shader errors**: Check wgpu validation messages for WGSL issues
5. **Color issues**: Ensure colors are in linear space (use `to_linear_rgba()`)

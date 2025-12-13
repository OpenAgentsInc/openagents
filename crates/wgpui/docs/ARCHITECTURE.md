# wgpui Architecture

## Design Philosophy

wgpui follows the principle of **"own the pixels for core surfaces"** - providing low-level GPU rendering primitives that give you complete control over every pixel rendered.

### What wgpui IS

- A rendering engine for GPU-accelerated UI primitives
- A text system with high-quality shaping and efficient atlas management
- A layout engine wrapper for CSS Flexbox
- Platform abstraction for web and native targets

### What wgpui IS NOT

- A full UI framework (no reactivity, state management, or component model)
- A replacement for HTML/DOM (use for performance-critical surfaces only)
- A general-purpose graphics library (focused on UI primitives)

## Module Structure

```
crates/wgpui/src/
├── lib.rs              # Public API and WASM entry point
├── color.rs            # HSLA color type
├── geometry.rs         # Point, Size, Bounds, CornerRadii
├── theme.rs            # Design tokens (colors, spacing, typography)
├── scene.rs            # Quad, TextRun, Scene accumulator
├── layout.rs           # Taffy integration
├── text.rs             # cosmic-text + glyph atlas
├── renderer.rs         # wgpu render pipelines
├── shaders/
│   ├── quad.wgsl       # SDF quad shader
│   └── text.wgsl       # Glyph atlas sampling shader
└── platform/
    ├── mod.rs          # Platform trait and events
    └── web.rs          # Web platform (WASM)
```

## Rendering Pipeline

```
┌─────────────────────────────────────────────────────────────┐
│  Application                                                 │
│  └── Build UI → Scene                                       │
├─────────────────────────────────────────────────────────────┤
│  Scene Accumulation                                          │
│  ├── Quads: [Quad { bounds, bg, border, radii }, ...]      │
│  └── TextRuns: [TextRun { glyphs, origin, color }, ...]    │
├─────────────────────────────────────────────────────────────┤
│  Renderer Preparation                                        │
│  ├── Convert quads → GpuQuad instances                      │
│  ├── Convert text → GpuTextQuad instances                   │
│  └── Upload to GPU buffers                                  │
├─────────────────────────────────────────────────────────────┤
│  GPU Render Pass                                             │
│  ├── 1. Draw quads (instanced, SDF fragment shader)         │
│  └── 2. Draw text (instanced, atlas sampling)               │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### Frame Lifecycle

1. **Build Scene**: Application constructs `Scene` with `Quad` and `TextRun` primitives
2. **Layout**: Optional - use `LayoutEngine` to compute positions via Taffy
3. **Text Shaping**: `TextSystem` shapes text and updates glyph atlas
4. **Prepare**: `Renderer::prepare()` converts scene to GPU buffers
5. **Render**: `Renderer::render()` executes GPU commands
6. **Present**: Swap chain presents frame to display

### Type Hierarchy

```
Scene
├── Quad
│   ├── bounds: Bounds
│   ├── background: Option<Hsla>
│   ├── border_color: Hsla
│   ├── border_width: f32
│   └── corner_radii: CornerRadii
│
└── TextRun
    ├── glyphs: Vec<GlyphInstance>
    ├── origin: Point
    ├── color: Hsla
    └── font_size: f32

GlyphInstance
├── glyph_id: u16
├── offset: Point
├── size: Size
└── uv: [f32; 4]  // Atlas coordinates
```

## Platform Abstraction

### Web Platform

```rust
// Initialize
let platform = WebPlatform::init("canvas-id").await?;

// Render loop
run_animation_loop(|| {
    let mut scene = Scene::new();
    // ... build scene
    platform.render(&scene)?;
});
```

### Event Types

```rust
enum Event {
    Resize { size: Size, scale_factor: f32 },
    MouseMove { position: Point },
    MouseDown { position: Point, button: MouseButton },
    MouseUp { position: Point, button: MouseButton },
    Wheel { delta: Point },
    KeyDown { key: Key, modifiers: Modifiers },
    KeyUp { key: Key, modifiers: Modifiers },
}
```

## Memory Layout

### GPU Structs (Pod/Zeroable)

All GPU-facing structs are `#[repr(C)]` with `bytemuck::Pod` for safe casting:

```rust
#[repr(C)]
#[derive(Pod, Zeroable)]
struct GpuQuad {
    origin: [f32; 2],       // 8 bytes
    size: [f32; 2],         // 8 bytes
    background: [f32; 4],   // 16 bytes (RGBA)
    border_color: [f32; 4], // 16 bytes (RGBA)
    border_width: f32,      // 4 bytes
    corner_radii: [f32; 4], // 16 bytes
    _padding: [f32; 2],     // 8 bytes (alignment)
}   // Total: 76 bytes
```

## Extensibility

### Adding New Primitives

1. Define struct in `scene.rs`
2. Add GPU representation
3. Create shader in `shaders/`
4. Add pipeline to `renderer.rs`
5. Add draw method to `Scene`

### Custom Shaders

Shaders are embedded via `include_str!()` and compiled at runtime:

```rust
let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    source: wgpu::ShaderSource::Wgsl(include_str!("shaders/custom.wgsl").into()),
    ..
});
```

## Performance Considerations

1. **Instanced Rendering**: All quads/text use GPU instancing (one draw call per primitive type)
2. **Glyph Atlas**: Text glyphs are cached and rendered from a single texture
3. **Scene Clearing**: `Scene::clear()` reuses allocations
4. **Buffer Reuse**: Future optimization - reuse GPU buffers between frames
5. **Virtual Scrolling**: Render only visible items for large lists

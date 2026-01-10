# Layer System for Z-Ordering

The wgpui rendering system uses a layer-based approach to control z-ordering of elements. This ensures overlays like command palettes, modals, and dialogs render on top of underlying content.

## Overview

By default, all draw calls go to layer 0. Higher numbered layers render on top of lower numbered layers. Within a layer, quads are rendered first, then text.

## Usage

### Setting the Layer

Use `Scene::set_layer()` to change the current layer for subsequent draw calls:

```rust
// Draw on default layer 0
scene.draw_quad(background_quad);
scene.draw_text(content_text);

// Switch to layer 1 for overlay
scene.set_layer(1);
scene.draw_quad(overlay_backdrop);
scene.draw_text(overlay_text);
```

### In Components

Components using `PaintContext` can access the scene to set layers:

```rust
impl Component for MyOverlay {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Render on layer 1 to be on top of layer 0 content
        cx.scene.set_layer(1);

        // All subsequent draws in this paint call use layer 1
        cx.scene.draw_quad(backdrop);
        // ...
    }
}
```

## Rendering Order

The renderer processes layers in ascending order:

1. Layer 0 quads
2. Layer 0 text
3. Layer 1 quads
4. Layer 1 text
5. ... and so on

This ensures that layer 1 content (both quads and text) completely covers layer 0 content.

## Common Layer Assignments

| Layer | Usage |
|-------|-------|
| 0 | Default - main content, chat, sidebars |
| 1 | Overlays - command palette, modals, dialogs |

## Implementation Details

### Scene

The `Scene` struct tracks:
- `current_layer: u32` - the layer for new draw calls
- `quads: Vec<(u32, Quad)>` - quads with their layer
- `text_runs: Vec<(u32, TextRun)>` - text runs with their layer

Key methods:
- `set_layer(layer: u32)` - set current layer
- `layer() -> u32` - get current layer
- `layers() -> Vec<u32>` - get sorted unique layers used
- `gpu_quads_for_layer(layer, scale)` - get GPU quads for a specific layer
- `gpu_text_quads_for_layer(layer, scale)` - get GPU text quads for a specific layer

### Renderer

The `Renderer` prepares separate GPU buffers for each layer during `prepare()`, then renders them in order during `render()`.

```rust
// In prepare()
for layer in scene.layers() {
    let quads = scene.gpu_quads_for_layer(layer, scale_factor);
    let text = scene.gpu_text_quads_for_layer(layer, scale_factor);
    // Create buffers...
}

// In render()
for layer in &self.prepared_layers {
    // Render layer's quads
    // Render layer's text
}
```

## Performance Considerations

- Each unique layer requires separate GPU buffer uploads
- Minimize the number of distinct layers used
- Most applications only need layers 0 and 1
- Layer switching within a single paint call is cheap (just changes an integer)

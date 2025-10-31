# mermaid_viewer

A small viewer for Mermaid diagrams with OpenAgents theming. It renders either:

- Mermaid source text (e.g., `sequenceDiagram`, `graph TD`) using a vendored `mermaid.min.js`, or
- Raw SVG (e.g., SVG produced by Mermaid elsewhere).

The window provides smooth pan/zoom, a Fit control, right-click drag panning, and fully disables text selection to avoid accidental highlights.

## Features
- Dark theme using app colors (`#08090a` background, light mono text, neutral greys for strokes).
- Embedded Berkeley Mono (Regular) to keep typography consistent and crisp.
- Smooth wheel zoom (exponential), left/right mouse drag pan, Fit button and `f` shortcut.
- No external network calls: Mermaid is vendor-bundled and injected as a `data:` URL.

## APIs

```rust
use mermaid_viewer::{render_mermaid_code, render_mermaid};

// Mermaid → viewer
let view = render_mermaid_code("sequenceDiagram\nA->>B: hi")?;
view.run()?;

// SVG → viewer
let svg = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 10'>...</svg>";
let view = render_mermaid(svg)?;
view.run()?;
```

- `render_mermaid_code(&str)` accepts Mermaid source text and returns a `MermaidView`.
- `render_mermaid(&str)` accepts SVG text and returns a `MermaidView`.
- `MermaidView::run()` opens the window and blocks until closed.

## Examples

- Demo (uses the Tinyvex threads+tails sequence):

```bash
cargo run -p mermaid_viewer --example demo
```

- Render a diagram from a file (Mermaid `.mmd`, Markdown `.md` with a ```mermaid block, or raw `.svg`):

```bash
# Mermaid inside Markdown
cargo run -p mermaid_viewer --example from_file -- docs/tinyvex/threads-and-tails-sequence.md

# Mermaid source file
cargo run -p mermaid_viewer --example from_file -- path/to/diagram.mmd

# Raw SVG file
cargo run -p mermaid_viewer --example from_file -- path/to/diagram.svg
```

The `from_file` example will:
- If given Markdown, extract the first ```mermaid fenced block.
- If the content starts with `<svg`, route to the SVG viewer; otherwise use the Mermaid renderer.

## Controls
- Zoom: mouse wheel (accelerated exponential) or toolbar `+`/`-`.
- Pan: left drag or right-click drag.
- Fit: toolbar “Fit” or press `f`.
- Selection: disabled globally (including buttons and diagram text) to prevent accidental highlights while navigating.

## Theming
- Colors are aligned with `expo/constants/theme.ts` (dark background, neutral greys for lines, light mono text).
- All strokes use `vector-effect: non-scaling-stroke` for crisp lines at any zoom.

## Platform Notes
- macOS/Windows: no extra setup.
- Linux: requires WebKitGTK 4.1 (`webkit2gtk-4.1`) headers at build/runtime (same as `wry`/`tao`).

## Caveats / Follow-ups
- Only the Regular Berkeley Mono face is embedded. If you need bold/italic in diagrams, we can embed additional weights and map `font-weight`/`font-style` accordingly.
- The viewer intentionally blocks text selection and the context menu to keep interactions focused on pan/zoom.

## License
Follows the repository’s license. Mermaid is included in `assets/mermaid.min.js` under its upstream license.


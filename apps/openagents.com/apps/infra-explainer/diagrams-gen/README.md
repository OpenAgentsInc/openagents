# infra diagrams-gen

Build-time renderer for the `/infra` explainer diagrams. Reads the
`../diagrams/*.mmd` mermaid sources, renders them with merman
(`zed-industries/merman` tag `v0.6.2-with-patches`, licensed
`MIT OR Apache-2.0` — verified at the pinned tag), themes them with the
Aiur palette, flattens text to paths with usvg 0.46 (matching the gpui
pin's resvg), and writes the committed SVGs into `../assets/diagrams/`.

The vendored `fonts/Lilex-Regular.ttf` and `fonts/Lilex-Bold.ttf` are
used only for deterministic build-time text shaping; Lilex is licensed
under the SIL Open Font License (`fonts/OFL.txt`).

```sh
cargo run --release
# optional: also write review PNGs of the exact resvg rasterization
DIAGRAMS_PNG_DIR=/tmp/diagram-pngs cargo run --release
```

This tool never ships to the browser; only its SVG output is embedded
by the wasm document.

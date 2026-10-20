# lander-gl

The painter behind `/lander4`: a raw WebGL2 wasm module, built with
`web-sys`, that draws the page's first frame in the same task as the page's
one script. It draws three things:

- The Grid's white floor: 121 lines each way on y = 0, seen from 1.6 units
  up and 7 degrees below the horizon, fading out with distance. Unless the
  visitor asks for reduced motion, the floor drifts slowly toward them.
- A GPUI-style button: a rounded quad with a 1 px border and a soft drop
  shadow, lighter under the pointer or keyboard focus. The fragment shader
  ports GPUI's rounded-rect signed distance and its closed-form Gaussian
  shadow (`gpui_wgpu/src/shaders.wgsl`, Zed Industries, Apache-2.0) to GLSL
  ES 3.00.
- "OpenAgents" and "Download" in Geist SemiBold, the brand face. Their
  outlines are shaped (with kerning) and baked at build time into
  `src/glyphs.rs`; the module fills them with `ab_glyph_rasterizer` at the
  exact device-pixel size it draws them and uploads each word as one R8
  texture, drawn 1:1. The page downloads no font, and every pixel is
  computed for the screen it lands on, at any size or pixel ratio.

The page lays out a real `<h1>` and a real `<a href="/download">`. The
module reads their boxes, paints over them, and adds the `gl` class to the
root element, which turns their text transparent. Links, focus, keyboard,
readers, search, and the no-WebGL2 fallback all stay plain HTML.

## Building

```sh
CARGO_TARGET_DIR=~/work/openagents-target-1 ./scripts/build-lander-gl.sh
```

The script builds with the workspace's `guest` profile (opt-level z, LTO,
panic=abort), runs `wasm-bindgen --target no-modules` and `wasm-opt -Oz`,
and writes `crates/openagents-web/static/lander4/`, which is checked in.
`openagents-web` compiles those files in and inlines them into the page, so
the website image builds nothing for it. Rebuild after any change here.

On wasm32 the crate is `no_std`, with a small power-of-two allocator and an
aborting panic handler (`src/heap.rs`), which keeps dlmalloc and most of
the formatting machinery out of the module.

`LANDER_GL_WRITE=1 cargo test -p lander-gl --test glyphs` regenerates
`src/glyphs.rs` from `crates/openagents-web/static/fonts/geist/`.

## Relation to the GPUI first-frame painter

A GPUI first-frame painter (a scene snapshot plus a small WebGL2 painter) is
being built for `/agent/foreman`. It had not landed on `main` when this
crate was written. The two should merge into one painter: this crate's
quad, shadow, and coverage-text paths are the same primitives a GPUI scene
snapshot needs.

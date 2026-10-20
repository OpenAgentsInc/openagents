# lander5-web

The procedural background of `/lander5`: the Grid at twilight, drawn by two
GLSL ES 3.00 passes through a small raw-WebGL2 runner (web-sys, no engine).
The site (`crates/openagents-web/src/pages/lander5.rs`) inlines the minified
shaders, the wasm-bindgen glue, and the module into one HTML response.

## Credits

The shaders are built with [Shader Skills](https://github.com/gokayfem/shader-skills)
by gokayfem (MIT, Copyright (c) 2026 gokayfem), following its
`shader-art-coding` workflow. `shaders/library.glsl` is that library's code,
unchanged, under its MIT notice: exactly the functions the passes call, bundled
by the skill's `scripts/glsl_tools.py`. The techniques' own sources (Hillaire's
atmosphere, Quilez's filtered patterns, the AgX fit, PCG hashes, and others)
are listed in that repository's `CREDITS.md`. The page carries the notice in
an HTML comment beside the shaders.

## Files

| File | What it is |
|---|---|
| `shaders/library.glsl` | The Shader Skills functions both passes call (generated, MIT). |
| `shaders/scene.glsl` | The sun, the sky table's size and mapping (Hillaire 2020, section 5.3). |
| `shaders/sky.glsl` | Pass 1: `atmosphereSky` into a 128 x 128 RGBA8 table, once. |
| `shaders/grid.glsl` | Pass 2: the floor, sky, haze, and post, every frame. Art controls and `DEBUG_MODE` views (0 to 6) are at the top. |
| `src/shaders.rs` | The sources as complete units, and the minifier. |
| `src/web.rs` | The runner: half-resolution first frame, dynamic resolution, a still frame under reduced motion. With the `worker` feature it can also run in a dedicated worker on an `OffscreenCanvas` (`start_worker`, `pause`, `scene_time`, `frame_times`), which `/lander6` hands the Grid to; the build script writes that build to `OUTDIR/worker/`. |
| `src/bump.rs` | A bump allocator in place of dlmalloc (the runner allocates only while it starts). |

## Build and check

```sh
scripts/build-lander5-web.sh OUTDIR          # the glue and module the site reads (--lander5 OUTDIR)
cargo test -p lander5-web                    # compiles both passes with glslangValidator when installed
```

To regenerate `shaders/library.glsl` after the passes call a new library
function, with a checkout of Shader Skills at `$SKILLS`: concatenate
`scene.glsl`, `sky.glsl`, and `grid.glsl` (renaming `mainImage` so the file
has one), run `python3 -I $SKILLS/skills/shader-art-coding/scripts/glsl_tools.py
bundle` on it, and keep the MIT header and the library section (from the first
`// ---- shader-skills library` line to `// ---- shader ----`).

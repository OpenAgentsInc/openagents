# Film: the take

The film engine makes 2D and 3D videos from Verse content, desktop
components, and app mockups. Its first contract is the **take**: a scene
recorded once as one stream of messages, which any renderer draws on demand.
The take is the seam between what a video shows and how it is drawn, so the
same take plays in a window, in a browser, or encodes to MP4.

The design is carried over from ManimGX (`rust/engine/src/take.rs`,
MIT, Academa Inc.), reimplemented in `crates/verse-film` with Verse's own
records. ManimGX itself is not a dependency: its engine is a private PyO3
`cdylib` on wgpu 30 that builds x264 and FFmpeg from source, while Verse
draws with wgpu 29 and captures through the ffmpeg already installed. See
issue #11424 for the decision and the slices.

## Wire format

Little-endian. A message is its length (`u32`), its op (`u8`), then its
fields. A byte string is its length (`u32`), then its bytes. Version 1.

| Op | Name | Fields |
| --- | --- | --- |
| 1 | `START` | version `u32`, width `u32`, height `u32`, fps `f64` |
| 2 | `PATH` | key `u64`, closed `u8`, count `u32`, count × (`f32`, `f32`): the control points of cubic Bézier curves, `3n + 1` of them |
| 3 | `IMAGE` | key `u64`, width `u32`, height `u32`, RGBA8 pixels as a byte string |
| 4 | `FRAME` | view `f32 × 4` (x, y, width, height in scene units), hold `u32`, count `u32`, count × record |
| 5 | `NOTE` | JSON as a byte string |
| 6 | `END` | failed `u8` |

A record is 84 bytes: key `u64`, kind `u8` (0 path, 1 image), transform
`f32 × 6` (a 2×3 affine, column-major), fill RGBA `f32 × 4`, stroke RGBA
`f32 × 4`, stroke width `f32`, reveal `f32 × 2` (the fraction of the path
shown), depth `f32`.

Uploads carry a **content key**: FNV-1a over the content, so the same shape
is uploaded once and a frame refers to it by key. A record of a key that was
not uploaded is refused by the writer and by the reader. A stream cut short
reads as far as it goes, with `failed` unknown.

## Commands

```
openagents film demo OUT.take [--pictures N] [--hold N]
openagents film inspect FILE [--json]
openagents film render FILE DIR [--picture N] [--background R,G,B]
```

`demo` writes a 1280×720, 60 fps take of a square sliding across the frame.
`inspect` prints the header, the counts of uploads, pictures, frames, and
records, the duration, and the notes. `render` draws every picture (or
picture N) with wgpu into `DIR/frame-NNNN.png`; set `WGPU_BACKEND=vulkan` on a
box without a display to use lavapipe.

## Drawing

`verse_film::render::Renderer` (feature `gpu`) keeps a device and the take's
uploads between pictures. Paths are flattened on the CPU (16 steps per cubic),
filled on the GPU by stencil-then-cover under nonzero winding and 4× MSAA,
and stroked as one quad per segment; images are textured quads over the unit
square; records draw back to front by `depth`; `reveal` trims the flattened
polyline by length. Analytic coverage, as ManimGX computes it, is a later
step if MSAA proves too soft.

## Rust

```rust
let mut film = verse_film::Film::new(1280, 720, 60.0);
let key = film.path(&points, true)?;
let mut record = verse_film::Record::new(key, verse_film::Kind::Path);
record.fill = [0.35, 0.77, 0.87, 1.0];
film.frame(view, 1, &[record])?;
let bytes = film.finish(false);
let take = verse_film::read(&bytes)?;
```

## Slices

1. This page: the take, `verse-film`, `openagents film inspect` and `demo`.
2. This page: draw pictures offscreen with wgpu 29, `openagents film render`.
3. Records that reference Verse content (a zone capture, a `verse-engine`
   scene) and desktop components as layers of a frame.
4. `openagents film export FILE OUT.mp4` through ffmpeg, holding unchanged
   pictures.
5. App mockups: a frame that lays out a web, phone, or desktop screen from
   the `/ui/screens` gallery's sample states.

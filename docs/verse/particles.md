# Particle effects

Status: implemented, October 5, 2026. Meteor Swarm in the demolition yard is
the first effect set built with it.

Verse draws fire, smoke, sparks, shockwaves, and dust as textured sprite
particles. An effect is data: a TOML file that names its sprite sheet and
describes its emitters. Sprite sheets come from Blender scripts. A preview
example renders an effect over time, so you can look at it without playing
the game.

## The pipeline

| Step | Where | Command |
| --- | --- | --- |
| Make sprites | `scripts/blender/fx/*.py`, one script per sheet | `scripts/blender/build-fx.sh [SHEET...]` |
| Store sprites | `assets/verse/fx/*.png`, provenance in `assets/verse/fx/PROVENANCE.md` | Commit the PNGs with their scripts |
| Describe effects | `assets/verse/fx/effects/*.toml` | Edit the file |
| Register | `crates/verse/src/fx/library.rs` (`SOURCES`) and, for a new sheet, `crates/verse-pbr/src/fx/sheet.rs` (`SHEETS`) | Add one line |
| Check | `crates/verse/src/fx/tests.rs` | `cargo test -p verse --lib -- fx` |
| Preview | `crates/verse/examples/fx_preview.rs` | `cargo run --release -p verse --example fx_preview -- EFFECT OUT_DIR` |
| Gallery | The same example | `cargo run --release -p verse --example fx_preview -- --all OUT_DIR` |
| Tune live | The desktop build | `VERSE_FX_RELOAD=1 cargo run --release -p verse` |

A new effect is usually one TOML file and one line in `SOURCES`. A new look
is one Blender script, one line in `build-fx.sh`, and one line in `SHEETS`.

### Make sprites

Each script under `scripts/blender/fx/` builds a small scene, renders one
cell per frame with Cycles on the CPU, and packs the cells into a 512-pixel
square sheet. `common.py` holds the shared setup, the sheet packer, and the
PNG writer. Run one sheet or all of them:

```sh
scripts/blender/build-fx.sh             # every sheet, about 6 minutes
FX_SAMPLES=16 scripts/blender/build-fx.sh smoke    # a quick draft
FX_PREVIEW=/tmp/fx-sheets scripts/blender/build-fx.sh fireball
```

`FX_PREVIEW` also writes each sheet composited over a dark field, alpha
blended on the left and added on the right, for a quick look before the
engine sees it.

**Encoding.** A sheet's RGB is premultiplied linear color, sRGB-encoded;
its alpha is linear coverage. Verse uploads the sheets as one sRGB texture
array, so the GPU decodes premultiplied linear color and mipmaps filter it
correctly. Additive particles add RGB and ignore alpha; alpha particles
composite RGB over the scene with alpha. Fire keeps its glow where coverage
is low, which a straight-alpha PNG can't store.

### The sheets

| Sheet | Cells | Frames | What |
| --- | --- | --- | --- |
| `fireball` | 4 × 4 of 128 px | 0–15 | An explosion from a white-hot core through rolling orange fire to sooty smoke: a Principled Volume in a sphere, density from a radial falloff broken by 4D noise, heat driving a black-body-like emission ramp. |
| `smoke` | 4 × 4 of 128 px | 0–15 | A pale puff billowing out and thinning, lit by a sun from above and a soft sky. Particles tint it (soot, dust, steam). |
| `sparks` | 2 × 2 of 256 px | 0 spark, 1 flare, 2 ring, 3 dust | A white-hot dot with a horizontal streak for velocity-stretched sparks; a soft glow with six faint rays; a shockwave ring broken by noise; a lit, wispy dust puff. |
| `butterfly` | 4 × 4 of 128 px | 0–7 orange, 8–15 lemon | Two butterflies' wingbeats, from wings spread flat up to nearly closed and down, seen from above and a little behind: lit wing surfaces with dark rims. |

### Describe effects

An effect file has a `description` and one `[[emitter]]` table per
emitter. The file stem is the effect's name. Values that change over a
particle's life are curves, written three ways:

- A constant: `alpha = 1.0`.
- Start, middle, and end, with the middle at half the life: `scale = [0.6, 1.2, 1.4]`.
- Keys at explicit times from 0 to 1: `alpha = [[0.0, 0.0], [0.1, 0.9], [1.0, 0.0]]`.

Colors work the same way with `[r, g, b]` values, and keyed colors are
`[t, [r, g, b]]`.

| Field | Default | Meaning |
| --- | --- | --- |
| `name`, `sheet` | Required | The emitter's name and its sheet. |
| `frames` | The whole sheet | First and last frame used, inclusive. |
| `animate` | `life` | `life` plays the frames once over the particle's life, blending neighbors; `random` holds one frame; `loop` cycles at `fps`. |
| `blend` | `additive` | `additive` adds light; `alpha` covers what's behind. |
| `additive` | From `blend` | A curve from 0 (alpha) to 1 (additive), so fire can cool into smoke that covers. |
| `light` | `emit` | `emit`: color times `luminance` in cd/m² before exposure. `lit`: a surface color in the scene's display scale; the sheet carries its own shading. |
| `orient` | `camera` | `camera` faces the viewer; `ground` lies flat. |
| `delay`, `burst`, `rate`, `duration` | 0 | Start delay (s), particles at once, particles a second, and how long the rate runs (0 runs until the effect is stopped). |
| `life`, `size`, `speed`, `radial`, `spin` | Required for `life` and `size` | `[min, max]`: lifetime (s), half size (m), speed along `direction` (m/s), speed out from the center (m/s), and turn rate (rad/s). |
| `shape`, `radius` | `point`, 0 | Birth area: `point`, `sphere`, `disc`, or `ring`. |
| `direction`, `spread` | `[0, 1, 0]`, 0 | Launch direction in the effect's frame and the widest angle from it, in degrees. |
| `inherit` | 0 | How much of the effect's velocity particles keep. |
| `gravity`, `drag` | 0 | Upward acceleration (m/s², negative falls) and speed lost per second. |
| `wander` | 0 | A random push each moment, m/s², in a direction drawn afresh every step, so particles flutter about; pair it with `drag`. |
| `scale`, `color`, `alpha` | 1, white, 1 | Curves over life. |
| `rotate` | `true` | Start each particle at a random angle. |
| `stretch`, `stretch_max` | 0, 50 | Lay the quad along the velocity, the tail as long as this many seconds of flight, at most `stretch_max` m. |
| `bounce` | 0 | Bounce off the ground, keeping this much speed. |
| `priority` | 0 | When a frame's budget runs out, higher priorities are kept first. |

Unknown fields are refused, so a typo fails the tests instead of being
ignored.

### Run effects from code

`verse::fx::Particles` runs effects. A zone owns one, starts effects, moves
the ones that follow something, stops them, steps it, and draws it:

```rust
let mut fx = Particles::new(seed);
fx.start("meteor_explosion", Spawn::at(center));
let trail = fx.start("meteor_trail", Spawn::at(head).moving(velocity).along(-velocity));
fx.place(trail?, head, velocity); // every frame while it falls
fx.stop(trail?);                  // when it lands; the trail lingers
fx.tick(dt, height);              // `height` is the ground, for bouncing
fx.draw(&mut mesh.sprites);
```

`Spawn::along` sets the effect's +Y axis, which emitter directions turn
with: a trail points it back along the flight. `Spawn::scaled` scales sizes,
speeds, and radii. A simulation that moves its own particles, such as the
demolition site's dust, draws them in an effect's look with `fx::Style`:
the emitter's sheet, curves, and blend at a life fraction it supplies.

The simulation is deterministic: the same seed, calls, and time steps give
the same particles. `Particles` keeps at most 4096 particles and 256 effects.

### The renderer

A frame's sprites go to the physical renderer in `Mesh::sprites`. The
renderer keeps at most `fx::budget(tier)` of them (160 on the low tier,
768 on medium, and 1536 on high), highest priority and largest on screen
first, sorts them back to front, and builds six vertices each. One
pipeline draws them all, after the glows, through one premultiplied blend
state: additive particles write no alpha, so they add light in any order,
and alpha particles cover what's behind them. A particle can move between
the two over its life.

The fragment shader samples two flipbook frames and blends them, both
samples first in uniform control flow, from one texture array with every
sheet as a layer. It uses only vertex buffers and a 2D array texture,
which WebGL2 and OpenGL ES 3.0 support; the low tier uploads the sheets at
half size. Sprites near the camera fade out, as the glows do. Soft depth
fade against the scene isn't implemented: it needs the scene depth as a
texture during the scene pass, which the multisampled targets don't offer.

### Preview

```sh
cargo run --release -p verse --example fx_preview -- meteor_explosion /tmp/fx
cargo run --release -p verse --example fx_preview -- meteor_trail /tmp/fx --frames 12 --sequence
cargo run --release -p verse --example fx_preview -- --all /tmp/fx
```

The preview installs the demolition yard from the pinned pack and runs the
effect on the lawn in daylight. It writes a contact sheet of frames, each
with a time bar along its top edge, and prints the particle count per
frame. An effect that runs until stopped is moved along a meteor's falling
path for most of the time, then stopped. `--all` writes `gallery.png`, a
four-frame strip of every effect.

### Tune live

On the desktop, `VERSE_FX_RELOAD=1` rereads `assets/verse/fx/effects/` at
most once a second when a file changes (`VERSE_FX_DIR` points it
elsewhere). A file that fails to parse keeps the last good library and logs
why. A reload ends the running effects, because their emitters may have
changed.

## Techniques we adopted

These come from studying how our removed World of Warcraft tooling
described and drew spell effects (the `wow-import` converter and the
chamber renderer, in history before commits `93fe80568e` and `b9dd2025fb`),
and from the published, community-documented layout of the M2 model
format's particle and ribbon emitters that the tooling read. We kept
techniques only. No Blizzard texture, model, or data was copied, extracted,
or converted; every sprite here is our own Blender render.

- **Atlas sheets and flipbooks.** M2 particle emitters name a texture split
  into rows and columns and pick cells over a particle's life (a "head"
  range, sometimes a "tail" range). Our `frames` and `animate = "life"` do
  the same, and blend neighboring frames so 16 frames play smoothly.
- **Three-key tracks.** M2 emitters animate color, opacity, and scale with
  three keys (start, middle, end) and a configurable middle time. Our curves
  take that shape directly, plus explicit keys when three aren't enough.
- **Blend modes as a material choice.** The tooling mapped each surface to
  opaque, alpha-tested, alpha-blended, or additive, and skipped modulate
  modes. We keep additive and alpha, unify them under one premultiplied
  blend, and let a particle move between them over its life.
- **Motion fields.** Emitters carry emission speed and its variation,
  spread angles, gravity, a slowdown (drag), lifespan with variation, and
  a rate. Ours carry the same, plus a radial speed for rings and bursts.
- **Spin and random start angle,** so a few billows don't read as copies.
- **Tail particles.** M2 "tail" particles stretch along their velocity by
  a tail length; the chamber's projectile trails sampled quads back along
  the velocity. Ours lay the quad along the velocity (`stretch`).
- **Ribbons.** M2 ribbon emitters draw a textured strip behind a moving
  bone; the chamber drew a stretched quad along the flight. A dense trail
  of short-lived, stretched, slow-inheriting particles covers the meteor's
  case; a true ribbon strip is a later addition.
- **Effects as layers.** The chamber composed a spell from several simple
  pieces: a bright core, fire, smoke behind it, sparks, and a ground rune.
  Each effect file here is that list of layers.
- **A budget with priorities.** The chamber truncated its effect instances
  to a fixed count, keeping primary cues first. Ours keeps the highest
  priority and largest sprites within a per-tier budget.

## Effects

| Effect | Used by | Layers |
| --- | --- | --- |
| `meteor_head` | Each falling meteor | Hot glow, churning fire |
| `meteor_trail` | Each falling meteor | Cooling fire, smoke, streaking embers |
| `meteor_explosion` | Each meteor's landing | Flash, fireball billows, fire column, shockwave ring, sparks, dust ring, smoke pall, ground fire |
| `cast_embers` | Meteor Swarm's cast | Embers and motes rising around the caster |
| `scorch_embers` | Each scorch mark | Smoldering embers and smoke wisps |
| `debris_dust` | The demolition site's dust puffs, through `fx::Style` | One lit dust puff |
| `chimney_smoke` | Everglade's chimneys, the bakehouse's stack, and the smithy's forge (`layout::details::chimneys`) | A thin, pale plume leaning downwind |
| `butterflies` | Every other spring and summer flower drift in Everglade | Orange and lemon butterflies wandering over the flowers |

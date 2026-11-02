# ARWES in Coax: inventory, adaptation and reuse

Date: 2026-11-01. This extends the [Aethyr HUD library](2026-11-01-hud-library.md)
with source-based ports of ARWES's relevant visual and animation algorithms.
The result is Coax product logic on the existing Grid/WebGPU systems. It is
not an installed ARWES runtime or a TypeScript, React, CSS or DOM compatibility
layer.

## Source identity and inspection boundary

The inspected source is [arwes/arwes at
`bdbaa0324900ee978d42036d1304a053c1fe54b5`](https://github.com/arwes/arwes/tree/bdbaa0324900ee978d42036d1304a053c1fe54b5).
Its standalone package metadata says `1.0.0-next.25020502`. At that pin, the
[README](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/README.md)
says the project is no longer maintained and is outdated, while describing
it as still functional. This is the pinned source's statement, not a claim
that an npm install was tested.

All 23 package entry indexes were inspected. The ten vanilla/core source
families were inventoried, including internal routines needed to understand
their public functions. The [source ledger](../../aethyr/ui/arwes/SOURCES.md)
records exact upstream paths and SHA-256 hashes for 72 non-test, non-index
core source files, all 23 entry indexes, and the license/README/standalone
metadata: 98 files. A pinned file in that ledger may contain host-only code
that is deliberately excluded; pinning it is not a feature-parity claim.

The upstream checkout was read as untrusted source. Its installation,
build, test and demo scripts were not executed. No upstream framework app,
JavaScript bundle, CSS overlay, image or sound file is shipped. The exact
[MIT license](../../aethyr/ui/arwes/LICENSE), including the original Romel
Perez copyright notice, is retained with the translated/adapted code.

## What counts as relevant code

The complete inventory below defines the boundary instead of treating
"all relevant" as an unbounded promise of framework compatibility:

- **Formula/geometry port:** numerical easing, interpolation, frame coordinates
  and shape/mask arithmetic can be translated into ordinary checked Coax.
- **System adaptation:** useful behavior is represented as typed Coax state,
  supplied time, caller-owned buffers and existing host effects. Equivalent
  intent does not imply identical JavaScript callback or browser timing.
- **Existing host:** graphics upload, input delivery, resource lifetime,
  font rasterization and any future audio decoding/playback remain platform
  effects. ARWES's DOM/Canvas implementation of those effects is not copied.
- **Excluded compatibility:** React/Solid ownership, hooks, CSS string/object
  serialization, arbitrary DOM/SVG trees, promises and browser globals do not
  belong in this Coax component library. Their public families are still
  accounted for below.

This is broad source coverage with explicit platform boundaries. A formula
port is not automatically bit-identical: upstream JavaScript numbers are
binary64, current drawing coordinates are usually `f32`, and GPU
antialiasing/blending differs from SVG or Canvas 2D. Source inspection and
local checks establish the implemented Coax behavior; they do not establish
pixel equality with every upstream browser.

## Complete vanilla/core public-family inventory

Every family re-exported by `packages/standalone/src/index.ts` is listed
here. Types are assessed with their owning runtime family rather than
copied as TypeScript declarations.

| Package | Public exports/families at the pin | Coax disposition |
| --- | --- | --- |
| `tools` | `isBrowser`, `isBrowserSafari`, `cx`, `loadImage`, `createTOScheduler`, `randomizeList`, `filterProps`; `PartialDeep`, `NoInfer`, `Writable` types | Adapt keyed timeout scheduling and seeded Fisher–Yates. Browser detection/image loading belong to the host. Class-name merging, undefined-property filtering and TypeScript type utilities are replaced by typed records/composition, not exported as DOM helpers. |
| `styles` | `styleSteps`, `styleStrip`, `styleSeparator` and their prop shape | Port the visible/transparent band arithmetic into scalar masks and Coax drawing. CSS gradient strings are excluded. |
| `theme` | `createThemeMultiplier`, `createThemeUnit`, `createThemeColor`, `createThemeStyle`, `createThemeBreakpoints`, `createCreateTheme`; multiplier/unit/color/style/breakpoint creator types | Port multiplication, bounded series selection, numeric breakpoints and typed color channels. Adapt style/unit/theme assembly to typed values. Arbitrary CSS strings, deep JavaScript-object factory behavior, arbitrary callbacks and LCH/CSS parsing are not compatibility targets. |
| `animated` | `easing`, `easeAmong`, `easeSteps`, `formatAnimatedCSSProps`, `applyAnimatedCSSProps`, `transition`, `fade`, `flicker`, `draw`, `createAnimation`, `createAnimatedElement`, `createAnimatedXElement`, `animateDraw`; easing/animation/settings types | Port all 31 easing curves, piecewise/step interpolation, transitions and path-progress arithmetic. Adapt time/direction/repeat into a pure caller-time sample and typed properties. CSS/DOM application, RAF callbacks, promises and element ownership stay outside Coax. |
| `animator` | `createAnimatorSystem`; `ANIMATOR_STATES`, `ANIMATOR_ACTIONS`, `ANIMATOR_MANAGER_NAMES`, default duration/settings; node, system, condition, manager and duration types | Adapt four lifecycle phases and six scheduling modes to bounded Coax motion/timeline/switch records. Preserve useful order, reverse, offset, stagger, limit and switch concepts; do not recreate a JavaScript subscriber graph. |
| `frames` | `createFrame`; nine builders: Underline, Lines, Corners, Octagon, Nero, Nefrex, Kranox, Header, Circle; `animateFrameAssembler`; `styleFrameClipOctagon`, `styleFrameClipKranox`; frame/path/element/settings types | Port preset geometry, family-specific numeric transition tracks and clip-point arithmetic; adapt generic drawing/shared assembly to Coax paths and twelve-float stroke output. Generic DOM/SVG construction, class names, definitions, masks, arbitrary SVG attributes and CSS clip strings are excluded. |
| `bgs` | `createBackgroundDots`, `createBackgroundPuffs`, `createBackgroundGridLines`, `createBackgroundMovingLines`; settings/props/cancel types | Adapt all four visual families to deterministic seeded Coax geometry/samples and the existing GPU. Preserve origin/phase/envelope concepts. ResizeObserver, Canvas 2D, browser DPR reads and timer ownership remain host responsibilities. |
| `effects` | `createEffectIlluminator`, `createEffectIlluminatorSVG`; props/cancel types | One typed pointer/radial-field model covers both visual concepts; Coax renders the glow through the existing GPU. DOM insertion, SVG gradient IDs, global document listeners and CSS opacity transitions are excluded. |
| `text` | `animateTextSequence`, `animateTextDecipher`, `getAnimationTextDuration`; text animation settings and controller types | Port duration and adapt reveal/decipher to bounded UTF-8 scalar strings and existing GPU text. No DOM cloning/walking, hidden text mutation or CSS visibility ownership. |
| `bleeps` | `createBleep`, `createBleepsManager`, `BLEEPS_CATEGORIES`; source/general/master/category/manager/update/controller types | Adapt categories, inheritance, gains, mute/disable, play/stop ownership and cue/loop state as pure Coax. Audio fetch, MIME choice, decoding, Web Audio nodes, window focus and actual playback remain platform effects. The demo visualizes silent cues. |

`standalone` is the eleventh vanilla package, an aggregator of those ten,
not an extra implementation family. Its npm metadata also names `tslib`;
that TypeScript runtime is unnecessary for the Coax translation.

## Every framework wrapper and aggregator

The remaining twelve packages contain integration rather than additional
independent visual mathematics. They are reviewed, classified and excluded
as framework runtime code. Their useful core behavior comes from the
Coax equivalents above.

| Package | Public wrapper surface | Treatment |
| --- | --- | --- |
| `react-tools` | `memo`, `mergeRefs`, `NoSSR`, `useUpdateEffect` | React lifecycle/ref/SSR glue; not applicable to a Coax frame loop. |
| `react-animator` | `Animator`, `useAnimator`, `AnimatorGeneralProvider`, `useAnimatorGeneral`, related types | Replace provider/context/subscriptions with caller-owned motion state and explicit parameters. |
| `react-animated` | `useAnimated`, `Animated`, `useAnimatedX`, `AnimatedX` | Use typed Coax animation samples; no React component or element ref is retained. |
| `react-bleeps` | `BleepsProvider`, `useBleeps` | Cue settings/state can be passed explicitly; hooks/providers are not ported. |
| `react-core` | `BleepsOnAnimator` | Adapt lifecycle-to-cue intent as data; do not trigger sound during headless or owner-screen review. |
| `react-text` | `Text` | Use reveal/decipher output with the existing font and fitted text. |
| `react-frames` | `FrameBase`, the nine frame components, `useFrameAssembler` | Use Coax frame geometry and assembler progress directly. |
| `react-bgs` | `Dots`, `Puffs`, `GridLines`, `MovingLines` | Use the four Coax background families directly. |
| `react-effects` | `Illuminator`, `IlluminatorSVG` | Use the single Coax pointer/radial model directly. |
| `react` | Re-exports `arwes` and all nine React packages above | No React dependency or compatibility aggregator is introduced. |
| `solid-animator` | `useGetAnimator`, `Animator` | Solid ownership/context glue; replace with explicit Coax state. |
| `solid` | Re-exports `arwes` and `solid-animator` | No Solid dependency or compatibility aggregator is introduced. |

The source repository's documentation, playgrounds, performance apps,
community examples, build scripts and publishing infrastructure are not
library runtime functionality. They are not imported into OpenAgents.

## How the useful algorithms change systems

### Theme and decorative styles

Upstream multiplier settings are a scalar times an index, a rounded series
lookup or a callback. Units and styles choose or join values; breakpoints
emit minimum-width and maximum-width-minus-one CSS conditions. Coax keeps
numeric values and explicit breakpoint predicates. It can select a typed
style record without constructing CSS or dynamically cloning a JavaScript
object tree.

Colors become numeric channels suitable for the existing vertex ABI.
HSL/RGB/HWB math and alpha are useful; a string such as `var(--color)` or an
arbitrary LCH CSS expression has no meaning to the shader writer. The
OpenAgents white/grey/cyan style remains the default rather than importing
a new palette throughout the production Grid.

`styleSteps` partitions a unit interval into `2 * length - 1` equal spans,
alternating visible and transparent regions. `styleStrip` repeats hard
color bands whose boundary positions are explicit. `styleSeparator` puts
paired active bars and gaps at one or both ends around a static middle.
Coax represents these patterns as masks and geometry. No CSS gradient
parser or stylesheet engine is required.

### Frames, assembly and clipping

ARWES's presets combine line/path, polygon and circle/ellipse definitions,
with named outline/background/decorative regions. The relevant information
is their coordinates and parameter relationships. Coax resolves dimensions
against a `HudBox`, draws original vertex records, and reuses the existing
font and renderer.

Nine presets are distinct from the generic `createFrame` entry point.
The generic upstream function accepts SVG settings, creates elements,
observes size, applies styles and updates paths. Those browser operations
are replaced by explicit Coax calls; they are not secretly reproduced in
an added JavaScript host. Frame assembler and draw transitions become
progress-controlled geometry. A clip outline or point predicate is not a
claim that the complete arbitrary SVG mask/clip/filter model was ported.

Shared assembly and source family transitions are separate profiles:

- `af_frame_settings`/`af_assembly` apply the assembler: background in its
  first half, line drawing across the interval, and the decorative
  `[0, 1, 0.5, 1]` track in the second half. Out-sine easing applies to the
  whole decorative track before its piecewise interpolation.
- `af_family_transition` produces `AfTransform` values, and
  `af_family_point`/`af_frame_family` apply each preset's own source tracks.
  Background fades, entering out-expo/exiting out-sine draw progress,
  delayed opacity, Nero corner x/y offsets, Nefrex x offsets, Circle
  rotation/scale, and Header dash/box timing are represented numerically.
  These calls do not also apply the shared assembler.

| Family behavior | Numeric profile |
| --- | --- |
| Common background and line | Default fade; line entering outExpo draw and exiting outSine erase |
| Lines decoration | Flicker over half of the transition duration |
| Nero | Half-corner-length signed x/y offsets with outExpo, early/late decorative flicker and delayed entering background |
| Nefrex | Signed square-size x offset, delayed entering background and faster exiting background fade |
| Header | Separate dash/box tracks: short dash entrance, delayed short box entrance, and their source-specific exit timings |
| Circle | OutExpo rotation from -45 to 0 degrees entering and 0 to 45 exiting; decorative scale 0.9 to 1 or 1 to 0.9; staged fades/flicker |

Several frame settings omit an explicit easing. The pinned
[package lock](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/package-lock.json#L2099)
resolves Motion One 10.18.0. Read-only inspection of its published
`@motionone/utils` defaults, `@motionone/animation` easing/animation code
and `@motionone/dom` style animation code establishes the CSS default
`cubic-bezier(0.25, 0.1, 0.25, 1)` for these tracks. Coax evaluates that
curve with scalar bisection. No Motion runtime is installed or shipped;
the [source ledger](../../aethyr/ui/arwes/SOURCES.md) links the locked
archives used to resolve that inherited default.

Family progress means 0 to 1 through one requested entering or exiting
transition. It does not reproduce per-element browser interruption and
inheritance semantics. `ArMotion`'s scalar continuity behavior is a separate
API, so callers choose their interruption policy explicitly.

### Motion and child scheduling

The easing catalog has 31 curves: linear plus in/out/inOut variants of
quad, cubic, quart, quint, sine, expo, circ, back, elastic and bounce.
Piecewise `easeAmong` and start/end `easeSteps` remain separate tools;
step boundaries and overshooting back/elastic curves need explicit checks.
Fade and flicker are property tracks, while drawing reveals path extent.

The upstream animation controller owns requestAnimationFrame and a
promise. The Coax controller samples supplied elapsed time, with direction,
finite/infinite repetition and explicit finished state. The caller cancels
by discarding its sample/state and completes by supplying the endpoint; it
does not receive the upstream promise/controller object.
Lifecycle phases remain exited, entering, entered and exiting. Retargeting
can capture a current value for continuity; this is an adaptation, not an
unobserved upstream guarantee.

The child manager names are parallel, stagger, staggerReverse, sequence,
sequenceReverse and switch. A bounded timeline expresses their start/end
relationships and pending children with explicit limits. Browser timeout
callbacks and subscriber reentrancy are not part of that data model.
The keyed timeout helper similarly replaces an existing pending ID on
start and lets the caller take due events. Equal due times retain scheduling
order, including when a vacated storage slot is reused. The keyed-timer clock domain is
finite, nonnegative seconds below one billion. Background sample APIs also
validate their screen boxes; these are explicit bounded profiles rather
than general validation of every possible frame setting. A timer whose due
time would fall outside its admitted clock domain is refused. It does not own
a browser event loop or execute a stored JavaScript callback.

### Text sequence and decipher

The duration rule is `min(length / charactersPerSecond, maxDuration)`;
upstream defaults are 100 characters per second and a four-second cap.
Sequence reveals/removes content according to progress. Decipher combines
correct characters with a changing cipher alphabet and randomized order.

Coax works over validated UTF-8 scalar boundaries, not JavaScript UTF-16
code units or DOM text nodes. The bounded text path supports at most 256
scalars/1024 bytes, refuses malformed input or insufficient output, and
uses a supplied seed/frame for repeatable deciphering. That makes screenshots
and replayable demonstrations stable; it does not promise the same random
sequence as upstream `Math.random()`.

Text must still use real advance widths and fitted columns. The operation
produces a display string; it does not rewrite a screen reader's source
text or provide the semantic platform mirror planned in Aethyr chapter 15.

### Backgrounds and pointer light

All four background families have a role on the Grid: dots/crosses provide
scale, dashed grids provide structure, moving lines supply quiet directional
motion, and puffs provide a soft atmospheric layer. Keep their intensity
below text and selection so they do not consume the entire visual hierarchy.

Dots use evenly spaced positions with centered leftover margins. Origin
progress is an axis fraction for an edge, or distance from an arbitrary
origin divided by its farthest-corner distance. Enter/exit and inversion
change which direction the reveal travels. Size, shape and opacity are
separate from the geometry of the lattice.

Puffs have initial position/radius and per-puff x/y/radius offsets. Sets
receive wrapped phase offsets, an out-sine progression, and a triangular
opacity envelope. Moving-line sets likewise have wrapped phase offsets;
in-out cubic motion carries segments across the visible axis. Seeded samples
replace browser randomness so the caller controls identity and repeatability.
The wrapped phase preserves upstream's special endpoint `1`; a negative
phase produces zero, matching its clamped JavaScript remainder convention.

The illuminator computes pointer-relative visibility and a radial falloff.
The DOM and SVG versions differ in insertion/listener mechanics; a Coax
canvas needs neither version's DOM construction. The GPU adaptation renders
radial gradients and filled shapes through Coax shader modes while
the existing host continues to upload its buffers and bind the canvas.
Canvas/SVG raster output and GPU output are different rendering profiles.

### Bleep state without browser audio code

The five categories are background, transition, interaction, notification
and voice. Upstream manager precedence is common settings, then category,
then named bleep; master gain is separate. Play/stop caller identity,
restart/loop state, clamped gain and mute/disabled decisions are useful pure
state. They can become platform commands later without moving the decision
logic into a browser library.

Pending loads are an intentional bounded adaptation: Coax records a single
pending state with the most recent request time and a bitset of up to 16
callers. Upstream registers a callback for each play request while loading
and adds caller names after a buffer is available. Coax aggregates pending
callers earlier, can cancel its pending state through stop/disable, and
uses the current pending timestamp when a loaded fact arrives. It does not
reproduce that callback queue or claim identical pending-caller behavior.
`AxBleep.load_ticket` also binds asynchronous completion to the current
load generation. An adapter captures that ticket when requesting data and
passes it to `ax_bleep_loaded_for`; stale, disabled or unloaded generations
cannot start a cue. A stop that ends the cue, unload, and disable invalidate the ticket. The
`ax_bleep_loaded` convenience call uses the current ticket and is suitable
only when the caller already knows the load fact belongs to that state.
The adapter still supplies meaningful load facts and respects the configured
maximum playback delay; the Coax sample has no browser decoder lifecycle.

Loading sources, choosing MIME support, fetching bytes, decoding buffers,
creating AudioContext/GainNode objects and playing sound are host effects.
The HUD demo stays silent and shows a cue visualization. That demonstrates
Coax scheduling/state without claiming audio decode or playback parity.
It neither asks for microphone access nor downloads upstream sound assets.

## Reuse within OpenAgents

The three value/geometry units are
[arwes-frames.coax](../../aethyr/ui/arwes-frames.coax),
[arwes-motion.coax](../../aethyr/ui/arwes-motion.coax), and
[arwes-fx.coax](../../aethyr/ui/arwes-fx.coax).
[arwes-draw.coax](../../aethyr/ui/arwes-draw.coax) composes them with the
existing `hud.coax`/`draw.coax` layout, hit, font and rendering foundation.
The demo integration is
[arwes-demo.coax](../../apps/openagents-web/demos/hud/arwes-demo.coax),
with a sixth ARWES tab alongside the five existing compositions. The standalone
build and review commands remain in the [demo README](../../apps/openagents-web/demos/hud/README.md).

The sixth tab adds 25 specimens, grouped once each as follows. Together with
the original eighteen HUD components, the laboratory now exposes 43.

| Group | Count | Specimens |
| --- | --- | --- |
| Frames | 9 | Underline, Lines, Corners, Octagon, Nero, Nefrex, Kranox, Header, Circle |
| Motion | 6 | Text sequence, text decipher, all 31 easing curves, child timing, flicker, keyed timers |
| Fields | 5 | Dots, grid lines, moving lines, puffs, pointer illuminator |
| Tokens | 5 | Steps, strip, separator, theme scales, silent bleep cue model |

Group controls replay the local transition, enter/exit it, change easing or
child manager, reseed procedural content, and change dot shape. Frames can
switch between ASSEMBLY and the source FAMILY transition profile. Token
controls select cue category, issue a silent cue, and toggle loop state.
The enlarged inspector exposes the appropriate additional controls. These
interactions operate on the same Coax values being drawn. Category labels
and cue bars represent state; they do not play audio.

### Coax rendering and the existing ABI

`aw_frame`, `aw_frame_family`, `aw_background` and `aw_visual` in `arwes-draw.coax` convert
resolved geometry/samples to the HUD's existing twelve-float records.
`aw_stroke` clips segments to their logical box and vertical viewport strip.
`aw_disc`/`aw_quad` add filled primitive records. The Coax-authored
[demo GPU wrapper](../../apps/openagents-web/demos/hud/gpu.coax) loads the
existing projects passes and handles only the added fragment modes:

| Mode | Primitive | Meaning |
| --- | --- | --- |
| 5 | Radial field | Linear center-to-edge falloff, used for actual filled puffs and the pointer illuminator |
| 6 | Solid rectangle | Flat color/intensity region, used for tokens, bars and polygon scan spans |
| 7 | Filled disc | Radius boundary with a half-device-pixel smooth edge, used for circle dots |

For these modes, the normal twelve-float positions remain device-space
x/y. The along/across fields carry position relative to the radial origin,
the length field carries radius, the light field carries intensity, RGB
carry color, and the final mode field selects the interpretation. Existing
stroke/sans modes retain the original shader path. The browser host needs
no extra DOM, CSS, Canvas 2D effect, framework or application logic.

`aw_fill` uses sorted even/odd edge crossings so concave frame paths retain
their cut corners. It emits rectangle spans across
`min(256, ceil(box.height / 2))` rows rather than a convex triangle fan.
The 256-row cap means taller boxes use coarser spans; this is a bounded
raster approximation, not arbitrary SVG filling/filter parity. Frame
`move` boundaries, fill flags, layer alpha and stroke widths are carried
through the conversion. The component rendering helpers can return partial
geometry on a full GPU buffer, so the caller still owns its frame budget.

All positions remain logical CSS points in the browser, y down; the existing
`HdPen.d` converts once to device-space vertices. The caller owns time,
selection, buffers and lifetimes. Source-provided arithmetic is drawing and
UI state, not a new simulation clock, money rule or admission decision.
New components do not overwrite the homepage's verified live observations.

The retained [source ledger](../../aethyr/ui/arwes/SOURCES.md) and
[license](../../aethyr/ui/arwes/LICENSE) travel with the Coax adaptation.
Final verification counts, screenshots and the pushed commit are recorded
separately from this source-analysis record.

## Implementation recipes and properties

The recipes below describe explicit Coax composition. Exact procedures and
records are declared in the value/geometry units and drawing adapter; use those declarations
rather than treating upstream JavaScript prop objects as accepted input.

1. Choose a frame preset and a logical `HudBox`; supply finite dimensions,
   geometry settings and a caller-owned path/vertex budget. Sample motion or
   assembler progress, then draw the result using the existing `HdPen`.
2. Keep one motion/timeline record per animated UI group. Advance it with the
   frame's supplied time, derive property values, and pass the same boxes to
   `HudControl` registration. A drawing transition never owns pointer input.
3. Provide theme values as numeric channels/units. Apply a steps/strip/
   separator mask to a line or filled-shape region; preserve readable text
   and the existing selection/focus convention above background decoration.
4. Supply deterministic seeds for puffs, moving lines and decipher. Keep
   seed/identity stable while resizing or changing emphasis unless an explicit
   reset requests a new realization. Freeze and reduced motion should affect
   the caller's decorative clock rather than replaying a partial transition
   on every frame.
5. Treat cue output as data. The current demo visualizes it silently; any
   future audio adapter receives category/source/gain/loop decisions and
   reports lifecycle facts back without duplicating policy.

The principal Coax properties and entry points are:

| Unit | Properties and APIs | Interpretation |
| --- | --- | --- |
| Frames | `AfSettings`, `AfPoint`; `af_settings`, `af_frame_settings`, `af_frame` | Preset indices 0–8 mean Underline, Lines, Corners, Octagon, Nero, Nefrex, Kranox, Header, Circle. Settings preserve padding, stroke, cuts, line/decorative dimensions, corner bitmask and Header orientation. |
| Family motion | `AfTransform`; `af_family_transition`, `af_family_point`, `af_frame_family` | Each family's own fade/draw/translation/rotation/scale tracks, sampled at entering/exiting progress 0–1. Point `role` preserves the original corner/decorative identity; the shared assembler is a separate profile. |
| Clips/assembly | `af_clip`, `af_clip_contains`, `af_assembly` | Clip indices 0/1 mean Octagon/Kranox. Frame geometry and clip geometry are distinct calls. Output paths carry move/layer/fill/alpha/stroke data, not only a list of connected outline points. |
| Rendering | `aw_frame`, `aw_frame_family`, `aw_frame_profile`, `aw_background`, `aw_visual`, `aw_stroke`, `aw_fill`, `aw_disc`, `aw_quad` | Convert shared-assembly or source-family frame paths and background samples to existing stroke/sans records plus modes 5/6/7. `HdPen` owns scale/clip/style; the caller owns storage and its final vertex budget. |
| Values | `ar_ease`, `ar_steps`, `ar_among`, `ar_transition`, `ar_flicker`, `ar_draw` | Progress/value functions; `ar_draw` returns length, offset and visible share using outExpo enter/outSine exit defaults. |
| Motion | `ArMotion`; `ar_motion_to`, `ar_motion_tick`, `ar_motion_value`; `ar_animation_sample` | Caller-owned state and seconds. A repeated sample uses elapsed time; `repeat=-1` means unbounded repetition. |
| Children | `ArChild`, `ArTimeline`, `ArSwitch`; `ar_child_timing`, `ar_children_duration`, `ar_schedule_offer/remove`, `ar_switch_select` | Up to 64 timeline children, explicit keys, caller-provided conditions and observed exit completion. |
| Text | `ar_text_duration`, `ar_text_length`, `ar_text_sequence`, `ar_text_decipher`, `ar_text_blink` | String input and caller-owned byte output. Negative result means refusal; a nonnegative result is a byte count. |
| Theme/style | `AxColor`; `ax_multiplier`, `ax_series`, `ax_breakpoint`, `ax_rgb/hsl/hwb`, `ax_steps_mask`, `ax_strip`, `ax_separator` | Numeric channels, predicates and masks. Color functions take RGB percentages or hue degrees/S/L/W/B percentages, then output normalized channels. |
| Background | `AxDot`, `AxLine`, `AxPuffSettings`, `AxPuff`; `ax_dot_count`, `ax_origin`, `ax_dot_alpha`, `ax_dot`, `ax_moving_lines`, `ax_puff_sample`, `ax_puff` | Caller-supplied viewport, spacing, seed, phase and output storage; generated samples are independent of browser global state. |
| Pointer light | `ax_illuminator_visible`, `ax_illuminator` | Pointer/box coordinates and size produce visibility/intensity, leaving rendering to Coax drawing/shaders. |
| Scheduling | `AxTimer`; `ax_schedule`, `ax_pending`, `ax_stop_timer`, `ax_stop_all`, `ax_take_due` | Caller-owned keyed slots, due times, insertion order and positive action IDs; equal due times are FIFO. Zero from take means no due action. |
| Cues | `AxBleepConfig`, `AxBleep`; `ax_bleep_config/update/play/loaded/loaded_for/stop/unload` | Explicit override flags distinguish unset values from zero/false. Up to 16 caller IDs. `load_ticket` binds asynchronous load results. Play output 0/1/2 means no source request/start/restart; no audio effect happens in these functions. |

A root-level entry can prepare geometry and display text without a host
callback or allocation:

```coax
#load "aethyr/ui/arwes-frames.coax";
#load "aethyr/ui/arwes-motion.coax";
#load "aethyr/ui/arwes-fx.coax";

example_frame :: (points:[]AfPoint,elapsed:f32)->int {
  settings:=af_settings(3); // Octagon.
  settings.square=12;
  settings.padding=4;
  progress:=ar_animation_sample(elapsed,0.6,AM_OUT_SINE,false,0).progress;
  return af_frame_settings(3,
    HudBox.{x=24,y=180,width=320,height=180},settings,progress,points);
}

example_text :: (bytes:[]u8,elapsed:f32,frame:int)->int {
  text:="AETHYR / LINK READY";
  duration:=ar_text_duration(ar_text_length(text),4,100);
  progress:=ar_animation_sample(elapsed,duration,AM_LINEAR,false,0).progress;
  return ar_text_decipher(text,progress,true,17,frame,bytes);
}
```

The returned frame count indexes `AfPoint`s. It is not a GPU vertex count.
A consumer honors `move` boundaries, fills closed `fill` paths, applies layer
alpha/stroke width and converts the resolved geometry to its declared
renderer ABI. `aw_frame` performs that conversion. Ellipses and dashed-circle
geometry use a 64-segment approximation; Header skewed decorations use the
declared viewport origin. Use `af_frame_family` or `aw_frame_family` for
the source-specific numeric transforms, and `af_frame_settings`/`aw_frame`
for shared assembly; the two profiles are not implicitly combined.
Kranox frame defaults use stroke 2, while the standalone ClipKranox source
default is stroke 1: set the clip's settings deliberately when matching it.

For text, the caller slices its byte buffer to the returned length and
passes that display string to fitted text drawing. On a negative result it
keeps the previous complete string or presents a refusal state. Unicode
scalar safety is not grapheme shaping or complete font coverage.

A local delayed cue can likewise be modeled entirely as state:

```coax
example_cue :: (timers:[]AxTimer,cue:[]AxBleep,now:f32)->int {
  scheduled:=ax_schedule(timers,7,now,0.1,42);
  if !scheduled {return 0;}
  config:=AxBleepConfig.{volume=0.25,volume_set=true,
    loop=false,loop_set=true};
  ax_bleep_update(cue,config,0.8,false);
  return ax_take_due(timers,now); // 0 until the caller's time reaches due.
}
```

That example only prepares a timer/configuration. Schedule once at the
trigger; later frames take due events rather than repeatedly replacing the
same key. A due action can update the silent cue display or request a
separate platform adapter. Loading and playback remain outside this pure
recipe. Code examples describe composition patterns, not standalone programs
or additional checked acceptance claims.

A consumer using the GPU adapter can load its combined unit instead of the
three value units separately:

```coax
#load "aethyr/ui/arwes-draw.coax";

example_vertices :: (vertices:[]f32,pen:HdPen,elapsed:f32)->int {
  share:=ar_animation_sample(elapsed,0.6,AM_OUT_SINE,false,0).progress;
  return aw_frame(vertices,0,3,
    HudBox.{x=24,y=180,width=320,height=180},pen,share);
}
```

That result is a GPU vertex count with twelve floats per vertex. The
consumer must bind the demo's compatible Coax GPU wrapper or equivalent
support for modes 5/6/7; the original projects fragment function alone does
not interpret these added primitives. A native renderer can implement the
same declared records, but native raster parity is a separate qualification.

To choose the source-family profile instead of shared assembly, sample
normalized time without applying an extra easing to the whole profile:

```coax
example_family_vertices :: (vertices:[]f32,pen:HdPen,elapsed:f32,entering:bool)->int {
  phase:=ar_animation_sample(elapsed,0.6,AM_LINEAR,false,0).progress;
  return aw_frame_family(vertices,0,4,
    HudBox.{x=24,y=180,width=320,height=180},pen,phase,entering);
}
```

The selected Nero family applies its own track easings and offsets. Each
new direction starts a phase at zero and ends at one. A consumer that needs
interrupted per-element continuation must implement that separate policy;
passing an `ArMotion.value` as family time is not equivalent.

For asynchronous cue loading, capture `cue[0].load_ticket` when the current
pending load is issued. Deliver completion through
`ax_bleep_loaded_for(cue, captured_ticket, now, maximum_delay)` rather than
substituting whatever ticket happens to be current later. A stale completion
then leaves the current cue untouched. The demo remains a silent state
consumer and does not perform that platform loading work.

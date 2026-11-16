# GPU components on native: one kit, one display list

Written 2026-11-15. Status: specification, with stage G0 built (the frame
context, the keyed state store, the semantic table and the surface commit;
see [Stages](#stages)). Tracked in
[#13645](https://github.com/OpenAgentsInc/openagents/issues/13645).

This document decides how native Coax Native apps draw their components. It
builds on [the component spec](spec.md) and turns the recommendation in
[rendering.md](rendering.md) into the plan of record. Where the two disagree
about native presentation, this document wins. The requirements in
`rendering.md` for motion, effects, native text, DOM pages with GPU islands
and scene views stay in force.

## Decision

On every native target (iOS, Android, Mac, Linux), the shared display-list
kit draws every component through `backend/gpu.coax`. The platform supplies
only what a GPU surface cannot:

- the active text field: input methods, AutoFill, the keyboard;
- the accessibility tree;
- system presentations: passkeys, biometrics, pickers, share, permissions.

On the web, text surfaces stay DOM. A DOM page can mount a GPU island that
uses the same display-list components.

The native app runs compiled Coax linked into the binary. New work under
this spec does not depend on the JavaScriptCore executor or on Coax Native
Updates.

## Why

Two component kits exist today, and they reach the screen three ways.

| Kit | Output | Used by |
| --- | --- | --- |
| Tree kit (`kit/frames.coax`, 63 `k_*` procedures) | A keyed node tree (`core/tree.coax`) that `backend/dom.coax` compares with the last one and writes as changes | Web DOM; UIKit, AppKit and GTK controls in the native clients |
| Display-list kit (`kit/controls.coax`, `kit/markdown.coax`, `kit/hud.coax`, `kit/animated.coax`, `zeron/`) | Primitives on a sheet (`paint/sheet.coax`), drawn by `backend/gpu.coax` | The desktop recovery shell on Mac and Linux, `/chat`, the gallery, realm HUDs |

Since build 1087, iOS combines both: UIKit holds and measures the controls,
and `native/scene.coax` draws their decoration in a Metal scene on top.

This costs three things:

- Each component family is built twice, and the two kits do not share
  component state or skin roles.
- A native update rebuilds the whole tree, compares it, sends changes across
  a bridge and applies them to platform controls. Rebuilding a display list
  in compiled code is a much shorter path.
- The hybrid adds a seam: UIKit owns bounds, Coax owns paint, and the two
  must agree every frame.

GPU drawing is not faster or lower power than native views by itself. The
gain is one implementation, one look and one motion system across targets.

## Relation to GPUI

`backend/gpu.coax` and `backend/gpu-shader.coax` already port GPUI's drawing
model: its quads, shadows and sprites, its 28-float vertex, its pixel
snapping and its atlas contract. This spec extends that backend. It does not
add Rust GPUI as a dependency.

This spec does not adopt GPUI's state model (entities and `notify`) or a
signals model. See [State and invalidation](#state-and-invalidation).

## What changes and what stays

| Area | Today | After |
| --- | --- | --- |
| Native visuals | UIKit, AppKit and GTK controls, plus a GPU overlay on iOS | Shared display-list components on Metal and Vulkan |
| Native layout | Platform measures control bounds | `layout/` measures; the host reports only window, safe-area and keyboard facts |
| Native text display | Platform labels and text views | Glyph atlas text with selectable lines |
| Native text entry | Platform fields | A platform editor overlay on the focused field only |
| Accessibility | Platform controls expose themselves | Semantic rows from the components, bridged per platform |
| Native app logic | Coax compiled to JavaScript, run in JavaScriptCore | Compiled Coax linked into the app |
| Web text surfaces | Tree kit and DOM | Unchanged |
| Web GPU surfaces | `/chat` and the gallery | The same kit, mounted as islands in DOM pages |
| Realm rendering | Metaverse Engine host contract | Unchanged |

## The component contract

A component is a Coax procedure. One call does four things:

1. **Measures and lays out** from its inputs, an available width and the
   skin's sizes, using `layout/`.
2. **Paints** primitives onto the sheet through `paint/`. It reads only
   skin roles. A colour or size written in a component is a defect, as in
   `spec.md`.
3. **Records one semantic row** for each thing a person can press, type
   into or read as a unit: stable ID, role, label, state, value, bounds,
   parent and the action it dispatches.
4. **Records its hit region** against the same ID and bounds.

A component never calls the host, starts a request or changes app state. It
writes requested actions to the outbox, and the app takes them, as today.

### Frame context

Components today take the sheet, measure and skin as separate arguments.
The kit gains one frame context that carries them, so a component call
needs only its ID, its inputs and its position. The context holds:

- the sheet, the text measure and the skin;
- the semantic table for this frame;
- the keyed state store;
- the input snapshot: pointer, pressed ID, focused ID, hovered ID;
- the frame clock and the reduced-motion and reduced-transparency facts.

The context is `UiFrame` in `kit/frame.coax`. It lives in `kit/`, not
`core/`, because it names the sheet (`paint/`) and the skin (`skin/`), and a
layer loads only the layers before it. What it refers to stays in its own
layer, and the context is a set of views over those, made new each frame:

| Piece | Type | File |
| --- | --- | --- |
| Frame context, input snapshot | `UiFrame`, `UiInput` | `kit/frame.coax` |
| Committed surface: display list, hit regions and rows as one generation | `UiSurface` | `paint/surface.coax` |
| Semantic table | `ElTable`, `El` | `core/elements.coax` |
| Keyed state store | `KeyedStore`, `Keyed` | `core/keyed.coax` |
| Motion times | `UiMotion` (`skin.motion`) | `skin/skin.coax` |

One frame on a surface:

```jai
// The caller declares the capacities: two sheets and two row arrays (one
// per generation), and the keyed rows.
sf := [1] UiSurface.[surface_over(sheet_a, rows_a[..], sheet_b, rows_b[..])];
ks := [1] KeyedStore.[KeyedStore.{rows = keyed_rows[..]}];

f: [1] UiFrame;
in := uiinput(pointer_x, pointer_y, true, pointer_down, pressed_id, focused_id);
uiframe_begin(f[..], sf[..], ks[..], measure, skin, in, clock, dt, reduced_motion, reduced_transparency);
inside := ctx_panel(f[..], "login", x, y, w, h, "Log in");
w := ctx_button(f[..], "login.send", inside.x, inside.y, "Send code", true, 0, ACT_SEND);
k := uiframe_key(f[..], "login.send");            // keyed state for this id, or -1
p := uiframe_press(f[..], k, "login.send");       // 0 to 1 while pressed
ctx_panel_end(f[..]);
if uiframe_end(f[..], sf[..]) { /* gpu_draw surface_sheet(sf[..]) */ }

// A press is routed through the committed rows.
row := surface_hit(sf[..], tap_x, tap_y);
id := surface_id(sf[..], row); action := surface_action(sf[..], row);
```

A component that is not one of the controls records itself with
`uiframe_row(f, id, role, label, x, y, w, h, state, action)`: its semantic
row and, for a role a person presses or types into, its hit region over the
same box. `uiframe_clip` and `uiframe_unclip` cut paint and presses
together; `uiframe_open` and `uiframe_close` nest rows under a parent.

Existing `kit/controls.coax` signatures keep working unchanged. Each of
panel, button, toggle, slider, tabs, list and bar has a context form
(`ctx_panel`, `ctx_button`, `ctx_toggle`, `ctx_slider`, `ctx_tabs`,
`ctx_list`, `ctx_bar`) that takes a stable ID in place of a hit kind and
number, paints through the existing procedure, and records the row and the
hit. A check holds the two forms to the same primitives in every skin. A
caller that has only a sheet, a measure and a skin makes a context with
`uiframe_over(sheet, measure, skin)`: the same components paint and nothing
is recorded.

### Identity and keyed state

Every component call has a stable ID built from its part and its name
(`composer.send`, `message.4.7`), never an index. This is the existing rule
in `core/elements.coax` and `core/tree.coax`.

State that outlives one frame belongs to the ID in a keyed store with a
declared capacity: entry and press motion, spring position and velocity,
scroll offset, expanded or collapsed, caret blink. `native/scene.coax`
already does this for its nodes; that store generalizes and moves to
`core/`. A node not seen in a frame retires. Reordering rows never moves
state from one ID to another.

A duplicate ID or a full store refuses that component's keyed state and
counts the refusal. It never silently shares another component's state.

The store is `KeyedStore` in `core/keyed.coax`, over rows the caller
declares. `keyed_begin` starts a frame, `keyed_see(store, id)` returns the
ID's row or -1 when refused, and `keyed_end` retires every ID not seen.
`keyed_entry`, `keyed_press`, `keyed_spring`, `keyed_scroll`, `keyed_open`
and `keyed_slot` read a row; each takes -1 and answers as a stateless
component would, so a refusal degrades motion and never correctness. The
refusals are `refused_full`, `refused_duplicate` and `refused`.
`native/scene.coax` keeps its own integer-keyed store for now; moving it
onto `KeyedStore` is follow-up work.

### One semantic table

`core/elements.coax` becomes the one semantic table for a presentation
surface. It gains state, value, parent, action and focus order, and a
capacity declared per surface in place of the fixed 192 rows. Four readers
use the same committed rows:

- hit testing and pointer routing;
- keyboard focus order;
- the accessibility bridge;
- `coax drive` (`tree`, `tap ID`, `type ID TEXT`).

The tree's roles and attributes (`AT_LABEL`, `AT_ROLE`, `AT_EXPANDED`,
`AT_PRESSED`, `AT_DISABLED`, `AT_CURRENT`, `AT_LIVE`) map onto semantic
rows, so one description serves DOM attributes and native accessibility.

The table is `ElTable` over rows the caller declares (`elt_over`,
`elt_begin`, `elt_row`). A row's state is `EL_S_*` bits (disabled, busy,
pressed, expanded, current, checked, focused, live); `elt_value`,
`elt_action`, `elt_focus` and `elt_focusable` set the rest, and `elt_open`
and `elt_clip` give following rows their parent and clip. The readers'
lookups are `elt_hit` (the topmost row under a point, honouring nested
clips), `elt_focus_step` (next or previous in focus order), `elt_find`, and
for a bridge the rows in order with `elt_visible` and `elt_parent`.
`elt_tree` prints `coax drive`'s `tree` lines; `elt_detail` adds state,
value, action, focus place and parent. The fixed 192-row `Elements` and its
`el_*` procedures keep working for the callers that hold one, with the same
`tree` output.

## The frame

```text
input, timer or completion
        -> action (validates, changes state, marks surfaces dirty)
        -> frame: components lay out, paint and record rows
        -> commit: display list + hit regions + semantic rows, together
        -> gpu_draw -> present
```

- Draw, hit-test, selection and accessibility geometry commit together. A
  reader never sees new paint with old hit regions.
- A static surface sleeps. No frame runs and nothing is submitted to the
  GPU while no surface is dirty and no motion is due.
- Motion wakes the frame clock only while an animation is running and its
  surface is visible.
- If a list, pool or table overflows, the frame counts what was lost. An
  overflow must not drop a label or leave an invisible active hit target;
  the surface refuses the frame and keeps the last committed one.

`UiSurface` (`paint/surface.coax`) is the commit. It holds two generations
over the caller's arrays. `surface_begin` empties the back one for the
frame to fill, and `surface_commit` makes it the committed one, so readers
(`surface_sheet`, `surface_rows`, `surface_hit`) see the old set or the new
set, during a frame as well as between frames. A frame that lost anything
is refused: `surface_commit` returns false, counts it in `refused`, `lost`
and `last`, and the committed generation stays. Lost means anything the
sheet counted (`Sheet.lost`), any row or nesting the table counted, or a
display list left full, since a component that appends to a list itself is
not counted by the sheet. Declare each list one longer than its largest
frame needs. `keyed_moving` says whether motion alone needs another frame.

## State and invalidation

The explicit dirty model stays. An action marks what it changed; a view
never starts work by being rebuilt. Three changes make that model cheaper:

- **Dirty is per presentation surface**, not per app. A change to the
  composer does not rebuild the sidebar's display list.
- **Each surface caches its committed display list.** A clean surface
  redraws from the cache; a surface whose only change is a transform or
  opacity does not re-rasterize text.
- **Lists realize only visible rows.** Cost follows the rows on screen, not
  the rows in the data.

Dependency tracking (signals) is out of scope. The contract leaves room for
it: a component call is keyed by ID and takes explicit inputs, so a later
change can skip a subtree whose inputs did not change without altering
callers. Decide that from measurements, not in advance.

A forgotten dirty mark shows stale UI. Until the compiler can check it, the
rule is that state fields change only inside actions, and a check compares
an incremental frame against a full rebuild for each migrated screen.

## Platform services

Requirements for text editing, accessibility and system integration are in
[rendering.md](rendering.md#native-text-and-system-integration). This spec
adds how they attach to GPU components.

### Text entry

A field paints on the GPU like any component. When it takes focus, the host
places a platform editor over the field's committed bounds: `UITextInput`
on iOS, `NSTextInputClient` on Mac, `InputConnection` on Android and the
Wayland text-input protocol on Linux. There is one editing authority at a
time. The platform owns the active composition; `core/edit.coax` owns the
buffer and receives complete typed edits. Never draw two carets or the same
glyphs twice.

Secure fields, one-time-code AutoFill and password AutoFill use the
platform's own field for the whole edit, styled from the skin's exports.

### Accessibility

The host builds platform accessibility elements from the committed semantic
rows: `UIAccessibilityElement` on iOS, `NSAccessibilityElement` on Mac,
virtual nodes on Android and AT-SPI on Linux. The GPU view itself is hidden
from accessibility. Activating an element dispatches the same action as a
tap. A virtualized list can realize a row that accessibility requests.

### Scrolling

Coax owns the scroll offset as keyed state, and `core/scroll.coax` owns the
end-follow rule. On iOS and Android the host drives the offset from the
platform's own scroll physics, so momentum and overscroll feel native; on
desktop the host delivers wheel and trackpad deltas. One gesture has one
owner, as `rendering.md` requires.

### Text display

Display text uses the glyph atlas and `layout/text.coax`. Shaping and font
fallback use the platform's text services where the Coax rasterizer falls
short. A missing glyph is not an acceptable regression. Selection comes
from the sheet's selectable lines; copy goes through the action outbox.

## Backend work

`rendering.md` lists the drawing gaps. The ones this migration needs first:

| Need | Why |
| --- | --- |
| Images (`PRIM_IMAGE`; `gpu_draws` returns false for it today) | Avatars, attachments, pictures in messages |
| Nested clips and affine transforms | Scroll areas inside panels; drawers and sheets |
| Group opacity | Enter and exit of whole components |
| Blur and composition | Menus and sheets over content |

A new primitive is added to `paint/list.coax` once, then to each backend.
The one shader source in Coax stays the only source of primitive math.

## The web

`kit/frames.coax`, `core/tree.coax` and `backend/dom.coax` stay the
presentation for web text surfaces. Browser text input, selection, Find and
accessibility are not reimplemented on a canvas.

To keep one library with two presentations and not two libraries:

- Component rules and state move out of both kits into `core/`: what a
  button's states are, how a composer accepts input, how a transcript
  groups parts. Each kit keeps only its presentation.
- Both kits use the same IDs, roles and actions for the same component, and
  a check holds them equal for each migrated family.
- Skins stay the single source of tokens for both.

A DOM page mounts a GPU island as specified in
[rendering.md](rendering.md#dom-pages-with-gpu-components).

## Migration

### Component families to port

The display-list kit already has: panel, button, toggle, slider, tabs,
list, bar (`kit/controls.coax`), Markdown (`kit/markdown.coax`), the chat
set (`zeron/`: composer, transcript, tool chips, pickers, attachments), the
HUD set and the animated frame, ring, switch and drawer.

To port from the tree kit, grouped by the screens that need them:

| Group | Tree-kit components |
| --- | --- |
| Basics | text, icon, icon button, link, card, notice, header, menu |
| Forms | field, input, secret, select, code field |
| Chat | composer, user message, agent message, meta line, tool call, tool line, code block with copy |
| Navigation | side, places, folds, rows, who |
| Files and media | drop, file, image, remote image, document |
| Panes | dock, pane, prop, fact, stepper, switch row (`kit/dock.coax`) |

Port a family when a screen needs it. Do not port the whole kit ahead of a
consumer.

### Stages

iOS goes first. Each stage has a real consumer and its own checks. A stage
does not start the next one's removals.

| Stage | Work | Exit condition |
| --- | --- | --- |
| G0 (built 2026-11-15) | The frame context (`kit/frame.coax`), keyed state store (`core/keyed.coax`), semantic table (`core/elements.coax`) and surface commit (`paint/surface.coax`), with adapters for existing callers | Existing gallery and chat pictures unchanged; checks for identity, retirement and refusal |
| G1 | The iOS login screen, in the installed compiled runtime that already hosts the component gallery: email-code login drawn by the kit over a live GPU scene (see [The first screen](#the-first-screen-login)). Includes the text-field overlay and the VoiceOver bridge | The screen has no UIKit controls except the focused field's editor; login works with the keyboard, an input method, one-time-code AutoFill and VoiceOver on a physical phone; the scene holds the frame-rate objective while typing |
| G2 | Settings, with the scroll driver | The screen has no UIKit controls; large text lays out; scrolling feels native |
| G3 | Backend gaps the chat needs: images, nested clips, transforms, group opacity, blur | Each has a gallery specimen and a generated-shader check on Metal and WebGPU |
| G4 | The rest of iOS, screen by screen: chat and composer, history, Projects, Usage, world library and editor chrome | Each screen passes its acceptance case on a physical phone, and the transcript workloads under [Measurements](#measurements) are recorded |
| G5 | Delete the UIKit control projection on iOS | No iOS screen builds controls from the tree |
| G6 | Desktop on Mac and Linux: move the native environment client's screens from AppKit and GTK controls onto the kit, on the shared AppKit/Metal and Wayland/Vulkan hosts the recovery shell already uses; then delete those projections | Login, history and chat run on both desktops through the kit, with real text entry, keyboard focus and a screen reader |
| G7 | Android on the existing Vulkan host | Real input method, TalkBack and device evidence before any parity claim |

iOS is first because it is the product in focus, and login is the first
screen (owner, 2026-11-15). The shared pieces built for it in G0 to G3 are
what desktop and Android reuse.

### The first screen: login

Login is the first thing a person sees, so it is where the GPU kit should
show what a native-control screen cannot: the email login over a live scene,
like the home splash, in one Metal surface.

- **Flow:** unchanged. Email, then the code, through the existing
  `auth.emailStart` and `auth.emailVerify` services. The page says "Log in".
- **Scene:** a full-screen animated scene behind the form, drawn in the
  same surface as the components. It reuses existing scene and shader code
  (the home splash, the Metaverse Engine or the ARWES backgrounds in
  `core/arwes/`) and adds no app-private renderer. The implementation
  change names which one.
- **Form:** the kit's title, field, button, notice and busy state, in the
  production skin, composited over the scene with the scene dimmed or
  blurred behind the form so text stays readable.
- **Text entry:** the focused field uses the platform editor overlay. The
  code field supports one-time-code AutoFill.
- **Motion:** the form enters with the kit's motion. With reduced motion,
  the scene is a still frame and the form appears at once.
- **Limits:** the scene never delays the first interactive frame or typing.
  If the scene cannot load or the GPU budget refuses it, the form shows on
  the plain canvas colour and login still works.
- **Lifecycle:** the scene stops presenting in the background and releases
  its resources after login.

Screens that have not moved keep their current presentation. A screen is
either on the kit or on the old projection, never half of each.

### The JavaScript and Updates path

The owner decided on 2026-11-15 to drop Coax Native Updates and the
JavaScriptCore executor, and has not yet said to start. Until then:

- The shipped UIKit and JavaScriptCore runtime keeps working and keeps
  receiving fixes. Screens that have not moved still run on it.
- A migrated screen runs as compiled Coax in the installed runtime, the way
  the component gallery does today. It ships in a store build, not an
  update.
- No work under this spec adds a dependency on it.
- Its removal is a separate change with its own owner approval. Stage G5
  removes the iOS control projection; it does not remove Updates.

When that removal starts, the sections of `rendering.md` and
`coax/native.md` that describe what Updates can deliver stop applying to
native components.

## Bounds

Every capacity is declared per surface and admitted before allocation:
primitives, vertices, text pool, semantic rows, keyed state entries,
realized list rows, images and render targets. The current fixed values
(`EL_MAX` 192, `ARCADE_NODES` 512, `ARCADE_PRIMS` 8192) become per-surface
declarations sized from the first migrated screens. Refusals are counted
and visible to `coax drive`.

Since G0, semantic rows, keyed state entries and a surface's primitives,
text runs, lines, hits and pooled bytes are the caller's arrays, so each
surface declares them. `uiframe_counts` writes a surface's and a store's
counts and refusals as text; a host's drive operation that prints it is
follow-up work. `EL_MAX` remains only as the size of the older `Elements`,
and `ARCADE_NODES` and `ARCADE_PRIMS` remain until `native/scene.coax`
moves onto the keyed store and a surface.

## Measurements

No performance result exists for this design. Record these before a stage
claims a performance outcome, on named devices with build, OS, viewport and
sample counts:

- frame CPU time split into layout, paint and `gpu_draw`, and GPU time;
- input to presented frame;
- idle wakeups on a static screen (the target is none);
- peak application-owned bytes per surface.

Use four workloads on a 3,000-message transcript so results compare with
published GPUI measurements: change one label, hover a row, type one key in
the composer, and write a value equal to the old one. Report cost against
rows on screen and rows in the data.

Initial objectives, to validate and not to advertise: 60 Hz on the lowest
supported phone and 120 Hz where the device offers it, as in
`rendering.md`.

## Not in scope

- A signals or dependency-tracking state model.
- Rust GPUI as a dependency.
- Replacing browser text, selection or accessibility with a canvas.
- Several embedded scene views; `rendering.md` owns that work.
- New wire formats. In-process records here are not wire messages; any
  record that crosses a module boundary, is stored or is sent gets its
  schema and registry row first, under the protocol rules.

## Decisions still open

- Decided in G0: the frame context is `UiFrame` in `kit/frame.coax`.
  Existing `kit/controls.coax` procedures stay as the painters, each control
  gains a `ctx_*` form that takes the context, and `uiframe_over` makes a
  context for callers that hold only a sheet, a measure and a skin. See
  [Frame context](#frame-context).
- Whether the iOS scroll driver is an invisible platform scroll view or
  Coax physics tuned to match (G2; default: the platform's).
- How a migrated screen and a not-yet-migrated screen share app state
  while both runtimes are installed (G1).
- The Linux input-method and accessibility bridges' minimum supported
  desktop environments (G6).
- Which platform shaping calls the atlas uses on Android and Linux (G6,
  G7).

## References

- [Component spec](spec.md): the five layers and the display list.
- [Rendering and composition](rendering.md): motion, effects, native text,
  DOM islands and scene views.
- [Coax Native](../coax/native.md): the app framework, hosts and services.
- [Reuse](../docs/reuse.md): where each shared piece lives.
- Code: [display list](paint/list.coax), [sheet](paint/sheet.coax),
  [GPU backend](backend/gpu.coax), [element table](core/elements.coax),
  [keyed state](core/keyed.coax), [surface](paint/surface.coax),
  [frame context](kit/frame.coax),
  [tree](core/tree.coax), [controls](kit/controls.coax),
  [frames](kit/frames.coax), [native scene](native/scene.coax).

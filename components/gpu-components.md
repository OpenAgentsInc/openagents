# GPU components on native: one kit, one display list

Written 2026-11-15. Status: specification, with stage G0 built (the frame
context, the keyed state store, the semantic table and the surface commit;
see [Stages](#stages)) and stage G1 built and checked in the iOS Simulator
(the chat composer as its own surface, and the login form as a card over
the chat; see [G1 as built](#the-ios-app-grown-screen-by-screen)). G1's physical-phone exit checks
are not done. Tracked in
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

The app set (`components/app/`, [README](app/README.md)) is built on the
frame context: the basics, chat, task, account and navigation components
the native apps' screens use, with a gallery of all of them
(`components/app/gallery/gallery.coax`).

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

On iOS the composer and every login field send the page committed text
only: while `markedTextRange` is set (an input method's provisional text),
the platform editor keeps it and the page keeps the last committed value.
The email field carries `textContentType = .emailAddress` and the code field
`.oneTimeCode`, so AutoFill can offer the address and the code. Paste is the
platform editor's own. Debug builds play edits through the editor that
holds the keyboard with `-OAEdit`, for example
`-OAEdit "press:chat.composer|type:Fix|mark:に|commit:日本|paste:Go |print"`,
and print what the editor and the page each hold on `OA-EDIT` lines.

Secure fields, one-time-code AutoFill and password AutoFill use the
platform's own field for the whole edit, styled from the skin's exports.

Focus follows a press. A press that begins on a text field (a row with role
`EL_FIELD` that is not disabled) gives that field the keyboard; a press
anywhere else, on another control or on nothing, takes the keyboard from
the text field that has it. `uiframe_begin` applies the rule, so every
screen built on the frame gets it: after `uiframe_begin` the app keeps
`uiframe_focus(f)` as its focused ID, `uiframe_focus_moved(f)` says this
press moved it, and the host hides the keyboard while `uiframe_keyboard(f)`
is false. A host whose tap can begin and end between two frames also calls
`uiframe_press_focus(surface, input)` at touch down. Focus on a control the
keyboard stepped to is not a text field's, and a press elsewhere leaves it.

### Liquid Glass controls

On iOS 26 and later, a control that the system draws in Liquid Glass may be a
system view over the GPU view, because the display list cannot sample and
refract what is behind it as the system does. The iOS chat page uses two: the
round menu button (`UIGlassEffect`) and the environment and repository picker
(a SwiftUI popover anchored to its chip), both in
`apps/openagents-ios/GlassChrome.swift`. The page still lays out both and
records their semantic rows and hits; with `chat_native` on it paints
neither. The system view reads what to show from those committed rows (label,
value, state bits) and sends a press back by row ID, so Coax still decides
what each row says and whether it can be chosen. Older iOS gets a material
blur in the same place.

### Accessibility

The host builds platform accessibility elements from the committed semantic
rows: `UIAccessibilityElement` on iOS, `NSAccessibilityElement` on Mac,
virtual nodes on Android and AT-SPI on Linux. The GPU view itself is hidden
from accessibility. Activating an element dispatches the same action as a
tap. A virtualized list can realize a row that accessibility requests.

On iOS one bridge, `apps/shared/native/KitAccess.swift`, does this for any
surface that conforms to `KitAccessSource` (a stamp, the live layer, the row
count, one row, and a press by row ID). The chat page and the dev gallery
use it:

```swift
access = KitAccess(container: view, source: self)
access.stand = { row in row.id == "nav.menu" ? .view(glass.menu) : .own }
access.refresh()   // after each frame; rebuilds only when the stamp or layer changes
```

- Each committed row that is not a region, hidden or empty becomes one
  element. Traits come from its role and state bits; the value carries
  Expanded or Collapsed, Checked, and In progress.
- Reading order is top to bottom in 4-point bands, then left to right.
- A row's label is capped at `EL_LABEL` bytes. Its whole spoken text, such
  as a whole reply, is kept in the generation's sheet pool with
  `uiframe_say` (up to `UIFRAME_SAY` bytes) and read with `surface_say`.
- A row marked `EL_S_LIVE` (an agent's reply) is announced once it is not
  busy and has been still for 0.8 s, and only the part added since the last
  announcement. Rows present when the page first reads, and history that
  arrives at once, are not announced.
- Three-finger scroll pages the scroll area by 0.8 of its height. The escape
  gesture closes the cover: the environment page, the login page, then the
  sidebar.
- A row that a system view stands for (Liquid Glass, the platform editor)
  is that view's element, not a second one.
- Large text: the Dynamic Type size sets the page's text zoom
  (`chat_text_zoom`, 1 to 1.6), and the whole page lays out and draws
  larger. The host rebuilds the glyph atlas at the new scale and steps the
  zoom down until it fits in 8192 by 8192. Past what four subpixel offsets
  fit, a body face keeps one (`Atlas.sub`). On a 3x phone the largest
  accessibility size draws at 1.35 today.

Debug builds print the elements with `-OAAccessDump YES` (between
`OA-ACCESS-BEGIN` and `OA-ACCESS-END`, plus `OA-ANNOUNCE` and
`OA-TEXT-SIZE` lines) when launched with `xcrun simctl launch --console-pty`.

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

| Need | Status | Why |
| --- | --- | --- |
| Images (`PRIM_IMAGE`; `gpu_draws` returns false for it today) | Not started. No iOS screen shows an image yet: avatars are initials | Avatars, attachments, pictures in messages |
| Nested clips | Built through the semantic table's 16-level budget. Overflow suppresses the subtree; surface commit refuses unbalanced, crossed or over-budget clip/opacity scopes | Scroll areas inside panels |
| Affine transforms | Not started. The sidebar and covers move by offset (`prim_move`), which is all they need | Drawers and sheets that scale or rotate |
| Group opacity | Done: `PRIM_ALPHA` (`prim_alpha`, `prim_unalpha`, nested) and `prim_opacity`; `app_enter` fades a message in as it rises. Each primitive takes the opacity; overlaps inside a group are not composited as one layer, which needs a render target | Enter and exit of whole components |
| Blur and composition | Not started. On iOS the blurred controls are Liquid Glass system views | Menus and sheets over content |

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
| G1 (in progress; composer 2026-11-15; VoiceOver bridge, large text and one editing authority 2026-11-16) | The iOS app as one compiled page: the chat page (composer first, then chat logic, transcript and the login card) on the kit, with the text-editor overlay and the VoiceOver bridge. No JavaScript, no Updates, no UIKit control projection (see [The iOS app](#the-ios-app-grown-screen-by-screen)) | The app links no JavaScriptCore or WebKit and holds no `.js`, `.wasm` or `.html`; typing, sending, stopping, scrolling and login work against the account fixture; a still page presents no frames |
| G2 (Settings, account history and platform scroll driver built 2026-11-16; physical scroll qualification pending) | The next screens of the iOS app, one at a time, each on the kit: the sidebar with chat history, then Settings, with the scroll driver | Each screen has no UIKit controls; large text lays out; scrolling feels native on a physical phone |
| G3 (nested clips and per-primitive alpha built; composited group opacity, images, transforms and blur remain) | Backend gaps the chat needs: images, nested clips, transforms, group opacity, blur | Each has a gallery specimen and a generated-shader check on Metal and WebGPU |
| G4 | The rest of iOS, screen by screen: Projects, Usage, environment and repository choice, the world library | Each screen passes its acceptance case on a physical phone, and the transcript workloads under [Measurements](#measurements) are recorded |
| G6 | Desktop on Mac and Linux: move the native environment client's screens from AppKit and GTK controls onto the kit, on the shared AppKit/Metal and Wayland/Vulkan hosts the recovery shell already uses; then delete those projections | Login, history and chat run on both desktops through the kit, with real text entry, keyboard focus and a screen reader |
| G7 | Android on the existing Vulkan host | Real input method, TalkBack and device evidence before any parity claim |

iOS is first because it is the product in focus. The shared pieces built
for it are what desktop and Android reuse.

### The iOS app, grown screen by screen

The iOS app (`apps/openagents-ios`) is one compiled Coax page on one Metal
view. It compiles nothing to JavaScript or WebAssembly and has no Updates
path (owner, 2026-11-15). The earlier app (a JavaScript runtime that
projected a tree into UIKit controls, with worlds, the wallet and Updates)
is deleted from the iOS app; git history is the archive. The update service
is removed. The app keeps its bundle ID, so it updates the installed app in
place.

Status, 2026-11-15: the page is a title over the GPU composer. The composer
takes text through the platform editor, grows with it and stays above the
keyboard. Send does nothing yet. The page makes no chat request and has no
transcript, no login, no chips and no navigation button.

The app grows one screen at a time. A part is added only when it runs on
the compiled path with real data.

| Part | Owner |
| --- | --- |
| Page state, layout, paint and semantic rows | `apps/openagents-ios/chat.coax` |
| Composer | `components/kit/composer.coax` (`ctx_composer`) |
| GPU carrier | `apps/openagents-ios/host.c` on `components/gallery/linux/main.c` (built with `COMPONENTS_NO_GALLERY`) and `apps/shared/host/apple/ui-host.m` |
| Atlas tables for a host without the gallery page | `components/raster/host-tables.coax` |
| Platform editor | `apps/shared/native/KitEditor.swift` |
| Lifecycle, keyboard, VoiceOver | `apps/openagents-ios/App.swift` |

Each frame the host gives the window's facts (`chat_env`) and takes the
vertices (`chat_frame`). While the composer holds the keyboard, one
`UITextView` sits over its text and Coax paints nothing under it. VoiceOver
gets one element per committed semantic row. The display link sleeps while
nothing moves.

Next, in order: the chat logic on `apps/shared/durable-chat.coax` with the
host doing HTTP, the transcript on `components/kit/markdown.coax`, and the
login card (`apps/shared/login/`, which runs as a card on this carrier).

`apps/openagents-ios/check.sh` fails if the built app links JavaScriptCore
or WebKit, holds a `.js`, `.wasm` or `.html` file, or has a symbol of the
removed projection.

The Coax application the earlier iOS app ran (`home/`, `extras/`) is still
the desktop client's program. It lives in `apps/openagents-desktop/home`
and `apps/openagents-desktop/extras`, and `coax/native/generate.sh` now
generates only what the desktop build uses.

Status, 2026-11-16: the chat page, the sidebar, the login page and the dev
gallery read with VoiceOver through `KitAccess`, scale with large text, and
take text through one editing authority (see [Accessibility](#accessibility)
and [Text entry](#text-entry)). The login page's Back is a Liquid Glass
button where the menu button stands. The iOS target compiles no tree-kit
procedure and no UIKit control projection. `apps/shared/environment-app.coax`
still loads `components/kit/frames.coax` through
`apps/shared/app/view/model.coax`, but nothing on the iOS path calls it, so
the generated C has none of it. The only UIKit views are the two
could-not-start labels, the composer's placeholder, the platform editors and
the Liquid Glass controls.

Not checked: a physical phone, VoiceOver by hand on a device, a real input
method's candidate window (the check drives `setMarkedText`), dictation,
AutoFill offering a real address or code, and iPad layouts.

Left for iOS: images, transforms and blur in the backend when a screen needs
them; dropping the `kit/frames.coax` load from the shared app model once the
desktop client stops using `eu_render`; a drive case that reads the
accessibility rows.

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
- Decided in G2: an invisible `UIScrollView` drives native pan and
  deceleration. Coax retains the keyed/list offset, draws all pixels and
  clips paint, presses and accessibility together. Desktop still delivers
  wheel/trackpad deltas.
- How a migrated screen and a not-yet-migrated screen share app state
  beyond the session. G1 shares only the session, through `AppAuth` (see
  [Sharing the session](#sharing-the-session-with-the-javascript-runtime)).
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

## Account identity on the compiled page (2026-11-16)

Settings is a regular GPU page reached from the sidebar. Its reusable
`components/app/identity.coax` draws the public key and confirmation states.
`apps/shared/app/settings/identity.coax` owns public app state and requests a
closed native custody effect. Every signed-in account reads identity-v1,
which provisions older keyless accounts through the existing encrypted
identity service. Existing linked identities retain their authentication
meaning; a managed identity does not upgrade email login to L2.

The active secure platform editor alone holds restore/backup text. No
private text enters the component state, effect outbox, semantic rows or
drive output. Fresh system passkey proof gates backup, rotation and restore;
backup also requests local device authentication. Pending writes stay in
device-only protected storage until exact replay or terminal discard settles
them. The versioned change/state/cancel contracts are in
[account-identity-change-v1](../docs/protocol/account-identity-change-v1.md).

Settings and the full-page login use keyed scroll offsets and nested clips.
The native scroll view supplies momentum; keyboard changes reveal the focused
field. Large text uses the existing `chat_text_zoom` and shared editor
restyling, bounded at 2.5 times the default. The public identity specimen is
also in the app component gallery; it contains no connected service or secret.
The platform does not project labels or buttons into UIKit.

Qualification: the real compiled screen checks cover confirmation, empty
private semantic values, large text, clipped targets, keyboard reveal and
settled motion. Native SDK XCTest checks real compiled crypto with a
controlled HTTP transport, protected-store refusal, account fencing, exact
retry and secret-free discard. The real PostgreSQL API suite exercises key
rotation/restoration, revision ABA refusal and cancellation of delayed
commands. Physical passkey, VoiceOver and scrolling acceptance remains an
owner check. This work does not claim that G3, G4, G6 or G7 is complete.

The account history drawer now uses the existing conversations-v2alpha1
index and the core's `ACT_OPEN`. `apps/shared/app/history/` schedules reads
through the effect outbox while visible, with a single sleeping host timer;
it cancels/fences reads on close or background. Failed reads retain the
previous list. Paging refreshes its current cursor. `app_side_window` lays
out only visible rows, with one-line titles normalized by the shared Unicode
text helper. The sidebar slides at its full width, so animation never changes
row wrapping. Paired-computer session history is still a separate G4 consumer;
this list is explicitly named Account chats.

### Desktop accessibility foundation (2026-11-16)

The Apple GPU host accepts a typed `UiElement` source, copied from the
surface's committed rows. The phone page's Mac preview uses it for all
fourteen semantic roles, state, clipped geometry and activation by stable
ID. Invalid or over-capacity input retains the previous accessible frame.
Tab and Space use the same rows and actions; a focused row's ring follows
its committed bounds. The older recovery client's string-tree reader is
retained only for clients that have not installed the typed source.

`apps/shared/host/apple/tests/access.coax` qualifies the real AppKit bridge
offscreen: roles, private-field metadata, disabled actions, direct
activation, focus, capacity refusal and retirement. The phone page's preview
also drives Settings and reads its actual accessibility rows. This is a
host foundation, not completion of G6: the desktop environment application
still uses its old projection, Linux still needs AT-SPI, and physical screen
reader acceptance remains open.

The desktop SDK qualification target in `apps/openagents-desktop/native/gpu/`
now links the same compiled page/core and `AppRuntime` as iOS. Its hidden
AppKit window verifies Settings/login, actual text insertion, provisional
composition, committed text, replacing the virtual field with the active
platform editor, and detaching it on blur. It opens no normal window and
uses an isolated account/preferences store. The target refuses ordinary
launch; it is not yet the replacement desktop application. Its README lists
the local-service, persistence and migration exits before that switch.

Apple's shared native service wiring is in `apps/shared/native/AppRuntime.swift`
and the core ABI in `AppCoreC.swift`. Passkeys use `ASPresentationAnchor` on
both Apple targets; iOS supplies its screen-capture fact to the identity
custody adapter. `KitEditorMac.swift` uses AppKit's own active text system,
with no second application edit buffer. The Apple host can take an external
display clock so it does not wait or drain events inside a display-link
callback. This does not change the older recovery clients' polling contract.

The same four identity SDK XCTest cases run on the standalone Mac runner
(`apps/shared/native/identity-check.coax`), without Simulator, network or a
real private store. The iOS device SDK build and package check pass; the
simulator runner also needs a successful test-manager connection before its
results can be claimed. Earlier successful iOS test results remain recorded
with the Settings release.

Desktop view persistence is now a separate public record,
[`openagents.account.native-view.v1`](../docs/protocol/account-native-view-v1.md).
The one Coax codec preflights the whole record before returning an email
draft, opaque rejoin reference or existing environment-choice text. The
desktop carrier writes on change through `AppFileViewStore` under
`~/.openagents/desktop/`; its qualification target uses memory instead.
Credentials and pending key changes never enter that record.

Mac identity custody now uses separate owner-only pending files, created
exclusively and flushed before a private command can be sent. A restarted
SDK replays the original bytes; the strict Coax command codec checks both
retry and cancellation before transport. Seven standalone XCTest cases
cover the original identity cases, file-backed unknown/restart/exact replay,
atomic view state, existing-folder permissions, exclusive writes and symlink
refusal. These checks do not replace a real passkey or screen-reader ceremony.

Linux now has an [ATK SDK projection](../apps/shared/host/linux/ACCESSIBILITY.md)
of the same committed rows. Its real SDK queries and retained-object
retirement pass ordinary and ASan/UBSan/LSan checks on a Linux build host.
Its AT-SPI registration now passes a separate actual D-Bus client's discovery,
redacted-field and activation queries. The Vulkan host's integration compiles
against its generated Wayland and actual Vulkan/ATK SDKs. Active text/focus,
the production OpenAgents Linux client and an Orca ceremony remain separate
gates; G6 is still open.

The actual shared page/core now also has a hidden Linux Vulkan qualification
target. Ordinary and ASan/UBSan/LSan builds render Settings/login and pass
committed-row activation, text synchronization, bounded animation settling
and blur. Their PNGs match exactly on Lavapipe; this is not an IME or physical
GPU performance claim. See the [record and picture](../docs/verification/gpu-native-13645/README.md).

The desktop SDK now has `AppMacDevice`: asynchronous discovery and bounded
process carriage through the installed `openagents-environment key` and
`local-rpc --request-file` commands. The existing Coax CLI retains request and
reply codecs and kernel-peer authentication. Remote calls still use
`AppDeviceLink`; a failed local call never switches to a different computer.
The SDK retains failed/unknown outcomes, keeps request files owner-only and
removes them after the bounded call. Ten Mac XCTest cases now include this
adapter and the real process deadline/output limits. A separate effect-bridge
check proves discovery cancellation, duplicate and retirement fencing. This
is SDK qualification, not proof of a real resident-service chat or full G6.

# Aethyr HUD library: reference study and reusable demo

Date: 2026-11-01. Scope: the existing Grid visual language, the requested
Expanse image collection, a callable Coax component library, and five local
demonstration compositions. This record explains the design and reuse
boundary. The implementation's final check results belong in its separate
verification record.

The implementation is [aethyr/ui/hud.coax](../../aethyr/ui/hud.coax),
[aethyr/ui/draw.coax](../../aethyr/ui/draw.coax), and the standalone
[HUD demo](../../apps/openagents-web/demos/hud/README.md). All new application
and build logic is Coax. The demo uses the existing browser host, C buffer
adapter, font atlas and Coax-authored projects shader.

## Evidence and limits

The requested [HUDS+GUIS article](https://www.hudsandguis.com/home/2021/theexpanse) was read, and all 137 linked gallery stills were inspected in five contact sheets. Twelve selected stills were inspected again at larger resolution. This is visual reverse engineering of published reference imagery, not access to the show's UI code. Stills establish appearance, composition and displayed states. They do not establish hit regions, animation curves, timing, touch handling, safety behavior or physical accuracy.

The article groups its gallery into curved displays (13 images), holographic displays (11), mobile devices (34), touchscreens (22), miscellaneous displays (32), followed by Agatha King (6), Behemoth (4), Razorback (6) and Rocinante (9). Its interview with designer Rhys Yorke describes reusable ship/set style guides, practical interfaces for actors, and storytelling as a constraint. These are useful production principles for a component library.

Two primary artist sources reinforce this method. [Yorke's Rocinante portfolio](https://rhys.artstation.com/projects/q9Am1L) describes establishing a guide from existing designs and a common palette. [Yorke's Razorback HUD portfolio](https://rhys.artstation.com/projects/q9VKgD) explains using a 3D mockup to communicate how interface elements repositioned in the curved HUD. Neither source implies that a flat demo must implement real holography.

## Existing HUD and the books

The current implementation already supplies reusable parts:

| Source | Existing responsibility | Consequence for this library |
| --- | --- | --- |
| [verse/hud.coax](../../verse/hud.coax) | Logical module `verse.hud`, exported packet types and HUD procedures; safe-area-aware hotbar slots | Preserve existing consumers and their geometry. A new instrument library does not need to replace the hotbar. |
| [verse/hud/frame.coax](../../verse/hud/frame.coax) | Six-edge outline, 45-degree cuts at top right/bottom left, bright arms at the other corners, shared greys and cyan | Load the original geometry and palette instead of approximating a second frame. |
| [newland.coax](../../apps/openagents-web/newland.coax) | The current interactive Grid homepage, project labels, lists, frame drawing, stroke output and live observation presentation | Keep the world primary; use its twelve-float vertex convention and preserve its verified-data boundary. |
| [font.coax](../../apps/openagents-web/font.coax) and [font specification](../../coax/font.md) | Filament stroke text, advance widths, small-text treatment and the system sans atlas path | Reuse text layout and rendering. Do not add a second font system or rasterize labels in the application host. |

The frame's source values are white `0.94`, light grey `0.78`, mid grey
`0.60`, dim grey `0.42`, and cyan `(0.216, 0.851, 0.851)`. The faint outline
has light `0.30`; the maximum bracket arm is 22 points and its short side is
60 percent of that. Frame cuts are 7, 10 or 12 points for buttons, panels
or cards. These are authored style constants, not measured contrast ratios.

The books set a larger destination than this demo:

- [Aethyr 15: UI and the HUD](../../aethyr/book/ch015-ui-and-the-hud.md)
  is partly implemented at the visual component foundation level. The new
  `HudBox`/`HudControl`, shared hit geometry, focus helper and typed display
  properties are that foundation. The full screen-space tree, safe areas,
  typed-step actions, semantic roles/states, platform accessibility mirror
  and complete keyboard/screen-reader acceptance journey remain a design
  draft. Eighteen drawing functions and a demo action array do not complete
  that larger contract.
- [Aethyr 11: the renderer](../../aethyr/book/ch011-the-wgsl-renderer.md)
  plans shared frame, pass and binding admission over the existing shader
  and native renderer paths, plus a qualified CPU reference. This library
  submits to the existing projects path. It does not implement the common
  admitted pass graph, a new GPU backend or full CPU/GPU raster parity.
- [Aethyr 86: Everyone can play](../../aethyr/book/ch086-everyone-can-play.md)
  is a design draft for remapping, text scale, contrast, captions, stable
  text IDs, language support and reduced-motion behavior. The demo consumes
  the host's reduced-motion input and supplies keyboard controls and hidden
  explanatory text. Dynamic screen-reader state, general text scaling,
  remapping, caption infrastructure and translation still need that work.
- [Coax 10: organizing a larger program](../../coax/book/ch10-organizing-programs.md)
  establishes `#load` composition, canonical declaration identity, optional
  logical modules, exports, and source-owned checks. The new drawing file
  loads the foundation, existing frame and font directly. The new files
  currently use the unannotated shared declaration scope; they are not a
  versioned package or a completed private/exported module API.

The local demo is intentionally separate from the homepage: synthetic
contacts, readings, receipts-like labels and traces cannot be presented as
verified network observations. It changes no admission, money or protocol
contract.

### What the user's screenshot contributes

The OpenAgents image already has a strong identity: almost black ground, continuous isometric grid, white wireframe project islands, clipped corners and bright corner arms, extended uppercase headings, sparse teal accents, and activity/XP/orbit panels arranged around the outside of the world. The world remains the primary content. This should be the base, rather than replacing it with a generic saturated spaceship console.

The extension opportunity is stronger information hierarchy inside those frames, more kinds of instrumentation, a consistent family of actions and lists, and several compositions for different jobs. Retain the black/white palette by default and use teal to mean selection or a live local demo state. Give warnings text, shapes and inversion so meaning survives monochrome.

## Observed interface families

### 1. Curved bridge and tactical consoles

Gallery images 1–13 show wide dark displays whose center carries the main object, image or map. Thin angular borders and brackets isolate the main view; side columns carry readouts, tables and small plots. Some views are red/orange; another is pale blue. This is evidence of multiple display treatments, not enough to infer a universal faction palette.

**Borrow:** a large central viewport, slim instrument rails, stable section headers, a low command rail and bounded secondary widgets. A frame's border should help orient the viewer without becoming brighter than the data.

**Components:** view frame, section label, data rail, heading/scale tape, corner target bracket, readout, miniature waveform and state chip.

### 2. Holographic spatial maps

Images 14–24 show orbital paths, depth cues, labeled bodies, sparse points and floating information cards. Image 17 is especially legible: orbit arcs and a field of small reference points make a spatial relationship visible; a small identity/comms card sits at the base rather than covering the map. Images 21–23 show circular tactical geometry with a center reference and perimeter data.

**Borrow:** orbital overview, concentric range rings, selected body, projected paths, a crosshair, a faint coordinate lattice and a leader-line label. The grid's project islands could become contacts without acquiring spaceship fiction in production.

**Suggested consumer semantics:** selecting a contact in a list selects the same contact in the plot; changing range changes plot scale; pause freezes demo time. Any mock trajectory must be labeled illustrative, not an orbital solver.

### 3. Handheld glass and personal devices

Images 25–58 vary substantially. Transparent portraits, messages, access panels, diagnostic panes and large landscape tools recur. Image 33 has a clipped double outline with a main portrait and a narrow right rail of icons/status facts. Image 42 uses one main imaging surface, left action list, top-right keypad-like selectors and two small histograms. Image 46 combines an anatomical diagram with prose and small side panes. The transparent effect is primarily edge emphasis, sparse strokes and uncluttered content regions.

**Borrow:** a narrow inspector, folded identity card, message panel, compact diagnostics card, icon rail and context actions. Do not literally mirror illegible back-facing text from photographed clear props. Put a dark translucent backing behind important labels over the busy grid.

**Suggested consumer semantics:** compact mode uses one primary card at a time, with explicit tabs; a contact card reflects selection; a message can be marked read; opacity is a view preference, never a disappearance of the only control.

### 4. Touch workstations, kiosks and access terminals

Images 59–80 include media viewers, large lists, maps, keypads and airlock controls. Image 75 has a multi-column selection list over a map, persistent top/bottom action rows and one highlighted row. Image 78 shows labeled OPEN/CLOSE areas, pressure and oxygen readouts, a prominent status/INITIATE region and a keyboard. Images 72–74 show finger and glove operation on dark surfaces.

**Borrow:** large labeled command tiles, list selection, bounded meter, numeric readout with unit, a step/status panel, keypad specimen and clear confirmation/acknowledgment affordance. Screenshot geometry does not prove which controls are safe or how the airlock state machine works; the demo should use harmless actions such as scan, pause, select or acknowledge.

### 5. Engineering and physiological instrumentation

The miscellaneous collection includes stacked green traces and small arc gauges (86, 91, 94), body/medical scans (95, 102), an environmental bar dashboard (106), public information boards and projected table cards (107). The repeated pattern is a header, compact summary instruments, several aligned traces, and small status/data regions below.

**Borrow:** telemetry stack, sparkline, waveform, segmented horizontal meter, radial progress, status matrix, threshold mark and selected row. Histories need one consistent time direction; numeric units and an explicit range matter more than ornamental ticks.

**Suggested consumer semantics:** demo signal and segment fill share one state value; pause stops both; changing the local level changes the trace amplitude; fault/nominal state changes text and shape as well as brightness.

### 6. Agatha King production art

Image 113 shows a SYSTEM MONITOR with several distinct views of related information: circuit connection diagrams, a deck-by-system cell matrix, a longitudinal outline with callouts, and dense status text. Image 114 has a left control/list region and a technical viewport containing several orthographic views of a mechanical assembly. Image 115 centers a radar-like disc with readouts; image 116 gives a large comms video region and status; image 118 combines a channel diagram, terminal text, outgoing/incoming message lists and a conspicuous incoming-message card.

**Borrow:** coordinated instruments, status matrix, circuit diagram, technical schematic, message queue and comms card. Most useful for Aethyr: several views tied to the same selected entity/resource, rather than independent decorative widgets.

### 7. Behemoth production art

Images 119–122 show a shared control room with distributed stations and a broad table-like central surface. Small text cannot be read reliably in these scene shots. The evidence supports a composition distinction: many operators, shared overview, local side workstations, and different screen proportions.

**Borrow:** operations scene with a shared overview and subordinate panels. Do not claim exact component semantics or a unique interaction model from these wider set shots.

### 8. Razorback production art

Images 123–128 show curved, transparent screens around seated occupants. Image 124 includes a large circular body/orbit plot, thin coordinate lattice, very bright perimeter arcs and small attached panels. Image 126 places a detailed local card beside the person's face; image 128 combines broad mapping and a closer task card. The visible content is layered in depth without needing a single rectangular dashboard to hold everything.

**Borrow:** arc instruments and lightweight floating callouts around one primary plot. On a flat canvas, use curved outlines, radial guides and a few attached cards to suggest the relationship; preserve planar text and pointer hit areas. Actual 3D panel surfaces, stereo depth and gaze interaction remain separate future work.

### 9. Rocinante production art

Images 129–137 show working consoles with task-specific layouts. Image 133 is particularly useful: a left list of remote systems, short health indicators, a large named alert with explanatory text, connection status, and three terminal/log panes across the bottom. Image 135 is a large touch workstation with system diagrams and an alert. Image 136 is a NAVIGATION PANEL: a square coordinate lattice, concentric rings, a highlighted curved route, labeled bodies, and narrow side rails of attitude/sensor instruments. Image 137 splits an upper map from a lower touch control surface.

**Borrow:** inspectable system list, readable alert state, connection/session card, structured terminal/log, navigation plot and a lower command rail. Small terse status text earns its place by describing one system. Repeated fake console output should not substitute for working controls.

## Perceptual hierarchy, density, type and motion

The reference images repeatedly allocate a large region to the task and
smaller regions to its evidence. That relationship is more useful than any
particular spaceship silhouette. In the Grid composition, the world occupies
the center; narrow rails hold capacity, contacts, signal and attention.
Navigation substitutes a local plot. Engineering substitutes a system map.
Communications substitutes a terminal. Their outer grammar remains stable.

There are four useful visual levels: dim coordinate/construction lines,
secondary labels and separators, bright primary geometry/text, and selected
or focused emphasis. The existing frame arms already express that hierarchy.
The new components preserve it. Default monochrome keeps the screenshot's
identity; the optional cyan mode emphasizes selection and active geometry.
The demo does not implement a faction palette or an amber theme. A change
of status also needs words or shape, so future production meaning cannot
depend on cyan alone.

Density is adjustable by composition before changing type size. The catalog
uses one column below 700 logical points, two from 700, and three from 1100,
with 24-point outer margins, 18-point gaps, 224-point cells and 242-point row
pitch. Scenes use one scrollable column below 1000 points. Wider scenes use
a primary surface between two rails. The central display is allowed to keep
air around its content; the library is not a mandate to fill every gap with
small numbers.

The demo's current sizes are a compact instrumentation specimen: metadata
around 7–8 logical points, widget labels around 9–12, and primary titles or
values around 16–26. Small text uses the existing system sans atlas when
`font.coax`'s threshold selects it; larger chrome uses Filament strokes.
That hybrid preserves the screenshot's geometric titles while improving
dense labels. It is not a completed accessible text-scale policy. Future
production layouts should promote actionable text before copying this dense
catalog scale to a phone, and require real long-label/large-text checks.

Motion is a reading aid: orbit markers, signal traces and alignment markers
move around a stable reference. The demonstration uses a local clock and
trigonometric drawing geometry. FREEZE/Space stops that clock; the generic
host's reduced-motion input prevents its advance. No audio is generated.
These are demonstration animations, not physical simulation, instrument
measurements or evidence that the film used the same timing. A future
attention pulse should be brief and subordinate to a persistent text state;
continuous flashing is unnecessary for this style.

## Eighteen callable visual components

All functions live in [draw.coax](../../aethyr/ui/draw.coax), with
`hd_component` dispatching the catalog's zero-based index. Each writes into a
caller-owned vertex slice and returns the next vertex index. The `time` and
`value` arguments are demonstration inputs; several functions keep fixture
labels and readings or reserve an argument. Consumer-supplied typed display
properties are available through the shared helpers below. The eighteen
catalog entries are callable rendering components, not eighteen complete
live-data models.

| Index | Function | What is drawn | Reference relationship and reuse boundary |
| --- | --- | --- | --- |
| 0 | `hd_orbits` | Five elliptical paths, moving diamonds, cross axes and a center reference | Holographic maps and Razorback navigation. Illustrative screen geometry, not gravity, ephemerides or a route solver. |
| 1 | `hd_telemetry` | Two traces on an aligned time/value lattice | Engineering instrumentation. Synthetic waves; replace their sample producer to display real history. |
| 2 | `hd_target` | Broken acquisition ring, cross ticks, center marker and range label | Tactical center-view brackets. Does not acquire, predict or authorize a target. |
| 3 | `hd_attitude` | Pitch ladder, central aircraft-like reference and numeric labels | Flight instrument grammar. Fixture pitch/roll, not a quaternion-to-attitude instrument. |
| 4 | `hd_meter` | Three labeled segmented banks | Capacity and engineering dashboards. Its catalog wrapper uses fixtures; `hd_meters` accepts consumer-supplied labels, fractions and readouts. |
| 5 | `hd_schematic` | Core outline, six branches, node markers and labels | Agatha King system/circuit displays. Fixed demonstration topology, not a graph layout engine. |
| 6 | `hd_contacts` | Four named rows, ranges, markers and separators | Tactical registers and list workstations. The catalog wrapper uses fixtures; `hd_rows` accepts consumer-supplied rows and emphasis. The Grid scene binds its rows to the same contact selection as the world. |
| 7 | `hd_waveform` | Envelope-shaped vertical signal strokes and channel metadata | Communications and telemetry. Generated display signal, not captured microphone audio. |
| 8 | `hd_terminal` | Six diagnostic/prompt lines | Crew-facing terminals and message consoles. The wrapper is a specimen; `hd_terminal_lines` accepts a supplied transcript. Neither is an interpreter or verification receipt. |
| 9 | `hd_timeline` | Four waypoints, progress line and ETA | Navigation sequence and operational progress. Supplied fraction affects the line; labels and ETA are illustrative. |
| 10 | `hd_alert` | Framed advisory, exclamation diamond, explanation and status | Rocinante system-alert composition. `hd_advisory` accepts a supplied title/detail and active state; the demo owns its acknowledgment action. |
| 11 | `hd_commands` | Six large labeled command tiles | Touch workstations and access controls. Tile specimen; drawn labels do not themselves execute commands. |
| 12 | `hd_spectrum` | Rows of intensity-coded frequency bands | Sensor/engineering density plots. Procedural pattern, not a Fourier transform or spectrum measurement. |
| 13 | `hd_heading` | Curved guide, major/minor heading ticks, center marker and value | Flight/navigation instrument rail. Fixture bearing, not a calibrated compass. |
| 14 | `hd_docking` | Nested approach boxes, moving marker and relative-axis labels | Tactical alignment and close maneuvering grammar. No collision or docking decision is implemented. |
| 15 | `hd_manifest` | Resource rows, amounts and separators | Engineering/logistics list hierarchy. Uses `hd_rows` with fixture amounts; no accounting or resource admission. |
| 16 | `hd_hand_terminal` | Clipped incoming-message card with title, text and read cue | Transparent handheld personal/task displays. `hd_message` accepts consumer-supplied content; the demo derives its read state from local command selection. No network inbox or video surface. |
| 17 | `hd_beacon` | Elliptical base rings, vertical stem and attached leader label | Grid-compatible world annotations. A screen-space specimen; a realm still supplies its real camera projection and anchor. |

The lower-level reusable drawing vocabulary is `hd_line`, `hd_text`,
`hd_text_fit`, `hd_panel`, `hd_ring`, `hd_diamond` and `hd_rect`.
`hd_text_fit` uses the shared font metrics and appends an ellipsis when a
string exceeds its supplied column width. `hud_segment` clamps a
fraction to a whole lit-segment count. `hud_arc_point` defines elliptical
coordinates in clockwise, y-down radians. `HudBox`, `HudCoord` and
`HudControl` form the small layout/input vocabulary.

## Five demo compositions and their designed semantics

These semantics are new design choices for the demo. A still image does
not establish the show's corresponding behavior.

| Scene | Composition | Working demo interaction |
| --- | --- | --- |
| 1 / Components | Scrollable catalog of all eighteen specimens | Open a component inspector; close with its button or Escape. The inspector removes background controls from pointer/focus activation. |
| 2 / Grid command | Original isometric project-island drawing surrounded by capacity, contacts, signal and attention | Cycle the selected project/contact; the world and supplied contact rows emphasize the same selection. Acknowledge or reopen the demo event. The Grid geometry is synthetic, separate from the live homepage. |
| 3 / Flight navigation | Central orbital plot with labeled local contacts, attitude, heading, approach alignment and route sequence | Arm/disarm the local route preview: its line overlay, route status and timeline progress change together. Cycle the displayed 1X/2X/4X range mode to change the contact overlay's scale. This is illustrative drawing, not a calibrated orbital solver or physical maneuver. |
| 4 / Engineering | System map, telemetry, capacity, resource manifest, selected subsystem matrix and advisory | Cycle selection to emphasize the matching subsystem labels and matrix row. Acknowledge/reopen the event to change its status, supplied demonstration value and advisory-related cell emphasis. |
| 5 / Communications | Selected crew-link card, command transcript, waveform, spectrum, handheld message and six command tiles | SCAN, TRACK, ROUTE, FOCUS, HOLD or CLEAR selects a tile, changes the local transcript, and marks the incoming message read. Cycle the local contact to update the crew-link title. These are local command-feedback states; no external messages, media or microphone are used. |

Global controls select scenes 1–5, toggle MONO/CYAN with M, freeze/resume
with Space, and reset local time, event/route state and selection with R.
Tab/Shift Tab traverse the published actionable controls; Enter activates
the focused one. Wheel, touch drag and page movement scroll content.
Pointer activation requires release over the same control that was pressed;
a drag does not activate it. The demo's small controls array is rebuilt from
the drawn boxes. `hud_hit` uses half-open rectangles and gives the last
eligible overlapping control ownership. Empty labels, disabled entries and
nonpositive action IDs do not activate.

The catalog's `hd_commands` tile drawing remains a visual specimen. The
communications composition builds real controls with the same six labels
through `demo_commands`; each receives its own `HudControl` and transition.
Selecting CLEAR changes its transcript line and selection; the label does
not imply a general simulation reset. Contact-register rows and terminal
prompts are displays, while the explicit NEXT buttons change selection.
The handheld read cue reflects command state rather than exposing a second
independent read action. Drawing, data and action ownership stay explicit.

## Reusing the library

### Screen coordinates and vertex ABI

`HudBox` coordinates, text baselines/cap heights, radii and `HdPen.top` /
`bottom` are logical screen points: CSS pixels in the browser, x right and
y down. `HdPen.d` is device pixels per logical point. Multiply exactly once
at the output boundary; the component writer does that. Pointer coordinates
from the existing projects face are divided by the same factor before hit
testing. Scroll translation belongs to the caller and must be applied to
both drawing and controls.

`out` is a slice of `f32` values. `at` and the returned count are vertex
indices, not float offsets or bytes. Every vertex has twelve floats, in the
same order as `newland.coax` and the projects shader:

| Offset | Field |
| --- | --- |
| 0–1 | Device-space x, y |
| 2–3 | Along-stroke and across-stroke coordinates |
| 4 | Stroke length |
| 5 | Core radius |
| 6 | Light |
| 7 | Glow radius |
| 8–10 | Red, green, blue |
| 11 | Rendering mode; the shared font also uses the existing sans mode |

A line expands into two triangles/six vertices. `lit` refuses a stroke that
cannot fit all six vertices, is effectively dark or has zero length.
Components can therefore return partial geometry when the buffer fills;
the API does not return an atomic whole-component admission receipt. Budget
enough vertices for the composition and record final counts at the consumer.
`HdPen.top`/`bottom` clip line centerlines and suppress text outside the
vertical strip; they are not a general rectangular scissor or per-panel
layout engine.

For a root-level Coax entry, the basic calling pattern is:

```jai
#load "aethyr/ui/draw.coax";

draw_example :: (out:[]f32,device_scale:f32,time:f32)->int {
  pen:=HdPen.{d=device_scale,cyan=false,top=0,bottom=900};
  box:=HudBox.{x=24,y=180,width=360,height=224};
  n:=hd_panel(out,0,box,pen,false);
  return hd_telemetry(out,n,
    HudBox.{x=42,y=232,width=324,height=140},pen,time);
}
```

The caller supplies at least `returned_vertices * 12` floats of storage,
the existing shader/atlas bindings, and its frame budget. Load paths resolve
relative to the file containing them, so a nested entry adjusts the path.
The example is a composition pattern, not a standalone executable.

### Data, actions and platform portability

The typed display-property APIs are available independently of the catalog:

| Property/input | Drawing helper | Contract |
| --- | --- | --- |
| `[]HdRow`, each with `label:string`, `value:string`, `status:int=1` | `hd_rows(out, at, rows, box, pen)` | Fit labels/readouts into two columns; `status` chooses an ink kind and marker emphasis. It is not a health, admission or authority verdict. |
| `[]HdMeter`, each with `label:string`, `value:f32`, `readout:string` | `hd_meters(out, at, meters, box, pen)` | Draw each label/readout and a 24-segment bank. `hud_segment` bounds the lit count; the caller keeps its display string and numeric value consistent. |
| `[]string` | `hd_terminal_lines(out, at, lines, box, pen)` | Render a supplied transcript with fitted lines. No parsing, execution or verification is performed. |
| `HdMessage`, with `channel`, `title`, `line1`, `line2`, `footer` strings | `hd_message(out, at, box, pen, message)` | Draw a supplied card. The caller owns message lifecycle, unread/read state and any actions. |
| `HdAlert`, with `title:string`, `detail:string`, `active:bool` | `hd_advisory(out, at, box, pen, alert)` | Render active/ACK-required or acknowledged/monitoring text and emphasis from the supplied state. The caller owns the condition and acknowledgment decision. |

These helpers borrow caller-supplied slices/strings for the draw call and
write only into the output slice. They do not retain a session, fetch data,
decide policy or register actions. `HdPen` and `HudBox` have the same logical
coordinate contract as the specimen functions. Empty row/meter/transcript
slices draw nothing. Row pitch derives from the box height and input count,
so the consumer still budgets readable density and suitable box dimensions.

The foundation's geometry and action selection are pure Coax. Drawing uses
the existing WebGPU stroke/sans convention. A native consumer can reuse the
geometry and controls, but must bind compatible buffers, shader modes and
font resources through its platform adapter. This record does not qualify
native rendering merely because the frame API was already shared with iOS.

The standalone demo reuses `projects-face.c` and the generic host unchanged.
The Coax build tool emits the local canvas shell and hidden text twin and
generates the existing projects shader from Coax. No new host-side product
logic, CSS overlay UI, third-party UI runtime or film bitmap is required.

A production consumer can already supply rows, meter readings, transcript
lines, messages and advisories through these typed properties. Components
with procedural specimen content still need actual entity/sample inputs
when adopted for live instrumentation. A consumer should publish controls
from the same rectangles, map actions
into the realm's typed-input path, and attach the semantics planned in
chapter 15. Physical calculations and authorization stay in their owning
engine/policy units; drawing a gauge, capacity label or command tile grants
no authority. Preserve the distinction between stale, unknown, refused and
healthy data rather than drawing an unavailable reading as zero.

Future work is broader typed instrumentation and semantic APIs, shared pass admission,
accessible text scaling, complete dynamic accessibility mirrors and the
book's scripted keyboard/screen-reader journey. None is inferred from this
visual demo. The existing homepage and realm HUDs can adopt components
incrementally after those consumers supply and check their real state.

## Exact representative image evidence

The gallery was inventoried in article order, starting with its first
curved-display image as 1; the article's header illustration is excluded.
All 137 linked stills were inspected at contact-sheet scale. The twelve
larger detail inspections were 17, 33, 42, 75, 78, 86, 113, 114, 118, 124,
133 and 136. The following twelve links preserve representative evidence
across the families; the analysis above identifies what each establishes.
The downloaded study sheets are temporary research aids. No external image
is copied into the repository or bundled with the demo.

| Gallery image | Visual evidence |
| --- | --- |
| 1 | [Curved console: center technical view and instrument rails](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619159152902-YFCALLEG69R19FFE4JRB/dR4fjn8.png) |
| 17 | [Holographic map: paths, reference points and attached identity card](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619159417028-EDV4BAYEN73K484XUBYW/PFu0YCZ.png) |
| 42 | [Handheld: primary imaging area, action rail and histograms](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619159596304-CG0UXZSYRGQ2JZ5CUEEN/mTihm5D.png) |
| 75 | [Touch workstation: selection list, map and persistent actions](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619159632339-7F121LP422AT0KVB2QEM/Jx0EGt6.png) |
| 78 | [Airlock touchscreen: large actions and pressure/status regions](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619159655110-ADPVEK0HTLH3PBCF4YXA/Vho0lAu.png) |
| 86 | [Engineering: aligned traces and compact arc instruments](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619159821555-MLVP2IQN2MCR710FJ8J0/4PUYsWT.png) |
| 113 | [Agatha King: circuit paths, system matrix and hull callouts](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619419778638-NQ84IOU16QD0GV554FT2/rhys-yorke-ex-agathaking-01.jpg) |
| 118 | [Agatha King: channel diagram, terminal and message queues](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619419786201-JXQDXOHSRYUEJ910A7Z3/rhys-yorke-ex-agathaking-06.jpg) |
| 121 | [Behemoth: shared central surface and distributed operators](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619419818243-JR8Z4K50KPM6CRFB6X20/rhys-yorke-ex-behemoth-03.jpg) |
| 124 | [Razorback: curved orbital HUD and attached instrument cards](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619419864057-M7WMWA4EIZ3QG7LWOHFZ/rhys-yorke-ex-razorback-02.jpg) |
| 133 | [Rocinante: systems list, connection state, alert and logs](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619419899428-0Y8TPYA4Q08NTLMUYM6O/rhys-yorke-ex-rocinante-05.jpg) |
| 136 | [Rocinante: navigation lattice, route, rings and side instruments](https://images.squarespace-cdn.com/content/v1/5438bd1be4b0446f6692b906/1619419901710-RA675O577UNY3OZ3JVS8/rhys-yorke-ex-rocinante-09.jpg) |

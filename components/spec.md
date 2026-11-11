# Components: one component system for every OpenAgents and Verse app (spec, draft)

Written 2026-11-10. Status: proposal. Nothing here is built. The code this
replaces is listed under "What exists today".

## The goal

One set of UI components, written once in Coax, used by every app: chat on
openagents.com, OpenAgents for desktop, Everglade for desktop (the
Golemancers' Workshop), Aethyrforge, Coder's terminal and the phone apps.

- A component is written once. Markdown is parsed in one file, text is
  wrapped in one file, a transcript is laid out in one file.
- A component doesn't know where it is drawn. The same transcript draws on
  Metal, WebGPU, Vulkan or a terminal.
- A component doesn't know how it looks. Zeron Dark, the Verse's instrument
  look and Everglade's iron and gold are skins over the same component.

## Where it lives

- **`components/`** at the repository root: the library. It sits beside
  `coax/`, `aethyr/` and `psionic/` because apps, realms and tools all use it,
  and `apps/shared/` reads as belonging to the apps.
- **openagents.com/components**: the component library's page. It shows
  every component in every skin, live, drawn by the library itself. It is
  also what the picture checks shoot.

```
components/
  spec.md          this file
  README.md        how to use a component and add one
  core/            what a component is: state and rules, no pixels
  layout/          sizes and positions from state, a width and a skin
  paint/           the display list: the primitives a frame is made of
  skin/            tokens and frames: zeron.coax, verse.coax, everglade.coax, terminal.coax
  backend/         display list to a target: gpu.coax (and its shader), terminal.coax
  kit/             the components themselves, one file each
  gallery/         the /components page and the picture checks
  fonts/, icons/   moved from apps/shared/zeron/, with their licences and NOTICE
```

## What exists today

Three drawing stacks and four Markdown implementations:

| Code | Draws | Problem |
| --- | --- | --- |
| `apps/shared/zeron/` (gpui, composer, markdown, tools, transcript, picker, attachments; about 5,700 lines) | Chat on the web and the Mac app | Parsing, layout, Zeron's colours and vertex output are in the same functions. `markdown.coax` loads `composer.coax` to get the atlas |
| `aethyr/ui/` (hud, draw, tree, feed, hud-style, arwes; 4,130 lines) | Realm HUDs | A second vertex format (12 floats, strokes) and its own text. It has the right ideas: `hud-style.coax` separates tokens from drawers, and `tree.coax` is one table for drawing, hit tests, focus and accessibility |
| `everglade/hud/`, `apps/everglade-desktop/text.coax`, `golem-board.coax` | Everglade's HUD and the Workshop's boards | A third text path with its own atlas layout |
| `coax/lib/tui/tui.coax` | Coder's terminal | Its own Markdown and composer |
| `apps/openagents-web/chat-hud.coax`, `apps/openagents-cli/project-page.coax` | The old HUD chat, a CLI page | Two more Markdown parsers |

## The five layers

Each layer loads only the layers above it in this list. A check enforces it
(see "Checks").

### 1. Core: state and rules

Plain data and functions. No colours, no pixels, no fonts, no platform calls.

- **Text**: UTF-8, grapheme clusters, the edit buffer (selection, undo,
  marked text for an IME, UTF-16 offsets for hosts that need them).
- **Markdown**: source to a document (`MdDoc`: blocks, items, styled runs,
  tables, code with its language). Streaming-safe: an unfinished reply
  parses.
- **Conversation**: a transcript as parts (text, thinking, tool call, tool
  result, diff, notice, approval). This is the session record of the apps
  roadmap, Step 2 ([docs/apps/2026-11-10-apps-roadmap.md](../docs/apps/2026-11-10-apps-roadmap.md)):
  engines write it, components show it. A component never talks to an engine.
- **The tree**: every element on screen in one table, with its role, label,
  state and action. Drawing, hit tests, focus order and accessibility read
  the same rows. This is `aethyr/ui/tree.coax`, moved.
- **Actions**: a component never calls the host. It writes what it wants
  (send, stop, open a file picker, copy) to an outbox, and the app takes it.

### 2. Layout: sizes and positions

State, an available width and a skin's sizes in; rectangles and text lines
out. Units are logical pixels, y down.

Layout measures text through one interface:

```
Measure: (face, size, text) -> advance, and the face's ascent, descent, line height
```

A pixel target answers from font metrics. A terminal answers in cells. This
is the only thing layout knows about a target.

### 3. Paint: the display list

A component paints into a list of primitives, not vertices:

| Primitive | Carries |
| --- | --- |
| Box | bounds, corner radius, fill, border width and colour |
| Shadow | bounds, radius, blur, colour; inset or drop |
| Gradient | bounds, radius, two stops |
| Line, ring, arc | points, width, colour (the HUD's bars, rings and pie sweeps) |
| Text run | position, face, size, colour, the text |
| Icon | bounds, icon id, tint |
| Image | bounds, image id |
| Clip, fade | a scissor rectangle; fade bands at an edge |

It is the union of what `gpui.coax` and `hud-style.coax`'s `HudPrim` paint
today. A new primitive is added here once, then to each backend.

### 4. Skin: how it looks

A skin is tokens plus frames.

- **Tokens**: colour roles (surface, ink, muted, accent, danger, hairline),
  type roles (body, title, code: face, size, line height), spacing, radii,
  border weights, motion times.
- **Frames**: how a panel, a button, a slot or a bar is built from
  primitives. A Zeron panel is a rounded box with a hairline and a shadow.
  An Everglade panel is an iron frame with studs. The component asks the
  skin for "a panel here"; it doesn't draw one.

First skins: `zeron` (Zeron Dark, from the constants in today's code),
`verse` (SCIFI) and `everglade` (FANTASY), the last two from
`aethyr/ui/hud-style.coax`, and `terminal`.

A component reads only roles. A colour or size written into a component is a
defect, and a check finds it.

### 5. Backend: where it draws

- **`backend/gpu.coax`**: the display list to vertices, with one shader
  written in Coax and emitted as WGSL, Metal and SPIR-V. It starts from the
  28-float vertex and the shader in `apps/openagents-web/gpu/chat.coax`. The
  hand-written `Zeron.metal` goes away.
- **`backend/terminal.coax`**: the display list to terminal cells, through
  `coax/lib/tui`'s output. Boxes become borders or nothing, colours map to
  the terminal's.

A realm draws components in a pass of its own after the scene, with the
component shader. Existing HUD parts keep their own pass until they move
(Phase 5).

The host supplies three things and no more: a glyph and icon atlas with its
metrics, a place to put vertices, and input events. That contract is written
once in `components/backend/host.h`, and is what the shared hosts of the apps
roadmap implement.

## What a component is

One file in `components/kit/`, with:

```
State     a struct, fixed buffers, no pointers into the host
layout    (state, width, skin, measure) -> its boxes and height
paint     (state, boxes, skin) -> primitives appended to the display list
pointer   (state, x, y, phase) -> handled?    key  (state, key, mods) -> handled?
rows      its elements for the tree: role, label, action
#test     its rules, next to the code
```

### The first set: chat

Text, Markdown, code block (with highlighting), diff, message bubble, tool
group and tool chip, approval prompt, notice, transcript (scrolling, only
visible rows laid out), composer (editor, attachment chips, send and stop),
menu and popover, picker, button, toggle, slider, tabs, list with sections
(the session sidebar), scrollbar, panel.

Everything in `apps/shared/zeron/` today maps onto this list.

### The second set: game HUD

Bars, slots and the action bar, portrait plate, map ring, nameplate, damage
number, banner, talk window. These are `everglade/hud/` and
`aethyr/ui/draw.coax`, moved onto the same layers.

## Checks

- **Rules**: each file's `#test` and `#forall` checks. Core and layout need
  no GPU, so they run anywhere in seconds.
- **Pictures**: the gallery draws each component in each skin at fixed
  sizes; `gallery/check.coax` compares them with goldens. One golden per
  component, skin and platform (text rasterizes differently on each).
- **`coax/web/tool.sh components-check`**, run before a push:
  - a file in `core/` or `layout/` that loads `paint/`, `skin/` or
    `backend/` fails;
  - a colour literal or a font size in `kit/` fails;
  - a Markdown parser, a line wrapper or an edit buffer outside
    `components/` fails, with the list of files still to move printed until
    it is empty.

## Phases

Each phase ends with every existing check and golden unchanged.

1. **Move.** `apps/shared/zeron/` to `components/` with no content change;
   the web, the Mac app and Aethyrforge load it from there.
2. **Split Markdown first.** `core/markdown.coax` (parse),
   `layout/text.coax` (wrap), `kit/markdown.coax` (paint), the display list
   and `backend/gpu.coax`. Markdown is the proof: when chat's pictures don't
   change, the layers are right. Then the rest of the chat set the same way.
3. **Skins and the gallery.** The `zeron` skin from the constants now in the
   code, the `everglade` skin, and openagents.com/components.
4. **Chat in Everglade for desktop.** The Workshop gets the same transcript
   and composer in the `everglade` skin, on the shared engines. Its boards
   (`golem-board.coax`) move onto the list and panel components.
5. **The terminal and the leftovers.** Coder's terminal and the CLI take
   Markdown and the edit buffer from `core/`. `chat-hud.coax`'s and
   `project-page.coax`'s parsers are deleted. The HUD set moves.

## Decisions for the owner

1. **The root `components/` directory and the `/components` page**, as
   above. Recommended.
2. **One shader for every pixel target, replacing `Zeron.metal`.**
   Recommended. The cost: the Mac app's goldens were matched against
   Zeron's own renderer pixel for pixel, and a Coax-emitted Metal shader has
   to match them again.
3. **The glyph atlas.** Each host rasterizes text itself today. A
   rasterizer in Coax would give every platform the same text and the same
   goldens. Recommended, and it is the same question as Step 1 of the apps
   roadmap.

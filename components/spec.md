# Components: one component system for every OpenAgents and Verse app (spec, draft)

Written 2026-11-10. Status: Phase 1 (the move) is done. Phase 2 is under way:
the layers exist and the chat components draw through them with the same
vertices as before (see "Phases" for what has moved).
Tracked in GitHub #13153. The code this replaces is listed under "What exists
today".

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
  README.md        how to use a component and add one (not written yet)
  core/            what a component is: state and rules, no pixels
                   (color, text, markdown, syntax)
  layout/          sizes and positions from state, a width and a skin
                   (round, font, measure, text)
  paint/           the display list: the primitives a frame is made of (list),
                   and a sheet: two lists, selectable lines and hit regions (sheet)
  skin/            tokens and frames: skin.coax (what a skin is), zeron.coax, verse.coax,
                   everglade.coax, terminal.coax, all.coax (pick one by id or name)
  backend/         display list to a target: gpu.coax, gpu-shader.coax, terminal.coax
  kit/             the components themselves (markdown; controls: panel, button,
                   toggle, slider, tabs, list, bar)
  gallery/         page.coax, the /components page; linux.coax and linux/ (the same
                   page as a Linux program, for pictures); md-term.coax (Markdown
                   in a terminal through the whole stack); skin-parity.coax
  fonts/, icons/   with their licences and NOTICE
  zeron/           the chat components as moved from apps/shared/zeron/ (Phase 1, done),
                   emptied as Phase 2 splits each file into the layers above
```

## What exists today

Three drawing stacks and, when this was written, three Markdown
implementations beside the chat's (there is one now: see Phase 5):

| Code | Draws | Problem |
| --- | --- | --- |
| `components/zeron/` (gpui, composer, markdown, tools, transcript, picker, attachments; about 5,700 lines) | Chat on the web and the Mac app | Parsing, layout, Zeron's colours and vertex output are in the same functions. `markdown.coax` loads `composer.coax` to get the atlas |
| `aethyr/ui/` (hud, draw, tree, feed, hud-style, arwes; 4,130 lines) | Realm HUDs | A second vertex format (12 floats, strokes) and its own text. It has the right ideas: `hud-style.coax` separates tokens from drawers, and `tree.coax` is one table for drawing, hit tests, focus and accessibility |
| `everglade/hud/`, `apps/everglade-desktop/text.coax`, `golem-board.coax` | Everglade's HUD and the Workshop's boards | A third text path with its own atlas layout |
| `coax/lib/tui/tui.coax` | Coder's terminal | Its own composer. Its Markdown reading is `core/markdown.coax`'s since 2026-11-10; it still writes the style sequences itself |
| `apps/openagents-web/chat-hud.coax` | The old HUD chat (the Mac app still builds it) | Its Markdown reading is `core/markdown.coax`'s since 2026-11-10. `apps/openagents-cli/project-page.coax`, listed here at first, writes Markdown and never parsed it |

## The five layers

Each layer loads only the layers above it in this list. A check enforces it
(see "Checks").

### 1. Core: state and rules

Plain data and functions. No colour choices, no pixels, no fonts, no platform
calls. The colour *type* (`core/color.coax`: `Rgba` and GPUI's arithmetic on
it) is core, because layout, paint, skins and backends all carry colours;
which colour a component wears is its skin's to say.

- **Text** (`core/text.coax`): UTF-8 and grapheme clusters. The edit buffer
  (selection, undo, marked text for an IME, UTF-16 offsets for hosts that
  need them) is still inside `zeron/composer.coax`.
- **Markdown** (`core/markdown.coax`): source to a document (`MdDoc`: blocks,
  items, styled runs, tables, code with its language). Streaming-safe: an
  unfinished reply parses. `core/syntax.coax` gives a line of code its
  tokens' roles (keyword, string, type); a skin colours the roles.
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

Coax has no procedure values, so the interface is a value, not a callback
(`layout/measure.coax`): a `TextMeasure` says which target it is
(`measure_font(atlas)` or `measure_cells()`), and `measure_step`,
`measure_width` and `measure_face` answer for it. A pixel target's font
metrics, as its host measured them, are `layout/font.coax` (the `Atlas`: each
face's advances and kerning, and the glyph cache). The same struct carries
where each glyph's cell sits in the atlas texture, because the host hands
both over in one call; only the GPU backend reads that part.
`layout/text.coax` is the one line wrapper (`text_wrap`), and
`layout/round.coax` is GPUI's rounding.

#### Layout: flex

Decided 2026-11-10 (the audit in
[docs/research/exact/04-apps-audit-2026-11-10.md](../docs/research/exact/04-apps-audit-2026-11-10.md),
recommendation 4): boxes inside boxes are laid out by flex layout with CSS's
names, written in Coax, and Chrome is the oracle.

`layout/flex.coax` is built. A container has `direction`, `gap`, padding,
`align` (align-items) and `justify` (justify-content). An item has `width`,
`height`, `grow`, `shrink`, `basis` and `self` (align-self). `fx_layout` gives
every box its rectangle. An agent that knows flexbox knows this.

The subset is deliberate: one line (no wrapping), every box is a border box
that may shrink to its padding, lengths are logical pixels, and there are no
margins, percentages, `min-`/`max-` sizes or baselines. A leaf's content size
is the width and height its component measured.

The check is `coax/web/tool.sh components/gallery/flex-chrome.coax`: 200
generated trees, 1,136 boxes, laid out by `fx_layout` and as HTML by headless
Chrome, every rectangle within 0.05 px. It found two rules the first version
had wrong (a box's padding is a floor on its size and is left out of what
shrinks; a row's content width counts each item as its content, held to its
base size when it cannot grow or cannot shrink, while a column's content
height counts base sizes). Both are in the file's checks now.

Who uses it: nothing yet. The chat set keeps its arithmetic, which is one
column and matches Zeron to the pixel. New components use flex from the
start: the game HUD set (Phase 5), and the gallery's cards when they are next
touched. A component moves to flex only with its pictures unchanged.

### 3. Paint: the display list

A component paints into a list of primitives, not vertices
(`paint/list.coax`). A list is the caller's array of `Prim` and a count;
each `prim_*` procedure appends one and returns the new count:

| Primitive | Carries |
| --- | --- |
| Box | bounds, corner radius, fill, border width and colour |
| Shadow, inset | bounds, radius, offset, blur, spread, colour; an inset is the lit edge inside a shape |
| Gradient | bounds, radius, two stops |
| Line, ring, arc, elbow | points, width, colour (the HUD's bars, rings and pie sweeps; the transcript's activity rail) |
| Text run | position, face, line height, colour, the text; its selectable line and offset |
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

- **`backend/gpu.coax`**: the display list to vertices (`gpu_draw`), with one
  shader written in Coax, `backend/gpu-shader.coax`, emitted as WGSL
  (`coax wgsl`), SPIR-V (`coax spirv`) and Metal (`coax metal`). The vertex
  is GPUI's 28 floats, unchanged, and so is the atlas contract
  (`gp_set_font`, `gp_set_icon`, `gp_set_atlas`, the glyph cache). The
  `paint_*` procedures that write one primitive's vertices directly stay for
  the components not yet on the display list; both ways give the same
  floats, and a check holds them to it. Not drawn yet: an arc, a slanted
  line and an image (`gpu_draws` says so); they need a path in the shader.
  The hand-written `Zeron.metal` goes away when the Mac app builds the
  emitted Metal; that needs a Mac, and hasn't been done.
- **`backend/terminal.coax`**: the display list to a grid of cells
  (`term_draw`), and a row of the grid to the bytes a terminal reads
  (`term_row`: SGR with 24-bit colour). Text lands in its cells, a rule
  becomes a line glyph, a box's border becomes box-drawing glyphs and its
  fill a background; shadows, gradients, icons and the rest are left out.
  `components/gallery/md-term.coax FILE.md` prints a Markdown file through
  the whole stack in the terminal skin.

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

Everything in `components/zeron/` maps onto this list.

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
- **`coax/web/tool.sh components-check`**, run before a push
  (`coax/web/components-check.coax`):
  - a layer file that loads a layer after it in the order core, layout,
    paint, skin, backend fails; so does a component (`kit/`) that loads a
    backend, and any of them loading a file outside `components/` (the
    shader's library, `verse/gpu/wgsl.coax`, is the one exception);
  - a colour literal or a named face in `kit/` fails;
  - a file outside `components/` that reads Markdown's marks, wraps text or
    keeps an edit buffer fails unless it is on the tool's list of files
    still to move, which is printed until it is empty (22 files on
    2026-11-10: the realm HUDs' and pages' own wrappers, Coder's terminal's
    wrapper and edit buffer, three files that test for a code fence or a
    bold mark). The tests are signatures, not proofs. `components/zeron/`
    is exempt until it is emptied;
  - the shader must lower to SPIR-V and Metal.

## Phases

Each phase ends with every existing check and golden unchanged.

1. **Move.** Done 2026-11-10: `apps/shared/zeron/` is now `components/zeron/`,
   with no content change;
   the web, the Mac app and Aethyrforge load it from there.
2. **Split Markdown first.** Started 2026-11-10. Done: the parser is
   `core/markdown.coax` and the highlighter's roles are `core/syntax.coax`,
   with no drawing code loaded; wrapping is `layout/text.coax` behind
   `layout/measure.coax`; the display list is `paint/list.coax`;
   `backend/gpu.coax` writes its vertices and `backend/gpu-shader.coax` is
   the shader. `zeron/gpui.coax` and `zeron/markdown.coax` are now only ways
   in to those files. Chat's frames are pinned by
   `apps/openagents-web/chat-frames.coax` (eight scenes of `/chat`, hashed
   vertex buffers): the same input gives the same floats as before the
   split. Markdown is `kit/markdown.coax`: the transcript's Markdown
   blocks (paragraphs, lists, quotes, code blocks, tables) are laid out and
   painted by it onto a sheet (`paint/sheet.coax`) in the Zeron skin, and
   the transcript's whole display list is the paint layer's primitives,
   drawn by `gpu_draw`. Chat's frames did not change. The same file lays
   the same document out in cells in the terminal skin
   (`kit/markdown-checks.coax`). Still to move onto the display list and
   the skin: the rest of the transcript (bubbles, tool groups, diffs,
   notices), the composer, the pickers and the attachments, which still
   call `paint_*` with Zeron's constants.
3. **Skins and the gallery.** Done 2026-11-10: `skin/skin.coax` (`UiSkin`:
   colour roles, type roles, spacing, Markdown's measures, and the frames
   `uiskin_panel`, `uiskin_control`, `uiskin_slot`, `uiskin_bar`), with
   `zeron.coax` (from the constants that were in the components, which now
   live there), `everglade.coax` and `verse.coax` (the HUD's FANTASY and
   SCIFI colours; `gallery/skin-parity.coax` holds them equal to
   `aethyr/ui/hud-style.coax`) and `terminal.coax` (cells). The pixel skins
   share the atlas's faces (Geist): a skin's own typefaces need its fonts in
   the atlas, which no host loads yet. The page is
   `components/gallery/page.coax` (served as `/components` by
   `apps/openagents-web/components.coax`, `components-face.c`,
   `components/index.html` and `components.wgsl`, the same shader module
   `/chat` has): a card for each skin with the same components from the
   same calls (`kit/controls.coax`: panel, button, toggle, slider, tabs,
   list, bar; `kit/markdown.coax`), all of them working under the pointer.
   The terminal card is laid out in cells, painted into a cell grid by
   `backend/terminal.coax`, and the grid is shown. Its checks lay the page
   out, read the terminal grid's rows and drive the pointer. The same page
   is a Linux program on the shared Wayland and Vulkan host
   (`components/gallery/linux/build.sh`, then
   `components/gallery/build/linux/Components --shot OUT.png`, which opens
   no window): that is how it was looked at, at two widths. Not done: it
   has not been seen in a browser (no headless WebGPU capture on the Linux
   machine it was written on; a browser's own fonts will draw the terminal
   card's task boxes, which Geist Mono lacks), and there are no picture
   goldens (`gallery/check.coax`) yet.
4. **Chat in Everglade for desktop.** The Workshop gets the same transcript
   and composer in the `everglade` skin, on the shared engines. Its boards
   (`golem-board.coax`) move onto the list and panel components. Not
   started. What exists for it: the `everglade` skin, and Markdown, the
   panel and the list as components that draw in it (see `/components`).
   What it still needs: the transcript and the composer reading a skin
   (Phase 2's remainder: they are Zeron's constants today); and a way for
   the Workshop to draw a display list, which is one of two things. Either
   its Mac host gets a component pass (the 28-float vertices, the component
   shader and an atlas with Everglade's faces, Spectral and Cinzel, in the
   skin's slots), or a backend writes a display list as the Workshop's
   present 12-float HUD vertices with `apps/everglade-desktop/text.coax`'s
   text, measured from that file's advance tables. The Workshop has only a
   Mac host, so neither could be run where Phases 2, 3 and 5 were done.
5. **The terminal and the leftovers.** Done 2026-11-10: there is one
   Markdown parser. `core/markdown.coax` grew what the others read that it
   didn't (italic, strike, links with their targets, backslash escapes,
   task boxes, tilde fences, and a mode for a finished line where an
   unclosed mark stays as written); Coder's terminal (`coax/lib/tui/tui.coax`)
   and the old HUD chat (`chat-hud.coax`) take what a line is and where its
   spans are from it, and their own span, cell and delimiter parsers are
   deleted. `project-page.coax` writes Markdown and had no parser.
   `backend/terminal.coax` exists and `md-term.coax` uses it. Two things
   changed for Coder's terminal with the one parser: a table needs its
   delimiter row (rows without one are shown as lines), and a pipe inside
   backticks in a table cell splits the cell unless it is written `\|`, as
   GFM and the chat read it. Not done: Coder's terminal still writes its
   style sequences itself, line by line as a reply streams, instead of
   painting through `kit/markdown.coax` and `backend/terminal.coax`; its
   composer's edit buffer and the chat composer's are still two; the HUD
   set has not moved.

## Decisions for the owner

All three were taken on 2026-11-10 as recommended. A fourth, layout, was
taken the same day: flex with CSS's names, in Coax, checked against Chrome
(see "Layout: flex").

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

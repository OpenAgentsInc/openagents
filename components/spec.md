# Components: one component system for every OpenAgents and Verse app (spec, draft)

Written 2026-11-10. Status: Phase 1 (the move), Phase 2 (the layers and
the chat set on them) and Phase 3 (skins and the gallery) are done: the
chat components paint display lists in their skin's roles, colours, faces
and measures alike, with the same vertices as before. Phase 5 (the
terminal and the leftovers) is done: one edit buffer, Coder's terminal
painting through the terminal backend, and the game HUD set on the layers.
Phase 4 moved to GitHub #13156: the chat set draws in the `everglade` skin
on Linux, and the Workshop itself does not run on Linux.
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
                   (color, text, edit, markdown, syntax, hud: the game HUD's rules)
  layout/          sizes and positions from state, a width and a skin
                   (round, font, measure, text)
  paint/           the display list: the primitives a frame is made of (list),
                   and a sheet: two lists, selectable lines and hit regions (sheet)
  skin/            tokens and frames: skin.coax (what a skin is), hud.coax (the HUD set's roles), zeron.coax, verse.coax,
                   everglade.coax, terminal.coax, all.coax (pick one by id or name)
  backend/         display list to a target: gpu.coax, gpu-shader.coax, terminal.coax
  kit/             the components themselves (markdown; controls: panel, button,
                   toggle, slider, tabs, list, bar; hud: the game HUD set)
  gallery/         page.coax, the /components page (hud-cards.coax: its HUD cards);
                   linux.coax and linux/ (the same
                   page as a Linux program, for pictures); md-term.coax (Markdown
                   in a terminal through the whole stack); skin-parity.coax; arwes.coax
                   (the page's ARWES section: every ARWES port, from
                   aethyr/ui/arwes-paint.coax, in the Verse skin)
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
  is `core/edit.coax` (`EditBuf`: selection, undo in typed bursts, marked
  text for an IME, UTF-16 offsets for hosts that need them, UAX #29 word
  moves). The chat composer (`zeron/composer.coax`) and Coder's terminal
  (`coax/lib/tui/tui.coax`) both keep their text in one and edit it with
  its procedures; each keeps its own key bindings, layout and pointer.
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
- **The element table** (`core/elements.coax`, GitHub #13156): the first
  part of the tree. Every row a component draws that a person presses,
  types into or reads has a stable id made of its part and its name
  (`composer.send`, `session.2`, `message.4.7`), never an index that moves
  when another row comes or goes; a repeated id is numbered (`-2`, `-3`).
  The chat's desk builds it from where the last frame laid its rows out
  (`zeron/desk.coax` `dsk_elements`). `coax drive`'s `tree` prints it in the
  layout trace's format (`ID ROLE "LABEL" at X Y size WxH`, logical px), and
  `tap ID`, `type ID TEXT` and `drag ID DX,DY` press a row by its id, so a
  check names what it presses and never a coordinate. Focus order and
  accessibility do not read it yet.
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

Who uses it: the game HUD set (`kit/hud.coax`: the action bar, the unit
frame, the map ring, a plate, the talk window) and the page's HUD cards
(`gallery/hud-cards.coax`). The chat set keeps its arithmetic, which is one
column and matches Zeron to the pixel; the gallery's skin cards keep theirs
until they are next touched. A component moves to flex only with its
pictures unchanged.

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
| Polygon | its points (a view into the painter's pool), fill; filled even-odd (ARWES's frames) |

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
  floats, and a check holds them to it. A slanted line is drawn as its band
  with coverage by distance (the shader's segment kind) and a polygon as
  triangles cut by ear clipping (2026-11-11, for ARWES). Not drawn yet: an
  arc and an image (`gpu_draws` says so); they need a path in the shader.
  The Mac apps (OpenAgents and Aethyrforge) build the emitted Metal; the
  hand-written `Zeron.metal` is gone (2026-11-10, #13156), and the Mac
  goldens pass with it (at most 0.05% of pixels off, at glyph edges).
- **`backend/dom.coax`** (decided 2026-11-11, not built): the element table
  to browser elements, as a list of changes keyed by id that the web host
  applies. The browser lays out and draws the text. The web chat is its
  first user
  ([why and how](../docs/audits/2026-11-11-chat-zero-base-audit.md#decision-the-web-chat-is-drawn-by-the-browser)).
- **`backend/terminal.coax`**: the display list to a grid of cells
  (`term_draw`), and a row of the grid to the bytes a terminal reads
  (`term_row`: SGR with 24-bit colour, written by `term_sgr`, the one
  writer of style sequences; a face is a set of attribute bits). Text lands in its cells, a rule
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

Changed 2026-11-11: the web chat gets a new, small set with a grayscale
ARWES look, on the browser backend, and `components/zeron/` is not moved
into `kit/`. It stays for the desktop app until that moves to the new set
([the chat audit, section 11](../docs/audits/2026-11-11-chat-zero-base-audit.md#11-the-look-a-grayscale-arwes-set-and-a-new-component-set)).

### The second set: game HUD

Bars, slots and the action bar, portrait plate, map ring, nameplate, damage
number, banner, talk window: `kit/hud.coax` (`hk_vital`, `hk_slot`,
`hk_action_bar`, `hk_portrait`, `hk_map`, `hk_nameplate`, `hk_damage`,
`hk_banner`, `hk_talk`), checked in every skin by `kit/hud-checks.coax`. They
read the skin's `UiHud` roles (`skin/hud.coax`: the bar's fill, groove and
trail, a slot's size, the medallion, the map ring, the banner; Everglade's and
the Verse's are the realm HUD's FANTASY and SCIFI tokens, held equal by
`gallery/skin-parity.coax`) and are laid out with flex. The page shows them in
the `everglade` and `verse` skins (`gallery/hud-cards.coax`): press a slot,
strike the wolf, answer Mira.

The rules both HUDs follow are one file, `core/hud.coax`: a bar's fill and
damage trail over time (`HudTrail`, `hud_trail_step`), a banner's fade
(`hud_banner_alpha`) and the map ring's projection (`hud_map_project`).
`aethyr/ui/hud-style.coax` loads it, so every realm's HUD uses the same code.

The realms' drawing has not moved. Everglade's and the Verse's HUDs write the
realm pass's 12-float vertices (`hud_emit`, `aethyr/ui/draw.coax`'s strokes),
set their words in the realm's own faces (Spectral and Cinzel, the Verse's
Filament strokes), and draw pie sweeps, turned diamonds and soft rings that
the component shader has no path for yet (an arc, a slanted line). Drawing a
realm's HUD with these components would change its pictures, so each realm
keeps its own drawers until the component pass has those paths and the
realm's faces.

## Checks

- **Rules**: each file's `#test` and `#forall` checks. Core and layout need
  no GPU, so they run anywhere in seconds.
- **Text, row by row, before pictures** (GitHub #13156): an app's check
  writes each case's element table (`--tree`) and compares it line by line
  with its golden (`goldens/NAME.tree`: every row's id, role, label and box)
  before it compares the picture. A changed label or a moved button fails as
  the row that changed, on every host, without anyone looking at a picture.
  The OpenAgents app's checks (`check.sh`, `linux/check.coax`) do this; the
  gallery does not yet.
- **Pictures**: the gallery draws each component in each skin at fixed
  sizes; `gallery/check.coax` compares them with goldens. One golden per
  component, skin and platform (text rasterizes differently on each).
- **`coax/web/tool.sh components-check`**, run before a push
  (`coax/web/components-check.coax`):
  - a layer file that loads a layer after it in the order core, layout,
    paint, skin, backend fails; so does a component (`kit/`) that loads a
    backend, and any of them loading a file outside `components/` (the
    shader's library, `aethyr/gpu/wgsl.coax`, is the one exception);
  - a colour literal or a named face in `kit/` or in the chat set
    (`components/zeron/`) fails, outside checks (since 2026-11-11 for the
    chat set: its colours and faces are its skin's roles); so does a size,
    a number other than 0, 1, 2 or a half (2026-11-11). A count is no
    size: an array's length or an index, a range's end and an integer
    constant (`T_ROWS :: 256;`) pass, as do strings and characters, a
    fixture named for tests (`test_*`, `*_test*`) and a line that says
    `// not a size` (an epsilon, a sentinel, a demo's line numbers);
  - a file outside `components/` that reads Markdown's marks, wraps text or
    keeps an edit buffer fails unless it is on the tool's list of files
    still to move, which is printed until it is empty (22 files on
    2026-11-10: the realm HUDs' and pages' own wrappers, Coder's terminal's
    wrapper and edit buffer, three files that test for a code fence or a
    bold mark). The tests are signatures, not proofs;
  - the shader must lower to SPIR-V and Metal.

## Phases

Each phase ends with every existing check and golden unchanged.

Who does each phase and by when (the audit's recommendation 7,
docs/research/exact/04-apps-audit-2026-11-10.md; set 2026-11-11, #13156). A
phase with no implementer by its date is cut from this list, not left open.

| Phase | Implementer | Date |
| --- | --- | --- |
| 1. Move | The apps agents on `coderos-4080` (#13153) | Done 2026-11-10 |
| 2. Split Markdown first: the rest of the transcript, the composer, the pickers and attachments onto the display list and the skin | The apps agent on `coderos-4080`, under #13153's follow-up | Done 2026-11-11 |
| 3. Skins and the gallery: picture goldens (`gallery/check.coax`) and a look in a browser | The apps agent on the owner's Mac (headless Chrome renders WebGPU there) | 2026-11-14 |
| 4. Chat in Everglade for desktop | The apps agent on the owner's Mac, after the Workshop's host draws a component pass (moved to #13156) | 2026-11-17 |
| 5. The terminal and the leftovers: Coder's terminal painting through `backend/terminal.coax`, one edit buffer, the HUD set | The apps agent on `coderos-4080` | Done 2026-11-11 |
| Hosts: OpenAgents, Everglade and Aethyrforge on the shared Mac host, and the Android window (the hosts themselves draw and take pictures since 2026-11-11, #13156) | The apps agent on the owner's Mac | 2026-11-17 |

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
   (`kit/markdown-checks.coax`).
   Done 2026-11-11: every colour and face the chat set paints is its skin's
   role. The transcript's bubbles, tool groups, chips, diffs, notices and
   working trailer (`t_skin`), the composer expanded and docked
   (`composer_prims`, `composer_skin`), the model panel, model list and
   access menu (`pk_prims`, `pk_skin`), the attachments' chip tints
   (`at_skin`), and the desktop app's sidebar and tabs (`dsk_sidebar_prims`,
   `dsk_tabs_prims`, `dsk_skin`) paint the paint layer's primitives, drawn by
   `gpu_draw`; the skin gained the roles they read (`UiSkin`'s chat-set
   roles, `UiPlate`, `uiskin_plate`). `components-check` holds
   `components/zeron/` to its skin as it holds `kit/`.
   `components/zeron/skins-checks.coax` lays the transcript, the composer
   and the menus out in Everglade and fails on any Zeron colour; the desktop
   app takes `--skin` and `linux/check.coax`'s `chat-everglade` case pins
   the whole chat in Everglade. Chat's eight frames, the menus' other states
   (hashed before and after) and the Linux app's pictures (byte for byte)
   did not change.
   Done 2026-11-11: the chat set's measures are its skin's too. `UiSkin`
   gained `tr` (the transcript: gaps, bubbles, notices, the working
   trailer, tool rows, chips and their badges, details and diffs), `cmp`
   (the composer and its chips), `menus` (the cards, the model panel and
   slider, the list, the access menu and its switch), `desk` (the desktop
   app's sidebar, tabs and room), `chrome` (the status strip, the failure
   notice's margins, the dictation button, a text button, a filled button,
   a one-line field) and `lifted` (a floating card's two shadows). Zeron's
   numbers moved into `skin/zeron.coax` unchanged, derived values folded
   there as the old constant expressions were. The rules that were numbers
   but no size moved to core (`core/media.coax`: extensions, media types,
   text or not; `text_put_int`, `text_elapsed`, `text_ends` in
   `core/text.coax`). The screen's status strip, failure notice and
   dictation button, and `/chat`'s own buttons, sign-in field and notice,
   are `components/zeron/chrome.coax`'s display lists in the chat's skin,
   drawn by `gpu_draw`. `components-check` fails a size in `kit/` or the
   chat set. `components/zeron/frames-checks.coax` pins the transcript,
   composer and menus in Zeron, Everglade and the Verse by hashing their
   prims (hashes taken before the move); they, chat's eight frames, the
   page's buttons, notice and key card (hashed before and after) and the
   Linux app's eight pictures (byte for byte) did not change.
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
   no window): that is how it was looked at, at two widths. The same
   `main.c` and Coax build for the Mac on the shared Mac host
   (`apps/shared/host/apple/ui-host.m`, the same `ui-host.h`;
   `components/gallery/apple/build.sh`, then
   `components/gallery/build/apple/Components --shot OUT.png`, which opens
   no window and no Dock icon), with the shader emitted as Metal and the
   atlas from `components/raster/` (2026-11-11, #13156). Not done: it
   has not been seen in a browser (no headless WebGPU capture on the Linux
   machine it was written on; a browser's own fonts will draw the terminal
   card's task boxes, which Geist Mono lacks), and there are no picture
   goldens (`gallery/check.coax`) yet.
4. **Chat in Everglade for desktop.** The Workshop gets the same transcript
   and composer in the `everglade` skin, on the shared engines. Its boards
   (`golem-board.coax`) move onto the list and panel components. Started
   2026-11-11: the whole chat set (transcript, composer, menus, attachments,
   sidebar and tabs) draws in the `everglade` skin on the shared engines'
   session record, on the shared Linux host, offscreen
   (`OpenAgents --demo --skin everglade --shot OUT.png`, pinned by
   `linux/check.coax`'s `chat-everglade` golden). The Workshop itself does
   not run on Linux, and cannot without new host work: its temple and your
   model are drawn by the Mac host's own Metal pipelines from hand-written
   `Workshop.metal` and `Model.metal` (`Sources/Workshop.swift`, a layer on the
   shared Mac host since 2026-11-11), its words are
   12-float HUD strokes over `text.coax`'s Spectral and Cinzel atlas drawn by
   a third Metal pipeline, its input is `aethyr/host/appkit-input.coax`, and
   the shared Linux host (`apps/shared/host/linux/ui-host.h`) has one
   pipeline, the component shader over 28-float vertices, and no scene or
   depth pass. The Workshop has no chat yet either: where it opens and
   which engine it starts is still to be decided. What it needs is a way
   for the Workshop to draw a display list, which is one of two things. Either
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
   GFM and the chat read it.
   Done 2026-11-11, the rest:
   - One edit buffer: `core/edit.coax` (`EditBuf`, `eb_*`), the chat
     composer's editing moved out of `zeron/composer.coax`; the composer
     (`Composer.ed`) and Coder's terminal (`TuiEditor.eb`) keep their text
     in it. For Coder this means a key steps a grapheme, word moves use the
     composer's UAX #29 rules, and undo takes back typed bursts and pastes.
   - Coder's terminal writes no style sequence of its own: a styled line
     (Markdown, highlighted code, a table, a diff row, the ladder) is a
     display list of runs, drawn into cells by `term_draw` and written by
     `term_row` and `term_sgr`. A terminal face is a set of attribute bits
     (bold, italic, dim, underline, strike). The terminal still lays out a
     reply line by line as it streams, with its own palettes, not through
     `kit/markdown.coax` and the terminal skin (the skin has no roles for
     its ladder, and its syntax colours differ). What a terminal shows did
     not change: `coax/lib/tui/tui-cells-checks.coax` and
     `coder/tui/diff-cells-checks.coax` read the bytes back into cells and
     hold the hashes taken from the old code.
   - The game HUD set is components (`kit/hud.coax`, the `UiHud` roles in
     `skin/hud.coax`, laid out with `layout/flex.coax`) in the `everglade`
     and `verse` skins, with their cards on `/components`. The realms
     still draw their HUD as before; what moved into them is the rules
     (`core/hud.coax`: the bar's fill and trail, the banner's fade, the
     map's projection), loaded by `aethyr/ui/hud-style.coax`. The drawing
     did not move: the realm pass writes 12-float strokes in the realms'
     own fonts (Spectral, Cinzel, the Verse's Filament), which the
     component atlas does not hold, so their pictures would change.
   - ARWES on `/components` (the owner's request): the 25 ported specimens
     are painted once, as display lists (`aethyr/ui/arwes-paint.coax`),
     drawn on the page by the component shader (`gallery/arwes.coax`) and
     written as the HUD laboratory's 12-float records by `arwes-draw.coax`
     (`aw_hud`; its records are unchanged over 225 frames). The paint
     layer gained `PRIM_POLY` and the GPU backend a segment kind for it.

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

   **On the Mac, CoreText's atlas stays for now** (2026-11-11, #13156).
   `components/raster/atlas.coax` draws only what Geist has: a grapheme the
   font lacks (an emoji, a CJK character, most symbols) becomes the
   missing-glyph box, where CoreText falls back to the system's fonts and
   draws emoji in colour into the colour atlas. It also has one dilation
   level, where CoreText thickens light text on dark by the fill's
   luminance, so the Mac's text would get visibly thinner. Moving the Mac
   onto it would make real conversations worse to match a picture check.
   It moves when the Coax rasterizer has font fallback (a list of faces,
   tried in order) and colour glyphs (an sbix or COLR reader); until then
   Linux and Android, which have no platform rasterizer, use the Coax one,
   and the Mac and iOS keep CoreText. The pictures are already per platform
   (see "Checks"), so nothing else changes.

# HUD styles

A realm's HUD has two parts: what it shows (health, the action bar, a map, names
over creatures) and how it looks. This page covers how it looks. A realm picks
one HUD style and draws every HUD part through it.

Two styles exist:

- **SCIFI**: the Verse's instrument look. Grey and cyan glow lines, cut corners
  and thin bars. Most realms use it, and its drawing code stays where it is:
  `verse/hud/frame.coax`, `aethyr/ui/draw.coax` and the Arwes frames in
  `aethyr/ui/arwes-*.coax`.
- **FANTASY**: dark iron and bronze frames, warm gold type, parchment, rounded
  corners with diamond studs, framed segmented bars, recessed slots and a gold
  ribbon banner. Everglade uses it.

The code is in [`aethyr/ui/hud-style.coax`](../../aethyr/ui/hud-style.coax).
Run its checks with:

```sh
coax/bin/coax test aethyr/ui/hud-style.coax --library
```

## The style API

A style is a set of tokens plus drawers.

- `hud_style_named(name)` returns `HUD_FANTASY` for `"fantasy"` (any case) and
  `HUD_SCIFI` for anything else, so a realm with no setting keeps the look it
  had.
- `hud_tokens(style)` returns a `HudTokens`: the palette (frame dark, mid and
  light; trim; text, dim text and title; parchment; health, its back and low
  health; magic; good and bad; damage and healing), the faces for titles and
  labels, the base line weight, the corner radius, and the corner, ornament,
  bar, slot, banner and damage-number treatments.
- `hud_text(tokens, role)` says which face, colour and size a text role takes.
  The roles are `HUD_TEXT_NAME`, `TITLE`, `NUMBER`, `LABEL`, `KEY`, `BANNER`,
  `DAMAGE`, `HEAL` and `WARN`. FANTASY sets names, titles and banners in the
  title face and outlines damage numbers.

The drawers add primitives to a `HudList`: filled rounded boxes, rings, soft
shadows and pie sweeps, each with a top and bottom colour and an optional turn.
Each part has one drawer:

| Drawer | Part |
| --- | --- |
| `hud_panel` | A framed panel: shadow, body, metal frame, inner trim and studs |
| `hud_button` | A raised plate; `lit` trims the chosen one gold |
| `hud_well` | A sunk row or field |
| `hud_bar` | A framed bar with segments and a gloss |
| `hud_slot` | A recessed slot, trimmed gold when ready |
| `hud_tray`, `hud_tray_round` | An action bar's tray; `hud_tray_round` leaves each slot's picture clear |
| `hud_medallion`, `hud_badge`, `hud_shield`, `hud_gem` | A portrait or map frame, a level badge, an AC shield, a spell slot |
| `hud_banner` | A place reached or a level gained |
| `hud_plate` | A name plate's bar over a creature |
| `hud_map_ring`, `hud_map_point` | A map ring, and where a thing goes on it |

The style module draws no glyphs and knows nothing about vertices. A realm's
renderer turns primitives into its own vertices and draws text in its own
faces.

## How a realm uses a style

Everglade's renderer is in
[`apps/openagents-web/charactercreator.coax`](../../apps/openagents-web/charactercreator.coax):

- `EG_HUD` names the realm's style, `HUD_FANTASY`, and `eg_tokens()` returns its
  tokens.
- `hud_emit` writes each primitive as one quad of six vertices. Its kind and
  alpha go in the vertex's mode float, and its bottom colour in the four floats
  that follow.
- `hud_seal` marks every vertex the frame drew. The fragment shader
  (`fivelab_hud_fs` in
  [`apps/openagents-web/gpu/fivelab.coax`](../../apps/openagents-web/gpu/fivelab.coax))
  draws marked vertices by coverage with straight alpha. Unmarked vertices draw
  as before.
- The page's face file exports `coaxial_fx_alpha_v1`, which tells
  `coax/web/host.js` to blend the HUD pipeline with straight alpha. With that
  blend, a dark panel can darken the world behind it. The older HUD blend can
  only add light.
- `hud_say` draws text in a role: a drop shadow or an outline, then the text.

Everglade's HUD parts are in
[`everglade/hud/parts.coax`](../../everglade/hud/parts.coax):
the portrait plate, the action bar's tray, the map ring, name plates, damage
numbers, the place banner and the frame rate's plaque. Each part draws its
frames before its words.

A realm that keeps SCIFI changes nothing. It doesn't export
`coaxial_fx_alpha_v1` and doesn't seal its vertices, so it draws exactly as
before. The 5E Lab shares Everglade Experimental's wasm and stays SCIFI:
`fivelab-face.c` turns the alpha blend on only on Experimental's page.

## Fonts

FANTASY uses two faces, both under the SIL Open Font License:

- Spectral Medium for reading: numbers, values and labels.
- Cinzel at weight 600 for titles: names, places, tabs and buttons.

The fonts and their licences are in
`apps/openagents-web/realms/everglade2/assets/fonts/`. A realm names them in
`apps/openagents-web/realms.json`: `ui_font_src` and `ui_font_family` for the
reading face, and `ui_title_font_src`, `ui_title_font_family` and
`ui_title_font_weight` for the title face. `coax/web/host.js` draws both into
one glyph atlas. The reading face goes in the red channel and the title face in
the green channel. Coax lays text out with the advance widths in
[`everglade/hud/fonts.coax`](../../everglade/hud/fonts.coax),
which were measured from the same font files.

## Engine widgets

[`aethyr/ui/hud-menu.coax`](../../aethyr/ui/hud-menu.coax) holds widgets any
realm uses with its own data:

- `hud_trail_step`, `hud_vital` and `hud_plate_trail` (in
  [`hud-style.coax`](../../aethyr/ui/hud-style.coax), GitHub #13147): a health
  bar that eases to its value and, after a blow, leaves a pale trail where it
  stood, which waits 0.3 s and then catches up; healing takes the trail along.
  A realm keeps one `HudTrail` a bar and steps it each frame with the true
  share. `hud_vital` draws the bar: a dark groove with a gold hairline, the
  trail, the fill running dark to bright toward its end, a gloss.
- `hud_vignette_step` and `hud_vignette` (in
  [`hud-style.coax`](../../aethyr/ui/hud-style.coax), GitHub #13147): the
  screen's edges in red. A blow on you flashes them, harder and longer for a
  bigger blow (gone within a second), and they pulse every 1.25 s while you are
  under a quarter of your Hit Points. A realm keeps one `HudVignette`, steps it
  with your share of Hit Points, and draws it before the rest of its HUD.
- `hud_side_box`, `hud_side_top` and `hud_side_draw`: a side bar of framed
  buttons over an action bar. Rows wrap to the page's width, so no button
  overlaps another at any width.
- `hud_menu`, `hud_menu_row` and `hud_menu_draw`: a framed menu that opens
  above a button, in as many columns as fit.
- `hud_tip`: a plaque with one line over what the pointer is on.
- `HudWords`: the words a widget sets. The realm's renderer draws them after
  the primitives (Everglade's `hud_draw`).

Pass a 44 px row on a phone so each button fits a finger.

## Plates follow what you can see

A name plate, its health bar, a player's name and a damage number show only
while their body shows from the camera. One engine path decides it:

1. The zone gives the eye, the body (its feet, its height) and its own
   colliders and land: boxes, capsules and convex hulls in world heights,
   and the heights of its land under the line.
2. `hud_sight_look` in `aethyr/ui/hud-sight.coax` asks the visibility
   service, `vis_line` in `aethyr/visibility.coax`, about three points on
   the body: its knees, chest and head. The body shows when any of them
   does. The service doesn't cross hulls yet, so `hud_sight_hull_cut` tests
   them on the same lines.
3. `hud_sight_step` settles the answer with `vis_settle`, so a plate at a
   wall's edge doesn't flicker, and fades the plate in or out over 0.15 s.
   The health bar uses the same answer as its plate.

Everglade II gathers its trunks, rocks and the camp's props
(`everglade2-sight.coax`), and Everglade Experimental its village, castle and
dungeon walls (`xc_sight_in`). Everglade v1 doesn't use it yet.

## The HUD system map

Each part is engine (any realm), game (every Everglade zone) or zone wiring
(one page's state).

| Layer | Part | Where |
| --- | --- | --- |
| Engine | Style tokens, text roles and part drawers, SCIFI and FANTASY | `aethyr/ui/hud-style.coax` |
| Engine | Side bar, menu, tip and words | `aethyr/ui/hud-menu.coax` |
| Engine | Sight for plates, health bars, names and damage numbers: a body as the visibility service's target, the settled answer faded over 0.15 s | `aethyr/ui/hud-sight.coax` on `aethyr/visibility.coax` |
| Engine | Where plates over bodies go: reach, order, lift or fold | `aethyr/ui/hud-plates.coax` |
| Engine | The straight-alpha HUD blend and the two-face glyph atlas | `coax/web/host.js` (`coaxial_fx_alpha_v1`, `sansAtlas`) |
| Game | Everglade picks FANTASY and draws primitives and words as vertices | `EG_HUD`, `hud_emit`, `hud_draw`, `hud_say` in `apps/openagents-web/charactercreator.coax` |
| Game | Unit frames (`eg_portrait`: your portrait plate, and your target's frame as its red mirror: a gold-rimmed medallion, a glass plate, the health bar with its damage trail), bar tray, a slot's cooldown sweep (`eg_slot_sweep`), map ring, name plates with their trails (`eg_plates`), the screen's red edges (`eg_vignette`), place banner, plaque; the world's layout (`eg_world`) | `everglade/hud/parts.coax` |
| Game | The class menus: the class features (Healer's Kit, Wild Companion, Find Familiar) and the rest menu with how the rest stands. Nothing of them is a button over the action bar (GitHub #13147): a press on your portrait plate opens them, hung under it (`eg_side_geo`, `eg_side_press`, `eg_side_draw`). Knocking out has no control on screen; in Experimental K turns it on and off | `everglade/hud/class-bar.coax` |
| Game | The CLASSES and LEVEL picker shared by Everglade's test pages: buttons a finger tall on phones and tablets, the classes as a grid that keeps over the action bar, the wild-shape swap (`cm_pick`, `cm_pick_hit`, `cm_pick_draw`) | `apps/openagents-web/class-menu.coax` |
| Game | The upright phone (#13189): `zone_compact` says when a zone's HUD is compact (a touch screen, portrait, under 700 points wide). The bar is one row with MORE opening the rest (`xc_slot`, `xc_more_box`), with no class plate over it (`eg_bar_frame` with no initial); the GFX, FPS and coordinate lines show only with `?net` (`xc_corner`); GYRO over JUMP opens the gyro's menu (`zt_gyro`, `zt_gyro_menu` on `hud_menu`) | `everglade/zone-touch.coax`, `apps/openagents-web/experimental-combat.coax`; the gyro in `aethyr/cam/gyro.coax` |
| Game | The faces' widths | `everglade/hud/fonts.coax`; the faces in `realms.json` |
| Zone wiring | Everglade II: its facts and its room's requests | `e2_world_view` in `everglade2.coax`; `e2k_facts`, `e2k_menu_do`, `e2k_side_bar` in `everglade2-cast.coax`; its colliders and land for sight in `everglade2-sight.coax`; plates in `e2_heads` (`everglade2.coax`), which adds the monsters' and the guard's from `everglade2-monsters.coax` and `everglade2-guard.coax` |
| Zone wiring | Character Create's wardrobe | `everglade2-create.coax` |
| Zone wiring | Everglade Experimental: its facts for the class menus and its rest requests (`xc_facts`, `xc_side_act`), where the picker sits (`xc_pick`), the coordinates and FPS lifted over the bar (`hud_floor`), and its walls and meadow for sight (`xc_sight_in`) | `experimental-combat.coax` |

Everglade v1 (`/worlds/everglade` and `/worlds/everglade-classic`) draws its HUD
with the Filament stroke font and its own shaders, and doesn't use these parts
yet.

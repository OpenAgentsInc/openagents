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
[`apps/openagents-web/everglade-hud.coax`](../../apps/openagents-web/everglade-hud.coax):
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
[`apps/openagents-web/everglade-fonts.coax`](../../apps/openagents-web/everglade-fonts.coax),
which were measured from the same font files.

## The HUD system map

What is shared and what each realm owns:

| Shared | Where |
| --- | --- |
| Style tokens, text roles and part drawers | `aethyr/ui/hud-style.coax` |
| HUD primitives such as `HudCoord` | `aethyr/ui/hud.coax` |
| The SCIFI instrument frames | `verse/hud/frame.coax`, `aethyr/ui/draw.coax`, `aethyr/ui/arwes-*.coax` |
| The straight-alpha HUD blend | `coax/web/host.js` (`coaxial_fx_alpha_v1`) |
| The two-face glyph atlas | `coax/web/host.js` (`sansAtlas`, `data-ui-title-font-*`) |

| Per realm | Everglade's |
| --- | --- |
| Which style | `EG_HUD` in `charactercreator.coax` |
| The renderer from primitives to vertices | `hud_emit`, `hud_seal`, `hud_say` in `charactercreator.coax` |
| The HUD parts and their data | `everglade-hud.coax`; Everglade II's world HUD in `everglade2.coax` (`e2_world_hud`), its bar in `everglade2-cast.coax`, its plates in `everglade2-monsters.coax` and `everglade2-guard.coax`; Experimental's in `experimental-combat.coax` |
| Character Create's wardrobe | `everglade2-create.coax` |
| The faces | `realms.json` and `everglade-fonts.coax` |

Everglade v1 (`/worlds/everglade` and `/worlds/everglade-classic`) draws its HUD
with the Filament stroke font and its own shaders. It doesn't use this
renderer and keeps its look.

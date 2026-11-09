# HUD overlap check: first findings (2026-11-09)

GitHub #13076. This page records what the HUD overlap check found on the first run
after the fantasy HUD landed, and which issue fixes each finding.

## How the check works

Each HUD part records the rectangle and kind (panel, text, button, bar) of each
widget it draws, through an optional layout trace (`aethyr/ui/trace.coax`). With no
trace attached, recording costs one count check per widget.

The trace is filled in shared code, so every realm is covered from one place:

- The engine widgets in `aethyr/ui/hud-menu.coax`: the side bar
  (`hud_side_draw`), menus (`hud_menu_draw`) and tips (`hud_tip`). Their words keep
  the box that holds them.
- `hud_draw` and `hud_say_ui` in `charactercreator.coax`, the one place a HUD run of
  text is traced.
- Everglade's parts in `everglade/hud/parts.coax`: the portrait plate, the map
  ring, and the bar's tray with its slots and emblem.
- The CLASSES column (`class-menu.coax`), Character Create (`e2c_hud`), the frame
  rate and coordinates, the GFX control, and the developer panels.

The harnesses lay out each realm's HUD at 390x844, 844x390, 1024x768 and
1920x1080. The first three count as touch screens. They cover the studio, each
phone tab, the world for every class, the rest controls, each side-bar tip and
menu, a slot's card, a druid's wild shape, being down, and the developer panels
open.

The check (`uit_check`) reports these faults:

- overlap: two widgets share area and neither holds the other.
- overflow: a widget comes out of the widget that holds it.
- small: a button is under 44 CSS px either way on a touch screen.
- offscreen: a widget runs past the screen's edge.

Each fault is reported once per realm, where it starts. A panel over a button
isn't reported again for each widget the panel holds. Words that overflow their
own button count as one overflow, not also as overlaps with each neighbour.
Findings in developer tools (the animation list, the admin panel, and any screen
with one open) are counted apart and don't gate.

Run the check:

```sh
coax/remote/remote.sh run -- coax/web/tool.sh apps/openagents-web/hud-overlap-everglade2.coax
coax/remote/remote.sh run -- coax/web/tool.sh apps/openagents-web/hud-overlap-experimental.coax
```

Each finding prints as `NEW`, `known` or `debug`. A NEW finding in the player's
HUD fails the run. A known key that no longer shows up prints as `gone`; take it
out of the harness's known list (`H2_KNOWN`, `HX_KNOWN`).

## Counts

One finding is one fault on one pair of widgets in a realm, wherever it shows up.
"Before" is the first run, on the HUD as it stood before the fantasy HUD.
"Now" is the run on main at a2fc80c036, after the side bar, the CLASSES picker
(#13092) and the engine widgets' fixes (#13103).

| Realm | Before | Now | Developer tools (now) |
|---|---|---|---|
| Everglade II | 82 | 59 | 73 |
| Everglade Experimental | 67 | 17 | 50 |

The check now covers more than the first run did: the side bar, its menus and
tips, each phone tab, and the desktop studio at 1024x768. The #13103 fixes
cleared every engine-widget finding: tips wrap inside their plaque, menu and
side-bar labels fit their buttons, and side-bar rows are 44 px on a tablet.

## Everglade II

### Character Create (#13089), 52 findings

- At 1024x768 (a tablet, so touch), every studio row is 30 px tall and each
  row's `<` and `>` buttons are 23 px wide. NAME, RESET, SURPRISE ME, SAVE and
  USE are 34 px tall. The phone layout passes.
- At 1024x768 the title "Character Create" is wider than its card.
- "USE IN EVERGLADE" is wider than the USE button.

### The GFX and frame-rate plaque (#13090), 5 findings

- On a 390x844 phone, the GFX control sits under the bar's emblem, the side
  bar's tray (with the rest controls showing) and a slot's card. The frame rate
  sits under the card.
- At 844x390 the GFX control overlaps the bar's tray.

### The bar's emblem (#13092), 2 findings

- The emblem sits 2 px above the slots (`eg_bar_frame`, shared with
  Experimental). On a phone it rises into the side bar's tray and a slot's card.

## Everglade Experimental

### The bar, the picker and the side bar's place (#13092), 17 findings

- On a 390x844 phone the coordinates and the frame rate sit on the bar's tray,
  on the open REST menu, and on a side-bar tip.
- The emblem rises into the side bar's tray and a slot's card, and the card
  overlaps the side bar's tray.
- The familiar slot's caption "Cat · Fey" is wider than its slot.
- At 844x390 the CLASSES picker covers the emblem, the "class and target" line,
  the bar's tray (with the wild-shape row showing) and a slot's card. The open
  REST menu and its tip overlap the picker and the portrait plate, and the
  coordinates sit on the side bar's tray.

## Developer tools (not gated)

- The animation list's rows are 24 px.
- The admin panel's boxes and its CLASSES column are under 44 px.
- With the animation list open, Experimental's picker and the bar meet it.

# Appendix A. A fantasy HUD

> **Status: partly implemented.** The style is in
> [`aethyr/ui/hud-style.coax`](../../aethyr/ui/hud-style.coax). Everglade's HUD
> parts are in
> [`everglade/hud/parts.coax`](../../everglade/hud/parts.coax),
> and its renderer is in
> [`apps/openagents-web/charactercreator.coax`](../../apps/openagents-web/charactercreator.coax).
> [HUD styles](../../docs/verse/hud-styles.md) is the reference.
> This appendix names functions and has no source listings yet.

This appendix answers the player's question: "Why does Everglade's screen look
like a fantasy game and not a starship?"

The Verse's HUD began as an instrument panel: grey and cyan glow lines, thin
bars and cut corners. That suits a space realm. Everglade is a fantasy world, so
it gets its own HUD style, FANTASY. The rest of the Verse keeps SCIFI and looks
exactly as before.

## What this appendix teaches

- How to split what a HUD shows from how it looks.
- How to draw dark panels over a bright world.
- How to set text in two faces with one glyph atlas.
- How to lay out a studio for a phone held either way.

## What exists today

- **One style per realm.** `hud_tokens(HUD_FANTASY)` returns iron and bronze
  frames, warm gold type, parchment, framed segmented bars, recessed slots and
  a gold ribbon banner. `hud_text` gives each text role one face, colour and
  size. Drawers such as `hud_panel`, `hud_bar`, `hud_slot` and `hud_banner`
  add primitives to a list. They don't know about vertices or glyphs.
- **Ornaments drawn as shapes.** Every frame, stud, gem and shield is built
  from rounded boxes, rings, shadows and sweeps. The HUD loads no pictures for
  its frames.
- **Dark panels.** Everglade's page exports `coaxial_fx_alpha_v1`, so the host
  blends the HUD with straight alpha. `hud_seal` marks the frame's vertices,
  and the shader draws each marked primitive by its coverage. The older HUD
  blend could only add light, so a dark panel used to vanish.
- **Two faces.** Spectral Medium sets numbers and lines. Cinzel at weight 600
  sets names, places, tabs and buttons. Both are under the SIL Open Font
  License, and their licences sit beside them in
  `apps/openagents-web/realms/everglade2/assets/fonts/`. The host draws both
  into one atlas, in its red and green channels.
- **The world's HUD.** Everglade II shows a portrait plate at the top left
  (name, class, level, health, AC, spell slots and conditions), a round map at
  the top right (you, the path, open ground, creatures, a north mark, the place
  and the time of day), a framed action bar with the class's emblem, name
  plates over creatures, outlined damage numbers and a banner when you enter a
  new place (`eg_world`, which Everglade II wires in `e2_world_view`). Everglade Experimental shows the same parts.
- **Character Create on a phone.** Held upright, the studio is a short card
  at the top (the title, ROTATE, SAVE and USE IN EVERGLADE) and a sheet at the
  bottom. The sheet has five tabs: BODY, FACE, GEAR, PARTS and CLASS. You stay
  in view between them. Held sideways, the card is on the left and the sheet on
  the right. Every control is at least 44 px tall, and the layout keeps clear of
  the notch and the home bar (`e2c_layout`).
- **Plates you can see.** A plate, its health bar and a damage number show
  only while the body under them shows from the camera, not through a trunk,
  a rock, a wall or a hill. The engine asks its visibility service
  (`hud_sight_look`) and fades the plate over 0.15 s (`hud_sight_step`).
- **The frame rate.** The frame rate and the GFX control sit on a small plaque
  in the bottom-right corner. On a phone it sits above the action bar and the
  side bar, and in the studio it sits clear of the sheet (`e2_fps`,
  `e2_fps_clear`).

## The checks

In `aethyr/ui/hud-style.coax`:

- The style named `"fantasy"` is FANTASY, and anything else is SCIFI.
- A tray leaves each slot's picture clear.

In `apps/openagents-web/charactercreator.coax`:

- `#test "eg portrait: a plate with the name, the class, health and AC; nothing
  it does not know"`.
- `#test "eg map: you in the middle, ahead up; the clock reads the day"`.

In `everglade/client/create.coax`:

- `#test "create: the wardrobe fits a phone held either way and a desktop, a
  finger's size, clear of you"`.

The headless captures are in
[`docs/verification/everglade-fantasy-hud-2026-11-09/`](../../docs/verification/everglade-fantasy-hud-2026-11-09/),
before and after, at 1920×1080, 390×844 and 844×390.

## Mistakes

- **Black boxes.** The first FANTASY panels drew as solid black squares. The
  host blended the HUD with straight alpha, but some vertices weren't sealed,
  and an unsealed vertex returns an alpha of 1. Now `hud_seal` marks every
  vertex the frame draws, and SCIFI realms don't turn the blend on.
- **A studio built for a desktop.** On a phone, Character Create's panel
  covered most of the screen. Labels ran through their dividers, swatches
  spilled out of their rows and buttons were too small to tap. The tabs and the
  layout check above replaced it.

## What's left

- Safe-area insets are fixed iPhone values. The host doesn't yet pass the
  browser's real insets.
- Everglade has no quests, chat or experience points yet, so the HUD has no
  quest tracker, chat box or experience bar.
- Everglade v1 keeps its own HUD.

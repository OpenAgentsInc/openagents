# 86. Everyone can play

> **Status: unimplemented — design draft.** Little exists today. The
> browser host honours `prefers-reduced-motion` by drawing the finished
> frame at once ([`coax/web/host.js`](../../coax/web/host.js)). Each realm
> page keeps its text in the HTML for screen readers while the canvas is
> `aria-hidden`. Everglade's voice shows a caption of what it heard
> (`everglade-voice.coax`). The stroke font draws 77 glyphs, with lowercase
> drawn as capitals and Latin-1 accents as their base letter. There is no
> input remapping, no text-size setting, no subtitles for realm sounds and
> no translation. This chapter plans them. Nothing in `aethyr/` runs yet.

A realm should work for a player who can't hear its sound, can't see small
text, can't use a mouse, or reads another language. This chapter adds the
settings and checks that make that true for every realm at once, because
they live in Aethyr's input, UI and sound modules, not in each realm.

## What this chapter teaches

- How the action table of [chapter 4](ch004-input-becomes-typed-actions.md)
  becomes a remapping a player can change.
- How text size, contrast and reduced motion apply through the UI of
  [chapter 15](ch015-ui-and-the-hud.md).
- How every sound that carries information gets a caption.
- How text gets stable IDs, so it can be translated and checked.

## Outline

1. **Remapping.** The device-to-action table is data, so a player can bind
   any action to another key, button or touch control. Hold-to-run and
   toggle-to-run are both offered. The table is saved in the realm's save
   slot ([chapter 82](ch082-save-and-come-back.md)).
2. **Timing.** A setting slows the realm's simulation clock for timed
   challenges, through the time controls of
   [chapter 1](ch001-a-frame-has-a-fixed-step.md), and replays still match.
3. **Reduced motion.** The host already reads `prefers-reduced-motion`.
   Aethyr passes it to the camera of [chapter 10](ch010-the-camera-follows.md)
   (no shake, shorter flights) and to effects.
4. **Text size and contrast.** A text scale (1.0, 1.25, 1.5) applies to the
   stroke font, and the UI layout still fits the safe area. Information
   never depends on colour alone.
5. **Captions.** Each sound source of [chapter 68](ch068-sound-in-the-world.md)
   that carries information has a caption ID and a direction. Captions
   draw in the HUD with the speaker's name, as Everglade's voice caption does
   today.
6. **Screen readers.** The page's hidden HTML text stays the accessible
   copy of the realm; Aethyr adds a short live region for captions and
   HUD changes.
7. **Text IDs and languages.** Every HUD string gets a stable ID in a
   table. The stroke font gains lowercase and accented glyphs (its stage
   3). Pseudolocalization with 40 percent longer strings checks that
   layouts fit.

## What exists today

- [coax/web/host.js](../../coax/web/host.js): `prefers-reduced-motion`
  and the typed-text field's `aria-label`.
- [coax/web/realms.ts](../../coax/web/realms.ts): realm pages with
  `lang="en"`, an `aria-hidden` canvas and text kept for screen readers.
- [apps/openagents-web/everglade-voice.coax](../../apps/openagents-web/everglade-voice.coax):
  the heard caption.
- [apps/openagents-web/font.coax](../../apps/openagents-web/font.coax):
  the stroke font and its glyph list.
- [docs/research/game-engine-systems.md](../../docs/research/game-engine-systems.md),
  section 42: the inventory this chapter draws from.

## What it builds

- `aethyr/access.coax`: the settings record (remapping, run mode, time
  scale, reduced motion, text scale, captions) and its defaults.
- `aethyr/ui/strings.coax`: the text ID table and the pseudolocalization
  pass.
- Caption records attached to sound sources.

## Acceptance checks and oracles

The chapter is done when:

- Every action in every realm can be rebound, and a test plays a realm
  with all keys moved.
- With reduced motion on, no camera move exceeds a stated angular speed
  and no screen shake runs.
- At text scale 1.5, no HUD element in any realm leaves the safe area at
  390x844.
- Every sound marked as information has a caption, checked by a scan of
  the realm's sound table.
- Pseudolocalized strings fit, and a missing text ID fails the build.
- The settings change no session digest except through the time scale,
  which is recorded in the session.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md) (time control),
[4](ch004-input-becomes-typed-actions.md) (the action table),
[10](ch010-the-camera-follows.md) (camera motion),
[15](ch015-ui-and-the-hud.md) (text and layout),
[68](ch068-sound-in-the-world.md) (sound sources) and
[82](ch082-save-and-come-back.md) (saved settings).

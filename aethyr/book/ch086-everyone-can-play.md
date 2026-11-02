# 86. Everyone can play

> **Status: unimplemented — design draft.** Little exists today. The
> browser host honours `prefers-reduced-motion` by drawing the finished
> frame at once ([`coax/web/host.js`](../../coax/web/host.js)). Each realm
> page keeps its text in the HTML for screen readers while the canvas is
> `aria-hidden`. Everglade's voice shows a caption of what it heard
> (`everglade-voice.coax`). The stroke font draws 77 glyphs, with lowercase
> drawn as capitals and Latin-1 accents as their base letter. A made realm's
> controls are also real ones: the page describes them a line each
> (`apps/openagents-web/myrealm-a11y.coax`) and the host mirrors them as
> buttons and checkboxes in a named toolbar beside the canvas, first in the
> tab order, with the page's note as a polite live region, so the made-realm
> route works keyboard-only and with a screen reader; a controller's stick
> and button go through `aethyr/input.coax`. There is no input remapping, no
> text-size setting, no subtitles for realm sounds and no translation. This
> chapter plans them. Nothing else in `aethyr/` runs yet.

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
   layouts fit. The languages this chapter supports are listed in
   [Supported languages](#supported-languages).

## Supported languages

> **Design draft.** Not implemented.

The chapter's scope is explicit. Aethyr supports a language only when the
font draws its script, the layout handles its direction and line breaking,
and the input path accepts its text. This chapter's first scope is:

| Scope | Status in this chapter | What it requires |
| --- | --- | --- |
| English | Supported | Lowercase glyphs and the string table. |
| Languages written in Latin script with Latin-1 accents (for example French, German, Spanish and Portuguese) | Supported after the font's stage 3 | Accented glyphs drawn as accented letters, not as their base letter, and the pseudolocalization check. |
| Right-to-left scripts (Arabic, Hebrew) | Not supported | Bidirectional layout, mirrored UI and glyph shaping. A separate chapter plans them. |
| Chinese, Japanese and Korean | Not supported | Thousands of glyphs, line breaking without spaces and a glyph atlas instead of strokes. A separate chapter plans them. |
| Text entry through an input method editor (IME) | Not supported | Composition events from the host, a composition preview and candidate windows. A separate chapter plans it. |

Pseudolocalization establishes less than it appears to. It replaces Latin
letters with accented Latin letters and lengthens strings by 40 percent, so
it checks that a layout fits longer text and that no string is hard-coded. It
doesn't check right-to-left layout, the glyphs and line breaking of Chinese,
Japanese or Korean, or IME composition. A realm can't claim any of those from
a pseudolocalization pass. Each needs its own font coverage, layout rules and
checks with real text in that script.

## What exists today

- [coax/web/host.js](../../coax/web/host.js): `prefers-reduced-motion`
  and the typed-text field's `aria-label`.
- [coax/web/realms.ts](../../coax/web/realms.ts): realm pages with
  `lang="en"`, an `aria-hidden` canvas and text kept for screen readers.
- [apps/openagents-web/everglade-voice.coax](../../apps/openagents-web/everglade-voice.coax):
  the heard caption.
- [apps/openagents-web/font.coax](../../apps/openagents-web/font.coax):
  the stroke font and its glyph list.
- [apps/openagents-web/myrealm-a11y.coax](../../apps/openagents-web/myrealm-a11y.coax):
  a made realm's accessible layer. Every control the canvas draws is a line
  `ID ROLE STATE LABEL` (`button`, `checkbox`, `status`; pressed, checked or
  neither); `realm_a11y_do` does what a tap on it does. The owner's MOVE,
  UNDO, REDO, SAVE and PUBLISH, the picked thing's TURN IT, MAKE IT BIGGER,
  MAKE IT SMALLER, TAKE IT AWAY and LET IT GO, the BUILD pane's things and
  the board's rows (checkboxes) are all there; a visitor gets only what a
  visitor may press. R, + and - and Delete turn, size and take away the
  picked thing from the keyboard. `realm_pad` takes a controller's stick and
  first button through `aethyr/input.coax` (its dead zone, a junk number
  counted as no push, a held button one press).
- [coax/web/host.js](../../coax/web/host.js) `a11yMirror` and `padPoll`: the
  host only mirrors. It keeps the module's order, keeps focus in the toolbar
  when a control goes, lets Tab, Enter and Space be the browser's in it, and
  hides the toolbar until something in it has focus.
- [coax/web/forge-access.coax](../../coax/web/forge-access.coax): the check in
  muted headless Chrome (keyboard only, names and roles, the live region,
  lifecycle, a guest's keys and a stand-in controller). On coderos-4080 it
  runs only on SwiftShader, whose WebGPU device Chrome destroys every few
  seconds, so the run stops when the host gives up after four losses in a
  minute: its keyboard and accessibility steps passed there; the whole run
  needs a machine whose Chrome has a real GPU (NEEDS_OWNER.md).
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
- Every string in the supported Latin-script languages draws with its
  accents, checked by rendering each glyph of Latin-1 and comparing it with
  its base letter.
- The realm's settings list only the supported languages. A request for an
  unsupported language or script falls back to English and says so.
- The settings change no session digest except through the time scale,
  which is recorded in the session.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md) (time control),
[4](ch004-input-becomes-typed-actions.md) (the action table),
[10](ch010-the-camera-follows.md) (camera motion),
[15](ch015-ui-and-the-hud.md) (text and layout),
[68](ch068-sound-in-the-world.md) (sound sources) and
[82](ch082-save-and-come-back.md) (saved settings).

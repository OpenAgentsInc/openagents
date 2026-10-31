# 15. UI and the HUD

> **Status: unimplemented — design draft.** Today the hotbar layout
> (`verse/hud.coax`) and the HUD frame (`verse/hud/frame.coax`) are shared
> Coax, while each realm draws its own panels, labels and touch controls.
> This chapter plans Aethyr's UI layer: layout in screen space, text, hit
> areas and actions, drawn in one pass over the scene. Nothing here runs yet.

The HUD is the one part of a realm that lives in screen space: points on a
glass that may be a narrow phone with a home indicator or a wide desktop.
It has to lay out the same everywhere, be touchable where it is drawn, and
turn touches into the same typed actions keys produce.

## What this chapter teaches

- Screen space in points, y down, with safe-area insets.
- Layout as pure functions from the viewport to rectangles.
- Hit areas that equal the drawn rectangles.
- Text from a stroke font drawn on the GPU.
- UI input becoming chapter 4's typed actions.

## The method

The hotbar centres `count` square slots of 44 points with 6-point gaps above
the bottom inset, shrinking the slots if the row is wider than the screen
less a 16-point margin:

```jai
total = count · (size + gap) − gap
if total > width − 16 { size = (width − 16 − gap · (count − 1)) / count }
slot_i.x = (width − total) / 2 + i · (size + gap)
slot_i.y = height − bottom − size − 12
```

The HUD frame is a faint outline with 45° cuts on two corners and bright L
brackets on the other two, its arm at most a quarter of the box's width.
A touch at `p` is the action of the slot whose rectangle holds `p`; the
rectangle comes from the same function that placed the drawing.

## What exists today

- [verse/hud.coax](../../verse/hud.coax): the `verse.hud` module,
  `verse_hud_slot`, with checks that the hotbar fits a 320-point phone above
  the home indicator and that slots stay separate in landscape.
- [verse/hud/frame.coax](../../verse/hud/frame.coax): the frame's points,
  arms, cuts and greys, shared by the web landing and the iOS app.
- [verse/light.coax](../../verse/light.coax): the slot and bar shading
  (`verse_slot_light`, `verse_slot_cover`, `verse_bar`, `verse_slot_icon`).
- Filament, the stroke font in `apps/openagents-web/font.coax`
  ([coax/font.md](../../coax/font.md)), and GPUI's atlas for Geist in
  `coax/web/gpui.js`.
- [verse/arrange.coax](../../verse/arrange.coax): a close-up view where
  point and click place or take out objects.

## The design in Aethyr

A UI tree is built each frame from the realm's state: panels, labels,
buttons and slots, each a rectangle from a layout function and an optional
action ID. The UI pass draws the tree after the scene and effects. The same
tree answers hit tests, so no realm keeps separate touch geometry. A touch
or click on an element becomes a typed action through chapter 4's mapping,
sampled once per step like any key. Text is laid out from the font's
advance widths and drawn as strokes in the UI pass.

## What it builds

- `aethyr/ui/layout.coax`: safe areas, rows, columns, the hotbar and the
  frame (moved from `verse/hud`).
- `aethyr/ui/tree.coax`: elements, hit tests and action IDs.
- `aethyr/ui/text.coax`: text layout over the stroke font.

## Acceptance checks and oracles

- For every element, the drawn rectangle equals the hit rectangle; a check
  over phone, tablet and desktop viewports in both orientations.
- Layout fits: nothing crosses a safe-area inset at 320×700 with a 34-point
  bottom inset, and no two slots overlap at 900×420.
- A tap and the matching key produce the same action in the same step.
- Goldens with HUDs (`home-phone.png`, `the-grid.png`) are unchanged after
  the move.

## Dependencies

Chapters 2 (screen space), 4 (typed actions) and 11 (the pass graph). From
Coax: R12 units for points and pixels.

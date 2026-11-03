# 15. UI and the HUD

> **Status: partly implemented.** Visual component foundations. The hotbar
> layout (`verse/hud.coax`) and frame (`verse/hud/frame.coax`) remain shared
> Coax. `aethyr/ui/hud.coax` adds rectangles, hit tests, focus traversal and
> responsive catalog layout; `aethyr/ui/draw.coax` adds eighteen HUD drawing
> components and consumer-supplied rows, meters, messages and advisories.
> The standalone HUD laboratory composes them into five local demo scenes.
> Its sixth tab adapts ARWES frame, motion/text, background and theme/effect
> algorithms into 25 more specimens, with a pinned source ledger and MIT notice.
> The full screen-space UI tree, typed-step integration, semantic platform
> mirror and complete accessibility journey below remain a design draft.

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
- Focus order, semantic controls and keyboard activation, so the whole UI
  works from a keyboard and a screen reader.

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
- [aethyr/ui/hud.coax](../ui/hud.coax): logical screen rectangles,
  half-open hit areas, labelled/enabled controls, keyboard focus traversal,
  responsive catalog cells and arc/segment geometry.
- [aethyr/ui/draw.coax](../ui/draw.coax): eighteen callable visual
  components using the existing frame and font, plus typed row, meter,
  message and advisory inputs. Drawing uses the existing twelve-float
  stroke/sans vertex convention; it does not own live measurements or
  authorize commands.
- [HUD laboratory](../../apps/openagents-web/demos/hud/README.md): a
  component catalog with inspectors and Grid command, navigation,
  engineering and communications scenes. Their explicit local actions
  demonstrate shared drawn/hit boxes, focus, contact selection, route/range
  previews, event acknowledgment and command feedback. The
  [reference study and reuse contract](../../docs/verse/2026-11-01-hud-library.md)
  distinguish these foundations from the full UI tree planned here.
- The [ARWES Coax ports](../ui/arwes/SOURCES.md): all nine frame families,
  31 easing curves, lifecycle and child timing, text reveal/decipher, typed
  theme/style patterns, backgrounds, pointer lighting and a silent cue model.
  The [porting report](../../docs/verse/2026-11-01-arwes-coax.md) records exact
  sources, the MIT notice and the boundary between reusable algorithms and
  DOM/framework/audio effects owned by hosts.
- [verse/light.coax](../../verse/light.coax): the slot and bar shading
  (`verse_slot_light`, `verse_slot_cover`, `verse_bar`, `verse_slot_icon`).
- Filament, the stroke font in `apps/openagents-web/font.coax`
  ([coax/font.md](../../coax/font.md)), and GPUI's atlas for Geist in
  `coax/web/gpui.js`.
- [aethyr/arrange.coax](../../aethyr/arrange.coax): a close-up view where
  point and click place or take out objects.

## The design in Aethyr

A UI tree is built each frame from the realm's state: panels, labels,
buttons and slots, each a rectangle from a layout function and an optional
action ID. The UI pass draws the tree after the scene and effects. The same
tree answers hit tests, so no realm keeps separate touch geometry. A touch
or click on an element becomes a typed action through chapter 4's mapping,
sampled once per step like any key. Text is laid out from the font's
advance widths and drawn as strokes in the UI pass.

## Accessibility: focus, semantics and a full journey

The UI tree is also the accessibility tree. Every interactive element has the
facts a keyboard user and a screen reader need, and the realm's UI is complete
only when every task in it works without a pointer.

### Semantic controls

Each element in the tree declares:

- **Role:** `button`, `toggle`, `slider`, `list`, `list item`, `tab`,
  `dialog`, `text` or `image`.
- **Label:** a text ID from [chapter 85](ch085-everyone-can-play.md)'s
  string table, never text drawn only as strokes.
- **State:** pressed, selected, checked, disabled or expanded, as the role
  requires, and a slider's value and range.
- **Action:** the action ID that `activate` produces.

An interactive element without a label is a layout error that the check
reports. An image that carries information has a label; a decorative image is
marked decorative and is skipped.

### Focus order

Focus moves through the interactive elements in reading order: top to bottom,
then start to end of the line, by the drawn rectangles. A container can
declare an explicit order when reading order is wrong, and the check refuses
an explicit order that skips an interactive element. These rules apply:

1. A dialog traps focus. `focus_next` from its last element goes to its
   first, and `back` closes it and returns focus to the element that opened
   it.
2. Focus is always visible. The focused element is drawn with an outline at
   least 2 points wide that contrasts with the panel behind it.
3. When the focused element disappears, focus moves to the next element in
   order, or to the container when none is left. It never moves to nothing.

### The host's accessibility tree

The UI is drawn on the GPU, so the host mirrors the tree to the platform's
accessibility layer each time it changes: hidden DOM elements with ARIA roles
in the browser (as Filament's HTML twins do for page text), `UIAccessibility`
on iOS and `NSAccessibility` on macOS. A screen reader's activation arrives
as `activate` through [chapter 4](ch004-input-becomes-typed-actions.md).

### The complete journey

The acceptance check is one scripted journey, run twice: once with the
keyboard only and once through the browser's screen-reader path. No step uses
a pointer:

1. On the landing page, move focus to the realm list and choose a realm.
2. In the realm, open the menu with `menu`.
3. Move to the settings, change the text scale and turn on reduced motion.
4. Close the settings with `back`, and confirm that focus returns to the
   settings button.
5. Use a hotbar slot with the keyboard.
6. Open a dialog, confirm that focus is trapped, and close it.
7. Leave the realm and return to the landing page.

At every step, the check records the focused element and what the
accessibility tree reports for it (role, label and state), and compares them
with the expected values.

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
- Every interactive element has a role and a label, and focus order visits
  every interactive element exactly once.
- The complete journey passes with the keyboard only and through the
  screen-reader path, with the expected focus, role, label and state at
  every step.

## Dependencies

Chapters 2 (screen space), 4 (typed actions) and 11 (the pass graph). From
Coax: R12 units for points and pixels.

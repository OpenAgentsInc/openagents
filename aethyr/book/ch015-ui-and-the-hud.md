# 15. UI and the HUD

> **Status: partly implemented.** The UI tree is
> [aethyr/ui/tree.coax](../ui/tree.coax): elements with a role, a label, a
> state and an action, drawn, tapped and focused from the same rectangles.
> It refuses an interactive element without a label, moves focus in reading
> order, traps it in a dialog and never moves it to nothing. The hotbar
> layout (`verse/hud.coax`) and frame (`verse/hud/frame.coax`) remain shared
> Coax, and [aethyr/ui/hud.coax](../ui/hud.coax) and
> [aethyr/ui/draw.coax](../ui/draw.coax) hold the rectangles and the
> eighteen drawing components. No realm builds its HUD as a tree yet, and the
> platform accessibility mirror and the complete keyboard and screen-reader
> journey remain a design (see [Still to do](#still-to-do)).

The HUD is the one part of a realm that lives in screen space: points on a
glass that may be a narrow phone with a home indicator or a wide desktop.
It has to lay out the same everywhere, be touchable where it is drawn, and
turn touches into the same typed actions keys produce.

## What this chapter teaches

- Screen space in points, y down, with safe-area insets.
- Layout as pure functions from the viewport to rectangles.
- Hit areas that equal the drawn rectangles.
- UI input becoming chapter 4's typed actions.
- Focus order, semantic controls and keyboard activation, so the whole UI
  works from a keyboard and a screen reader.

## The hotbar

The hotbar centres `count` square slots of 44 points with 6-point gaps above
the bottom inset, shrinking the slots if the row is wider than the screen
less a 16-point margin:

```jai
total = count · (size + gap) − gap
if total > width − 16 { size = (width − 16 − gap · (count − 1)) / count }
slot_i.x = (width − total) / 2 + i · (size + gap)
slot_i.y = height − bottom − size − 12
```

`verse_hud_slot` in [verse/hud.coax](../../verse/hud.coax) is this layout.
The HUD frame is a faint outline with 45° cuts on two corners and bright L
brackets on the other two, its arm at most a quarter of the box's width
([verse/hud/frame.coax](../../verse/hud/frame.coax)).

## The UI tree

A UI tree is built each frame from the realm's state: panels, labels,
buttons and slots, each with a rectangle from a layout function. The same
table is drawn, hit-tested and focused, so no realm keeps separate touch
geometry. It is also the accessibility tree.

<!-- source: aethyr/ui/tree.coax node -->
```jai
UI_TEXT :: 0; UI_IMAGE :: 1;                       // not interactive
UI_BUTTON :: 2; UI_TOGGLE :: 3; UI_SLIDER :: 4;    // interactive
UI_LIST_ITEM :: 5; UI_TAB :: 6;
UI_DIALOG :: 7;                                    // a container that traps focus
UI_ROLES :: 8;
UI_NONE :: -1;
UI_MAX :: 128;    // elements in one tree

// One element. `label` is a text ID in the realm's string table (chapter 85), 0 for none;
// `parent` is the index of the dialog it is in, or UI_NONE.
UiNode :: struct {
  box: HudBox;
  role: int;
  label: int;
  action: int;            // what activating it produces, 0 for nothing
  parent: int = UI_NONE;
  enabled: bool = true;
  pressed: bool;          // a toggle's or a tab's state
  value: f32; low: f32; high: f32;   // a slider's
  decorative: bool;       // an image that carries nothing: skipped
}

ui_interactive :: (n: UiNode) -> bool { return n.role >= UI_BUTTON && n.role <= UI_TAB; }
// What the keyboard and the screen reader can reach: interactive and enabled.
ui_focusable :: (n: UiNode) -> bool { return ui_interactive(n) && n.enabled; }
```

Each element declares its role, its label as a text ID in the realm's string
table ([chapter 85](ch085-everyone-can-play.md)), never text drawn only as
strokes, its state (a toggle's or tab's `pressed`, a slider's value and
range), and the action that activating it produces. An element inside a
dialog names the dialog as its `parent`.

### A tree that can't be read is refused

<!-- source: aethyr/ui/tree.coax check -->
```jai
UI_OK :: 0;
UI_ROLE :: 1;       // a role that isn't one
UI_LABEL :: 2;      // an interactive element, or an image that carries something, with no label
UI_ACTION :: 3;     // an interactive element that does nothing
UI_PARENT :: 4;     // a parent that isn't an earlier dialog
UI_RANGE :: 5;      // a slider whose value is outside its range
UI_COUNT :: 6;      // no elements, or more than UI_MAX
UiProblem :: struct { index: int = UI_NONE; reason: int = UI_OK; }

// The first element the tree can't have, and why; a tree is drawn only when there is none.
ui_check :: (t: [] UiNode) -> UiProblem {
  if t.count == 0 || t.count > UI_MAX { return UiProblem.{reason = UI_COUNT}; }
  for i: 0..<t.count {
    n := t[i];
    why := UI_OK;
    if n.role < 0 || n.role >= UI_ROLES { why = UI_ROLE; }
    else if n.label <= 0 && (ui_interactive(n) || (n.role == UI_IMAGE && !n.decorative)) { why = UI_LABEL; }
    else if ui_interactive(n) && n.action <= 0 { why = UI_ACTION; }
    else if n.parent != UI_NONE && (n.parent < 0 || n.parent >= i || t[n.parent].role != UI_DIALOG) { why = UI_PARENT; }
    else if n.role == UI_SLIDER && !(n.low <= n.value && n.value <= n.high) { why = UI_RANGE; }
    if why != UI_OK { return UiProblem.{index = i, reason = why}; }
  }
  return UiProblem.{};
}
```

`ui_check` refuses the first element that has no role, an interactive
element or an image that carries information without a label, an
interactive element that does nothing, a parent that isn't an earlier
dialog, or a slider outside its range. A decorative image is marked
`decorative` and needs no label. The realm doesn't draw a refused tree.

### Focus

<!-- source: aethyr/ui/tree.coax focus -->
```jai
// Whether element a comes before b in reading order: top to bottom by their top edges, then start
// to end along the line, then the table's order.
ui_before :: (t: [] UiNode, a: int, b: int) -> bool {
  if t[a].box.y < t[b].box.y { return true; }
  if t[a].box.y > t[b].box.y { return false; }
  if t[a].box.x < t[b].box.x { return true; }
  if t[a].box.x > t[b].box.x { return false; }
  return a < b;
}

// Whether focus may stand on element i while `trap` (a dialog's index, or UI_NONE) is open:
// inside an open dialog, only its own elements.
ui_reachable :: (t: [] UiNode, i: int, trap: int) -> bool {
  return ui_focusable(t[i]) && (trap == UI_NONE || t[i].parent == trap);
}

// The next element focus moves to from `current` (UI_NONE: none yet), forward or back in reading
// order, wrapping at the ends; UI_NONE when nothing is reachable. A dialog traps it.
ui_focus_next :: (t: [] UiNode, current_: int, back: bool, trap: int) -> int {
  current := ifx current_ >= 0 && current_ < t.count then current_ else UI_NONE;   // outside the table: none
  best := UI_NONE;    // the nearest reachable element past current, in the direction
  wrap := UI_NONE;    // the first (forward) or last (back) reachable one, for wrapping
  for i: 0..<t.count {
    if i == current || !ui_reachable(t, i, trap) { continue; }
    past := current == UI_NONE || (ifx back then ui_before(t, i, current) else ui_before(t, current, i));
    if past && (best == UI_NONE || (ifx back then ui_before(t, best, i) else ui_before(t, i, best))) { best = i; }
    if wrap == UI_NONE || (ifx back then ui_before(t, wrap, i) else ui_before(t, i, wrap)) { wrap = i; }
  }
  if best != UI_NONE { return best; }
  if wrap != UI_NONE { return wrap; }
  if current != UI_NONE && ui_reachable(t, current, trap) { return current; }
  return UI_NONE;
}

// Where focus goes when the focused element `gone` is removed (or disabled): the next reachable
// element after it in reading order, or else the dialog it was in. Never nowhere while something
// is left to stand on.
ui_focus_after :: (t: [] UiNode, gone: int, trap: int) -> int {
  next := ui_focus_next(t, gone, false, trap);
  if next != UI_NONE && next != gone { return next; }
  return trap;   // the dialog it was in, or UI_NONE outside one
}
```

Focus moves through the interactive, enabled elements in reading order: top
to bottom by their top edges, then start to end along the line. These rules
apply:

1. Focus wraps at both ends.
2. A dialog traps focus. While it is open, only its own elements can be
   reached, so `focus_next` from its last element goes to its first.
3. When the focused element disappears or is disabled, focus moves to the
   next element in order, or to the dialog when none is left in it. It never
   moves to nothing while something is left.

### Taps and keys

<!-- source: aethyr/ui/tree.coax act -->
```jai
// A tap at (x, y): the action of the last reachable element whose drawn rectangle holds it, the
// one drawn on top; 0 for none. The same rectangles were drawn, so what you see is what you touch.
ui_tap :: (t: [] UiNode, x: f32, y: f32, trap: int) -> int {
  action := 0;
  for i: 0..<t.count { if ui_reachable(t, i, trap) && hud_contains(t[i].box, x, y) { action = t[i].action; } }
  return action;
}
// The keyboard's or the screen reader's `activate` on the focused element: the same action a tap
// on it produces.
ui_activate :: (t: [] UiNode, focus: int, trap: int) -> int {
  if focus < 0 || focus >= t.count || !ui_reachable(t, focus, trap) { return 0; }
  return t[focus].action;
}
```

A tap is the action of the element drawn on top at that point, from the
rectangle that placed the drawing. `activate` on the focused element, from
a key or a screen reader, produces the same action. While a dialog is open,
nothing outside it can be tapped or activated.

## A worked example

Take a 320 by 700 phone with a 34-point home indicator and a hotbar of three
slots. Work these out by hand:

1. **The row.** 3 · (44 + 6) − 6 = 144 points, less than 320 − 16, so the
   slots keep 44 points.
2. **The first slot.** x = (320 − 144) / 2 = 88 and y = 700 − 34 − 44 − 12 =
   610. Its bottom, 654, is above the indicator at 666.
3. **The first focus.** The settings button's top is at 50 and the slots'
   at 610, so focus starts on the settings button, then the first slot, then
   the second.
4. **Inside the dialog.** The settings dialog holds the text scale (top at
   240), reduced motion (300) and Close (380). Six presses of next visit each
   twice and nothing outside it.
5. **The same action.** A tap inside slot 2's rectangle and `activate` with
   slot 2 focused both give action 2.

The example [aethyr/examples/focus-a-menu.coax](../examples/focus-a-menu.coax)
places the slots with `verse_hud_slot`, the function that draws them, and
runs the keyboard through the settings:

<!-- source: aethyr/examples/focus-a-menu.coax example -->
```jai
// Action IDs; labels are the realm's text IDs.
SLOT_1 :: 1; SETTINGS :: 10; TEXT_SCALE :: 11; REDUCED_MOTION :: 12; CLOSE :: 13;
DIALOG :: 4;   // the settings dialog's index; the tree is built each frame, with it only when open

slot_box :: (i: int) -> HudBox {
  r := verse_hud_slot(320, 700, 34, i, 3);
  return HudBox.{x = r.x, y = r.y, width = r.width, height = r.height};
}

main :: () {
  t: [8] UiNode;
  t[0] = UiNode.{box = HudBox.{x = 268, y = 50, width = 44, height = 44}, role = UI_BUTTON, label = 204, action = SETTINGS};
  for i: 0..<3 { t[1 + i] = UiNode.{box = slot_box(i), role = UI_BUTTON, label = 100 + i, action = SLOT_1 + i}; }
  t[4] = UiNode.{box = HudBox.{x = 20, y = 200, width = 280, height = 240}, role = UI_DIALOG, label = 200};
  t[5] = UiNode.{box = HudBox.{x = 40, y = 240, width = 240, height = 44}, role = UI_SLIDER, label = 201, action = TEXT_SCALE, parent = DIALOG, low = 1, high = 2, value = 1};
  t[6] = UiNode.{box = HudBox.{x = 40, y = 300, width = 240, height = 44}, role = UI_TOGGLE, label = 202, action = REDUCED_MOTION, parent = DIALOG};
  t[7] = UiNode.{box = HudBox.{x = 40, y = 380, width = 100, height = 44}, role = UI_BUTTON, label = 203, action = CLOSE, parent = DIALOG};
  closed := t[0..<4];
  print("closed tree of %: problem %; open tree of %: problem %\n", closed.count, ui_reason_name(ui_check(closed).reason), t.count, ui_reason_name(ui_check(t[..]).reason));
  b := t[1].box;
  print("slot 1 drawn at % % (% by %), above the indicator: %\n", b.x, b.y, b.width, b.height, b.y + b.height <= 700 - 34);

  // The keyboard: focus starts at the top of the screen, then reads down to the slots.
  f := ui_focus_next(closed, UI_NONE, false, UI_NONE);
  n1 := ui_focus_next(closed, f, false, UI_NONE); n2 := ui_focus_next(closed, n1, false, UI_NONE);
  print("focus order: element % (action %), then % and %\n", f, closed[f].action, n1, n2);
  opener := f;
  open := t[..];
  trap := UI_NONE;
  if ui_activate(closed, f, UI_NONE) == SETTINGS { trap = DIALOG; f = ui_focus_next(open, UI_NONE, false, trap); }
  print("settings open: focus on element %, the text scale\n", f);
  if ui_activate(open, f, trap) == TEXT_SCALE { open[f].value = 1.5; }
  f = ui_focus_next(open, f, false, trap);
  if ui_activate(open, f, trap) == REDUCED_MOTION { open[f].pressed = !open[f].pressed; }
  print("text scale %, reduced motion %\n", open[5].value, open[6].pressed);
  visits: [8] int;
  g := f;
  for k: 0..<6 { g = ui_focus_next(open, g, false, trap); visits[g] += 1; }
  print("six presses of next inside the dialog visit: % % % (and outside it %)\n", visits[5], visits[6], visits[7], visits[0] + visits[1] + visits[2] + visits[3]);
  print("a tap on slot 1 while it's open gives %\n", ui_tap(open, b.x + 1, b.y + 1, trap));
  // back: the dialog closes and focus returns to the button that opened it
  f = opener;
  print("closed: focus on element %, action %\n", f, closed[f].action);
  print("tap on slot 2 gives %, its key gives %\n", ui_tap(closed, t[2].box.x + 20, t[2].box.y + 20, UI_NONE), ui_activate(closed, 2, UI_NONE));

  // A button with no label can't be reached by a screen reader: the tree is refused.
  broken := t;
  broken[3].label = 0;
  q := ui_check(broken[..]);
  print("with an unlabelled slot: refused %, element %\n", ui_reason_name(q.reason), q.index);
}
```

```shell
coax/bin/coax run aethyr/examples/focus-a-menu.coax
```

```text
closed tree of 4: problem none; open tree of 8: problem none
slot 1 drawn at 88.0 610.0 (44.0 by 44.0), above the indicator: true
focus order: element 0 (action 10), then 1 and 2
settings open: focus on element 5, the text scale
text scale 1.5, reduced motion true
six presses of next inside the dialog visit: 2 2 2 (and outside it 0)
a tap on slot 1 while it's open gives 0
closed: focus on element 0, action 10
tap on slot 2 gives 2, its key gives 2
with an unlabelled slot: refused label, element 3
```

The unlabelled slot is the broken case: a screen reader couldn't name it, so
the whole tree is refused at element 3. Closing the dialog with `back`
returns focus to the button that opened it; the realm keeps that element as
the dialog's opener.

## Checks

```shell
coax/bin/coax test aethyr/ui/tree.coax --library
coax/bin/coax mutate aethyr/ui/tree.coax --library --root-only
coax/bin/coax test aethyr/ui/hud.coax --library
```

`aethyr/ui/tree.coax` has seven checks of its own (it loads
`aethyr/ui/hud.coax`, whose six also run). They cover:

- each refusal at its first element, every interactive role needing a label
  and an action, a dialog first in the table, a dialog that names itself,
  and a tree that is empty or past 128 elements;
- reading order on one line, down a column and at the same place;
- focus forward and back with wrapping, past disabled elements and text,
  trapped in a dialog, with nothing reachable, and with one element;
- every reachable element visited exactly once a round, both ways;
- focus after the focused element goes: the next, the dialog, or none;
- taps and keys giving the same action, the top element owning an overlap,
  half-open edges, and nothing outside an open dialog.

`coax mutate --root-only` kills all 158 mutants.

## What exists today

- [verse/hud.coax](../../verse/hud.coax): the `verse.hud` module,
  `verse_hud_slot`, with checks that the hotbar fits a 320-point phone above
  the home indicator and that slots stay separate in landscape.
- [verse/hud/frame.coax](../../verse/hud/frame.coax): the frame's points,
  arms, cuts and greys, shared by the web landing and the iOS app.
- [aethyr/ui/hud.coax](../ui/hud.coax): logical screen rectangles,
  half-open hit areas, labelled/enabled controls, keyboard focus traversal,
  responsive catalog cells and arc/segment geometry.
- [aethyr/ui/tree.coax](../ui/tree.coax): the UI tree above.
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

## The host's accessibility tree (design)

The UI is drawn on the GPU, so the host mirrors the tree to the platform's
accessibility layer each time it changes: hidden DOM elements with ARIA roles
in the browser (as Filament's HTML twins do for page text), `UIAccessibility`
on iOS and `NSAccessibility` on macOS. A screen reader's activation arrives
as `activate` through [chapter 4](ch004-input-becomes-typed-actions.md).
The focused element is drawn with an outline at least 2 points wide that
contrasts with the panel behind it.

### The complete journey (design)

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
with the expected values. The example above runs steps 3, 4 and 6 on the
tree itself; the journey runs them through a realm and its host.

## Still to do

- **Realms on the tree.** No realm builds its HUD as a `UiNode` table yet;
  each keeps its own slot boxes and touch handling. Building the Grid's HUD
  as a tree, with its goldens with HUDs (`home-phone.png`, `the-grid.png`)
  unchanged, is the next step.
- **Typed actions.** A tap and the matching key giving the same action in the
  same step needs the tree's actions mapped through chapter 4's input
  sampling.
- **The platform mirror and the complete journey** above.
- **An explicit focus order** for a container whose reading order is wrong,
  with the check that it doesn't skip an interactive element.
- **Text layout** over the stroke font's advance widths
  (`aethyr/ui/text.coax`), and moving the hotbar and frame from `verse/hud`
  into `aethyr/ui/layout.coax`. `verse/hud.coax` is also the Coax compiler's
  public-module fixture, so the move has to keep `verse.hud`'s exports.

## Dependencies

Chapters 2 (screen space), 4 (typed actions) and 11 (the pass graph). From
Coax: R12 units for points and pixels.

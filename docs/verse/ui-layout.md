# Aethyr UI layout: widgets that take space

Status: design (GitHub #13076, phase 2). This design isn't built yet. Work starts
only after the fantasy HUD (`aethyr/ui/hud-style.coax`, `everglade/hud/`) lands on
`main`, and only when the lead approves it.

## The problem

Each HUD part places its widgets at fixed device coordinates, for example
`b.x0 + 116 * k` or `h - 484 * k`. A part doesn't know where the other parts are,
so you get these failures:

- Two parts take the same corner. The frame rate sits over Character Create's
  RESET and SURPRISE ME on a phone, and over the bar's slots.
- A label that is longer than the space it was given runs into the next widget,
  for example a long wardrobe value into its `>`.
- A layout made for a phone held upright goes off the screen when you turn the
  phone sideways. At 844×390, the wardrobe's top row is above the screen.
- Hit areas and drawn areas drift apart, because each is worked out from its own
  copy of the arithmetic.

The layout trace (`aethyr/ui/trace.coax`) finds these failures. Phase 2 stops
them from happening: a widget asks for space, and the layout gives every widget
its own space.

## Goals

- One rectangle for each widget. The draw, the hit test, keyboard focus and the
  layout trace all read it.
- A widget can't overlap a widget that it doesn't contain.
- Words never come out of their box. A box grows, the words wrap, or the words
  shrink to a stated smallest size. If none of those fits, the layout refuses
  the widget, and the check reports it.
- On a touch screen, every control is at least 44 CSS px each way.
- One layout tree for each screen size. The tree, not each part's code, says
  what moves where on a phone held upright, a phone held sideways, a tablet and
  a desktop.
- No allocation, no callbacks, and a fixed bound on work, as everywhere in
  Aethyr. Laying out a full HUD costs less than 0.1 ms.

These things are out of scope: styling and the look (the fantasy HUD agent owns
them), text shaping beyond the font's advance widths, and animation. A widget
can animate inside its rectangle, but the layout doesn't animate.

## The model

The layout is a flat table of nodes. It sits next to the UI tree
(`aethyr/ui/tree.coax`, book chapter 15) and fills in that tree's `box` fields.

```
LayNode :: struct {
  parent: int;            // an earlier node, or LAY_NONE for the screen
  axis: int;              // LAY_ROW or LAY_COLUMN: how its children are placed
  anchor: int;            // for a child of the screen: one of the nine edges and corners
  min_w: f32; min_h: f32; // CSS px: the least the node takes (a word's width, 44 for a tap)
  grow: f32;              // its share of the space that is left over along the axis
  gap: f32; pad: f32;     // CSS px between children, and inside its edges
  wrap: bool;             // its children go onto a new line instead of shrinking
  text: int;              // a text ID, measured with the realm's font, or 0
  role: int;              // tree.coax's role: button, text, panel, bar
  out: HudBox;            // what the layout gives it, device px
}
```

The layout runs two passes over the table. The table order is parents first, so
neither pass needs recursion:

1. **Measure.** The pass goes from the last node to the first. Each node's
   smallest size is the largest of its own minimum, its measured text, and the
   sum of its children along its axis (with gaps and padding). On a touch
   screen, the measure for a button is at least 44 CSS px each way.
2. **Place.** The pass goes from the first node to the last. A child of the
   screen goes to its anchor, for example the bottom-right corner for the frame
   rate. Inside a parent, children go in order along the axis. Each child gets
   its smallest size, and then a share of the space that is left by its `grow`.
   When children don't fit, a `wrap` parent starts a new line. Otherwise the
   layout refuses the last child that doesn't fit.

The screen's own children share the screen through anchors. Two children with
the same anchor stack along that edge in table order. For example, the frame
rate and GFX are both bottom-right, so GFX goes to the frame rate's left
instead of over it. A child that would cross another child's rectangle is moved
away from its anchor's edge, and if it still doesn't fit, the layout refuses it.
This rule stops the FPS-over-RESET failure without any part knowing the other
parts.

### Screen sizes

A realm gives one table for each size class. The size class comes from the
screen's CSS size, not from a device guess:

| Class | Rule | Example |
| --- | --- | --- |
| phone, upright | width < 600 | 390×844 |
| phone, sideways | height < 500 | 844×390 |
| tablet | width < 1280 | 1024×768 |
| desktop | otherwise | 1920×1080 |

Most parts are the same in every table, and only their parent or anchor
changes. A part builds its subtree with a function that takes the size class.
For example, the wardrobe is a column on a desktop and a two-column wrap on a
phone.

### Refusals

The layout never overlaps two widgets to make them fit. When something doesn't
fit, the layout does these things:

- It marks the node `refused` and gives it an empty box. The part draws nothing
  for that node, and the hit test skips it.
- It reports the node in the layout trace as `refused`. The trace check counts
  a refusal as a finding, so a refusal can't be missed.

A part can name a fallback: a shorter label, a smaller text size (no smaller
than the size the style sets as its smallest), or an icon. The layout tries the
fallbacks in order before it refuses the node.

## How a part uses it

Today a part works out its rectangles, draws the widgets, and in a separate
function hit-tests the same arithmetic. After phase 2, a part does these things:

1. It adds its nodes to the frame's layout table, for example the wardrobe's
   fields, RESET and SAVE.
2. After `lay_run`, it draws each node at `out`.
3. It hit-tests with `ui_tap` on the same table.

The layout trace is still written, from `out`, by the same calls that draw. The
check from phase 1 stays as the regression gate.

## Migration

The parts move one at a time. Each move must leave the overlap check with no
more findings than before.

1. The engine parts: the frame rate and coordinates, the combat HUD's Hit
   Points, the ANIMATIONS browser, and the CLASSES column (`class-menu.coax`).
2. The bar: `xc_slot_at`, the class and target line, the rest, aid and familiar
   controls, and the hover card.
3. Character Create (`everglade/client/create.coax`): the title, DRAG, the wardrobe,
   the class pane, and the bottom buttons.
4. Everglade II's other parts: GFX, the light readout and admin mode.
5. The fantasy HUD's parts, which come from `aethyr/ui/hud-style.coax` and
   `everglade/hud/`.

When a part moves, the known entries for that part in the harness's
`hud_known` list are removed. The goal is an empty list.

## Checks

- `aethyr/ui/layout.coax` has `#test` and `#forall` checks for the layout
  rules. Across the four screen sizes and a range of label lengths, they check
  that siblings never overlap, that children are always inside their parents,
  that touch buttons are always at least 44 CSS px, and that a refusal happens
  only when the minimums really don't fit.
- The phase 1 harnesses (`apps/openagents-web/hud-overlap-*.coax`) keep running
  every realm's HUD on the four screens. Any new finding fails them.

## Open questions

- Should text measuring be in the layout, which would need the font in
  `aethyr/ui`, or should the part pass in each label's width? The second keeps
  `aethyr/ui` free of fonts, so it is the proposal.
- World-anchored labels (the names over heads, damage numbers) stay outside the
  layout. Should they avoid HUD rectangles? `e2_plates` already moves names
  away from each other. The same rule could also keep them off the bar.

# 23. Mouse look and the crosshair

> **Status: partly implemented.** Mouse-button action routing (left-click cast,
> right-click draw/release longbow, left-click cancel during draw), sight-line
> queries, and range-reticle state live in
> [`everglade/client/mouse.coax`](../client/mouse.coax),
> [`everglade/client/sight.coax`](../client/sight.coax), and
> [`everglade/client/range.coax`](../client/range.coax). A standalone runnable
> book example remains to be extracted.

This chapter answers the player's question: "Where does my camera point, and
how do I aim without fighting the UI?"

Third-person action combat depends on a clear contract between the pointer,
the camera, and the HUD. When you are in look mode, moving the mouse turns the
camera around your hero and places a centre crosshair on the world; pressing
`Tab` frees the cursor so you can inspect your bag, swap spells, or click HUD
controls without firing stray arrows.

## What this chapter teaches

- How pointer lock splits mouse input cleanly between camera look and UI clicks.
- How `E2Mouse` turns right-button hold/release into a smooth bow draw and lets
  left-click cancel a nocked draw instead of firing.
- How the sight ray (`everglade/client/sight.coax`) projects from the camera
  through the crosshair onto terrain, rocks, and creature capsules.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Mouse action state | `E2Mouse` (`BowDraw`, `BowShow`, draw/loose/cancel/cast counters for `e2m_probe`) | [`everglade/client/mouse.coax`](../client/mouse.coax) |
| Aim ray & sight | Resolves crosshair ray against ground, trunks, rocks, and targets | [`everglade/client/sight.coax`](../client/sight.coax) |
| Range & target UI | Distance readout, Archery Range reticle feedback, and target lock cues | [`everglade/client/range.coax`](../client/range.coax) |
| Follow camera | Non-overshooting spring camera stepped after physics | [`metaverse-engine/cam/follow.coax`](../../metaverse-engine/cam/follow.coax) |

## The method

### One state machine for mouse buttons

`everglade/client/mouse.coax` keeps mouse-button interpretation deterministic
and testable via `E2Mouse`:

- **Left button (idle):** casts the active hotbar slot (last selected slot,
  else default Attack, else slot 0) through `e2k_press`.
- **Right button held:** begins drawing the Longbow (`BowDraw`), raising the
  nocked arrow pose (`BowShow`) and narrowing the crosshair.
- **Right button released (while drawn):** looses the arrow on the exact frame
  you let go (`everglade/client/arrows.coax`), incrementing `looses`.
- **Left button pressed (while drawing):** cancels the draw cleanly (`cancels`
  increments, zero arrow spent, zero spell cast).

### Camera look never corrupts the physics tick

Following [Metaverse Engine chapter 10](../../metaverse-engine/book/ch010-the-camera-follows.md),
mouse look updates camera yaw and pitch as presentation state and writes only
the quantized facing direction into the fixed-step movement input. Releasing
cursor lock with `Tab` stops turning the camera and hands pointer events back
to the semantic UI tree ([Metaverse Engine chapter 15](../../metaverse-engine/book/ch015-ui-and-the-hud.md)).

## Worked trace: draw-and-cancel vs draw-and-loose

```text
Initial:       draws=0  looses=0  cancels=0  casts=0
Right down:    draws=1  (bow nocked, crosshair narrows)
Left down:     cancels=1, casts=0  (draw let down cleanly; no spell fired)
Right down:    draws=2
Right up:      looses=1  (arrow spawned on release frame along sight ray)
```

## The checks

```shell
coax/bin/coax test --library everglade/client/client.coax --only "e2 mouse"
```

## What comes next

Now that the crosshair points at a target, [chapter 24](ch024-the-longbow.md)
flies the arrow along a ballistic arc and resolves the SRD attack roll on
arrival.

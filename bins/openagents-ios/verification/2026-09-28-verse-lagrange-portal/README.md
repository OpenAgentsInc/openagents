# Verse tab: the Grid's portal to Lagrange 1

Simulator record (iPhone 17 Pro, iOS 26.5, simulator build of `main` at
the commit that added this file), launched with:

```sh
xcrun simctl launch <udid> com.openagents.app --tab verse \
  --verse-script face,wait,walk,walk
```

- [`grid-portal.png`](grid-portal.png): after `face`, the player walks toward
  the arch lettered **LAGRANGE 1**, drawn in white and gray like the grid.
- [`lagrange-1-neutral.png`](lagrange-1-neutral.png): walking through it with
  the stick entered Lagrange 1 with no button. The zone panel (caption,
  **Grab**, **Forces**, **Art**, **The Grid**) and the airlock's refill ring
  are white and gray; the station keeps its physical materials. The world
  log reported `local_zone` with zero players: `verse-bare` presence paused.

Returning (through the station's **THE GRID** arch or the button) and
presence rejoining `verse-bare` are covered by
`zones::tests::walking_through_the_grid_portal_enters_a_neutral_lagrange_1_and_flying_back_returns`
and
`bare_presence_tests::the_grid_portal_pauses_presence_in_lagrange_1_and_the_return_rejoins`.
See [the Grid's portal](../../../../docs/verse/mobile.md#the-grids-portal-to-lagrange-1).

# Verse tab: the Grid without its portal to Lagrange 1

The Grid's portal to Lagrange 1 is hidden for now
(`verse::zones::gate::GRID_PORTAL_OPEN = false`). Simulator record (iPhone 17
Pro, iOS 26.5, the `build.sh sim` build, which uses the Release configuration):

```sh
xcrun simctl launch <udid> com.openagents.app --tab verse
xcrun simctl launch <udid> com.openagents.app --tab verse --verse-script face,wait,walk
```

- [`grid-spawn.png`](grid-spawn.png): the spawn view. You see the ball, the
  blocks, and the Gym ahead. There is no **LAGRANGE 1** arch to the side.
- [`grid-face-no-portal.png`](grid-face-no-portal.png): after `face` turns
  toward where the arch stood and `walk` walks there, the player is still on
  the Grid. No arch, lettering, or zone panel is shown.

`zones::tests::the_grid_shows_no_portal_while_it_is_hidden` covers the
same. The portal's path still passes in tests via
`WorldRuntime::open_grid_portal_for_tests`. See
[the Grid's portal](../../../../docs/verse/mobile.md#the-grids-portal-to-lagrange-1).

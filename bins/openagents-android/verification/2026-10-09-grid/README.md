# Coder / Verse on Android (#11184), 2026-10-09

A debug build (x86_64, not a preview build) on CoderOS's Android 15
emulator, light and dark (`--es appearance light|dark`).

- `new-*.png`: a new chat with the **Coder** / **Verse** switch; the cards
  open on a random card and move on by themselves. The floating bar at the
  left is the emulator keyboard's own toolbar, not the app.
- `drawer-*.png`: the drawer (`--ez drawer true`) with **Verse** in every
  build.
- `verse-*.png`: the plain Grid (`--es tab verse`): your avatar and the
  sticks, no Gym or Everglade arch; light status bar icons over the dark
  world in both themes.

- `enter-the-grid-dark.png`: the live chat's answer to "What is the Verse?"
  ends with the **Enter the Grid** card (the worker's typed `verse` offer);
  `enter-the-grid-tapped-dark.png` is the Grid after a tap on it.

Also checked by hand on the emulator: **Try it** on a card, the switch into
the Verse and back to **Coder**, and the back button from the Verse.

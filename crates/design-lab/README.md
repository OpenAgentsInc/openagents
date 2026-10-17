# Design Lab

A Figma-style board with every screen of the current OpenAgents phone app
(`apps/openagents/`), so you can zoom around the whole app in one place.
Nothing on it is an old screenshot: every picture is captured from the app's
code.

```sh
apps/openagents/scripts/design-lab-capture.sh   # capture every screen (simulator)
cargo run -p design-lab -- serve                 # http://127.0.0.1:8798
cargo run -p design-lab -- export /tmp/board     # static index.html + board.json + img/
```

- **Capturing.** `design-lab-capture.sh` runs `DesignLabTourTests` (in
  `apps/openagents/apps/ios/OpenAgentsUITests`) on the offline demo, so
  nothing signs in: the first-launch tutorial and its landing chat, Home,
  the drawer, search, Settings, the new-session sheet, the cloud chat, every
  demo session and the voice stage. Each run is saved in `~/work/design-lab/captures/<time>-<commit>/`
  as `<area>--<screen>.png` (set `DESIGN_LAB_CAPTURES` to move it). The tour
  runs twice, in light and in dark mode, and the dark pictures are
  `<area>--<screen>@dark.png`. Add a screen by adding a `shot` to the tour.
- **Versions.** Each capture run is a version of every screen it took. A
  screen that looks exactly as it did in the run before keeps its earlier
  version, so a screen's versions are the commits that changed it.
- **The board.** Areas are sections, and screens sit in them with their
  versions side by side, oldest first. Drag or scroll to move; pinch or
  ⌘/Ctrl + scroll to zoom; `/` searches; `0` fits everything. Click a screen
  to open it and step through its versions with the arrow keys. `#q=words`
  in the address opens the board already searched. **Light / Dark** (or `D`)
  switches every screen to its dark-mode capture; a version captured before
  dark mode shows its light picture.
- **A Lab kind.** The board is a `board.v1` document and `design` is a Labs
  platform kind (`src/kind.rs`): `design_rename` and `design_move` are checked
  changes, and the card shows the newest version of each screen. See
  [the Labs plan](../../docs/labs/README.md).

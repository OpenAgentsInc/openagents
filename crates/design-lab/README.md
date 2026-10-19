# Design Lab

A Figma-style board with every screen of the current OpenAgents apps, the
iOS phone app, the Android app, and the desktop app (all in
`apps/openagents/`), and the website, so you can zoom
around a whole app in one place. Nothing on it is an old screenshot: every
picture is captured from the app's code, and every web screen is the live
page.

```sh
apps/openagents/scripts/design-lab-capture.sh          # every phone screen (simulator)
apps/openagents/scripts/design-lab-capture-android.sh  # every Android screen (emulator)
apps/openagents/scripts/design-lab-capture-desktop.sh  # every desktop screen (offscreen)
crates/design-lab/scripts/web-dev.sh                   # the web app, rebuilt on every save
cargo run -p design-lab -- serve                       # http://127.0.0.1:8798 (#app=web for the website)
cargo run -p design-lab -- export /tmp/board           # static index.html + board.json + img/
cargo run -p design-lab -- spec m1                     # a mock's spec as Markdown, plus its PNG
```

- **Capturing.** `design-lab-capture.sh` runs `DesignLabTourTests` (in
  `apps/openagents/apps/ios/OpenAgentsUITests`) on the offline demo, so
  nothing signs in: the first-launch tutorial and its landing chat, Home,
  the drawer, search, Settings, the new-session sheet, the cloud chat, every
  demo session and the voice stage. Each run is saved in `~/work/design-lab/captures/<time>-<commit>/`
  as `<area>--<screen>.png` (set `DESIGN_LAB_CAPTURES` to move it). The tour
  runs twice, in light and in dark mode, and the dark pictures are
  `<area>--<screen>@dark.png`. Next to each picture the tour saves its
  element tree, `<area>--<screen>[@dark].json`: every visible element's type
  (button, text field, text, image, cell, ...), identifier, label, value
  and frame in points, plus the picture's scale. Add a screen by adding a
  `shot` to the tour.
- **Capturing the Android app.** `design-lab-capture-android.sh` drives the
  Android app through its screens and saves each run in
  `<captures>/android/<time>-<commit>/`, with the same file names as the
  phone's: `<area>--<screen>.png`, `<area>--<screen>@dark.png`, and, when the
  tour writes them, element trees in the same JSON shape as the iOS tour's.
  Without element trees the board offers box selection.
- **Capturing the desktop app.** `design-lab-capture-desktop.sh` runs the
  `design-lab-tour` example in `apps/openagents/crates/ui`, which draws the
  whole desktop app offscreen (GPUI's headless renderer, no window) in a
  1280 × 800 point window at 2x: the boot gates, Home with the account row
  signed in and out, the sidebar, a chat with messages, tool rows and the
  composer, the Runs on and model menus, new session, the command palette,
  the voice stage, the terminal, files and changes panels, and every
  Settings page. The account is a fixture served from loopback, so no real
  account, chat or engine is used. Runs go in `<captures>/desktop/<time>-<commit>/`
  with the same file names as the phone's; desktop pictures have no element
  trees yet, so the board offers box selection on them. Add a screen by
  adding a `Shot` to `shots()` in `crates/ui/src/shell/design_lab_tour.rs`.
  To build it without running it:
  `cargo check -p openagents-ui --example design-lab-tour --features account-fixture`
  from `apps/openagents`.
- **The website, live.** **Web** shows every screen of the web app's
  screens gallery (`crates/openagents-web/src/screens/`), in every state,
  as the live page: each frame is `/ui/screens/{screen}/{state}?theme=`
  from a running web server, not a picture. The board reads the list from
  that server's `/ui/screens.json`, so a screen added to the gallery shows
  up by itself, and screens sit in the gallery's areas (Sign-in, Chat,
  Account, Traces, Site). Frames are drawn at 1280 × 800; **Phone width**
  draws them 390 wide. **Light / Dark** (`D`) switches `?theme=`. A frame
  loads its page once it is zoomed big enough to read. Click a screen to
  open it larger, still live and clickable, step through its states with
  the arrow keys, and **Open this page** in its own tab.
- **Editing the website and watching it.** `crates/design-lab/scripts/web-dev.sh`
  builds `openagents-web`, runs it on `127.0.0.1:4380` with an empty store
  of its own (the gallery uses sample data only, so no account, chat
  service or computer is needed), and rebuilds and restarts it whenever a
  file changes in a crate the web app is built from. The board checks the server every second
  and a half; when it comes back after a restart, every open frame reloads.
  So: save a web file, wait for the build, and the board shows the change,
  with no capture step. The loop builds incrementally (it skips the shared
  compile cache's wrapper, which would rebuild all of `openagents-web` on
  every save; set `DESIGN_LAB_WEB_WRAPPER` to use one): a save in
  `openagents-web` showed on the board in about 105 seconds on a busy Mac,
  and the first build after starting the loop takes longer. A build that fails keeps the last good server
  running and prints the error. `DESIGN_LAB_WEB_PORT` moves the port, and
  `web-dev.sh CHECKOUT` builds and watches another checkout. The chip in
  the toolbar says whether the server is live. To show another server, type
  its address in the toolbar's address field, open `#web=http://127.0.0.1:PORT`,
  or start the board with `serve --web URL` (or `DESIGN_LAB_WEB`). The web
  app lets pages on this computer frame its samples (they carry no session
  and can post nothing), so the board's frames work from another port.
- **Apps.** **Phone**, **Android**, **Desktop**, and **Web** in the toolbar
  switch between the apps' screens, and `P` steps to the next app;
  `#app=android`, `#app=desktop` or `#app=web` in the address opens that
  app's board.
  Each app's newest run decides which of its screens are current, and each
  app's screens are drawn at its own aspect ratio (412 × 915 dp for
  Android). An Android screen's id is `android:<area>--<screen>`, and a
  desktop screen's id is `desktop:<area>--<screen>`.
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
- **Selecting.** On an open screen, hover shows each element's outline;
  click selects one (its type and label show beside it) and Shift-click adds
  more. Drag to select a box instead; a screen captured before element
  trees offers only the box.
- **Mock screens.** **New mock screen** (or `N`) adds a blank frame the size
  of the showing app's screens (phone, Android, or desktop) to the Mocks section. On a
  screen, pick a mock under **Selected** and choose **Copy to mock**: each
  selection becomes a piece cut from that picture (light or dark, whichever
  is showing) that keeps its element.
  Click a mock to edit it: drag pieces to move them, drag the corner to
  resize, add a **Rectangle** (`R`) or **Text** (`T`), **Duplicate** (⌘D),
  **Delete** (⌫), arrange front to back, nudge with the arrow keys, and copy
  pieces to another mock. **Snap to 8 pt** keeps them on a grid.
- **Notes.** **Pin a note**, then click a spot or drag a box on a screen
  version or a mock and write the note ("Put the composer here"). Notes
  show as numbered pins with a list beside them, and each has its author
  and **Resolve**. A screen with open notes says so on the board.
- **Saving.** Mocks and notes are one `mock.v1` document,
  `design-lab-mocks.json` in the captures folder, so reloads and new capture
  runs keep them. The page sends each edit to `POST /mocks/change` as one
  checked change (`mock_new`, `item_add`, `item_set`, `note_add`,
  `note_set`, ... or a `batch` of them); `serve` takes changes only from
  this computer and refuses one made on an older version. An exported board
  shows mocks and notes read-only.
- **Specs for building agents.** `design-lab spec MOCK_OR_SCREEN` prints a
  mock (by id or title) as Markdown: every piece with its position and size,
  the screen version and element it was cut from, and every note with what
  it points at. It writes the mock's PNG to `<captures>/specs/` (or
  `--out DIR`). For a screen it lists the notes on each version and the
  elements under them. While `serve` runs, `/spec/m1` and `/spec/m1.png` are
  the same, and each mock and screen links to its spec.
- **A Lab kind.** The board is a `board.v1` document and `design` is a Labs
  platform kind (`src/kind.rs`): `design_rename` and `design_move` are checked
  changes, and the card shows the newest version of each screen. The same
  kind keeps `mock.v1` (`src/mock.rs`), changed through `design_mock_*`,
  `design_item_*` and `design_note_*`. See
  [the Labs plan](../../docs/labs/README.md).

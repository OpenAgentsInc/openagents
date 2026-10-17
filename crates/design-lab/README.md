# Design Lab

A Figma-style board with every version of every OpenAgents mobile app
screen, so you can zoom around the whole app's history in one place.

```sh
cargo run -p design-lab -- serve          # http://127.0.0.1:8798
cargo run -p design-lab -- export /tmp/board   # static index.html + board.json + img/
```

- **Where the screens come from.** Every commit on the current branch that
  added or changed a phone screenshot (`.png`/`.jpg` under a path with
  `mobile`, `ios`, `iphone`, `phone`, `android` or `appshots` in it) is a
  version of that screen. Screens deleted from the tree keep their past
  versions. Icons and images under 200 px are skipped.
- **Screens and areas.** A screen is the file name without a date, an order
  number or a trailing width (`appshots-chat-light-390.png` and `-1100.png`
  are two versions of one screen). Its area is the capture folder, without a
  date.
- **The board.** Areas are sections, screens sit in them with their versions
  side by side, oldest first. Drag or scroll to move; pinch or ⌘/Ctrl +
  scroll to zoom; `/` searches; `0` fits everything. Click a screen to open
  it and step through its versions with the arrow keys. `#q=words` in the
  address opens the board already searched.
- **A Lab kind.** The board is a `board.v1` document and `design` is a Labs
  platform kind (`src/kind.rs`): `design_rename` and `design_move` are checked
  changes, and the card shows the newest version of each screen. See
  [the Labs plan](../../docs/labs/README.md).

Next: a capture job that renders every screen state from the mobile app's
demo fixtures (so every screen has a version per commit, not only the ones
someone screenshotted), notes pinned to a screen version, and the board hosted
on openagents.com behind the Labs API.

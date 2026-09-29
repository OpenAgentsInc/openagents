# openagents-deck

The OpenAgents presentation deck, as a native desktop app. Slides are
Markdown files under [`decks/`](decks/), laid out on a fixed cell canvas,
checked in as golden text snapshots, and shown in a window that paints the
same cells.

The crate is a port of the Coder repository's `coder-deck` (the layout
engine, the snapshots, the text export, and the window). It carries the
cell grid from Coder's component core in [`src/grid.rs`](src/grid.rs) and
reuses [`coder-ui`](../coder-ui/)'s amber intensity ladder and
[`rust-native`](../rust-native/)'s bundled JetBrains Mono. The Coder
window drew through GPUI; this one paints each frame in software
([`src/paint.rs`](src/paint.rs)) and copies it into a `winit` window's
`wgpu` surface, so it adds no dependency the workspace did not already
have.

## Run it

```sh
cargo run -p openagents-deck -- --deck test-time-capabilities
```

The default deck is `test-time-capabilities`, so `cargo run -p
openagents-deck` opens it too. Build the release binary for presenting;
the debug build paints the arrival animation slowly.

| Option | What it does |
| --- | --- |
| `--deck NAME` | The deck filed under `decks/NAME.md` |
| `--decks` | Lists the decks |
| `--slide N` | Opens on slide N, counting from 1 |
| `--fullscreen` | Opens fullscreen |
| `--notes` | Opens with the presenter's notes showing |
| `--still` | Turns the arrival animation off |
| `--text` | Prints every slide as the text of its grid |
| `--check` | Lists the slides still waiting on facts; exits 1 while any do |
| `--capture DIR` | Paints every slide, and the overview, to PNG files in `DIR` |
| `--size WxH` | The capture size in pixels (default `1920x1080`) |

### The macOS app

```sh
scripts/bundle-openagents-deck.sh
open "target/release/OpenAgents Deck.app"
```

The script builds the release binary and wraps it as `OpenAgents
Deck.app` in the Cargo target directory's `release` folder (pass another
folder as the first argument). The app's icon is the OpenAgents iOS app
icon. The bundle is signed ad hoc for the machine that built it and is not
notarized. Double-click it, or drag it to `/Applications`.

## Keys

| Key | What it does |
| --- | --- |
| Right, Space, Page Down, `n`, `j`, or a click | The next slide, or the rest of this one while it arrives |
| Left, Page Up, `p`, `k` | The slide before |
| Home, End | The first slide, the last slide |
| A number, then Enter | Jump to that slide |
| `o` | The overview: one card a slide; a click opens one |
| `t` | The presenter's notes, under the slide |
| `.` or `b` | Black the screen, and back |
| `s` | Motion off, and on |
| `f` | Fullscreen, and back |
| Command or Control with `=`, `-`, `0` | Zoom in, zoom out, and fit |
| Escape | Close the overview, the notes, or the black screen; then leave fullscreen |
| `q`, Command-Q | Quit |

## The slides

The canvas is 108 cells by 28 rows, which is 16:9 at JetBrains Mono's
0.6 em advance and a 1.3 line height. The window picks the largest type
size that fits and centers the canvas, so a slide reads the same on a
laptop, on a projector, and in the text export.

A line of three dashes separates two slides. Inside a slide, a line that
opens with a key is a directive and every other line is the body, which
[`src/prose.rs`](src/prose.rs) lays out: paragraphs, bulleted and numbered
lists, `**bold**`, `*italic*`, and `` `code` ``.

| Key | What it names |
| --- | --- |
| `layout` | Which of the eight shapes the slide draws in |
| `id` | The slide's name, which is also its snapshot's and screenshot's name |
| `kicker` | A short label over the title, at half intensity |
| `title` | The line over the body |
| `lead` | The line under a banner, or a quote's attribution |
| `source` | Where the slide's facts come from. `owner` means they are not in this repository yet |
| `note` | Lines under the body, at half intensity |
| `metric` | A value and a label, separated by a bar |
| `column`, `row` | A comparison's headers and its rows, cells separated by bars |
| `step` | One stage of a flow |
| `notes` | The presenter's note, which draws only in the notes band |

The layouts are `banner`, `statement`, `points`, `metrics`, `compare`,
`flow`, `quote`, and `ask`, each a function in
[`src/layouts.rs`](src/layouts.rs). Coder's ninth layout, `live`, drew
Coder's own product screens and was not carried over.

## The rules the deck keeps

- **One full-intensity element a slide.** Full amber marks the one thing
  a slide says; prose draws at three quarters, labels at half, and rules at
  a quarter. A titled slide whose body holds its full element drops the
  title to three quarters. A test enforces it.
- **No invented numbers.** Every slide names a `source`. A metric with no
  value draws a dash at half intensity, and `--check` lists every slide
  still waiting on facts.
- **No account and no network.** The window reads nothing but the deck
  compiled into it.

## Add a deck

1. Write `decks/NAME.md`.
2. Add `("NAME", include_str!("../decks/NAME.md"))` to `SCRIPTS` in
   [`src/slide.rs`](src/slide.rs). The first entry is the default deck.
3. Record its snapshots and read the diff:

   ```sh
   UPDATE_SNAPSHOTS=1 cargo test -p openagents-deck
   git diff crates/openagents-deck/snapshots
   ```

4. Look at every slide: `cargo run -p openagents-deck -- --deck NAME
   --capture docs/decks/NAME`, then present it in the window.

## Snapshots and screenshots

Every slide's grid is checked in under `snapshots/<deck>/<id>.txt`: the
text, the intensity of every cell, and the style flags. `cargo test -p
openagents-deck` fails on any difference. The rendered slides of the
test-time capabilities deck are in
[`docs/decks/test-time-capabilities/`](../../docs/decks/test-time-capabilities/),
written by `--capture`; the `window/` folder there holds screenshots of the
bundled app running fullscreen, with the overview and the notes band.

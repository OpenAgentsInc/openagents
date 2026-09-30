# openagents-deck

The OpenAgents presentation deck, as a native desktop app built from the
same Rust Native pieces as the OpenAgents desktop app. Slides are Markdown
files under [`decks/`](decks/); the window, the text, the colors, and the
images are the desktop's own, so improving one improves the other.

## How it is built

A slide is a handful of Rust Native nodes, and the deck keeps only the
slide content, where each part sits, and the navigation:

- **Nodes.** A kicker is a `text` with the `status` role, a title a
  `heading`, prose, lists, and comparisons are `markdown` (a comparison is
  a Markdown table), a quote or a flow step is a `stack` card, and a large
  number is a `heading` set larger. [`src/compose.rs`](src/compose.rs)
  builds them and places them on an 800 × 450 point canvas whose column is
  the row layout's reading width.
- **Layout and text.** Each part is laid out by
  [`rust_native_desktop::rich`](../rust-native-desktop/src/rich.rs): the
  row layout the desktop and mobile chat transcripts use
  (`rust_native::layout`), painted by the desktop's transcript painter with
  the theme's bundled font pair (Geist and Geist Mono, as in the desktop
  chat). A part can be painted larger (a title at 1.5 times, the opening
  title and numbers up to 3 times) without a second type ladder.
- **Colors.** [`Theme::openagents`](../rust-native-desktop/src/theme.rs),
  which the desktop shell also uses, and the transcript palette: white and
  gradations of white on the desktop's near-black.
- **Images.** An image slide shows one image through
  [`rust_native_desktop::image`](../rust-native-desktop/src/image.rs), the
  painter the desktop's composer uses for image previews: fitted to the
  slide with its aspect ratio kept, centered, never enlarged past one image
  pixel a screen pixel, shrunk with an area filter. The alternative text is
  the surface's label in the semantic view.
- **Window.** [`src/present.rs`](src/present.rs) is a
  `rust_native_desktop::App` whose view is one surface the size of the
  window, labeled with the showing slide's text. The window adapter shows
  it, turns keys and clicks into the presenter's moves, and captures it for
  `--capture` through the same layout and paint.

What the shared crates gained for the deck, which the desktop can use too:
`rich` (one node laid out by the row layout and painted anywhere, at any
size), `image` (bounded PNG decoding, `fit`, and filtered painting; the
composer's previews now use it), `Theme::openagents`, an `App` hook that
asks the window to enter or leave fullscreen, text rows that honor
`style.align`, and tables a little wider than the row wrapping instead of
scrolling.

Not carried over from the old cell-grid renderer: the block face and the
progress hairline in the foot (the foot keeps the slide's place, "6 / 22"),
and the one-full-intensity-element rule (emphasis is now size and weight,
as in the desktop). The Verse backdrop is not drawn: it needs a relay
connection, and the deck reads nothing but itself.

## Run it

```sh
cargo run -p openagents-deck -- --deck test-time-capabilities
cargo run -p openagents-deck -- --deck three-devdays-later
```

The default deck is `test-time-capabilities`, so `cargo run -p
openagents-deck` (the `deck` alias) opens it. Open the other with
`deck --deck three-devdays-later`; `--decks` lists them all. A slide appears whole the moment it is
opened, in the debug build and the release build alike: there is no
per-slide animation, and the window paints only on a key, a click, or a
resize.

| Option | What it does |
| --- | --- |
| `--deck NAME` | The deck filed under `decks/NAME.md` |
| `--decks` | Lists the decks |
| `--slide N` | Opens on slide N, counting from 1 |
| `--fullscreen` | Opens fullscreen |
| `--notes` | Opens with the presenter's notes showing |
| `--text` | Prints every slide's outline: each part, where it sits, and its text |
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
| Right, Space, Page Down, `n`, `j`, or a click | The next slide |
| Left, Page Up, `p`, `k` | The slide before |
| Home, End | The first slide, the last slide |
| A number, then Enter | Jump to that slide |
| `o` | The overview: one card a slide; a click opens one |
| `t` | The presenter's notes, under the slide |
| `.` or `b` | Black the screen, and back |
| `f` | Fullscreen, and back |
| Command or Control with `=`, `-`, `0` | Zoom in, zoom out, and fit |
| Escape | Close the overview, the notes, or the black screen; then leave fullscreen |
| `q`, Command-Q | Quit |

## The slides

The canvas is 800 by 450 points, 16:9. The window fits it to its size,
centered, so a slide reads the same on a laptop, on a projector, and in a
capture.

A line of three dashes separates two slides. Inside a slide, a line that
opens with a key is a directive and every other line is the body, which is
Markdown: paragraphs, bulleted and numbered lists, `**bold**`, `*italic*`,
and `` `code` ``. A body that is one `![alt text](assets/file.png)` line
makes an image slide.

| Key | What it names |
| --- | --- |
| `layout` | Which of the nine shapes the slide draws in |
| `id` | The slide's name, which is also its snapshot's and screenshot's name |
| `kicker` | A short muted label over the title |
| `title` | The line over the body |
| `lead` | The line under a title slide's title, or a quote's attribution |
| `source` | Where the slide's facts come from. `owner` means they are not in this repository yet |
| `note` | A muted line under the body |
| `metric` | A value and a label, separated by a bar |
| `column`, `row` | A comparison's headers and its rows, cells separated by bars |
| `step` | One stage of a flow |
| `notes` | The presenter's note, which draws only in the notes band |

The layouts are `title`, `banner`, `statement`, `points`, `metrics`,
`compare`, `flow`, `quote`, `ask`, and `image`, each a branch of `body` in
[`src/compose.rs`](src/compose.rs). An image lives under `decks/` and is
compiled in through `ASSETS` in [`src/slide.rs`](src/slide.rs).

## The rules the deck keeps

- **White on black.** White and gradations of white on the desktop's
  near-black, from the shared theme; a test paints every slide without an
  image and finds no warm hue.
- **No animation.** A slide appears whole the moment it opens, and the
  window paints only on a key, a click, or a resize.
- **Everything fits.** A test checks that every part stays inside the
  margins, above the foot, and that no table scrolls sideways.
- **No invented numbers.** Every slide names a `source`. A metric with no
  value draws a muted dash, and `--check` lists every slide still waiting
  on facts.
- **No account and no network.** The window reads nothing but the decks
  and images compiled into it.

## Add a deck

1. Write `decks/NAME.md`.
2. Add `("NAME", include_str!("../decks/NAME.md"))` to `SCRIPTS` in
   [`src/slide.rs`](src/slide.rs). The first entry is the default deck.
   Add each image it shows to `ASSETS` there.
3. Record its snapshots and read the diff:

   ```sh
   UPDATE_SNAPSHOTS=1 cargo test -p openagents-deck
   git diff crates/openagents-deck/snapshots
   ```

4. Look at every slide: `cargo run -p openagents-deck -- --deck NAME
   --capture docs/decks/NAME`, then present it in the window.

## Snapshots and screenshots

Every slide's outline is checked in under `snapshots/<deck>/<id>.txt`: each
part's kind, place and size on the canvas, and text. `cargo test -p
openagents-deck` fails on any difference, including one that comes from a
change to the shared row layout. The rendered slides of the test-time
capabilities deck are in
[`docs/decks/test-time-capabilities/`](../../docs/decks/test-time-capabilities/),
written by `--capture`.

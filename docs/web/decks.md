# Slide decks

`coax/web/deck.coax` builds a PDF slide deck in the Grid style of
openagents.com from a plain-text deck file. You write the content; the tool
takes the scenes, lays out the slides and writes the PDF.

```sh
coax/web/tool.sh deck docs/decks/demo.deck ~/Desktop/openagents-demo-deck.pdf
```

The tool builds itself on first use. It needs Chrome for scenes; nothing opens
on screen and no sound plays. `docs/decks/demo.deck` uses every kind of slide.

## What a slide looks like

- Pages are 16:9 (960 × 540 points) on the near-black ground and faint iso grid
  of the homepage.
- Titles, kickers and labels are set in Filament, the site's stroke font
  (`apps/openagents-web/font.coax`), drawn as vector strokes.
- Body text is Helvetica, with the widths the site uses for small text.
- A slide with cells has a header panel (kicker, title and subtitle) and a grid
  of panels under it. Each panel has a label bar: the label in the accent on the
  left, the caption on the right.
- A slide without cells is a title slide: kicker, title and subtitle over the
  ground, or over a dimmed scene.
- The footer shows the deck's name and the page number.

## The deck file

Each line starts with a word. Blank lines and lines that start with `#` are
skipped.

| Line | What it does |
|---|---|
| `deck NAME` | The name in every page's footer. |
| `base URL` | Where scene paths point. The default is `https://openagents.com`. |
| `slide` | Starts a slide. The lines after it, up to the next `slide`, belong to it. |
| `kicker A / B` | The small line above the title. The part after the last `/` is in the accent. |
| `title TEXT` | The slide's title. |
| `subtitle TEXT` | Beside the title on a slide with cells; under it on a title slide. |
| `background URL [wait=MS]` | A scene that fills the page, dimmed. Use it on title slides. |
| `grid COLUMNSxROWS` | The cell grid, 1 to 8 each way. The default is `1x1`. Set it before the cells. |
| `scene URL [wait=MS] \| LABEL \| CAPTION` | A cell with a live page, taken at the cell's shape. |
| `image PATH \| LABEL \| CAPTION` | A cell with a JPEG or PNG, fitted inside it. A relative path is relative to the deck file. |
| `stat VALUE \| LABEL \| CAPTION` | A cell with one large value. |
| `text \| LABEL \| CAPTION` | A cell with text. The indented lines after it are its content. |
| `diagram \| LABEL \| CAPTION` | A cell with boxes and arrows. The indented lines after it are its content. |
| `eval SCRIPT` | JavaScript to run in the scene above it after its wait, before it's taken. |

Cells fill the grid left to right, then top to bottom. The label and caption
are optional.

### Text cells

Indent each line. A line that starts with `- ` is a bullet; any other line is a
paragraph in the dimmer colour. The text takes the largest size, from 14 points
down, at which every line fits.

```
text | WHAT YOU DO | Invite only
  - Describe a world in a sentence.
  - Walk it, move anything, then publish.
  Every realm can be forked.
```

### Diagrams

Indent each line. `node ID COLUMN ROW TEXT` places a box on a small grid,
counted from 0. `edge FROM TO [TEXT]` draws an arrow between two boxes, with an
optional label.

```
diagram | THE STACK
  node coax 0 1 COAX
  node aethyr 1 0 AETHYR ENGINE
  edge coax aethyr compiles
```

## Scenes

A scene is any web page. A path such as `/realms/orbit-league` joins the deck's
`base`. The tool opens it in headless Chrome at the shape of the place it
fills, about 1000 pixels tall, waits, and takes it as a JPEG.

- `wait=MS` sets how long the page runs before it's taken. The default is 6000
  ms. Realms that load large worlds need 8000 to 9000 ms.
- `?stage=NAME` asks a realm for a staged moment, such as `?stage=highlight`
  (see `docs/verse/clips.md`). `&card=1` leaves the HUD out on the realms that
  support it.
- For a 3x2 grid of live realms, set `grid 3x2` and add six `scene` lines.

The tool keeps each scene in a `.captures` folder beside the deck file, such as
`docs/decks/demo.captures/`. It reuses a capture until that scene's line
changes. To take every scene again, add `--fresh`. Captures aren't committed.

## Check a deck

To look at the pages without opening the PDF, render them with `pdftoppm`:

```sh
pdftoppm -png -r 72 ~/Desktop/openagents-demo-deck.pdf /tmp/deck
```

# Filament: a stroke font for Coaxial

Proposal, 2026-10-28. Status: not built.

The openagents.com splash writes COMING SOON in seven hand-drawn glyphs
(`apps/openagents-web/splash.coax`). Each one is a single line through points
on a small grid, drawn on the GPU as a sharp core with a soft glow. This
proposal turns those seven glyphs into a complete typeface, kept as its own
project so the splash, the docs and blog pages, the HUD and later Metareal
all draw text the same way.

Working name: **Filament**. Each letter is one lit wire, and the name says
how it looks rather than what it's built from. Other candidates: Trace,
Wire, Vector Six. Taken or unsuitable: Hershey, the 1967 stroke fonts this
idea is closest to (public domain, but a different look and grid), and
Coaxium (a Star Wars term).

## What it is

- A **stroke font**: a glyph is a few polylines, with no outlines and no
  filled shapes. The renderer decides the stroke weight, so one font gives a
  hairline, a heavy line and a glow, all from the same data.
- **Data in Coax**: the glyph table is a Coax source file, so the font is
  compiled into the wasm module. Nothing is fetched, parsed or rasterized
  at load. The splash's glyphs take under 200 bytes;
  the full set should stay under 4 KB.
- **Animated by design**: every stroke has a direction and an order, so text
  can write itself on, as the splash does, with no extra data.

## Design rules

These come from the seven glyphs that already exist.

1. **The grid.** Capitals sit on a 4 wide × 6 high grid, with y up and the
   baseline at 0. Points are whole grid units. Half units are allowed only
   where a glyph can't be drawn without them (for example the middle bar of
   `E` or the dot of `i`), and each one is listed in the glyph table's
   comments.
2. **Corners are clipped at 45°.** A round letter turns its corners in one
   unit (`C` runs 4,5 → 3,6 → 1,6 → 0,5). There are no curves. A circle is an
   octagon.
3. **Diagonals run corner to corner or through the centre point.** `M` dips
   to 2,3, `N` runs 0,6 → 4,0. There are no arbitrary angles.
4. **As few strokes as possible, drawn the way a hand writes.** Write top to
   bottom, left to right, and start round letters at the upper right, as the
   splash's `C`, `G` and `S` do. Most capitals are one polyline; `A`, `E`,
   `F`, `H`, `K`, `T`, `X` and `Y` need two or three.
5. **Width varies.** `I` is 0 wide, `M` and `W` may be 5, and most glyphs are
   4. The advance is the width plus the gap; the splash uses a gap of 0.72
   cap heights for its letter-spaced look, and body text should use about
   0.3.
6. **Lowercase sits on the same grid**, with an x-height of 4 and
   descenders to −2. The ascenders of `b`, `d`, `h`, `k` and `l` reach 6,
   the cap height, so lowercase and capitals line up.

## Coverage

Built in this order. Each stage is usable on its own.

| Stage | Set | Count | Needed for |
|---|---|---|---|
| 1 | `A`–`Z`, space | 27 | headings, the splash, the HUD |
| 2 | `0`–`9`, `. , : ; ! ? ' " - / ( )` | 23 | numbers, dates, short sentences |
| 3 | `a`–`z` | 26 | docs and blog body text |
| 4 | `@ # $ % & * + = < > [ ] { } _ \| ~ ^` and `` ` `` | 19 | code samples, links, prices |
| 5 | `₿`, `→ ← ↑ ↓`, `…`, `–` and `—`, `•`, `×`, `°` | 12 | the product's own symbols |
| 6 | Latin-1 accents, built from a base letter plus a mark | ~60 | names and other languages |

That makes about 107 glyphs before accents. Anything else falls back to the
HTML text layer that every Coaxial page already keeps for search engines and
screen readers.

## Data format

```coax
Glyph :: struct {
  code: u32;        // the Unicode code point
  advance: u8;      // width in grid units, gap not included
  first: u16;       // the first point in POINTS
  strokes: u8;      // how many polylines
}
// POINTS holds signed grid coordinates in half units (so 8 means 4), and a
// polyline ends at a BREAK marker. The glyph table is sorted by code, so a
// lookup is a binary search.
```

Half units in an `i8` cover −64 to 63, which is plenty for the grid and its
descenders. Kerning is a short sorted table of pairs with an adjustment in
half units. Only the pairs that need it are listed (`AV`, `LT`, `To`, `r.`
and similar); everything else is spaced by advance alone.

## Rendering

The glyphs feed the stroke pipeline the splash already uses. One quad per
segment carries its position along and across the stroke, and `fx_fs` draws
the core and the glow from the distance to the centre line. Changes needed
to make it a font renderer:

- **Joins**: two segments that meet at a corner currently overlap with max
  blending. That looks right with a glow, but a thin core with no glow needs
  rounded joins. The shader's distance-to-segment test already gives round
  caps, so joins come free if quads extend by the core radius at both ends.
- **Small sizes**: below about 9 CSS px the glow goes and the core snaps to
  whole device pixels, so body text stays crisp.
- **Many glyphs**: a page of text is thousands of segments, so the vertex
  buffer grows past the splash's fixed 576 vertices. Layout writes into a
  buffer sized for the page, and stroke-by-stroke animation is optional per
  block of text.
- **Layout**: line breaking, wrapping at a width and alignment live in a
  `filament.coax` module next to the glyph table. They take text and a box
  and return segments, with no GPU calls, so they can be tested natively.

## Tests

- Every glyph's points lie on the grid, inside its advance and between the
  descender and the cap height.
- Every code point in a stage is present, and the table stays sorted.
- Rule checks: no angle other than 0°, 45° or 90° except the listed
  centre-point diagonals, and no zero-length segments.
- Layout: a known string has a known width, wrapping never overflows its
  box, and an empty string draws nothing.
- A contact sheet: a dev page that draws the whole set at three sizes, read
  back with the `?probe` path for a pixel check and opened by eye whenever a
  glyph changes.

## Open questions

1. **Name.** Filament, unless you prefer another.
2. **One weight or several.** A stroke font can be drawn at any weight, but
   very bold text may need its own glyph shapes. My suggestion is one design
   drawn at any weight, and a separate design only if bold looks wrong.
3. **Body text in Filament or in HTML.** Filament can set a whole blog post,
   but long reading may be easier in a normal typeface. My suggestion is
   Filament for headings, the HUD and code, with the body text in Filament
   at reading weight, then compared side by side with a system font before
   deciding.
4. **Licence.** If it's released on its own, CC0 like `immortal`, or kept
   inside openagents.

## Steps

1. Stage 1 capitals in `filament.coax`, with the rule tests, and the
   contact-sheet page under `apps/openagents-web/`.
2. Move the splash onto it, so `splash.coax` stops carrying its own glyphs.
3. Stages 2 and 3, then layout and wrapping, which unlock the Markdown docs
   and blog pages in Coaxial (`web.md`, phase W3).
4. Stages 4 to 6 as pages need them.

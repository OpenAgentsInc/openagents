# Paper Mono

Paper Mono v1.000 is the typeface for Rust Native, the Verse atlas, the
phone and desktop hosts, and CoderOS. OpenAgents web pages use a bundled
sans face for normal text and retain Paper Mono for monospace content and
the logo. `scripts/check-fonts.sh` rejects font files and family names
outside these scopes.

- Source: the Paper Mono v1.000 release from
  <https://github.com/paper-design/paper-mono>.
- License: SIL Open Font License 1.1 (`fonts/OFL.txt`). The files are
  unmodified; only `PaperMono[wght]` is renamed to `PaperMono-Variable` to
  keep brackets out of paths.

The files live in this crate so that every Rust build context (the Nix
packages, the container images) that already copies `crates/` has them. The
crate (`paper_mono`) exposes the faces to Rust and the CSS rules that HTML
pages use: `font_face(url)` for a page whose server routes
`/fonts/PaperMono-Variable.woff2`, and `font_face_inline()` for a page with
no such route.

| File in `fonts/` | Used by |
|---|---|
| `PaperMono-Variable.ttf` | Rust Native shaping and painting (`wght` 100 to 800), the Verse atlas |
| `PaperMono-Variable.woff2` | Web pages and server-rendered HTML, through `@font-face` |
| `PaperMono-{Regular,Medium,SemiBold,Bold}.ttf` | iOS and Android hosts, CoderOS (`os/pkgs/paper-mono.nix`) |

Paper Mono has no italic, so italic text is drawn upright. It also lacks
Greek, Cyrillic, braille, and some geometric symbols (such as U+2713 and
U+25A0); platforms draw those from their own fallback faces.

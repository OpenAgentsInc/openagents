# Paper Mono

Paper Mono v1.000 is the one typeface every OpenAgents surface uses: Rust
Native, the Verse atlas, the web pages, the phone and desktop hosts, and
CoderOS. `scripts/check-fonts.sh` fails when a surface names another family
or tracks another font file.

- Source: the Paper Mono v1.000 release from
  <https://github.com/paper-design/paper-mono>.
- License: SIL Open Font License 1.1 (`OFL.txt`). The files are unmodified;
  only `PaperMono[wght]` is renamed to `PaperMono-Variable` to keep brackets
  out of paths.

| File | Used by |
|---|---|
| `PaperMono-Variable.ttf` | Rust Native shaping and painting (`wght` 100 to 800), the Verse atlas |
| `PaperMono-Variable.woff2` | Web pages and server-rendered HTML, through `@font-face` |
| `PaperMono-{Regular,Medium,SemiBold,Bold}.ttf` | iOS and Android hosts, CoderOS |

Paper Mono has no italic, so italic text is drawn upright. It also lacks
Greek, Cyrillic, and some geometric symbols (such as U+2713 and U+25A0);
platforms draw those from their own fallback faces.

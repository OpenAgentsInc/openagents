# Aethyr

Aethyr is the engine every Verse realm runs on, written in Coax. This folder
holds its core: a small set of primitives each realm uses instead of keeping
its own copy (GitHub #12609). Aethyrforge, the realm builder and
world-creation tools, is built on top of it. `verse/` keeps realm content.

The [Aethyr book](book/README.md) builds the engine from its smallest
primitives, one checked piece per chapter. Each module below has a chapter,
named by its task slug in the chapter's status block; fill the chapter in when
the module lands.

Each primitive is a module with its own checks and, where it decides
behaviour, zero surviving mutants. One short section per primitive is in
[docs/verse/engine-core.md](../docs/verse/engine-core.md).

| Module | What |
| --- | --- |
| `loop.coax` | The one frame loop: fixed-step simulation, interpolated drawing, one clock |
| `space.coax` | Typed spaces (world, local, view, screen, grid) and units (metres, grid units), converted only by name |
| `refusals.coax` | Checks that what must not compile doesn't: the fixtures in `tests/` |

The older engine modules (`verse/player.coax`, `camera.coax`, `figure.coax`,
`contact.coax`, `terrain.coax` and the rest) move here with the realm
migration (ec-migrate).

```sh
coax/bin/coax test aethyr/loop.coax --library
coax/bin/coax mutate aethyr/loop.coax
coax/web/tool.sh aethyr/refusals.coax      # mixing spaces fails to compile
python3 psionic/tests/book.py --book aethyr --coax coax/bin/coax   # the book and its examples
```

The book's runnable programs are in `examples/`, with their exact output in
`fixtures/`.

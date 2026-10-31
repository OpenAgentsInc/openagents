# Aethyr

Aethyr is the engine every Verse realm runs on, written in Coax. This folder
holds its core: a small set of primitives each realm uses instead of keeping
its own copy (GitHub #12609). Aethyrforge, the realm builder and
world-creation tools, is built on top of it. `verse/` keeps realm content.

Each primitive is a module with its own checks and, where it decides
behaviour, zero surviving mutants. One short section per primitive is in
[docs/verse/engine-core.md](../docs/verse/engine-core.md).

| Module | What |
| --- | --- |
| `loop.coax` | The one frame loop: fixed-step simulation, interpolated drawing, one clock |

The older engine modules (`verse/player.coax`, `camera.coax`, `figure.coax`,
`contact.coax`, `terrain.coax` and the rest) move here with the realm
migration (ec-migrate).

```sh
coax/bin/coax test aethyr/loop.coax --library
coax/bin/coax mutate aethyr/loop.coax
```

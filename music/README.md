# Music (`music/`)

Pure Coax music production, orchestral arrangement, tempo mapping, SMF `.mid` codec, programmatic remix engine, and real-time scheduler bounded context for **OpenAgents Music Studio** ([specification](../docs/music/2026-11-22-coax-music-production-studio-spec.md)).

## Modules

| File | Responsibility | Gate |
| :--- | :--- | :--- |
| `model.coax` | Bounded project state (`960 PPQ`, 32 tracks, 32,768 notes, 256 tempo knots, 64 markers, atomic op-delta undo/redo ring, scale membership) | `coax test music/model.coax` |
| `tempo.coax` | `#authority` drift-free piecewise tempo map (constant + linear accelerando/ritardando ramp integrals & exact inverses) | `coax mutate music/tempo.coax --root-only` (0 survivors) |
| `midi_smf.coax` | Standard MIDI File (SMF Format 0 & 1) importer (replace or mashup merge at tick offset) and Type 1 exporter | `coax test music/midi_smf.coax` |
| `remix.coax` | Programmatic orchestral remix transforms (`transpose`, `quantize`, `humanize`, `copy_layer`, `explode_chords`, `velocity_swell`, `apply_pit_seating`) | `coax test music/remix.coax` |
| `transport.coax` | Real-time sample-accurate event emitter with mute/solo/transpose-preview, clean loop wrap (`ALL_NOTES_OFF`), and 128-bit live active-note masks | `coax test music/transport.coax` |
| `starter.coax` | Original CC0 4-movement symphonic starter suite (`Db Maj 64` → `Db Maj 72` → `D Min 122` → `F Min 132` → `Db Maj / D Min 68`) | `coax test music/starter.coax` |
| `studio.coax` | `#export_c` C11 library face consumed by `apps/music-studio/` | `coax test music/studio.coax` |
| `tools/remix-cli.coax` | Pure Coax CLI for programmatic MIDI inspection, merging, and transposition | `coax/web/tool.sh music/tools/remix-cli.coax help` |

## Build & Launch Studio (macOS)

```sh
apps/music-studio/build.sh --open
```

The app plays immediately using `~/.openagents/music/soundbanks/MuseScore_General.sf2` (MIT, auto-downloadable from inside the Studio toolbar) with automatic macOS built-in DLS fallback.

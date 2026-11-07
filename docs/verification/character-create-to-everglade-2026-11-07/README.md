# Character Create to Everglade: the walk

These checks ran on 2026-11-07 against `coax/web/serve.sh apps/openagents-web --static`
in headless Chrome at 1920x1080. The tool is `coax/web/tests/character-walk.coax`.
The road and the hand-off are described in `docs/verse/character-creator.md`
("The road to Everglade").

The film is 41.6 s at 1920x1080 and 30 fps (45 MB). It is kept outside the
repository at `~/work/film-out/character-create-to-everglade-2026-11-07/walk-1080p.mp4`.
The walk is recorded in clip mode, one fixed step per frame. The load between
the two pages is filmed at its real length, 1.76 s at 50 Mbit/s.

Stills: `1-studio.jpg` (before USE IN EVERGLADE), `2-town-in-the-distance.jpg`
(4 s into the walk), `3-the-gate.jpg` (19 s), `4-everglade.jpg` (arrival, same
character, no dialog).

## Timings, from the click on USE IN EVERGLADE

| | localhost | 50 Mbit/s, 40 ms |
|---|---|---|
| first step | 0.43 s | 0.42 s |
| Everglade's files warm (24.9 MB) | 0.10 s | 4.16 s |
| page changes | 30.39 s | 30.37 s |
| Everglade drawn and seated | 0.55 s after the change | 1.48 s after the change |
| Everglade opened cold, nothing warmed | 0.67–0.71 s | 5.22–5.24 s |

The warm-up finishes during the first 5 s of the walk, so the 30 s walk hides all
of the download. At 50 Mbit/s, the time between leaving the glade and seeing
Everglade drops from 5.2 s to 1.5 s. The rest is the engine starting and the
development host fetching wasm and WGSL with `no-store`. In production those
files come from the cache too.

Both runs passed: the saved appearance matched in Everglade (outfit, body, colorway,
dye, eyes), the room gave a seat, and no character dialog opened.

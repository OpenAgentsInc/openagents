# Speedrun HUD

Animated HUD diagrams over the Grid at https://openagents.com/demos/speedrun-hud?stage=hub, one stage per beat of the speedrun pitch video. Each stage draws its frames, labels, links and charts in the OpenAgents HUD look over a dim Grid floor, while the camera pans, zooms and turns slowly.

## What is here

- 14 stages: `ident`, `years`, `coax`, `wallet`, `compute`, `cost`, `pipeline`, `hub`, `realms`, `identity`, `open`, `timeline`, `rules` and `end`. Each stage runs for its line of the voiceover.
- Frames draw their outlines in and their glass fades up. Labels decipher left to right and settle. Packets travel the links, dot fields fade in from the centre, and streaks cross the picture.
- The words are the founder's own words or plain labels for real things.
- Everything stays above y=900 of a 1344x1080 picture, so captions fit underneath. The page draws at twice its pixels.

The page has a fixed clock and no live data, and it sends nothing. The same address gives the same frames every time.

## Options

- `stage=NAME` picks the stage. Without it the Grid draws on its own.
- `at=MS` starts the stage that many milliseconds in.

## Recording it

```sh
coax/web/tool.sh clip "http://127.0.0.1:8800/demos/speedrun-hud?stage=hub" ~/Desktop/hud_hub.mp4 --size 1344x1080 --seconds 8.51
```

## How it is made

`apps/openagents-web/speedrunhud.coax` holds the stages, the HUD components, the Grid floor and the camera. `gpu/speedrunhud.coax` holds the shaders, and `speedrunhud-face.c` is the browser face. The clip recorder is described in [docs/verse/clips.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/clips.md).

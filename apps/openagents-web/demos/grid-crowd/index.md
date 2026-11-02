# Grid Crowd

A demo of the Verse engine at https://openagents.com/demos/grid-crowd?stage=crowd: 340 characters on the Grid at once, 200 players and 140 agents, running across it and into the Everglade gate while the camera starts high and wide and drifts down to the portal.

## What is here

- The Grid's own art: the floor of lines, the stations, the gym, the Foreman and the Everglade gate with its portal.
- Players: the human character from the OpenAgents app, walking, running and jumping, each on their own clips.
- Agents: the Grid's robot, walking at its own pace.
- Everyone keeps their distance, steers round the stations, and goes through the portal in a lane of their own. Whoever goes through starts again far behind the camera, so the stream doesn't end.

The crowd is simulated from a seed, not anyone's presence, and the page sends nothing. The same address gives the same crowd every time, so a clip of it is the same every time.

## Options

- `stage=crowd` starts the crowd. Without it the Grid stands empty.
- `players=N` and `agents=N` change the counts (up to 1024 players and 1022 agents).
- `seed=N` draws another crowd.

## Recording it

```sh
coax/web/tool.sh clip "http://127.0.0.1:8800/demos/grid-crowd?stage=crowd" ~/Desktop/verse-crowd-portal.mp4 --size 1920x1080 --seconds 18 --skip 2
```

## How it is made

`apps/openagents-web/gridcrowd.coax` holds the stage, the crowd, the camera and the draws; `gpu/gridcrowd.coax` the shaders; `gridcrowd-face.c` poses the players with `verse/cast.coax`. The clip recorder is described in [docs/verse/clips.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/clips.md).

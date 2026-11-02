# Grid Crowd

A demo of the Verse engine at https://openagents.com/demos/grid-crowd?stage=crowd: 380 characters on the Grid at once: 260 players sprinting across it and into the Everglade gate, and 120 agents standing along the way and round the gate to watch them, while the camera starts high and wide and drifts down to the portal.

## What is here

- The Grid's own art: the floor of lines, the stations, the gym, the Foreman and the Everglade gate with its portal.
- Players: the human character from the OpenAgents app, sprinting, some hopping often and some seldom, each on their own clips. The app has one character, so each player gets their own skin tone, hair colour, shirt, trousers and height from the seed.
- Agents: the Grid's robot, standing beside the players' way and round the gate, idling or waving, turning to face the nearest player. None of them go in.
- The players keep their distance, steer round the stations and the agents, and go through the portal in a lane of their own. Whoever goes through starts again far behind the camera as someone new, so the stream doesn't end.
- The page draws at twice its pixels and the browser scales it down, so the robots' lines stay smooth.

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

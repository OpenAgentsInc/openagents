# Water Lab

The water system's test scene at https://openagents.com/realms/waterlab: a sea to the horizon breaking on a sloping beach, drawn in the painted water look. It shows the parts of Verse's water that have no home in Everglade yet. It is unlisted: the galaxy shows it only at https://openagents.com/verse/galaxy?realms=all.

## What it shows

- The spectral sea (phase W4): waves from a wind-sea spectrum, made fresh every frame on the water's own clock, so every visitor sees the same sea at the same moment. On a computer two layers of waves are drawn; on a phone the finer layer is a fixed tile made once.
- Whitecaps where crests sharpen, lingering a few seconds after they break; more of them the stronger the wind.
- Toward the beach the waves shorten, steepen and break into surf, and the sand is wet where it reaches.
- The sea is a mesh of nested rings round you, each twice as coarse as the one inside it, that reaches the horizon (phase W10). The waves lean their crests forward on it. The coast's depth and surf are kept in 8 m squares near you, a few loaded each frame as you move.
- Drag, or use the arrow keys, to turn the view; the mouse wheel or W and S to come closer or go back; Q dives under the water and E comes back up.
- Address options: `?wind=12` sets the wind in metres a second (7 by default), `?t=20` holds one moment, `?yaw=`, `?pitch=` and `?dist=` place the view, `?tier=phone` draws the phone's layers on any screen, and `?sea=traced` draws the sea the older way, traced a pixel at a time, to compare.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `verse/water-spectrum.coax` is the sea: the spectrum, the transform, the foam and the surf, with its checks.
- `verse/water-ocean.coax` is the rings and the 8 m squares, with the checks that every visitor's sea agrees.
- `apps/openagents-web/waterlab.coax` is the realm, `waterlab-face.c` binds it for the browser, `waterlab.wgsl` draws it.
- `docs/verse/water.md` in the OpenAgents repository says what of the water system is done and what is next.

## Make your own realm

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).

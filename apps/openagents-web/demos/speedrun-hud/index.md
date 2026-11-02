# Speedrun HUD

Animated HUD consoles over the Grid at https://openagents.com/demos/speedrun-hud?stage=hub, one stage per beat of the speedrun pitch video. Each stage reads as a console from The Expanse: one main diagram in the middle and instruments round it, in the OpenAgents HUD look over a dim Grid floor, while the camera pans, zooms and turns slowly.

## What is here

- 20 stages:
  - The video's beats: `ident`, `years`, `coax`, `wallet`, `compute`, `cost`, `pipeline`, `hub`, `realms`, `identity`, `open`, `timeline`, `rules` and `end`. Each runs for its line of the voiceover.
  - `payments`: a simulated Lightning network of 190 nodes and a Wavelength operator with its users. The payment rate ramps from 1 to 10,000 a second, with SETTLED, FAILED and ROUTED counters, a sats-volume trace and channel liquidity. The `wallet` beat uses the same storm.
  - `reed` and `fleet`, from Episode 200:
    - `reed` shows Metcalfe's n² against Reed's 2^n on a log scale, with agents running past the Dunbar limit of 150, and agents forming coalitions.
    - `fleet` takes copilots to autopilots: a roster, a task graph, budgets, receipts and royalties.
  - `frame_meteor`, `frame_grid` and `frame_play`: overlays to lay over gameplay footage. They are black with no floor, with dim rails and brackets round the edges, and the middle 60% is left clear.
- Round each stage's main view:
  - a heading tape that reads the camera's own heading;
  - rails of instruments from the HUD library (`aethyr/ui/draw.coax`): an orbit plot, telemetry traces, segmented meters, a spectrum, an arc gauge and a pitch ladder;
  - a terminal strip that streams the stage's own words, a voice waveform and a progress timeline;
  - a faint coordinate lattice and target brackets on the stage's subject.
- Frames draw their outlines in and their glass fades up. Labels decipher left to right and settle. Packets and payments travel the links.
- Stages whose numbers are made up carry a SIMULATED chip.
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

To lay an overlay over a shot, record `frame_grid` the same way and pass it to `labs cut` as `over=`.

## How it is made

`apps/openagents-web/speedrunhud.coax` holds the stages, the Grid floor and the camera. The rest:

- `speedrunhud-console.coax` holds the console round each stage and the overlays.
- `speedrunhud-storm.coax` holds the payment storm.
- `speedrunhud-ep200.coax` holds the Episode 200 stages.
- `gpu/speedrunhud.coax` holds the shaders, including the system sans that the library's small text uses.
- `speedrunhud-face.c` is the browser face.

The clip recorder is described in [docs/verse/clips.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/clips.md).

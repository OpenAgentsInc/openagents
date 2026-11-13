# Gameplay clips

How we get short clips of our games on demand: the height of the action in a
realm, a feature being shown off, or a moment a player really had.

## The pieces

1. **The recorder** (`coax/web/clip.coax`, a Coax tool). Headless Chrome with WebGPU, no
   window, no sound. Host.js has a clip mode (`coaxialClip`): the page draws
   only when told, and every frame is one fixed step of game time. The
   recorder steps a frame, captures it, and repeats, then encodes with ffmpeg.
   A slow machine or a slow capture never makes a clip stutter, and the same
   inputs give the same clip.

   ```shell
   coax/web/tool.sh clip URL OUT.mp4 --size 1280x720 --seconds 3 --skip 6
   ```

   `--skip` runs game time without capturing (the lead-up); `--eval` runs a
   script before it (inputs, camera); `--frames DIR` keeps the PNGs.
   `--audio` adds the page's own sound: nothing plays while the frames are
   drawn, host.js keeps each sound line with its frame's game time, and after
   the last frame it renders them in an `OfflineAudioContext` through the
   same voices the live page uses. The recorder muxes that WAV in as AAC, so
   the sound is in step with the frames however slowly they were captured.
   `tool.sh` builds the recorder on first use (the Coax compiler and a C
   compiler); it needs Chrome and ffmpeg. The recorder uses the checkout's
   resident browser, and `--keep` records on from where the last `--keep`
   clip of the same URL ended, without loading the page again
   ([fast-shots.md](fast-shots.md)).
2. **Staged moments** (`?stage=NAME` in the page's URL). Each game knows its
   own best moments and can set one up from a seed: positions, velocities,
   the camera, the bot's plan. A stage is game code with a test, so it keeps
   working as the game changes, and anyone (or any agent) can ask for it by
   name. Every realm should grow a `highlight` stage and a few named ones.
   `&card=1` leaves the HUD out (Hold the Stair, Orbit League, Belt 2200,
   Dead Gears): the link-preview cards are shot that way
   (`coax/web/og-render.ts`).
3. **Where clips come from**, in order of trust:
   - **Recorded:** a real session replayed. Games that are deterministic
     (fixed steps, seeded dice) only need the inputs and the seed, so a
     replay log of a few kilobytes re-renders the moment at any size, angle
     or frame rate. Analytics already sends events; a replay log is the same
     path with inputs in it, kept only with the player's consent.
   - **Simulated:** a bot plays (the scripted players our tests already use)
     and a director picks the best seconds by a score per game: hits per
     second, damage, speed, pops, goals.
   - **Staged:** a named `?stage=` setup, for a specific feature or shot.
   - **Generated:** a video model given a still or a staged clip as the
     first frame. Good for mood pieces; never shown as gameplay.
4. **A label on every clip** (a sidecar `.json`): source (recorded,
   simulated, staged, generated), game, commit, stage or replay id, seed,
   size and frame rate. Recorded clips name the session, never the player.

## Staged demos

- **Grid Crowd** (`/demos/grid-crowd?stage=crowd`, `apps/openagents-web/gridcrowd.coax`):
  380 characters: 260 players from Cast.bin sprint (and hop) across the Grid
  into the Everglade gate while 120 Grid robots stand along the way and round
  the gate watching, under a high-wide-then-drift camera. Each player gets a
  seeded skin tone, hair colour, shirt, trousers and height. The page draws at
  twice its pixels (`data-supersample="2"` in `coax/web/host.js`), so lines
  and edges are smooth. Seeded and stepped on whole ticks, so two recordings
  are byte-identical frame for frame.
  `&players=N&agents=N` (up to 1024 and 1022) and `&seed=N` change it.

  ```shell
  coax/web/tool.sh clip "http://127.0.0.1:8800/demos/grid-crowd?stage=crowd" ~/Desktop/verse-crowd-portal.mp4 --size 1920x1080 --seconds 18 --skip 2
  ```

- **Speedrun HUD** (`/demos/speedrun-hud?stage=NAME`, `apps/openagents-web/speedrunhud.coax`):
  Expanse-style HUD consoles over a dim Grid floor, one for each beat of the
  speedrun pitch video (`ident`, `years`, `coax`, `wallet`, `compute`, `cost`,
  `pipeline`, `hub`, `realms`, `identity`, `open`, `timeline`, `rules`,
  `end`). Episode 200's ideas are also staged: `payments` (a simulated
  Lightning and Wavelength payment storm), `reed` and `fleet`. The
  `frame_meteor`, `frame_grid` and `frame_play` stages are overlays for
  `labs cut over=`: black, with the middle 60% left clear. Instruments come
  from `metaverse-engine/ui/draw.coax`. Frames draw in, labels decipher and settle, and
  packets travel the links. Everything stays above y=900 so captions fit
  underneath. Stepped on whole ticks, so the frames are the same every time.

  ```shell
  coax/web/tool.sh clip "http://127.0.0.1:8800/demos/speedrun-hud?stage=hub" ~/Desktop/hud_hub.mp4 --size 1344x1080 --seconds 8.51
  ```

- **Darian Odiodei** (`/worlds/everglade?stage=darian` or `?stage=darian-15`,
  `apps/openagents-web/everglade-raid-preview.coax`): while the Cultist Raid's
  leader is drawn, the camera stands 4 m (or 15 m) off his left side, looking
  at him and his staff. It works in the offline preview (`&raid=preview`) and
  in a room with the raid running (`&relay=...&room=...`). Until he is drawn,
  the camera is yours.

  ```shell
  coax/web/tool.sh shot "http://127.0.0.1:8800/worlds/everglade?raid=preview&stage=darian" ~/Desktop/darian.png 1440x900 3000
  ```

- **Housewright time-lapses** (`/worlds/everglade-experimental?stage=house-leanto`, `house-hut`,
  `house-cottage`, `house-timber`, `house-stone`, `house-row`; Episode 293, #13107): one house
  builds on the open grass at (-60, -38), south of the glade's village, from its bare site to finished, drawn by the build sites
  at any progress (#13106), with no HUD. The game layer
  (`everglade/world/housewright-stage.coax`) holds each shot's progress keys, its camera track
  (`metaverse-engine/cam/track.coax`) and its sky clock. The sun moves over the build: the stone cottage
  passes a full day and night. The corner shows the speed (the tier's workdays over the clip) and
  the workday. Under the house, the text gives its starting materials and its work. `house-row`
  stands the five finished houses small to large, with the tool that unlocks each under it. The
  clock counts game steps of the frame time, so two recordings give the same frames.

  ```shell
  coax/web/tool.sh clip "http://127.0.0.1:8800/worlds/everglade-experimental?stage=house-cottage" ~/Desktop/cottage.mp4 --size 1920x1080 --seconds 10
  ```

  Lengths: lean-to 5 s, hut 8 s, cottage 10 s, timber house 8 s, stone cottage 10 s, row 6 s.

## Native film frames

A realm that runs natively renders a clip without Chrome: `vk-realm`
(`metaverse/gpu/vk-realm.c`) steps it headless, and writes every frame in a
range as it was drawn, at a film rate (#13003, phase F0 of
[film mode](../research/vork/2026-11-07-desktop-film-renderer.md)):

```sh
./vk-realm DRAWDIR GIVEN... --size 1920x1080 --headless 300 --fps 24 \
  --frames-dir OUT --film-start 48 --film-end 287 [--facts STAGE.facts] [--replay ROOM.log] [--keys F:KEY:DOWN,...]
coax/web/tool.sh metaverse-engine/tools/film-frames.coax OUT --out OUT/film.mp4 [--audio MIX.wav]
```

`--fps N` advances each frame 1/N s through the realm's own fixed step and
interpolation, so the world is the one the game would show at that time;
the default stays 60, and a golden run is unchanged to the bit.
`OUT/frame-NNNNNN.rgba16f` is the raw half-float output of the realm's final
targets; `film.txt` names the size, targets, rate, range and device, and
`timing.txt` the device open and each frame's step, GPU and read-back time.
`film-frames.coax` writes `meta.json` (`film.frames.v1`: each frame's SHA-256
of its raw bytes and its timings in microseconds), review PNGs of the
screen target in `OUT/review/`, and with `--out` an H.264 encode. Two runs of
the same input give the same hashes; hashes are not promised across GPU
vendors.

### The film profile

Each written frame can instead be the average of N samples (#13004, phase
F1; the record and its checks are `metaverse-engine/render/film.coax`):

```sh
./vk-realm DRAWDIR GIVEN... --size 1920x1080 --headless 300 --fps 24 \
  --frames-dir OUT --film-start 48 --film-end 287 \
  --samples 16 --shutter 0.5 --shutter-position center --seed 1 \
  --fstop 2.8 --focus 6 --film-view 8 --film-audio OUT/audio.wav
```

- `--samples N` (1 to 256) draws each frame N times. Sample times are
  stratified inside the shutter (`--shutter` frames open, 0 to 1, placed
  `start`, `center` or `end` of the frame's time) and kept in whole
  nanoseconds, so the realm runs the same fixed steps it runs at one sample.
- Each sample moves the realm's view-projection (the 16 uniforms at
  `--film-view U`; Everglade's is 8, `U_VIEW`) by its pixel jitter (a Latin
  square of strata; `--no-jitter` turns it off) and, with `--fstop` and
  `--focus` metres, its eye on the aperture, keeping the focus plane still.
  The lens's focal length is the one the realm's field of view gives on a
  36 mm sensor.
- The final targets of every sample are read back and added in f32; the
  mean is written, rounded to half as the targets are. Bloom and tone run in
  each sample, as the realm's graph draws them.
- `--film-audio OUT.wav` renders the realm's own mixer with no device, frame
  by frame from the same clock, as 48 kHz stereo f32.
- `--samples 4 --no-jitter` gives the one-sample frame to the bit; the
  film profile changes no simulation.

On the RTX 4080, Everglade at 1920x1080 with 16 samples, a half-frame
shutter and f/2.8 took 1.6 to 2.0 s a frame (85cfc9d1b1's producer).

### EXR layers

`film-frames.coax OUT --exr` also writes `OUT/exr/fNNNNN.exr` (#13005, phase
F2): one OpenEXR file a frame, written by `metaverse-engine/render/exr.coax`. Each file is
single-part scanline, ZIP compression (zlib over 16 lines, from
`coax/lib/image/deflate.coax`), half channels, scene-linear and
premultiplied. The screen target is `R`, `G`, `B`, `A`; any other target K is
`targetK.R` and so on. ffmpeg's EXR decoder reads the files.
`metaverse-engine/render/exr.coax` also reads any channel back as f32, for `labs cut`.
`metaverse-engine/tests/exr_oracle.py` decodes the files on its own and checks every
value bit for bit: run it bare for the sample files, or with `--frame
FRAME.rgba16f FILE.exr` for a film frame. A 1920x1080 Everglade frame
writes in under a second and takes 4.4 MB; its raw halves take 16.6 MB.

## Next

- Inputs on a timeline in `clip.coax` (key and pointer events at frame
  numbers), so a clip can be "played" without touching game code.
- A director: simulate N seeds, score each second, cut the top three.
- Replay logs for the deterministic games, then "clip that" from a session.
- Camera paths (orbit, dolly, slow motion by stepping smaller dt).
- Vertical 9:16 and square crops from the same run.

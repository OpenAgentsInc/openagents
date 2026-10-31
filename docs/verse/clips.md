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
   `tool.sh` builds the recorder on first use (the Coax compiler and a C
   compiler); it needs Chrome and ffmpeg.
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

## Next

- Inputs on a timeline in `clip.coax` (key and pointer events at frame
  numbers), so a clip can be "played" without touching game code.
- A director: simulate N seeds, score each second, cut the top three.
- Replay logs for the deterministic games, then "clip that" from a session.
- Camera paths (orbit, dolly, slow motion by stepping smaller dt).
- Vertical 9:16 and square crops from the same run.

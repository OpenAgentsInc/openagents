# Animation scenes and cuts v1

A5 plays admitted performances, scenes and edits in Verse. The realm has two
stations: `#scene` uses the first scene camera, and `#cut` follows the edit.
Both have a timeline, play/pause and frame stepping. **N** selects Scene;
**X** selects Cut. Add `?card=1` to the hash to hide the controls for capture.

## Project files

A local `project.json` supplies the files a scene references:

```json
{
  "schema": "anim.project.v1",
  "libraries": ["character.bundle"],
  "performances": ["actor.performance.json"],
  "scene": "film.scene.json",
  "cut": "film.cut.json",
  "voices": ["line.sat"]
}
```

Paths resolve beside the manifest. Libraries are ALB1 files from the import
or retarget tools. Each library owns its rig, clips and immutable binary.
The scene and performance documents keep their A0 formats. A performance's
clip references resolve within its rig's library. Voice entries follow the
scene's voice cues in order; music and effect cues reference a WAV span in
a library's binary.

The native commands assemble and admit an ALP1 project before rendering.
`play performance.json` creates a studio preview of that performance;
`play scene.json` uses the first camera over the scene's full duration;
`play cut.json`, `play project.json` and `play project.bundle` use the edit.
The JSON forms require the adjacent manifest. A referenced performance must
be listed in it. Source file names are literal paths, not directions.

ALP1 is a retained transport, not a replacement document format. Its
little-endian header is `ALP1`, total bytes, library count, performance count
and voice count, each a 32-bit word. A table of `(offset, bytes)` pairs
follows. Payload order is libraries, performances, scene, cut and voices.
Ranges are positive, contiguous and consume the complete file. The bounds
are 64 MiB, two libraries, 16 performances, 32 voices and 64 draw commands.
The runtime admits each stage in an inactive bank and publishes only after
all stages and references pass. Failure preserves the active project.

To package an admitted transport explicitly:

```sh
coax/web/tool.sh aethyr/tools/anim-project-pack.coax PRIVATE_DIR/project.json PRIVATE_DIR/project.bundle
```

## Render and check

Run the local web server with the Animation Lab engine built. The CLI uses
`http://127.0.0.1:8800` by default. Set `OPENAGENTS_ANIM_ORIGIN` to another
`http://127.0.0.1:PORT` when using a separate server.

```sh
openagents labs anim check PRIVATE_DIR/project.json
openagents labs anim play PRIVATE_DIR/project.json PRIVATE_DIR/film.mp4
```

Output and its existing parent must resolve under `~/.openagents/`. Existing
films and frame directories are refused. The CLI stages the admitted bytes
under their full SHA-256 in the private web directory; it does not change the
owner's inspection alias. Chrome stays headless and muted. Each frame seeks
the exact authored frame index, captures the actual WebGPU target and saves
a PNG. The first frame is time zero; the end is exclusive. Capture is
960 × 540 at the cut's rate. A5 limits a render to 60 seconds so jobs remain
bounded; use shorter edits for longer work.

The adjacent `film.mp4.frames/` directory contains PNGs, the mixed PCM WAV,
`receipt.json` and `timing.json`. The receipt records every RGBA frame hash,
the engine hash, viewport, capture timing and browser errors. Timing records
wall time, film duration and render seconds per film second. ffmpeg encodes
those frames and muxes audio after capture. It does not render the scene.
Exact hash comparisons apply to the recorded engine, shader and GPU backend;
they do not promise identical floating-point output across GPU vendors.

`check` emits one JSON object. It reports pose and camera jumps at every cut,
raw and composite face ranges, canonical elbow/knee screening, character
capsule intersections, contact drift and cumulative planted-foot travel,
head framing, and repeated CPU palette/face bits. Camera changes are labeled
as authored cuts. Foot checks sample contacts at 120 Hz; the other sampled
checks use 30 Hz. Source face values are checked before display clamping.
The GPU determinism evidence is in the play receipt.

The joint screen uses a 170° change from rest for canonical elbows and knees;
`rig.v1` has no authored per-joint limits. Collision diagnostics use upright
character-root capsules, radius 0.3 m and height 1.8 m. They do not substitute
for self-collision or prop shapes that the documents do not yet declare.
These profiles are included in the JSON so a missing model is visible.

## Voice and sound

A SAT1 entry binds an `aethyr/audio/speak.coax` track to audio. Its header is `SAT1`,
track bytes and WAV bytes, each a 32-bit word; track text and WAV follow.
Admission checks the complete envelope, track ranges and canonical PCM16
WAV format: mono/stereo, 22,050 or 48,000 Hz, up to 1,000,000 sample frames.
Music and effect spans use the same WAV admission. A raw speak track is a
silent stand-in for realm inspection; video rendering refuses a missing WAV.

An existing canonical WAV and its words become a SAT1 entry with:

```sh
coax/web/tool.sh aethyr/tools/anim-voice-pack.coax PRIVATE_DIR/line.wav "Leave. Now." PRIVATE_DIR/line.sat
```

The packer aligns phonemes to the audio envelope through `aethyr/audio/speak.coax`.
It invokes no paid or network voice service. The realm applies those visemes
to a `jawOpen` stand-in and starts sound cues as playback crosses their start.
Live seeking is silent; it does not resume a partially played cue. Offline
mixing samples cue offsets from the edit clock, including seeks into a cue,
uses linear resampling when needed, and clips the final PCM to its valid range.

## A5 render profile

Studio and Skyreach's original courtyard are implemented. Canyon and image
crossfades are refused during project admission rather than substituted.
Cuts and fades through black use the shared cut clock. Fades belong to the
incoming shot; they do not shorten edit duration.

Lights interpolate their authored keys. Their colour/intensity contributions
sum into the study shader's directional key. Effects are deterministic
procedural stand-ins: the guardian has a body, head and flapping wings;
other event kinds use a glowing marker. Stage weather/time and camera focus
remain metadata. A6 owns the finished render styles and depth of field.
This profile is a blocking renderer, not source-material parity or the
publishable Guardian.

The [Guardian recipe](../../verse/animlab/guardian/README.md) combines private
Valley motion with original staging, wand keys, props, extras, a procedural
bird and a speech-driven stand-in face. All licensed payloads, speech,
reports, frames and videos stay outside git. A8 replaces study motion,
faces and voice with the owner's original captures before publication.

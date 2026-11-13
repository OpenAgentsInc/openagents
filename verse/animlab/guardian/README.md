# The Guardian study recipe

These are original staging documents for A5's 30-second, six-shot study.
They contain no licensed motion, mesh, face or audio payload. The study uses
Valley's qualified A0 walk retargeted to the original mannequin. It remains
private until A8 replaces the study sources with original captures.

The generated character has an original jaw-opening morph and a wand mesh
weighted to the canonical right hand. Original upper-arm and forearm keys
raise it; a hand reach holds the upper pose. The two hooded stand-ins use
original recoil keys. The static gate post is a separate original prop rig.
The guardian bird is procedural geometry, with deterministic wing keys.
The scene's voice cue drives its stand-in face through `metaverse-engine/audio/speak.coax`.

## Prepare private files

Choose a new private study directory. Its parent must already exist.
The qualified A0 document directory must contain `clip-1.json`,
`echo.rig.json` and their hash-named binary. The following commands show the
existing owner-local layout; explicit paths can be substituted.

```sh
study="$HOME/.openagents/aethyrforge/animation-lab/guardian-study"
mkdir -p "$study"
openagents labs anim retarget "$HOME/.openagents/aethyrforge/animation-lab/a0-documents/clip-1.json" --to mannequin --out "$study/walk"
cp verse/animlab/guardian/*.json "$study/"
coax/web/tool.sh metaverse-engine/tools/anim-guardian-study.coax "$study/walk" "$study"
```

This writes `guardian.bundle`, `wand.bundle`, `guardian.performance.json`
and `extras.performance.json`. It retains the source identity in the body
clip's provenance. Contact annotations come from source foot height and
speed before fix-up targets are selected. The base intro/outro exclude
annotations over their blend intervals, as in A3. The completed study then
holds its feet while the upper body performs the keyed action.

For an internal stock-voice stand-in on macOS, write speech to a file. This
command does not play through the speakers or invoke a paid service.

```sh
/usr/bin/say -v Samantha -o "$study/leave-now.aiff" 'Leave. Now.'
ffmpeg -v error -y -i "$study/leave-now.aiff" -ar 48000 -ac 1 -c:a pcm_s16le -fflags +bitexact -flags:a +bitexact -map_metadata -1 "$study/leave-now.wav"
coax/web/tool.sh metaverse-engine/tools/anim-voice-pack.coax "$study/leave-now.wav" 'Leave. Now.' "$study/leave-now.sat"
```

This voice is for the private study. The publishable version needs an
original voice recording and the original face/motion sessions described
in A8. A5 does not grant publication rights to the study sources.

## Play and verify

Build the local Animation Lab Wasm and generated shader, then run the local
web server. The CLI uses its own full-hash private alias; it preserves the
owner's Echo inspection alias.

```sh
openagents labs anim check "$study/project.json"
openagents labs anim play "$study/project.json" "$study/guardian-1.mp4"
openagents labs anim play "$study/project.json" "$study/guardian-2.mp4"
```

Compare `frame_hashes` in the two adjacent `.mp4.frames/receipt.json` files.
Keep the reports, timing, WAV, PNGs and MP4s private. The render command
prints the output path; timing is in `.mp4.frames/timing.json`.

The scene is a blocking study: original mannequin bodies, a simple mouth
shape, a hand-bound wand and procedural effects. It does not claim final
MetaHuman appearance, clothing, source materials or depth of field. Camera
focus and stage weather/time are retained metadata. S1–S6 use authored hard
cuts; the reusable player also supports incoming fades. A6 supplies the
finished render styles; A8 supplies the publishable performance.

The [A5 profile](../../../docs/verse/anim-scenes-v1.md) defines file selection,
transport and audio bounds, diagnostics and the supported transitions.

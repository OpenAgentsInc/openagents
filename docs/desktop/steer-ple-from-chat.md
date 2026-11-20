# Steering the PLE Gym from a chat

Date: 2026-11-20. Issues: #13792 (the 3D PLE Workbench) and #13812 (what is left after this
slice). Code: `apps/openagents-desktop/ple-run.coax`,
`apps/openagents-desktop/page/shows.coax`, `metaverse-engine/import/ple-scene-out.coax`,
`coax/lib/agent/desktop/worker.coax` (`dc_tools_note`).

The question, asked in the PLE chat: *"How can I steer this programmatically via a chat in the
main OpenAgents desktop? Do I need tools or something? If I say 'Trigger a run and show me,'
can it show it to me in line?"*

## Short answer

Yes, now. Microcoder already runs any shell command, so a chat needed no new tool *protocol*.
It needed three things: a command that starts a PLE run and writes something to look at, a
way for the chat to know that command exists, and a way for the transcript to draw a picture
a command produced. This change adds all three. Say "trigger a PLE run and show me" in any
chat (with Experimental features on). Microcoder runs `ple-run feeders`, and the before and
after pictures of the run appear in the transcript under the message's activity, over the
reply. Press a picture and that run's scene opens in the right-hand 3D pane (the Metal
viewport from Stage 1), where you can orbit, slice and toggle layers. The run also shows up in
the Gym with its timeline.

## How a chat steered the Gym before this

- **The Gym and the 3D pane are host views, not chat tools.** `GymWorkbenchView.swift` and
  `SceneView.swift` are AppKit/Metal views in the app. The Gym finds runs by scanning
  `~/work/ple-kit/runs`, `../ple-kit/runs` and `~/.openagents/ple/runs` for `.ple-scene.json`
  and `output/model.ifc` files. Live mode polls the selected file every 1.5 s. No route or
  command lets a chat drive them.
- **A chat could already write the files the Gym reads.** Microcoder's `commands` tool runs
  any shell command (30 s each), so a chat could run a converter or a script that writes a
  scene into one of those folders. The Gym would then list it and tail it. But nothing told
  Microcoder those folders or tools exist, and nothing came back into the chat.
- **A reply could name a scene file.** A path in a reply's inline code is pressable
  (`CP_PATH`), and a `.ple-scene.json` opens in the 3D pane. That is a link, not something
  shown inline.
- **The transcript drew no pictures except a message's attachments.** `cp_turn_images` draws
  thumbnails of images attached to *your* message. A command's output was text only (its
  first 2,000 bytes).
- **The Foreman control API (`docs/coder/foreman-control-api.md`, `foreman-ctl`) steers
  chats, not the Gym.** It creates, prompts, stops and reads chats from outside. It has no
  route into the Workbench, and only the Foreman chat is told about it.
- **A real PLE benchmark run is heavy.** `ple-kit`'s `eval/run.py` needs Docker, Harbor, a
  Hugging Face token and the Vertex model proxy. It takes hours per task and writes
  `output/model.ifc`, which the Gym converts with `tools/ple-ifc-to-scene.py`. A chat command
  limited to 30 s can't run it in the foreground.

So what was missing was not a tool protocol. It was (1) a run command that fits in a chat
step and leaves pictures, (2) telling the chat about it, and (3) inline pictures in the
transcript.

## What this change adds

### 1. `ple-run`, a Coax tool the app ships

`apps/openagents-desktop/ple-run.coax`, built by `build.sh` into
`OpenAgents.app/Contents/Resources/ple-run` beside the workers:

```sh
ple-run feeders [--scene FILE] [--name NAME] [--out DIR]
ple-run show FILE.ple-scene.json [--view iso|plan|overhead|under_floor] [--out PICTURE]
```

- `feeders` is a routing run on a data-hall scene. By default it uses the bundled
  `data-hall-mep-routing` sample. It routes a feeder from every PDU up into the overhead
  plenum and along its row toward the middle of the hall. Each try is checked for box
  overlaps against every element, and a try that clashes moves to the next lane (row offset,
  then plenum height). Every try is written as a snapshot, `step_NNN.ple-scene.json`, which
  the Gym's timeline plays and its Live mode tails. The final scene is `run.ple-scene.json`,
  the log is `trial.log` (the Gym's diagnostics drawer reads it), and `before.jpg` and
  `after.jpg` are pictures of the scene before and after (added feeders in green). Runs go to
  `~/.openagents/ple/runs/feeders-<ms>/`, which the Gym lists. On the sample it routes 4 of 4
  feeders in 8 steps, in about 0.6 s.
- `show` pictures any `ple-scene.v1` file. Use it for a converted `model.ifc` or a real
  ple-kit run.
- **It is honest about what it is.** The router is local and deterministic, with no model.
  It keeps the scene's own `run.score` and writes `run.router` saying that its numbers are
  not a PLE grade. The clash and connectivity gates say what the feeders changed.

Its pictures are ray cast on the CPU in Coax (`ple_shot` in
`metaverse-engine/import/ple-scene-out.coax`) with the engine's own PLE ray tests. Shading is
one sun plus a sky, the architecture layer is see-through, clash points are red balls, and
elements are outlined. The pictures are encoded with the shared JPEG encoder
(`coax/lib/image/jpeg_enc.coax`). So the tool needs no window and no GPU. The same file
writes a scene back to JSON (`ple_scene_write`), and checks confirm it reads back.

### 2. The chat is told

`dc_tools_note` (Coax) words a short note that goes ahead of every message, next to the
skills note. It says where `ple-run` is, what its two verbs do, and how any command can show a
picture inline. The Swift host only finds the program (`coderToolsNote()` in `Worker.swift`).
The note is sent only while Experimental features are on, as the Gym is.

### 3. Pictures inline in the transcript

`page/shows.coax`: a command whose output has these lines

```
OA-SHOW: /absolute/picture.jpg
OA-SHOW-CAPTION: words              (optional)
OA-SHOW-OPEN: /absolute/file        (optional: what a press opens)
```

gets each picture drawn as a card up to 560 points wide. The cards sit under the message's
activity group (so they show while the group is folded) and over the reply, with the caption
under each. The host decodes the picture as it does an attachment (`cp_image_request`; the
decode limit is now 1024 px a side). A press opens `OA-SHOW-OPEN` through the same path as a
file a reply names, so a `.ple-scene.json` opens in the 3D pane. At most 4 pictures are kept
per message (the newest). The markers live in the command's output, which is kept with the
chat, so a reopened chat shows its pictures again while the files exist. Any command or
script can use the markers. Nothing is PLE-specific in the page.

## What is not done yet (#13812)

- **A live view inline.** The 3D viewport embedded in the transcript, or frames streaming
  while a run is going, instead of pictures at its end. Today, live means pressing the picture
  (3D pane) or the Gym's Live mode.
- **Real ple-kit runs from a chat.** Starting `eval/run.py` in the background (it outlives a
  30 s command), reporting its progress, then converting `output/model.ifc` and showing it.
  `ple-run show` already pictures any scene once it exists.
- **A Gym route for programs.** Selecting a run, a layer or a camera in the Workbench from a
  chat or from `foreman-ctl`.
- **Pictures in the flat path.** Pictures, like attachment thumbnails, draw only when the
  window composes (composition-v1). With Reduce Transparency on, a card shows its icon.
## The cylinder axis (ple-scene.v1)

A cylinder element may say which local axis its length runs along, before `rot_z`:

```json
{"id": "chw_pipe_01", "shape": "cylinder", "center": [13.5, 4.0, 3.9], "size": [22.0, 0.25, 0.25], "rot_z": 0.0, "axis": "x"}
```

`axis` is `"x"`, `"y"` or `"z"`. A missing field means `"z"` (upright), as every scene before
the field was read. `size` is the cylinder's box in its own frame, so the size on the axis is its
length and the other two are its diameters. Any other value refuses the scene. The engine reads
it (`PleAxis`, `ple_axis_in` and `ple_axis_out` in `metaverse-engine/import/ple-scene.coax`) for
picking, `ple_shot` and its normals, and `ple_scene_write` writes it back when it is not `"z"`.
The Gym's Metal viewport turns its unit cylinder the same way (`pleModelMatrix` in
`SceneView.swift`). The sample's chilled-water pipes used to be `[0.25, 0.25, 22]` with `rot_z`
1.571, which drew as 22 m poles. They are now `[22, 0.25, 0.25]` along `"x"`
(`tools/ple-ifc-to-scene.py --sample`, which wrote the fixture).

## Checks

```sh
coax/bin/coax test metaverse-engine/import/ple-scene-out.coax --library   # write and read back; normals; the picture
coax/bin/coax test apps/openagents-desktop/ple-run.coax                    # the feeder run: clash, next lane, no lane, no PDU
coax/bin/coax test apps/openagents-desktop/page/checks/shows-checks.coax --library   # markers, cards over the reply, press opens the scene
coax/bin/coax test coax/lib/agent/desktop/worker.coax                      # the tools note
coax/web/tool.sh apps/openagents-desktop/ple-run.coax feeders --out /tmp/ple-try   # a real run on the sample
```

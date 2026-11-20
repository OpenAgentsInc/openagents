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
the Gym with its timeline. The After card is itself a live 3D view of the run's scene, which
you can turn in the chat ("A live 3D card inline" below).

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
ple-run kit start [--task TASK] [--kit DIR] [--harness DIR] [--env-file FILE] [--name NAME]
ple-run status NAME|DIR
ple-run stop NAME|DIR
ple-run ifc FILE.ifc [--out SCENE] [--task TASK]
ple-run gym [RUN|latest] [--camera VIEW] [--only|--show|--hide L,L] [--step N|last] [--select ID] [--live on|off] [--no-open]
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
- `show` pictures any `ple-scene.v1` file.
- `kit start` starts a real ple-kit eval and returns at once; the run outlives a chat
  command's 30 s. It runs `uv run python eval/run.py HARNESS --tasks TASK [--env-file FILE]
  --out RUN/eval` in the kit, in its own session (`proc_detach`), its output in `RUN/run.log`
  and its pid in `RUN/run.pid`. `RUN/kit-run.json` (`ple-kit-run.v1`) records the task, kit,
  harness, env file, out folder, log, pid and start time. `RUN` is
  `~/.openagents/ple/runs/NAME` (`kit-TASK-<epoch ms>` unless `--name`), where the Gym lists
  it. The kit is `--kit`, `$PLE_KIT` or `~/work/ple-kit`; the harness `--harness`,
  `$PLE_HARNESS`, or the first of `../ple` and `../my-harness` beside the kit with a
  `submission.yaml`; the env file `--env-file`, else the kit's `local.env` when it has one; uv
  `$PLE_UV`, else on `PATH`, else `~/.local/bin/uv`. The task is `data-hall-mep-routing`
  unless `--task`. A real run takes hours and spends what the kit spends (the model and the
  judge through the model proxy), so the chat note says to start one only when asked.
- `status` says whether the run is running (its pid is alive), finished (the task's score and
  status from `RUN/eval/results.json`) or ended without results, and prints the log's last
  lines. Once an `.ifc` is anywhere in `RUN/eval` (12 folders deep; the newest), it is read
  into `RUN/model.ple-scene.json`, pictured as `RUN/model.iso.jpg` and shown inline with
  `OA-SHOW` lines, so pressing it opens the model in the 3D pane.
- `stop` sends the run a TERM (Harbor cancels its trials).
- `ifc` reads any IFC model into a scene and its picture.

The IFC reader is Coax (`metaverse-engine/import/ple-ifc.coax`), since a Mac rarely has
ifcopenshell. It reads the STEP text, follows each product's placement chain, and takes its
'Body' (else 'Box', else another) representation: extrusions of rectangles, circles and closed
profiles, bounding boxes, triangulated and polygonal face sets, booleans (the first operand)
and mapped items. One solid keeps its orientation when the scene can say it (a box turned about
z, an upright cylinder, a cylinder lying along a horizontal direction, `axis` `"x"` and
`rot_z`); anything else, and a product of several solids, is the box around it. The layer comes
from the class, or a proxy's name. A run's score is not in the model, so the converted scene
has score 0 and only the schema gate.
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
skills note. It says where `ple-run` is, what its verbs do (a real `kit start` only when asked),
and how any command can show a picture inline. The Swift host only finds the program (`coderToolsNote()` in `Worker.swift`).
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

Pictures draw through composition-v1, the path the frosted menu uses. Reduce Transparency turns
off the frost but not the pictures: the host says so with `coder_page_frost`, apart from
`coder_page_compose`, so a card still shows its picture. A frame the planner can't take (a window
over 4096 device pixels a side, or after the GPU refused a composed frame) is drawn flat, and the
flat shader has no picture source, so there each card shows its icon and caption rather than an
empty frame (`cp_image_request` hands a picture only to a frame that composes).

## A live 3D card inline (#13812)

A card can be the scene itself rather than its picture. Add a fourth line after its
`OA-SHOW-OPEN`:

```
OA-SHOW-LIVE: on                    (optional: with a .ple-scene.json OA-SHOW-OPEN)
```

`ple-run feeders` marks its After card live, `ple-run kit status` its run's model, and
`ple-run show FILE --live` any scene. While Experimental features are on, the newest live card
that is on screen records its rect and its scene each frame (`cp_show_live` in
`page/shows.coax`), along with the conversation's clip, and `coder_page_live_scene`
(`cd_live_scene`) hands them to the host. The host (`syncLiveScene` in `Window+Files.swift`,
called each tick beside the 3D pane's sync) mounts a compact `PleScenePanelView` over the card
inside a view cut to the clip, so it scrolls with the chat and never covers the composer or the
title bar. Compact means the scene and its summary only, no tool bars, layers or inspector. Drag
turns it; the wheel scrolls the chat and Option-wheel zooms. Twice a second the host checks the
file's size and time, and it reloads a changed file keeping the camera, so a run that rewrites
its scene is seen live in the chat. The picture stays under the viewport. It shows when
Experimental is off, for every other live card, when the file fails to load, and with Settings
or the Gym open, since the conversation isn't drawn then.

Not covered: page menus and popovers that open over a live card are drawn under the viewport (the
3D pane has the same limit), and only one card at a time is live.

## A Gym route for programs

A chat (`ple-run gym`) or a program (`POST /v1/gym`, `foreman-ctl gym`) steers the Gym: which run
it shows, the step, the camera, the layers, the selected element and Live. Both send one request,
`ple-gym-request.v1`:

```json
{"id": "r1", "run": "latest", "camera": "plan", "only": ["racks", "pdu"], "step": "last", "select": "pdu_01", "live": false}
```

Every field is optional. `run` is `"latest"` (the newest run's `run.ple-scene.json` or
`model.ple-scene.json`, else its newest scene), a run's name under `~/.openagents/ple/runs`, or a
run's folder or scene file. `only` hides every other layer; `show` and `hide` then change the
ones they name (short names such as `arch`, `pipes` and `all` work too). `step` counts from 1.
`open: false` leaves the Gym off screen. A key it does not know refuses the request.

The desktop core checks a request and words it as a plan (`dc_gym_plan`,
`coax/lib/agent/desktop/gym.coax`, exported as `oa_dc_gym_plan`). The Swift Gym carries it out
(`GymWorkbenchView.applyPlan`): it opens the Gym, selects the run and waits for its scene, moves to
the step and waits for that, then sets the camera, layers, element and Live through the same
calls its buttons make. The answer is the Gym's state, `ple-gym-state.v1`: `ok`, `error`, `open`,
`run`, `title`, `runs`, `step`, `steps`, `layers` (shown), `selected`, `live`, `elements`,
`clashes`, `score`.

`ple-run gym` writes the request to `~/.openagents/ple/gym-request.json`. While Experimental
features are on, the app takes that file twice a second (and removes it) and writes its answer to
`gym-state.json` beside it with the request's `id`. `ple-run gym` waits up to 20 s
(`--wait SECONDS`) for its answer and says what the Gym shows. If the app does not answer, it
removes the request so nothing acts on it later. `POST /v1/gym` answers on the connection, and
`GET /v1/gym` gives the state (docs/coder/foreman-control-api.md).

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
coax/bin/coax test metaverse-engine/import/ple-ifc.coax --library          # the IFC reader: scanner, numbers, shapes, products
coax/bin/coax test coax/lib/agent/desktop/gym.coax --library               # a Gym request checked and worded as a plan
coax/web/tool.sh apps/openagents-desktop/ple-run.coax ifc MODEL.ifc --out /tmp/model.ple-scene.json
# a stand-in kit (no real eval): PLE_UV a script running "python3 eval/run.py ...", a stub run.py writing results.json and a model.ifc
HOME=/tmp/h PLE_UV=/tmp/stub/uv coax/web/tool.sh apps/openagents-desktop/ple-run.coax kit start --kit /tmp/stub --harness /tmp/stub/harness
coax/web/tool.sh apps/openagents-desktop/ple-run.coax feeders --out /tmp/ple-try   # a real run on the sample
```

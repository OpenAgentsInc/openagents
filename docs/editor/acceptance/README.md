# The editor's acceptance run

The run for E14 (GitHub #13423): one scripted pass through the browser editor
on Splice Bots. It repeats, and it stops with a failure at the first step that does
not hold.

## Run it

1. Start the dev server on your own computer. It must watch Splice Bots'
   sources and rebuild them:

   ```sh
   coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9538
   ```

   This one command also starts the local API behind the server (#13463):
   its own Postgres, you as its admin with no login step, and Gemini on a
   Google token that refreshes itself (the service account's gcloud config
   when the machine has it, else your gcloud login, else openagents.com on
   your `openagents` login; `docs/chat.md` has the details). So the agent in
   the editor's foot (`/worlds/splice-bots?edit=agent`) answers with a real model.
   `--api PORT` uses an API you started yourself instead.

   Started like this it builds the whole site first. In a worktree whose
   pages are already built (`coax/remote/pages.sh`), skip that with
   `--build true` after building Splice Bots and the site's shaders once in
   development mode, which is what tells the server which sources to watch:

   ```sh
   (cd apps/openagents-web && COAXIAL_MODE=dev ../../coax/web/wasm.sh \
     ../../metaverse/splice-bots/splice-bots.coax ../../metaverse/splice-bots/splice-face.c \
     build/splice-bots.wasm animelab-style.coax)
   COAXIAL_MODE=dev coax/web/tool.sh coax/web/wgsl-module.coax --site
   coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9538 --build true
   ```

   The first steps use the command line, so build it too:
   `apps/openagents-cli/build.sh`.

2. Start from an empty world: stop the server and start it again. The world
   is kept in the local API's Postgres, which each start makes new, so a
   second run on the same server finds the first run's pieces and fails.
   (With `--api PORT` the world is in that API's database instead. With
   `OA_SERVE_NO_API=1` it is in the files named `splice.*` in
   `apps/openagents-web/build/.worlds/`: delete them.) Run the pass once it
   says `engine rebuilt`: a save from the Code pane starts its module's
   rebuild at once, even while the first compiler check is still slow
   (#13464).

3. Run the pass:

   ```sh
   coax/web/tool.sh drive web apps/openagents-web /worlds/splice-bots \
     --query "edit&stage=5" --size 1280x800 --live http://127.0.0.1:9538 \
     --ops docs/editor/acceptance/editor-run.ops
   ```

The run exits 0 when every step holds: 98 operations, about a minute. It
leaves a picture in `apps/openagents-web/build/`: `editor-run.png`.

The scene stands in a frame: the canvas is the part of the window between the
strip, the site's side on the left and the editor's side on the right (and the agent's pane while it is open). Taps and
drags in the runs are the window's CSS pixels, as a mouse gives them; the page
hears them in the canvas's own pixels. `hit` asks the HUD at the point less the
canvas's corner and says where that corner is. The game starts with Enter and
holds the pointer to look round; `type - key Escape` lets go of it, as the
browser's own Escape does, so the panes take presses again.

The code steps save two of Splice Bots' sources (`metaverse/splice-bots/splice-game.coax`
and `gpu/splice-bots.coax`) and then save them back as they were. If the run stops
between the two saves, `git diff metaverse/splice-bots` shows the one changed
number; put it back with `git checkout -- metaverse/splice-bots`.

## WorldBuilder

A second pass runs the same shell on WorldBuilder (E13, #13422). It needs no
empty world first: the driver's browser keeps none.

```sh
coax/web/tool.sh drive web apps/openagents-web /create-world \
  --query edit --size 1280x800 --live http://127.0.0.1:9538 \
  --ops docs/editor/acceptance/worldbuilder-run.ops
```

It adds a cottage at a tapped point, picks it in the world, moves, turns and
sizes it with the gizmos, undoes and redoes, removes it and brings it back,
then raises a hill and paints sand with the land's brush. Each edit of a thing
must be one revision; a stroke must be one undo step. It leaves
`worldbuilder-run.png` in `apps/openagents-web/build/`. On the build this was
written against every edit showed in the frame it was made in: the page
checks its own world, so nothing waits on a server.

It does not check the chat line against the real model: the run scripts the
model's answers, below.

## Foes

A third pass sets the foes' plans (#13466). Start from an empty world, as for
the first pass, then:

```sh
coax/web/tool.sh drive web apps/openagents-web /worlds/splice-bots \
  --query "edit&stage=5" --size 1280x800 --live http://127.0.0.1:9538 \
  --ops docs/editor/acceptance/foes-run.ops
```

It reads the actors and their plans, sets the editor's time to dusk, sets two
robots to guard the tunnel mouth and a raider to patrol, picks an actor,
simulates, sets dawn, and undoes the eight edits. Each edit verb fails the run
unless the world takes it and the page stands at its revision. It leaves
`foes-dusk.png`, `foes-simulate.png` and `foes-dawn.png` in
`apps/openagents-web/build/`.

It runs on the agent clock: it waits with `clock until` for the page to
reach its world's first revision from the dev server. Start it after the
server's log says `engine rebuilt` when a source changed: while the server
rebuilds, the page gets no world and every edit verb fails.

## Splice Bots' front door

A pass through the title, the creation screen and the start of play
(#13482), as a player opens the page. Use a fresh browser profile (the
driver's own), so no character or run is kept:

```sh
coax/web/tool.sh drive web apps/openagents-web /worlds/splice-bots \
  --size 1280x720 --live http://127.0.0.1:9538 \
  --real-time --ops docs/editor/acceptance/splice-front-run.ops
```

It opens CREDITS with the pointer and goes back with Escape, starts NEW
RUN with the keys, changes the body with D and the outfit with the
pointer, goes back with Escape and in again, and plays with USE THIS
CHARACTER. `state` shows each step's mode (0 the title, 7 the
character, 8 the credits, 1 play), the character's changes and its walk
poses. It leaves `splice-front-title.png`, `splice-front-credits.png`,
`splice-front-character.png`, `splice-front-play.png` and
`splice-front-walk.png` in `apps/openagents-web/build/`.

## Foes in the agent pane

A fourth pass sends the same request through the editor's agent pane
(#13466, #13465), with Claude Code chosen in the agent menu on Haiku 5.5:

```sh
coax/web/tool.sh drive web apps/openagents-web /worlds/splice-bots \
  --query "edit=agent" --size 1600x900 --live http://127.0.0.1:9538 \
  --real-time --ops docs/editor/acceptance/foes-pane-run.ops
```

Each `/local/` request the page makes is answered from
`docs/editor/acceptance/foes-pane/`, a trimmed copy of a real Haiku turn, so
no model or Coder runs. The agent's reply shows in the pane. The drive's edit
verbs stand in for the agent's realm tools, and each one fails the run unless
the open editor reaches its revision. The pass then undoes its seven edits.
It leaves `foes-pane-turn.png` and `foes-pane-dusk.png` in
`apps/openagents-web/build/`.

For the real turn, start the server with `coax/web/serve.sh`, open the same
page and send the prompt. Coder gives Claude Code's realm tools this
server's origin (`CODER_REALM_ORIGIN`), so they edit the world the editor
shows.

## Scripts

A pass through the Scripts pane (#13467). The local API compiles a script only
when the server is started with the compile job named, so start the server
like this, on a port of its own, from an empty world:

```sh
OA_SCRIPT_JOB=$PWD/coax/web/script-job.sh \
  coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9644
coax/web/tool.sh drive web apps/openagents-web /worlds/splice \
  --query "edit&stage=5" --size 1280x800 --live http://127.0.0.1:9644 \
  --ops docs/editor/acceptance/scripts-run.ops
```

It opens the pane, builds the starter script for the raiders, runs it (one
revision), and waits until the script's calls rise with none failed. Then it
changes one number in the source, builds and runs again with no reload (one
more revision, the same row), checks that the calls keep rising, and stops the
script. It leaves `scripts-run.png` and `scripts-swap.png` in
`apps/openagents-web/build/`.

## What it checks

| Step | What must hold |
| --- | --- |
| Place three pieces | Each is one revision, and the page shows all three. |
| Tap a piece in the world | The piece under the tap is selected (`vp_pick`, which is `query_ray`). |
| Drag with the Move tool | One snapped move, one revision. |
| Drag with the Turn tool | One turn, one revision. |
| Edit a content row | The kind's name changes in place, with no reload. |
| Remove | The piece is gone, one revision. |
| Undo, then Redo | The piece comes back, then goes again. |
| Change a line of game code | In the Code pane, one constant of `splice-game.coax` is changed and saved. The engine swaps, and again when it is put back. The run waits up to 30 s for each, since a rebuild takes 10 s or more on a busy machine; the step says how long it took, and the bar on an idle machine is 2 s. |
| Change a shader | The same in `gpu/splice-bots.coax`, the Coax that writes Splice Bots' WGSL. The shaders swap, twice. |
| The world after four swaps | The same revision and the same pieces: the run kept its state. The agent's turns that follow show at once (#13457). |
| Place through the agent | In the conversation in the editor's foot, a turn places two crates: one batch, a revision for each. |
| Move and remove through the agent | One turn, one batch. |
| Undo through the agent | One step takes the whole turn back. |
| Talk to the agent | Nothing changes. |
| Switch to an agent on this machine | Coder's answers are scripted (`local-agents.json`: Claude Code ready). The Agent menu lists Claude Code beside us, the conversation switches to it, and back to Gemini (#13465). |
| Where the player stands | The run kept its state: the player has not moved. |
| What a tap reaches | The shell's button in the strip, and the world in the frame. |

The swaps are timed in real time. On the build this was written against, on
a Mac that was busy with other builds, the engine swapped 950 to 1,040 ms
after Save and the shaders 180 to 550 ms after Save, in two runs.

Each other wait is at most 6,000 ms of the agent's clock. Against the dev
server a step of a wait also takes 25 ms of real time, so the clock does not
run away from the world's routes, which answer in real time. On the build
this was written against, each world edit showed after 300 to 400 ms of the
agent's clock.

## What it does not check

- **The model.** The run doesn't depend on a model, so the agent's four
  answers are scripted, though the dev server has Gemini behind it: `answer /api/chat/complete FILE` gives the page the bytes of
  `agent-place.json`, `agent-change.json`, `agent-undo.json` and
  `agent-talk.json` in place of the model's. Everything after the answer is
  the real path: the chat reads it under the world tools' schema, the shell
  makes the plan one batch, the page sends it and the world's rules answer.
  To try a real turn, open `/worlds/splice-bots?edit=agent` on the same server and
  send a message.
- **An agent on your own computer.** The run does not start Claude Code or
  Codex: Coder's answers (`/local/sessions`, `/local/claude` and
  `/local/agents`) are scripted, so the run checks the menu and the switch,
  not a turn. Their edits are the realm commands the first steps use
  (`place`), which is the path their `realm_world` tools take.
- **Real Chrome.** This run is headless Chrome. The pass with a hand on a
  mouse is the owner's step in `NEEDS_OWNER.md`.
- **A phone.** The runs are at desktop sizes only.

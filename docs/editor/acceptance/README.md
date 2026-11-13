# The editor's acceptance run

The run for E14 (GitHub #13423): one scripted pass through the browser editor
on Splice. It repeats, and it stops with a failure at the first step that does
not hold.

## Run it

1. Start the dev server on your own computer. It must watch Splice's
   sources and rebuild them:

   ```sh
   coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9538
   ```

   Started like this it builds the whole site first. In a worktree whose
   pages are already built (`coax/remote/pages.sh`), skip that with
   `--build true` after building Splice and the site's shaders once in
   development mode, which is what tells the server which sources to watch:

   ```sh
   (cd apps/openagents-web && COAXIAL_MODE=dev ../../coax/web/wasm.sh \
     ../../metaverse/splice/splice.coax ../../metaverse/splice/splice-face.c \
     build/splice.wasm animelab-style.coax)
   COAXIAL_MODE=dev coax/web/tool.sh coax/web/wgsl-module.coax --site
   coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9538 --build true
   ```

   The first steps use the command line, so build it too:
   `apps/openagents-cli/build.sh`.

2. Start from an empty world. Stop the server, delete the files named
   `splice.*` in `apps/openagents-web/build/.worlds/`, and start it again.

3. Stop the shared headless browser if `shot` was used in this checkout
   (`coax/web/tool.sh shot --stop`): a page it left open takes the run's taps
   (#13458).

4. Run the pass:

   ```sh
   coax/web/tool.sh drive web apps/openagents-web /worlds/splice \
     --query "edit&stage=5" --size 1280x800 --live http://127.0.0.1:9538 \
     --ops docs/editor/acceptance/editor-run.ops
   ```

The run exits 0 when every step holds: 86 operations, about a minute. It
leaves two pictures in `apps/openagents-web/build/`: `editor-run.png` and
`editor-run-phone.png`.

The code steps save two of Splice's sources (`metaverse/splice/splice-game.coax`
and `gpu/splice.coax`) and then save them back as they were. If the run stops
between the two saves, `git diff metaverse/splice` shows the one changed
number; put it back with `git checkout -- metaverse/splice`.

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

It does not check the chat line (the dev server has no model behind it) or a
phone's size: the driver's change of size mid-run garbles the page's own text,
so take that picture from a fresh load at 390x844.

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
| Place through the agent | In the conversation in the editor's foot, a turn places two crates: one batch, a revision for each. |
| Move and remove through the agent | One turn, one batch. |
| Undo through the agent | One step takes the whole turn back. |
| Talk to the agent | Nothing changes. |
| Change a line of game code | In the Code pane, one constant of `splice-game.coax` is changed and saved. The engine swaps within 4 s (the step says how long it took; the bar is 2 s), and again when it is put back. |
| Change a shader | The same in `gpu/splice.coax`, the Coax that writes Splice's WGSL. The shaders swap, twice. |
| The world after four swaps | The same revision and the same pieces: the run kept its state. |
| Where the player stands | The run kept its state: the player has not moved. |
| What a tap reaches | The shell's button over the canvas, and the world where no pane is. |
| A phone's size | The strip wraps and the panes stand side by side over the world. |

The swaps are timed in real time. On the build this was written against, on
a Mac that was busy with other builds, the engine swapped 950 to 1,040 ms
after Save and the shaders 180 to 550 ms after Save, in two runs.

Each other wait is at most 6,000 ms of the agent's clock. The agent's clock runs
ahead of the wall clock while it waits for an answer, so a step that passes
took less than that in real time. On the build this was written against, each
edit showed after 2,300 to 2,800 ms of the agent's clock.

## What it does not check

- **The model.** The dev server has no model, so the agent's four answers are
  scripted: `answer /api/chat/complete FILE` gives the page the bytes of
  `agent-place.json`, `agent-change.json`, `agent-undo.json` and
  `agent-talk.json` in place of the model's. Everything after the answer is
  the real path: the chat reads it under the world tools' schema, the shell
  makes the plan one batch, the page sends it and the world's rules answer.
  A turn against the real model is for staging, after a deploy.
- **An agent on your own computer.** The run does not start Claude Code or
  Codex. Their edits are the realm commands the first steps use (`place`),
  which is the path their `realm_world` tools take.
- **A world edit straight after a code swap.** The run does its world edits
  first and its code edits last. After an engine swap the page can take up
  to 10 seconds to follow its world again: a request that was in flight at
  the swap gets no answer, and the follower gives it up only then
  (#13457).
- **Real Chrome and a real phone.** This run is headless Chrome. The pass with
  a hand on a mouse and a finger on a phone is the owner's step in
  `NEEDS_OWNER.md`.
- **Tap sizes under a mouse.** Under a finger every control of the kit is at
  least 44 px either way (`@media (pointer: coarse)` in the mono skin,
  #13448); with a mouse the kit keeps its desktop sizes. Check a phone with:

  ```sh
  coax/web/tool.sh drive web apps/openagents-web /worlds/splice \
    --query "edit&stage=5" --size 390x844 --live http://127.0.0.1:9538 \
    "prefer pointer coarse" "clock +1500" taps
  ```

  It answers `0 under 44 px`.

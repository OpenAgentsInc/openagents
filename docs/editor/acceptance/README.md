# The editor's acceptance run

The run for E14 (GitHub #13423): one scripted pass through the browser editor
on Splice. It repeats, and it stops with a failure at the first step that does
not hold.

## Run it

1. Start the dev server on your own computer:

   ```sh
   coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9538
   ```

2. Start from an empty world. Stop the server, delete the files named
   `splice.*` in `apps/openagents-web/build/.worlds/`, and start it again.

3. Run the pass:

   ```sh
   coax/web/tool.sh drive web apps/openagents-web /worlds/splice \
     --query "edit&stage=5" --size 1280x800 --live http://127.0.0.1:9538 \
     --ops docs/editor/acceptance/editor-run.ops
   ```

The run exits 0 when every step holds. It leaves two pictures in
`apps/openagents-web/build/`: `editor-run.png` and `editor-run-phone.png`.

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
| Where the player stands | The run kept its state: the player has not moved. |
| What a tap reaches | The shell's button over the canvas, and the world where no pane is. |
| A phone's size | The strip wraps and the panes stand side by side over the world. |

Each wait is at most 6,000 ms of the agent's clock. The agent's clock runs
ahead of the wall clock while it waits for an answer, so a step that passes
took less than that in real time. On the build this was written against, each
edit showed after 2,300 to 2,800 ms of the agent's clock.

## What it does not check

- **A shader and a line of game code.** The code panel is not built
  (#13421), and this run uses a dev server that does not rebuild.
- **The same run from `/chat` through an agent.** The agent strip holds the
  conversation (#13418), and the turn does not carry the selection or the
  world tools yet.
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

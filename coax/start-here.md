# Start here

This is the one page you read before you build a Coaxial page or a realm in
Coax. Everything else is lookup: `coax vocab NAME` for a name, `coax explain
CODE` for a diagnostic, [LIBRARIES.md](LIBRARIES.md) for shared code, and the
[book](book/README.md) and [spec](spec.md) when you need the whole story.

## The workflow

1. **Make a worktree from `main`.** Never edit the main checkout.

   ```sh
   cd /Users/christopherdavid/work/openagents && git fetch origin
   git worktree add ../oa-TOPIC origin/main && cd ../oa-TOPIC
   ```

2. **Get the compiler.** `coax/build.sh` installs the published build for
   your tree in seconds. If it says the build is "missing", see pitfall 2.
3. **Write the checks first.** `#test` and `#forall` blocks go next to the
   code they check. A file that decides money, admission or policy is
   `#authority;`.
4. **Build and test on a remote host,** not the Mac. Run every heavy command
   from the worktree root through `coax/remote/remote.sh run --`:

   ```sh
   coax/remote/remote.sh run -- coax/bin/coax test apps/openagents-web/everglade.coax --only "the bar"
   coax/remote/remote.sh run -- coax/web/wasm.sh ...      # a page or realm module
   coax/remote/remote.sh status                           # hosts and free slots
   ```

   A small tool file (`coax/bin/coax test coax/web/vocab.coax`) is fine on
   the Mac. See [remote builds](../docs/metaverse/remote-builds.md).
5. **Get the built pages, then serve.** `coax/remote/pages.sh origin/main`
   puts main's built pages (every module and render package) into your
   worktree in about 10 seconds; a fresh worktree has none, and a page without
   them never draws a frame. Then rebuild only the module you changed with
   `wasm.sh`. Don't run a full `apps/openagents-web/build.sh` for this.
   `coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9123`
   serves the site with hot reload. Pick a port above 9000 that no one else
   uses.
6. **Test the page the way a player uses it.** Drive it (next section), and
   take screenshots in headless Chrome:
   `coax/web/tool.sh shot URL OUT.png [WxH] [WAIT_MS] [--eval JS] [--keep]`.
   Pass `--keep` on every shot of a build after the first: the page stays
   loaded in the checkout's resident browser, and a shot takes about a second
   ([fast shots](../docs/metaverse/fast-shots.md)). Never open a visible
   browser; set `BROWSER=/usr/bin/true` for anything that might.
7. **Mutate what decides.** `coax/remote/remote.sh run -- coax/bin/coax
   mutate FILE` on every `#authority` file you changed, with zero survivors.
8. **Push.** Run the tests for the units you touched and
   `coax/web/tool.sh tools-check`, then `git fetch origin && git rebase
   origin/main` and `git push origin HEAD:main`. Remove the worktree.

New tools are Coax too ([tools-in-coax.md](tools-in-coax.md)): write
`coax/web/NAME.coax` with `#test` checks and run it with
`coax/web/tool.sh NAME`.

## A realm's reference for agents

Each realm with a row in the authority table has a generated reference,
`apps/openagents-web/realms/PAGE/llms.txt`, served at `/worlds/PAGE/llms.txt`:
its wire version and pins, content tables, the shapes its contexts own, who
decides each kind of state, its limits, the driver's operations and the
diagnostic codes. Read it before you write against a realm; it describes the
build it is served beside, not `main`.

It is generated, never edited. When you change a pin, a limit it lists, a
content table, a registry row or a diagnostic code, run
`coax/web/tool.sh agent-reference --write` and commit the files. The pre-push
check runs `--check` and refuses a stale one. To add a realm, add its
`ar_realm` entry in `coax/web/agent-reference.coax` and its row in
`metaverse-engine/editor/authority.coax`.

## Drive a page or realm

`coax/web/tool.sh drive` runs a page in headless Chrome and applies one
operation per shell argument, in order. It answers from the engine, not
from the pixels, and names targets by stable ids, never by coordinates.

```sh
coax/web/tool.sh drive web apps/openagents-web /worlds/everglade-ii \
  "tree" "walk-to guard" "use attack on goblin-1" "clock settle" \
  "state you_x" "screenshot out.png"
```

The arguments are the host (`web`), the app directory, the page path, and
then the operations.

A page the browser draws (the chat at `/chat`, `/components`, the editor's
shell: the frames set through `coax/web/dom-host.js`) has no world. There the
driver reads the page's elements, by their ids, and takes `tree`, `hit`,
`taps`, `tap`, `type`, `screenshot`, `logs` and `prefer`.

| Operation | What it does |
| --- | --- |
| `tree` | Lists what is on screen and in the world, with stable ids, roles, labels and rectangles: HUD widgets, the player, monsters, characters, mounts and placed objects (`goblin-3`, `guard`, `horse-white`, `door-north`) |
| `screenshot FILE` | Writes a PNG of the page |
| `hit X Y` | Names what receives a tap at a point (CSS px), what it is drawn over, and any tap target there that it covers: an overlay that steals a click shows as `covered:`. A page's elements answer by the browser's own hit test; where a canvas is on top, the HUD answers |
| `taps` | Audits every tap target: `small` (under 44 px either way), `covered` (a tap at its middle lands on something else) and `overlap`. It works on a realm's HUD and on pages the browser draws (the chat, the editor's shell) |
| `tap ID` | Taps the centre of the node with that id |
| `type ID TEXT` | Types text into a field |
| `why ID` | Says why a thing isn't seen, reached or working: the first failed link of state, spawned, view, drawn, asset, collider and system, with the field, the expected and actual values, and whether a retry can help. `why caps` lists the limits the engine applied. The codes are in `proto/verse/engine-diagnostics-v1.json`. Splice Bots answers it first (`you`, `piece-N`, `foe-N`, `rubble`, `boss-wall`) |
| `state PATH` | Reads a value from the engine's state, such as `you_x` or `player.zone` |
| `layout ID` | Prints a node's rectangle and its children's |
| `logs` | Prints the page's console and the engine's log lines |
| `clock ...` | Steps simulation time (see below) |
| `prefer NAME VALUE` | Sets a preference such as the color scheme or text size |
| `perf` | Prints frame times |
| `walk-to ID` | Walks your character to an entity |
| `face ID` | Turns you toward an entity |
| `use ABILITY on ID` | Uses an ability on an entity, through the same deed a player's input makes |
| `mount ID` | Mounts a mount |
| `tap ID=WORDS` | Clicks the row under `ID` that says exactly `WORDS` (a list's row, whatever its place) |
| `swap ID OLD => NEW` | In the field `ID`, the first `OLD` becomes `NEW`, as a person's edit (a line in the editor's Code pane) |
| `answer PATH FILE` | Answers the page's own requests to `PATH` with `FILE`'s bytes, for a route the dev server doesn't have (a model's) |
| `swaps`, `swaps until KIND >= N within MS` | Prints what the tab swapped in live (`wasm`, `wgsl`, `asset`), or waits in real time for the Nth and says how long it took |
| `prefer pointer coarse` | Turns touch on, so `taps` measures the page a phone gets |

The driver opens the page in agent mode ([the agent clock](../docs/metaverse/agent-clock.md)):
nothing moves between two operations, and the page and the offline room
step together. `clock +MS` steps the clock, `clock settle` steps until
nothing is scheduled, and `clock until PATH OP VALUE within MS` steps until
a state field compares so. Every session has the same seed, date, locale
and time zone. Drive scripts land with phase 3: a file of
these operations with expectations, which is also a replay stream, so a
playtest recording can become a test. Until they land, read the header of
`coax/web/drive.coax` for what works today. `--ops FILE` runs a file of
operations, one a line, at most 128
([the editor's acceptance run](../docs/editor/acceptance/README.md) is one).
With `--live`, the page also takes the dev server's live swaps, so a run can
save a source and wait for the engine it builds.

A page that `shot` left in the checkout's shared browser does not take
drive's taps: `coax/web/tool.sh drive-hit-check` taps a page after a kept
shot (#13458). If taps still stop reaching a page, run
`coax/web/tool.sh shot --stop`, try again, and open an issue with the steps.

## Look things up

- `coax vocab NAME...` (or `coax/web/tool.sh vocab NAME...`) prints what a
  name is: a library procedure, struct or constant with its file, comment
  and signature; a keyword or built-in type; an error code; or a spelling
  from another language and the Coax spelling. For an unknown name it
  suggests the nearest ones.
- `coax explain CODE` explains a diagnostic. When you write a spelling from
  C, Rust, Go, JavaScript or Python (`let`, `x++`, `(int) x`, `printf`,
  `i32`, `fn`), the compiler reports E0225 and names the Coax spelling.
- [DEBUGGING.md](DEBUGGING.md) holds the pitfalls that no diagnostic
  catches yet.

## The five costliest pitfalls

Each is symptom, cause and fix.

1. **Builds wait for 30 minutes and the Mac stalls.** Heavy work (Coax to C
   for a unit with many checks, `wasm.sh`, `coax test` of a big unit,
   mutation) ran on the Mac's two build slots while other agents did the
   same; on 2026-11-09 the load reached 110, and too many builds at once
   once froze it (out of file handles). Run heavy commands through
   `coax/remote/remote.sh run --`, and keep at most two heavy builds or
   browser loops on the Mac.
2. **`coax/build.sh` says the build is "missing" and you wait on a
   bootstrap.** `main`'s compiler changed after the last publish (the
   release hosts catch up in about an hour and a half), or a surviving
   mutant stopped the publish. To use the compiler meanwhile, run
   `COAX_MUTATION=0 coax/build.sh` in the background and work on something
   else; a remote job builds its own compiler.
3. **You debug something that isn't the code you wrote.** The tool, page
   or compiler is older than its sources: `coax/bin/coax` was built from
   other compiler sources ("this compiler is out of date"), a page module
   was not rebuilt, or the worktree started from a broken `main`. Rebuild
   what the message names, rebuild the module, and start worktrees from
   `origin/main` (from `main-green` once the green-main pipeline lands,
   [green-main-pipeline.md](../docs/metaverse/green-main-pipeline.md)). The
   driver refuses a build older than its sources.
4. **A check passes but the feature fails for a player.** The check covered
   one piece, or a headless run that differs from the real flow (an iframe
   zone hand-off passed headless and failed live). Test the whole flow the
   player takes, on the exact build you hand over, and say which parts a
   headless run did not cover.
5. **Seven rounds on one bug.** The same approach was repeated with small
   changes: on 2026-11-09 the bow pose took seven diagnostic rounds before
   the approach changed. After three failed rounds on one problem, change
   approach or ask a person.

## Rules that save rounds

- `_ = f();` drops a result you don't need; write it as often as you like.
- A file with no `main` is a library: `coax test`, `check`, `mutate` and `c`
  need no `--library`.
- `print` needs no `#import "Basic";`.
- A local or parameter may share a procedure's name (`lo`, `hi`, `near`).
  Procedures are not values: to pick one at run time, pass an enum and
  branch on it.
- To read a constant or a parameter array through a view, type the receiver
  `[] const T`. A temporary goes into a local first.
- f32 and f64 don't mix: apply the cast the fix line names.

- Build features engine-wide in `metaverse-engine/` or game-wide in the game's shared
  code, never for one zone.
- Kill only the processes you started, by PID. Never `pkill` by name.
- Dev servers use ports above 9000.
- Commit only your files, and rebase before you push; when another change
  touched the same version number or error codes, keep both.

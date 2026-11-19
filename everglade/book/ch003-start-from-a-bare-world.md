# 3. Start from a bare world

> **Status: partly implemented.** The `gamebuilder` CLI, its `world.edit.v1`
> batch wire, `--attach` UI bridge and `play-test` runner exist today in
> GameBuilder (pinned to `OpenAgentsInc/openagents`). Everglade's custom Coax
> modules live in `everglade/` and `apps/openagents-web/`. This chapter walks
> through the GameBuilder workflow from a bare world and shows where Everglade
> drops down into Coax and the Metaverse Engine.

You don't start Everglade by hand-wiring a web page and a shader pipeline. You
start from a bare world in **GameBuilder**, driven programmatically from the
command line, dipping into the desktop UI whenever you want to look around or
tweak something by eye. This chapter answers the builder's question: "How do I
create a bare world, build it up with commands and batches, see it in the app,
and know when to drop down to engine code?"

## What this chapter teaches

- How to create a bare world and inspect its things, shape table and palette
  with the `gamebuilder` CLI.
- How to build up a level programmatically with `add`, `set-field`, `delete`
  and atomic `world.edit.v1` batches (`apply-edits --dry-run`).
- How to dip into the GameBuilder desktop UI with `--attach`, `open-level`,
  `show-changes` and one-step undo (`⌘Z`).
- How to verify a level headless with `play-test`, and where GameBuilder ends
  and Coax / the OpenAgents Metaverse Engine begins.

## The CLI at a glance

Every `gamebuilder` (or `gb`) command answers JSON (`{"ok":true,...}` or
`{"ok":false,"error":CODE,"why":WORDS}`). Exit code `0` means done (or a clean
dry run), `1` means refused with the exact limit that refused it, and `3` means
no such command.

| Group | Key verbs | What it does |
| --- | --- | --- |
| Discovery | `version`, `whoami`, `commands`, `list-templates` | Check the install, account, full JSON command table and starter game types |
| Games & levels | `new-game`, `list-games`, `new-level`, `list-levels`, `copy-level`, `rename-game`, `rename-level` | Create and organize games and levels |
| Reading | `read-level`, `read-state`, `read-content`, `list-palette`, `list-fields` | Inspect things, shape rows, revision (`rev`), allowed kinds and field ranges |
| Editing | `add`, `set-field`, `delete`, `apply-edits` | Change one thing or land a `world.edit.v1` batch atomically |
| UI bridge (`--attach`) | `open-level`, `changes`, `show-changes`, `dismiss-changes`, `undo`, `redo`, `focus` | Drive the open GameBuilder desktop app and highlight external edits |
| Testing & publishing | `play-test`, `publish`, `published`, `versions`, `set-live` | Run headless bot walk-throughs and publish playable web links |
| Engine boundary | `engine`, `engine-list`, `engine-read`, `triage`, `engine-ready`, `engine-mine` | Inspect the pinned Metaverse Engine commit and propose engine PRs |

Run `gamebuilder commands` whenever you want the complete machine-readable list
of verbs, arguments, flags and limits.

## Step 1: Start a bare world

Check that the CLI is installed, then list the starter templates:

```sh
gamebuilder version
gamebuilder list-templates
```

GameBuilder ships templates such as `third-person`, `adventure`, `village`,
`field`, `platformer`, `crates` and `arena`. For an open third-person world
like Everglade, start from `adventure` (a place to walk around, seen from
behind) or `third-person`:

```sh
gamebuilder new-game adventure
gamebuilder list-games
```

`new-game` creates a game and its first level—a starter place with a few
things already in it. Read the level and its palette before you change
anything:

```sh
gamebuilder read-level 1
gamebuilder list-palette 1
gamebuilder list-fields
```

`read-level` returns:

- `rev`: the revision you read. Every `world.edit.v1` batch must name this
  exact revision; if anything changed the level in between, the batch is
  refused as `stale`.
- `things`: every thing in the level—its `id`, `kind` (row in the shape
  table), `is` (kind name), position (`x`, `y`, `z` in centimetres, with `y`
  up and `z` north), `yaw` (whole degrees, `-180` to `180`) and `scale`
  (percent, `10` to `1000`).
- `content`: the level's shape table (`ID|KIND|WIDTH|HEIGHT|DEPTH` in cm).
- `locked`: `true` if the level isn't yours to change (use `copy-level` to
  make your own copy).

## Step 2: Build programmatically with `world.edit.v1`

For a single quick tweak, `add`, `set-field` and `delete` are enough:

```sh
gamebuilder add 1 tree --x 600 --z -400
gamebuilder set-field 1 12 yaw 45
gamebuilder delete 1 7
```

For real layout work—clearing starter props, framing a glade, placing a ring
of trees and a path marker—send one atomic `world.edit.v1` batch. Either the
whole batch lands as a single undoable step, or none of it does.

### A worked example: frame the first glade

Suppose `read-level 1` returned `rev: 3`, a starter prop at `id: 4`, and a
shape table where row `2` is a tree and row `5` is a rock. Write
`glade-batch.json`:

```json
{"schema":"world.edit.v1","rev":3,"do":"apply","edits":[
  {"op":"despawn","id":4},
  {"op":"spawn","id":0,"kind":2,"x":-600,"y":0,"z":-500,"yaw":15,"scale":110},
  {"op":"spawn","id":0,"kind":2,"x":650,"y":0,"z":-450,"yaw":-30,"scale":100},
  {"op":"spawn","id":0,"kind":2,"x":-550,"y":0,"z":600,"yaw":90,"scale":120},
  {"op":"spawn","id":0,"kind":5,"x":400,"y":0,"z":350,"yaw":20,"scale":95}
]}
```

Always dry-run first, then apply, then read the level back:

```sh
gamebuilder apply-edits 1 glade-batch.json --dry-run
gamebuilder apply-edits 1 glade-batch.json
gamebuilder read-level 1
```

Key rules of the wire:

- **Exact field order.** Each edit keeps its fields in the order shown above.
- **Bounded batches.** At most 64 edits per batch, coordinates within
  `±20000` cm on `x` and `z` (`0` to `20000` cm on `y`), and at most 64 shape
  rows per level.
- **Idempotent retries.** Pass `--key K` when a script might retry a call; the
  same key replays the first result without applying the edit twice.
- **New sizes.** Add a new shape row in the same batch before spawning it:
  `{"op":"cell","table":"shape","row":N,"col":"*","v":"N|block|400|50|200"}`.

## Step 3: Dip into the UI

Programmatic building doesn't mean working blind. When you want to orbit the
glade, check sightlines, or nudge a boulder by hand, open the level in the
GameBuilder desktop app:

```sh
gamebuilder --attach open-level 1
```

With `--attach`, CLI commands talk directly to the running app instead of
writing files underneath it:

```sh
gamebuilder --attach apply-edits 1 glade-batch.json
gamebuilder --attach changes 1
gamebuilder --attach show-changes 1
```

- `show-changes` swings the viewport camera to frame every thing your batch
  added, moved, changed or removed.
- `⌘Z` in the app (or `gamebuilder --attach undo`) takes back the entire batch
  in one step.
- `dismiss-changes` clears the highlights once you've checked them.

If you edit the level's files on disk without `--attach`, close the level in
the app first so the app's next save doesn't overwrite your CLI changes.

## Step 4: Test headless and publish

Before you call a layout done, run the headless play-tester:

```sh
gamebuilder play-test 1
```

For an `adventure` or `third-person` level, `play-test` runs a deterministic
bot walk-through from the player start to every reachable thing and reports
where the bot got stuck and which things it couldn't reach. The same call
answers the same every time.

When you want someone else to walk around in it, `gamebuilder publish 1 "Everglade Glade"`
publishes a playable web link that stays stable across versions.

## Step 5: Where GameBuilder ends and the engine begins

As Everglade grows from a bare glade into an MMORPG, you will hit a clear
three-way boundary:

1. **The level (GameBuilder CLI).** What is in the world, where it stands, how
   big it is, and how it is laid out. Solved with `add`, `set-field`,
   `apply-edits` and `play-test`.
2. **The game (GameBuilder app).** Built-in templates, palette kinds and
   editor tools.
3. **The engine (Coax + OpenAgents Metaverse Engine).** Terrain tiles,
   procedural forest planting, custom lighting profiles, rock hulls, room
   networking and SRD combat rules.

Before writing engine code, check what the pinned engine already has:

```sh
gamebuilder engine
gamebuilder engine-list
gamebuilder engine-read 4
```

When Everglade needs a new engine primitive—something any world can reuse—run
`gamebuilder engine-ready` to check your fork and tools, build the primitive
with `#test` checks in `OpenAgentsInc/openagents`, and open a pull request.

Part II of this book is where Everglade drops down to Coax and the Metaverse
Engine to build the glade's ground tiles (`everglade/world/ground.coax`),
procedural forest (`everglade/world/forest.coax`, `plant.coax`), lighting
(`everglade/world/environment.coax`) and exact colliders
(`everglade/shared/move.coax`), checking each with `coax/bin/coax test`:

```sh
coax/build.sh
coax/bin/coax test --library everglade/world/forest.coax
coax/bin/coax test --library everglade/world/plant.coax
coax/bin/coax test --library everglade/shared/move.coax
```

## Mistakes

- **Editing on disk while the level is open in the app.** Without `--attach`,
  the CLI writes to the level file on disk. If the app has that level open,
  its next save overwrites your batch. Either pass `--attach` or close the
  level in the app first.
- **Skipping `--dry-run`.** A typo in a field order or a coordinate past
  `20000` cm refuses the batch. Running `--dry-run` first validates the whole
  plan with zero side effects.
- **Ignoring a `stale` revision.** If you tweaked something in the UI and then
  re-ran an old batch file, `apply-edits` refuses it as `stale`. Run
  `read-level` again, inspect the new `rev`, and re-plan.
- **Fighting the palette instead of checking the engine.** If you find
  yourself stacking 50 blocks to fake terrain or lighting, check
  `gamebuilder engine-list`—you have crossed from level layout into engine
  primitives.

## What comes next

Part II builds the glade below the level layer. [Chapter 4](ch004-ground-you-can-stand-on.md)
starts with the ground.

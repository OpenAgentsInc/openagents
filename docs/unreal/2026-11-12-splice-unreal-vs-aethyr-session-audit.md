# Splice in Unreal and in Aethyr: a two-session audit

Audit, 2026-11-12. Status: analysis and recommendations. Nothing here changes
code.

Two Claude Code sessions built the same game, Splice, from the same requests
and the same design document. One built it in Unreal Engine (UE) 5.9. The
other built it in Aethyr. The Aethyr result looks better, covers more of the
design, and reached a public URL. The Unreal result took twice the effort and
is still on one Mac.

This audit reads both transcripts in full and says why.

| | Unreal session | Aethyr session |
|---|---|---|
| Session id | `84203750-fec1-485a-8aa1-4f096479c868` | `eaaf10a3-d37c-4358-968a-ee81a74d22a9` |
| Working directory | `~/games/cakbag` (Diversion repo, project `Rovers/`) | `~/work/openagents` (worktree `openagents-splice`) |
| Output | A packaged Wasm build on `127.0.0.1:8900` | `openagents.com/worlds/splice`, unlisted, in production |

## Short answer

The model is not the reason. Both sessions ran the same three models, and
both began on Haiku 5.5 with nearly the same one-line request for a robot on
Mars in a Moebius style. On Haiku, the Aethyr session pushed a correct scene
in 9 minutes. On Haiku, the Unreal session spent 65 minutes without one
correct frame.

Five things explain the difference, in order of weight:

1. **The agent could see its own work in Aethyr from minute 7.** In Unreal it
   could not see anything for 63 minutes, so the owner did the looking.
2. **Aethyr fails loudly. Unreal failed silently, four times.** A shader that
   does not compile, a setting written to a copy, a screenshot that never
   lands on disk: each one reported success.
3. **One change costs about 8 seconds in Aethyr.** In Unreal it cost 5 minutes
   for an editor restart, then 10 to 11 minutes for each web build.
4. **The art style already existed in Aethyr.** The repo rule "never
   hand-roll" sent the agent to Anime Lab's cel shader and ink pass. In
   Unreal the agent wrote a post-process shader from nothing, against an
   engine whose defaults (physical lighting, auto exposure, Lumen) all work
   against a flat comic look.
5. **The Aethyr session started from pictures. The Unreal session started
   from a gray-box checklist** and added a look afterwards, twice.

The sections below give the evidence for each, then what to change.

## What was compared

Sources:

- Both transcripts under `~/.claude/projects/`: 5,644 lines for Unreal and
  2,662 for Aethyr.
- The Unreal session's own gap list, `docs/design/aethyr-gap-analysis.md` in
  the `cakbag` repo. That file compares the two builds feature by feature.
  This audit compares the two sessions and does not repeat that list.
- Screenshots from both builds: the Unreal web build
  (`docs/devlog/media/0006/` in `cakbag`) and the Aethyr build (contact sheets
  from the Aethyr session's scratch folder).
- The code each session wrote.

Times below are minutes from the request, not clock times.

## The numbers

| | Unreal | Aethyr |
|---|---|---|
| Wall-clock span | 20.7 hours | 19.7 hours |
| Active time (gaps under 10 minutes) | 4.0 hours | 2.0 hours |
| Tool calls | 579 | 275 |
| Failed tool calls | 18 | 8 |
| Output tokens | 2.12 million | 1.03 million |
| Context compactions | 3 | 2 |
| Models, by assistant turns | Opus 5.5 547, Fable 5.2 359, Haiku 5.5 325 | Fable 5.2 241, Haiku 5.5 203, Opus 5.5 103 |
| Build commands | about 38 editor launches in the first 80 minutes, then about 17 web-build runs | about 50 compiles |
| Self-captured checks | 18 desktop screen grabs in the first 80 minutes | 23 headless screenshot runs |
| Owner complaints that nothing visible worked | 6 in the first 65 minutes | 0 |
| Code written | 3,812 lines of Python generators, plus Blueprint text files, producing binary assets | 2,684 lines of Coax and C, of which 753 are reusable engine kits |
| Shipped size | 299 MB output folder (Development build) | 177 KB of Wasm for the world |
| Where it runs | One Mac, localhost | Production, released twice |

### First picture

| Minute | Unreal (Haiku 5.5) | Aethyr (Haiku 5.5) |
|---|---|---|
| 0 | Request: a robot on Mars, Moebius effect, "take some pics and show me" | Request: an unlisted world called Splice, same scene, "show me a screenshot" |
| 1 | Starts in Three.js. The owner redirects it to Unreal. | Starts in Three.js in the wrong repo. The owner redirects it to Aethyr. |
| 3 | Editor target builds | Compiler confirmed: a realm builds in about 8 seconds |
| 5 | | World compiles to 43 KB |
| 7 | | **First screenshot.** The agent sees two faults (speed lines on, camera too close) and fixes them. |
| 8 | | Two more shoot-and-fix loops for a dune in the foreground |
| 9 | | **Pushed to `main`.** |
| 12 | Level script finishes | |
| 22–41 | Seven capture attempts. Every frame is black or never written. | |
| 42 | The agent opens the editor for the owner to look instead | |
| 45–60 | The owner reports, one message each: too dark; fell through the world; no art style; no difference; no difference again | |
| 63 | First working screen grab, after the owner grants Screen Recording | |
| 65 | The owner switches the model to Opus 5.5 | |
| 75 | Opus connects to the editor's MCP server and edits live | |
| 80 | **First verified picture** | |

Both sessions made the same first mistake and both were corrected inside a
minute. After that, the same model on the same task took 9 minutes in one
repo and never finished in the other.

### The game

| Milestone | Unreal | Aethyr |
|---|---|---|
| Gray-box loop (mine, parts, edit bay, Digger, rubble, end screen) | 17 minutes for steps 2 to 7, after the MCP connection existed | Included in the next row |
| Tier 1 complete (night party, Heat, raiders, boss, memory ending) | Three passes: 24 minutes, then 33 minutes with the party redesign, plus an 18-minute restyle | 18 minutes, one pass, from the design document to a pushed, playable build |
| Tier 2 (title, village, scanner gate, rail drive), both fonts, interface to the board | Not built at the time of comparison | 22 minutes |
| Party redesign (rooms, one blue piece, fights, DJ, Digger follows, cylinder player) | 33 minutes | 9 minutes |
| A new render mode (graybox with three accent colours) | Not asked | 7 minutes, with the rule placed in the engine |
| Production release | None | Two, about 7 minutes each |

Once the Unreal session had live MCP control, its speed on game rules was
close to Aethyr's. Blueprint logic written as text through the editor's
Blueprint tools compiled quickly, and the first six Blueprints took 17
minutes. The lasting gap is in the picture, the scope, and delivery, not in
writing rules.

## Why, in detail

### 1. The agent could not see

Aethyr has one command that renders a URL in headless Chrome and writes a
PNG (`coax/web/tool.sh shot`). The agent used it at minute 7 and 22 more
times. Every claim it made about the picture came after it had looked.

The Unreal session had no working capture for 63 minutes. It tried, in
order:

- `take_high_res_screenshot` from editor Python. The call is asynchronous,
  and later shots replaced earlier ones.
- One shot per editor run. The frame was black because the capture fired
  before the first frame rendered.
- A tick callback that waits 60 frames. `-ExecutePythonScript` exits the
  editor when the script returns, so the callback never ran.
- `-ExecCmds="py ..."`, which keeps the editor alive. The capture logged
  success and wrote no file.
- The `HighResShot` console command. No file.
- macOS `screencapture`. It failed without Screen Recording permission. With
  the permission it grabbed whichever window was in front, which twice was
  another session's terminal.

During that hour the agent wrote "I haven't verified it" after nearly every
change and asked the owner to check. The owner became the test harness. Each owner check cost
a screenshot, a message, and patience.

The fix, found at minute 75, was the editor's own MCP server
(`docs/unreal/mcp-tools.md`). Through `execute_python` the agent changed the
open editor in about 2 seconds and rendered shots from the editor itself,
whichever window was in front. `docs/unreal/` already described that server.
The session had read those docs at its start and did not use them for 75
minutes.

### 2. Unreal reported success when it had failed

Four silent failures cost most of the first 80 minutes:

| What happened | What Unreal said | Real cause |
|---|---|---|
| The post-process shader did nothing | "attached to PP_Moebius", no error in the script | The custom HLSL used `line` as a variable name. The generated Metal shader rejects it, and the engine draws the default material instead. The error was only in the editor log. |
| The shader still did nothing after it compiled | Volume enabled, unbound, priority 1000, weight 1 | In editor Python, `volume.settings` returns a copy. The material was added to the copy. |
| Every material was gray | No error | All generated materials had fallen back to the default. |
| The player floated, then fell | No error | A 200 m sky dome kept its collision, and the level had no player start. |

The first of these came back. Eighteen hours later, after three context
compactions, the session named a variable `line` again, got a brown frame,
and bisected post-process settings for a few minutes before it found the
same cause. The lesson lived only in the conversation, and compaction
dropped it.

Aethyr's checks stopped the same class of mistake at once:

- The compiler printed three syntax errors and the agent fixed them in one
  step.
- `bun coax/web/realms.ts --check` refused a world with no render adoption
  entry, and named the file to edit.
- The pre-push hook refused a commit with no `Agent:` line. The agent read
  the rule and complied.
- The size budget file forced an explicit number for the new world.

Each of these is a small gate. Together they mean the Aethyr agent never
believed something worked when it did not.

### 3. The cost of one change

| Loop | Unreal | Aethyr |
|---|---|---|
| Change, build, look (early) | About 5 minutes: close the editor, run the level script, reopen, wait for load | About 30 seconds: compile in 8 seconds, shoot, read |
| Change, build, look (after MCP) | About 2 seconds in the editor | Same as above |
| Change, build, look in the real target (a browser) | 10 to 11 minutes per Wasm build, then a browser test on two renderer tiers | The same 30 seconds. The dev build is the web build. |
| Ship | Not reached | About 7 minutes: staging, checks, production |

The Unreal session's own gap document says it plainly: each look iteration
cost a 10-minute web build, "so I iterated less". The agent ran four
shoot-and-fix loops on the first Aethyr scene in the time one Unreal editor
restart takes.

The browser matters because the Unreal editor and the Unreal web build are
different renderers. The first Moebius shader worked in the editor and on the
deferred web tier, and failed on the forward tier, which is what phones and
Firefox get. It read the normal buffer, which that tier does not have. The
agent then rebuilt the shader to read no extra buffers, and that is where the
look went flat. In Aethyr there is one renderer, so a picture that is right
locally is right in production.

### 4. The style existed in one engine and not the other

The Aethyr agent spent its first three minutes reading, not writing. It
found Anime Lab's mesh writers, uniform layout, cel shader, ink pass and
dusk preset, and loaded them. The first Splice world was 43 KB because it
added only a Mars landscape and a robot. `docs/reuse.md` told it to do
this, and the layer rules made the reuse path obvious.

On the second request the owner said to move shared pieces into the engine.
The agent moved the mesh writers to `aethyr/render/cel-mesh.coax` and wrote
`aethyr/kit/machines.coax` and `aethyr/kit/land.coax`: robots, walkers,
mesas, arches, canyon walls, domed houses. Seventeen still scenes then came
from those kits in 10 minutes. When the game arrived twelve hours later, its
characters, canyon and village were kit calls.

The Unreal agent started from the Third Person template. That template
gives a lit, physically based scene with auto exposure and Lumen. To get a
flat comic look the agent had to turn each of those off or fight it:

- Sun at 9, then 22, then 100,000 lux, with three exposure settings.
- Lumen off.
- Materials changed to emissive so colour showed with no lighting at all.
- A custom post-process for outlines, hatching and grain.
- A second shader that works without the normal buffer.

Its characters stayed primitives. The comparison screenshots show it: the
Unreal player is a white cylinder with a black slab, the Digger is an orange
cube, and raiders are black cylinders. The Aethyr build has cloaked raiders
with conical hats, a wheeled rover, banded canyon walls and a blue mech.

The Unreal session also made three art choices that hurt it, which its own
gap document lists: heavy hatching on every shadow face, thick ink on every
edge, and Epic's close third-person camera, which puts the player's body
over a third of the frame. The reference pins have thin lines, flat warm
colour and soft gradients.

### 5. Pictures first, or checklist first

The Aethyr session's first three requests were all pictures: one scene, then
five, then seventeen, each checked against the reference pins. By the time
it wrote a game rule it had a palette for each time of day, an open desert,
and a library of machines.

The Unreal session's game work began from a 7-step gray-box plan that says
cubes are fine, Blueprints only, and stop after each step. It followed that
plan well. But the plan treats the look as a stretch goal for the last
evening, and the session's two look passes were each bolted onto a sealed
gray corridor. A canyon open to the sky was never in the plan, so the Unreal
mine has a ceiling and no horizon.

The Aethyr agent also changed the design where the medium asked for it, and
said so each time: an open canyon instead of a roofed tunnel so the camera
and sky work, the edit bay showing the Digger itself, and a list of every
place it differed from the document. When the owner said to bring it in line
with the document, it did, in 22 minutes.

### 6. Text against binary assets

The Aethyr game is one file, `apps/openagents-web/splice-game.coax`, 1,457
lines. Layout, rules and drawing share constants. A design change is an edit
and an 8-second compile, and git shows exactly what changed.

The Unreal game is binary assets: a level, fifteen Blueprints, widgets and
materials. The agent cannot edit those directly, so it wrote 3,812 lines of
Python that generate them through the editor, plus Blueprint logic in the
editor's text format. That is a second program to keep correct, and the
generated state can drift from the scripts:

- Three duplicate post-process volumes built up from earlier runs.
- The game actor and the rubble wall vanished from the level once, with no
  cause found. The scripts now recreate them if missing.
- An existence check read the disk while the asset was only in memory.
- Editor queries returned nothing while Play mode was running, which gave a
  false "no volume in the level" reading.
- The first commit of the project was 1,924 files, and the game was not in
  version control for its first 13 hours.

### 7. One session, several jobs

The Unreal session was also the project's devlog writer, site publisher,
document author and version-control operator. Between game steps it built a
devlog process, a GitHub Pages site with eight pages, six build notes, a
design document as a Claude doc and then again as Markdown, an artifact
index, a publish script with a link checker, and a pass to correct dates. It
read a shared Drive folder and six shared artifacts.

All of that was asked for and most of it was done well. It also filled the
context three times. Each compaction kept the list of requests and dropped
working knowledge such as the `line` fault.

The Aethyr session had one job. Its side tasks were small: upload 17 images
to a Drive folder, summarize the other session twice.

## What was not the cause

- **The model.** Shown above. Opus 5.5 did rescue the Unreal session, but by
  finding the MCP server and the copy-of-settings fault, both of which are
  tooling facts, not reasoning feats.
- **The prompts.** Both sets are short and informal. The Aethyr prompts
  include one advantage: a file of reference pins named in the first
  request. The Unreal session got the same file 44 minutes in.
- **Effort or care.** The Unreal agent was candid throughout. It said
  "I haven't verified it" when that was true, stopped a guessing loop on its
  own, wrote down two bugs its scripted playtest found, and warned about a
  risk in how the devlog was dated. The Aethyr agent was equally careful to
  list what it had not tested.

## Where the comparison is unfair to Unreal

- **Second mover.** The Aethyr session read the Unreal session's transcript
  twice and built from the design document the Unreal session had written.
  It did not have to discover the game.
- **Other people's rules.** The Unreal build carries constraints the Aethyr
  build ignores: Blueprints only, a fixed asset path, both web renderer
  tiers, and a jam whose rules require Unreal. The Aethyr build cannot be
  the jam entry.
- **Unreal is ahead in three places.**
  - Characters collide with the level through the engine. In the Aethyr
    build, Imperial robots notice the player through walls.
  - Its scripted playthrough runs 46 checks in the editor, from first pickup
    to the end screen, and found two real bugs. A separate browser test
    presses real keys on both renderer tiers and takes a shot at each beat.
  - A designer can drop authored meshes and sounds into it.
- **Aethyr has its own gaps.**
  - Nobody has played it by hand. Every report says so.
  - Two pushes skipped the test step because no build machine was free.
  - A test of all 17 still scenes exceeded the compiler's time limit and was
    removed.
  - No sound.
  - Each release shipped `main`'s head, including other agents' commits.
  - The font files return 404 to a direct fetch although the page draws
    them. The cause is not traced.

## What to change

### For any agent session in Unreal

These are the lessons the Unreal session paid for. They belong in the
project repo as a file the agent reads at the start, so compaction cannot
drop them.

1. **Connect to the editor's MCP server first.** Start the editor once with
   the server on, and do all scene and asset work through `execute_python`
   and the Blueprint tools. Do not relaunch the editor per change. See
   [mcp-tools.md](mcp-tools.md).
2. **Prove one screenshot before any scene work.** Render it from the editor
   itself, not from the desktop. The session's `tools/devlog/shot.sh` in
   `cakbag` does this. Do not claim a visual result without a shot.
3. **Check every generated material for compile errors, and fail on the
   fallback material.** Read the editor log after each material build.
   Never name an HLSL variable `line`, or any other Metal or HLSL keyword.
4. **Write struct settings back.** `volume.settings` returns a copy. Change
   the copy, then assign it.
5. **Make level scripts safe to rerun.** Delete by label before creating, so
   reruns do not stack duplicate volumes or actors.
6. **Stop Play mode before editor queries.** Queries return nothing during
   Play.
7. **Add a player start and check collision** on every generated level
   before telling anyone to press Play. Large backdrop meshes need collision
   off.
8. **Do not use `-ExecutePythonScript` for anything that needs a rendered
   frame.** It exits when the script returns.
9. **Decide the shading model on day one, for the weakest web tier.** If the
   look is flat colour, use unlit materials with your own tone and light
   terms, turn Lumen and auto exposure off, and read no buffer the forward
   tier lacks. Package a web build before the look is "done", not after.
10. **Stage still pictures before game rules.** Build three or four framed
    scenes against the references, with the camera the game will use, and
    get them approved. Then build the gray box inside that world.
11. **Build kit pieces, not single primitives.** A raider, a rover and a
    wall segment should each be a reusable part list from the first hour.
12. **Commit the project on the first day.**

This session verified some facts that [mcp-tools.md](mcp-tools.md) lists as
unchecked. The server answers on `127.0.0.1:8000`. `execute_python` allows
top-level `await`, has `unreal` imported, and does not allow `time.sleep`.
The Blueprint text tools can create, write and compile Blueprints and
widgets. Scripted keystrokes are blocked by macOS, so key input needs a
browser test or an event call. That doc should be updated from the session's
`tools/ue/` scripts.

### For Aethyr

The session shows what to protect and what to add.

Protect:

- The one-command headless screenshot.
- The 8-second world compile.
- The loud gates: realm check, render adoption, size budget, pre-push rule.
- `docs/reuse.md` and the layer rules, which turned one world's needs into
  engine kits.

Add:

1. **A browser playthrough harness with real input.** The Unreal build's
   test presses real keys in a real browser and takes shots at each beat.
   The Aethyr tests place the player at each spot. Mouse look, TAB and
   dragging are untested in a real browser.
2. **Line-of-sight for non-player characters.** Notice checks ignore walls.
3. **Sound.** Neither build has any, and in Aethyr there is no path for a
   designer to drop a sound in.
4. **A way for a non-programmer to supply a mesh or texture.** This is the
   one reason the jam entry stays in Unreal.
5. **A test for the 17 still scenes that fits the compiler's time limit.**
6. **A push path that never skips tests silently.** When no build machine is
   free, run the world's own tests locally and say so in the commit.

### For how the work is assigned

1. **Give the picture target in the first message.** Reference images at
   minute 0 instead of minute 44 would have set the palette and the camera
   before any geometry existed.
2. **One session, one job.** Run the devlog and documents in a second
   session that reads the first one's transcript. The Aethyr session did
   this to the Unreal session twice and it cost a few minutes each time.
3. **Ask for a self-taken screenshot in every visual request.** Both first
   requests did. The engine that could honour it won.
4. **Write lessons to a file, not to the chat.** A `GOTCHAS.md` next to the
   scripts survives compaction. The chat does not.

## Related docs

- [mcp-tools.md](mcp-tools.md): the editor's MCP server.
- [web-games-in-the-verse.md](web-games-in-the-verse.md): hosting Unreal Wasm
  builds. This session adds two data points: an 11-minute Wasm build for a
  small project, and a forward tier that lacks the normal buffer.
- [2026-11-11-learning-from-unreal-5-9.md](2026-11-11-learning-from-unreal-5-9.md):
  what Aethyr should take from UE 5.9. The scripted in-browser playthrough
  is one more item for that list.
- `docs/reuse.md`: the rule that produced the engine kits.

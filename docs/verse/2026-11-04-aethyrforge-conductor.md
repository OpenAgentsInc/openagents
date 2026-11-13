# The Metaverse Engine as the conductor: one app for building, playing and editing Everglade

Status: audit and spec, 2026-11-04. Desktop only. Builds on
[the Metaverse Engine for Mac](2026-11-04-aethyrforge-desktop.md), whose shell, local
room and setup pane exist today (d1f14ec560, 5c93c7523d).

Today Everglade is built, tested and played by up to eight agents and one owner
on one Mac, with nothing in charge. Each agent has its own worktree, its own
builds, its own servers and its own idea of when to test. The owner plays in a
browser tab served by a ninth process tree. This doc proposes that the OpenAgents Metaverse Engine
becomes the one place that decides what gets built, in what order, on which
cores, and what the owner is playing. Agents become workers it schedules, not
peers that compete.

The web gets the same flow later. Desktop comes first because it can count on
the local coding agents (Claude Code, Codex, Cursor), the local voice model
(Parakeet) and the local edit model (gpt-oss-20b).

## What went wrong on 2026-11-04

Every item below cost the owner time today, and none of them was a game bug.
Each one is a coordination failure that one owning process would prevent.

| # | What happened | Root cause | What prevents it |
|---|---|---|---|
| 1 | The load average reached 86 on an 18-core Mac. The owner's game and their desktop-app agent lagged. | A full site build (`apps/openagents-web/build.sh`) starts every world's wasm build at once, about 40 compiles. A room build (`coax/web/room/build.sh:23-30`) starts every C file at once. A shared 3-slot gate counted builds, not cores. Test suites and headless Chromes ran beside builds. | One scheduler that owns a core budget and runs jobs as units of work, not whole scripts. |
| 2 | The owner played a build from before three movement fixes, and reported bugs that were already fixed. | The page they played was served from a worktree built at an older commit. Nothing told them, or me, which commit they were on. | The app shows the build it's running and moves the owner to new builds itself. |
| 3 | Restarting the room under the owner dropped their page into offline play: the full spell bar, no WANTED, guards that didn't fight. | A room restart was a process kill. The page treated a lost room as "go offline" (fixed by d5c9c345b0, but restarts still drop the session). | The owner's world is never restarted under them. New builds swap in with the state handed over. |
| 4 | `git pull` failed in the owner's main checkout. | Some process wrote to the main checkout. `apps/openagents-web/build.sh` also rewrites tracked files when it runs (`projects/*/index.md`, `projects/list.coax`, `newland-data.coax`: the project pages baked from Nostr), so every worktree that builds the site goes dirty. | Builds never write tracked files. The app owns the worktrees, and the main checkout stays read-only. |
| 5 | A video restarted at 19.5 s. | The recording page was connected to the dev server's live reload, and another agent's rebase pushed a new engine mid-recording (fixed by c63e9dc442). | Live reload is the app's decision, per session, not a side effect of files changing. |
| 6 | The page and the room disagreed on the heading's units for hours. The server walked players the wrong way and the camera swung. | Two halves of one contract were changed in separate commits by separate agents, with no shared check. | One wire contract file, with a check both builds run, and the integration lane rejects a change that touches one half only. |
| 7 | A drift test passed and failed for reasons unrelated to the change being tested: bots that only read the old wire, headless key input that couldn't hold W, a check that walked into the town wall. | Each agent wrote and repaired its own harness against a moving target. | Shared, app-owned test fixtures: bots, input and routes that the app keeps current with the wire. |
| 8 | Agents conflicted on shared numbered resources, such as page probe fields 70 to 76, and on rebases of the same files. | Numbered slots are handed out by whoever edits first. | Registries for shared numbers, and a serialized integration lane. |
| 9 | The owner was told an issue was closed while the game still played badly. | Checks tested state, not what a player sees, and nothing required the owner to have played it. | "Ready to try" is a state the owner accepts in the running world, not a closed issue. |
| 10 | Status messages from eight agents arrived as interrupts, most of them "still waiting". | No shared view of what is running. | One status panel in the app. Agents report there, not to the chat. |

The Metaverse Engine itself showed two of the same patterns when the owner opened it:

- Everglade originally kept simulating and rendering behind the setup pane.
  #12865 now pauses it after the first real frame; setup/status use the shared
  native renderer and the measured process CPU drops about 97%.
- The original claim that WKWebView is slower than Chrome was unmeasured.
  #12864 compares the same 5E producer and recorded sixteen-bot siege in
  Chrome/WK on the M5 Max and Chrome/native Vulkan on coderos-4080. The
  [record and one-command harness](../verification/everglade-hosts-2026-11-05/README.md)
  separate ordinary cadence from GPU timestamp overhead, and label software
  completion latency separately from physical input-to-photon latency.

Hashing Parakeet on launch was already removed: launch calls
`refreshSetup(verify: false)`, and only the Verify button hashes.

## The idea

The Metaverse Engine owns four things that today nobody owns:

1. **The build graph:** what's built, when, and on how many cores.
2. **The owner's world:** the room and page the owner is in, which never
   restart under them.
3. **The workspaces:** every agent's worktree, and the one lane that lands
   changes on `main`.
4. **The edits:** world edits, voice edits and code changes, each through one
   path, with one undo.

Agents keep doing the work that needs judgment: writing code, fixing tests,
designing things. They stop deciding when to build, which port to use, when to
restart a server, or when to push.

## 1. The build scheduler

One long-running process in the app runs every build and test on this Mac. Agents,
the owner's world and Coder all ask it for work. Nobody runs `build.sh`, `wasm.sh`
or `coax test` directly any more. The CLI gets `openagents build` and
`openagents test`, which forward to the scheduler.

**Jobs are units, not scripts.** The scheduler reads each target's unit list
(the same list `coax/web/tool.sh wasm-size sources` gives today) and runs units
as separate jobs:

- each Coax unit's C
- each C object
- each link

A "page build" becomes about 40 small jobs that share the queue with everything
else, rather than one opaque script with its own fan-out.

**A core budget, not a slot count.** The scheduler knows the machine:
`hw.perflevel0.physicalcpu`, memory, and the load from processes it doesn't own.

- It keeps a reserve for the owner (default: 4 performance cores and the GPU's
  headroom for the world view), and never schedules into it.
- It slows down when outside load rises, for example when the owner opens
  Cursor or Chrome.
- The budget is shown in the status panel, with what is using it, so no limit
  is hidden.

**The compiler enforces the floor.** The Coax compiler keeps a machine-wide
worker budget itself: every coax process draws its check and mutation workers
from one pool of `flock` tokens in `~/.cache/coax/jobs/`. The pool defaults to
the performance cores less a third left for the owner, and it backs off under
outside load. So builds that bypass the scheduler still can't overload the Mac.
The scheduler builds on this pool: it sets the budget and reserve with
`COAX_JOBS_BUDGET` and `COAX_JOBS_RESERVE`, and reads `coax status --jobs --json`
for the panel.

**Priority classes**, highest first:

| Class | Example | Rule |
|---|---|---|
| Owner live | The build the owner is about to swap into, the owner's own edit | Runs first and preempts lower classes between units |
| Integration | The rebase-and-check of a change about to land on `main` | One at a time |
| Agent check | An agent's affected tests before asking to land | Fair-shared between agents |
| Agent dev | Speculative builds while an agent iterates | Only on spare cores |
| Background | Goldens, load tests, film renders, model hashing | Only when everything else is idle |

**One shared cache.** `coax/web/wasm.sh` already keys each unit's C, header and
object by SHA-256 in `~/.cache/coax-wasm`. The scheduler extends that to the
room's C objects and the native faces. Eight worktrees that share 95% of their
sources then share 95% of their compiles. Today each rebuilds Everglade from
scratch after every rebase.

**Builds write only into build directories.** Steps that write tracked files
today become separate, explicit commands that an agent runs when it means to
change them. For example, the project-page bake (`apps/openagents-web/build.sh`
lines 196-200) becomes `openagents project bake`, and the scheduler refuses a
build step that dirties the tree. That ends the dirty worktrees and the
"local changes would be overwritten" failures.

**Measured.** Every job records its time, cores and cache hit. The panel shows
what a change costs to build. Regressions in build time become visible the way
frame time is.

## 2. The owner's world

The owner always has exactly one world open: their local Everglade room, in
the Metaverse Engine's window. The app owns its processes, and three rules hold:

1. **It never restarts under the owner.** A new room build is started beside
   the old one, the old room hands over its state, and the page reconnects with
   the same seat, kit, level and WANTED. The room needs one new capability for
   this: write its world state to a snapshot and start from one. The handshake
   (wire `H`), the generation counters and the reconnect path
   (d5c9c345b0) already exist.
2. **It moves to new builds when the owner says so,** or at once when the owner
   turns on Follow main. The title bar always shows the commit it runs and how
   far behind `main` it is ("3 changes behind · Try them"). Trying a change
   swaps in a page and room built from that change.
3. **It's measured.** Frame time, tick time, the NET reading and input-to-ack
   are recorded per build, so "this build feels laggy" becomes a number with a
   commit next to it.

The view itself:

- **WKWebView for now,** measured against Chrome on the same build. If WebKit's
  WebGPU is materially slower, the decision gets made with numbers.
- **Pause what's hidden:** the world's frame loop stops while setup covers it
  or the window is hidden, and the room keeps ticking.

**Offline is a one-seat room** (#12784). The work so far is in
`~/work/openagents-worker`: a shared tick, the room built to wasm, and a
`local:` socket. That is a prerequisite here. Once offline and the room are the
same simulation, the app needs only one world to keep current, and every bug the
owner finds is a room bug.

## 3. Workspaces and the integration lane

**The app owns every worktree.** An agent asks for a workspace for a task. The
app creates `~/work/openagents-ws/<task>` from `origin/main`, shares the build
cache with it, and removes it when the task lands or is dropped.

- **The main checkout is read-only to agents.** The app checks this with a file
  watcher and names any process that writes to it.

**One lane lands changes.** When an agent says a change is ready, the lane:

1. Rebases it on `main`.
2. Runs the affected checks at Integration priority. These are the tests whose
   sources the change touches, from the same source-closure data the scheduler
   uses.
3. Pushes it.
4. Tells the other workspaces that `main` moved.

Only one change is in the lane at a time. A conflict goes back to the agent that
owns the change, with the conflicting commit named. Agents no longer push to
`main` themselves. This keeps the owner's "push straight to main" speed: a
change waits only behind other changes being landed, which takes minutes, not a
review.

**Contracts are checked as contracts.** Files that define a contract between two
builds are listed in one manifest:

- the wire file (`everglade-wire.coax`)
- the effect and kind tables
- the probe field registry

A change to a contract must update both sides in one commit, and the lane runs
the contract's paired check, such as the drift check for the wire. That would
have caught the heading-unit split on the first commit.

**Registries for shared numbers.** Probe fields, entity kinds, effect ids, ports
and wasm budgets are handed out by the app from a registry file. An agent asks
for "a probe field for door state" and gets one, so two agents can't take 70 to
73 on the same afternoon.

## 4. Edits, in one place

Three kinds of change reach Everglade, and the app treats them differently
because they are different:

| Kind | Example | Path | Undo |
|---|---|---|---|
| World data | Move the well, resize a crate, switch a feature off | The world document, through the room's typed edits (`metaverse-engine/editor/edit.coax`), saved locally at once | The edit log, durable across launches |
| Tuning | Guards' HP, WANTED thresholds, spell radii | A tuning table in the world document where the rules allow it, otherwise a code change | The edit log, or a revert |
| Code | Entangle catches everyone in its area, a new door rule | A task for an agent, in its own workspace, landed through the lane | Revert in the lane |

The owner shouldn't have to know which kind a request is. They say it, or point
at it, and the app routes it:

- **Voice and typing** go to the edit model first: gpt-oss-20b under the edit
  grammar, as the desktop spec describes. A sentence that maps to a typed world
  edit is applied at once. A sentence that can't ("guards should hit harder",
  "Entangle only caught one") becomes a task.
- **Pointing:** clicking something while holding the report key attaches what's
  under the cursor (its entity id, its replicated row and the last few seconds
  of wire) to the task. "This guard doesn't fight back" arrives with the
  guard's id, its state and the wire log. No screenshot or narration is needed.
- **A task** goes to a local coding agent: Claude Code, Codex or Cursor, from
  the setup pane's preferred engine, in a workspace the app creates. Its
  progress shows in the status panel. When its change passes the lane's checks,
  the owner sees "Entangle fix ready · Try it", and trying it swaps it into the
  running world.
- **Accepting** keeps the change on `main`. **Rejecting** reverts it in the
  lane and returns the task to the agent with the owner's words.

## 5. Tests the app owns

- **Shared fixtures:** the app owns the bots (`raid-check.coax`, guard-check,
  door-check, the drift bots), the routes they walk and the input they send.
  When the wire changes, the fixtures change in the same commit, through the
  contract manifest.
- **Real input:** WKWebView gets keyboard and mouse events from AppKit, so the
  app can drive the world with real key-down, hold and key-up. Headless CDP
  couldn't hold W today. The same driver records a play session and replays it
  as a test.
- **Test what a player sees:** checks run against the drawn frame and the HUD's
  probes, not only against state. The drift check already compares what's drawn.
  Every feature check does the same.
- **Affected only:** an agent's check runs the tests whose sources its change
  touches. The full suites run at Background priority, and on every Integration
  run for contract files.

## 6. The status panel

One panel replaces the stream of agent messages:

- **The machine:** the core budget, the owner's reserve, the load from outside
  processes, and the GPU frame time of the world view.
- **The queue:** each job's class, owner, time and cache hits.
- **Workspaces:** each task's agent, branch, state (working, checking,
  in lane, ready to try, landed, rejected) and the last line it reported.
- **The owner's world:** its commit, how far behind `main` it is, and its frame,
  tick, NET and input-to-ack readings.

An agent's "still waiting" is a row in this panel, not a message. Only three
things interrupt the owner: a change ready to try, a change that broke
something, and a decision only the owner can make.

## Meditations

**The scarce resource is the owner's attention, then the cores.** Today the
owner spent the day reporting bugs that were already fixed, waiting on builds
they didn't ask for, and fighting load from processes they couldn't see. Every
part above serves one of two goals: the owner's minutes go to playing and
judging, and the cores go to the build the owner is about to see.

**Builds are a scheduling problem, not a scripting problem.** Our build scripts
are correct but greedy: each assumes it owns the machine. Parallelism inside a
script and parallelism between agents multiply. The fix isn't smaller scripts.
It's one scheduler that sees all the work as units and owns the cores.

**The owner's world is a long-lived process, like production.** We run
production rooms carefully: deploys hand over, wire versions overlap for a week,
reconnects keep the seat. The owner's local room deserves the same care, and
today it got the least. Treating it as production also tests the handover,
reconnect and version paths players will hit.

**One simulation, then one world.** Most of today's game bugs came from the game
existing twice: offline on the page, and in the room. #12784 removes that. The
conductor removes the same duplication one level up: two copies of "what the
owner is playing" (a browser tab and a worktree) and eight copies of "how to
build and test".

**Data edits shouldn't wait for code.** Moving a crate or making it night is a
world edit that should land in under 2 s, with no build. Making that path fast
and safe, typed and undoable, keeps agents for the changes that need code. The
more of Everglade that is world data or tuning, the less the owner waits.

**Bug reports are state, not prose.** A report that carries the entity id, its
replicated row and a few seconds of wire can become a failing test almost
mechanically. That's the shortest path from "I attacked a guard and nothing
happened" to a fix that stays fixed.

**Contracts deserve a home.** The wire, the effect tables and the probe fields
are shared between builds and between agents. Today they're ordinary files that
anyone edits. Giving them a manifest, paired checks and registries is cheap, and
it would have prevented the day's worst bug.

**Desktop first, then the web.** The web version keeps the same four parts, but
moves the scheduler and workspaces to our own GCP hosts (the agent pool), keeps
the owner's world in the browser, and replaces local agents with hosted seats.
Desktop proves the flow where everything is in one place and nothing costs per
minute.

## Build order

Each step lands on `main` working end to end. Steps 1 and 2 pay for themselves
right away. Later steps assume #12784 (offline as a one-seat room) has landed.

1. **The scheduler, for builds only.**
   - It runs `openagents build` for the room, the Everglade page and the
     Metaverse Engine bundle, as units, on a core budget with the owner's reserve.
   - It extends the shared cache to the room's objects.
   - Build steps that write tracked files move out, starting with the
     project-page bake.
   - Agents' instructions say to use `openagents build`.
2. **The owner's world, kept current.**
   - The title bar shows the commit, and Try them swaps in a new build.
   - The room learns snapshot and restore, so a swap keeps the seat, kit, level
     and WANTED.
   - The world view pauses while hidden.
   - It measures WKWebView against Chrome.
3. **Tests through the scheduler.**
   - `openagents test` runs affected tests at the right priority.
   - The shared fixtures move into one place, with real input driven through
     AppKit.
4. **Workspaces and the lane.**
   - The app creates and removes worktrees.
   - The lane rebases, checks and pushes one change at a time.
   - It adds the contract manifest and the registries.
   - The main checkout watcher names any process that writes to it.
5. **The status panel**, fed by the scheduler, the lane and the world.
6. **Edits in one place.**
   - Voice and typing go to the edit model.
   - The report key attaches state.
   - Tasks go to local agents in app-owned workspaces.
   - Changes come back as Try it, with accept and reject.

## Open questions

- **Who runs the scheduler when the Metaverse Engine is closed?** A small launchd agent
  that the app talks to is simplest: agents can still build with the app shut,
  and the app attaches to it when it opens.
- **How much preemption is safe?** Preempting between units is simple. Stopping
  a running link or test suite isn't, so Owner-live work waits for the current
  unit to finish.
- **Does the lane replace pushing to `main` for everyone,** or only for agents
  the app starts? Starting with the app's agents, and moving others onto it as it
  proves itself, avoids a flag day.
- **WKWebView or a native view?** The desktop spec chose WKWebView so the owner
  plays the exact engine players get. If measured WebKit performance is far
  below Chrome's, the choice is between living with it, embedding Chromium, or a
  native Metal host for the same Coax engine. That's decided with numbers from
  step 2.

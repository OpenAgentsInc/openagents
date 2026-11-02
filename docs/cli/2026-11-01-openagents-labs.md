# openagents labs: repeatable media, game and agent work from the command line

Status: phases 1 to 3 shipped 2026-11-01 (see [What shipped](#what-shipped)):
`shot`, `clip`, `deck`, `frames`, `compare`, `cut`, `board` and `playtest
icecream`. `suite` (phase 4) and the MCP rows and skill (phase 5) are not
built. It extends
[the openagents CLI, v1.0.0](2026-11-01-openagents-cli-v1.md) with one more
domain, `labs`. The primitives it calls exist unless a row says **new**.

Owner request: look through the pre-Nuke Labs (Film Lab, Agent Lab and the
rest) and decide which of their functions belong in an `openagents labs`
CLI, for repeatable, programmatic control that users want too, built on our
primitives.

```sh
openagents labs shot /realms/orbit-league?stage=highlight orbit.png --phone
openagents labs clip /realms/belt-2200?stage=highlight belt.mp4 --seconds 6
openagents labs cut episode.cut episode.mp4 --quality review
openagents labs playtest icecream --set speed=0.8 --seeds 24
openagents labs deck pitch.deck pitch.pdf
```

## Verdict

Build it, small and local first. Every pre-Nuke Lab was the same loop: a
typed document, a checked edit, an immutable version, and an output made
from that version (a render, a playtest, a report). The chat cards, hosted
API and per-Lab services around that loop were the expensive part, and the
Nuke removed them. The loop itself is what users and agents want from a
command line: "make this file from that document, the same way every time,
and tell me what changed."

We already have most of the engines in Coax: headless Chrome shots and
clips, the deck builder, Ice Cream Man's rules and bot, Verse cameras and
cinematics, the Claude and Codex agent SDKs, and `openagents transcribe`.
Today you reach them through `coax/web/tool.sh` with repository paths. The
`labs` domain puts them behind one installed binary, with the CLI's
`--json`, exit codes and MCP tools, so a script or Claude Code can drive
them without a checkout.

## What the pre-Nuke Labs were

The Labs lived in `crates/*-lab`, `scripts/film/` and `docs/*-lab/` until
the Nuke of 2026-10-27 (`0b401561a0`). Read any of them with
`git show 0b401561a0^:PATH`. The shared design is
`docs/labs/README.md`: documents, versions, jobs, artifacts, notes, a card,
and a hosted `/v1/labs` API.

| Lab | What it did | Inputs | Outputs | Repeatable operations |
| --- | --- | --- | --- | --- |
| Video Lab ("Film Lab"): `scripts/film/lab.py`, `cut.py`, `review.py`, `crates/video-lab`, `openagents lab` | Edited films and Verse scenes by talking; rendered previews in the chat | `cut.v1` (shots with source, in point, duration, crossfade, titles, music, voice, captions), `shot.v1`, `scene.v1`; review notes at a time | MP4 at preview, review and master quality; stills and strips; side-by-side compares; music; transcripts | `render`, `frame`, `compare`, `edit --patch`, `versions`, `notes`; `inspect` refused cuts inside an animation beat, missing sources, titles outside the safe area |
| Film engine: `openagents film` (`verse-film`) | Recorded a scene once as a message stream and drew it with any renderer | A take file | PNG frames, MP4 | `inspect`, `render`, `export`, `launch` |
| Game Lab: `crates/game-lab`, `openagents game` | Changed Ice Cream Man's tuning by talking; a bot checked it was still winnable | `game.v1`: tuning numbers and words | Bot-run stats over 24 seeded houses; a playable link | `edit`, `open` (stats), `play`, `compare`, `notes` |
| Agent Lab: `crates/agent-lab`, `docs/agent-lab/` | Created, tried and compared agents; replayed a recorded bad day as a suite | `agent-draft.v1`, `agent-suite.v1`, `agent-experiment.v1` | `agent-results.v1`: per case, checks passed, runs that did not finish, links to conversations | `replay list`, `replay task`, `replay score CASE reply.json`, trials on a fixed suite, compare two versions |
| Design Lab: `crates/design-lab` | A board of every screen of every app, captured from code | A capture tour per app | PNGs with element trees, light and dark; a static board | `capture`, `serve`, `export DIR`, `spec` |
| CAD Lab: `crates/cad`, `crates/cad-lab`, `openagents cad` | Built parts and assemblies on a Rust B-rep kernel | `part.v1`, `assembly.v1` | glTF, STEP, 3MF, STL, drawings, mass properties | rebuild, export |
| Robot Lab: `crates/robot-*` | Robots in MuJoCo scenes, LeRobot data, policy runs | Robot, environment and experiment documents | Simulation runs, replays, policy scores | simulate, replay, compare |
| Ship Lab: `crates/ship-lab` | Checked a spacecraft against mass, thrust and burn budgets | `spacecraft.v1` | Checks and a Verse zone file | check, export zone |
| Briefing Lab: `crates/briefing-lab` | A read-only issue briefing from a pinned commit | Repo, commit, issue | `briefing.json`, `briefing.md` | `index`, `preview` |

The Labs README already ruled `briefing-lab`, `layertree-tee-lab` and
`verse-zone-lab` out as chat Labs. "Film Lab" was never its own crate: the
README listed "Photo, Film, others" as not built, and the film work lived in
Video Lab and `openagents film`.

## What belongs in `openagents labs`, and what doesn't

A function belongs when all of these hold:

1. **It runs on a user's machine or our public routes.** No private assets,
   no owner-only hosts, no GPUs.
2. **It is deterministic, or says exactly what it depended on.** The same
   document and flags give the same file, and every output carries a label
   that says how it was made.
3. **A Coax engine for it exists today, or is a small port.** No feature
   waits on a deleted Rust crate.
4. **A person would run it outside our team.** Makers, players and agent
   builders, not only our release checks.

| Function | Verdict | Reason |
| --- | --- | --- |
| Shots of a page or realm at desktop and phone sizes | **In** (`labs shot`) | `coax/web/shot.coax` works today on any URL. Makers want a picture of their realm; agents need it to see what they did |
| Clips of a realm, staged or simulated | **In** (`labs clip`) | `coax/web/clip.coax` records at fixed game time, so the same stage gives the same clip. `?stage=NAME` is already the contract (`docs/verse/clips.md`) |
| Slide decks from scenes | **In** (`labs deck`) | `coax/web/deck.coax` and `docs/web/decks.md`. Makers already pitch realms with it |
| Cut lists rendered to video | **In** (`labs cut`) | This is Video Lab's core, minus the chat. The `cut.v1` fields are known; `ffmpeg` does the encoding, as `clip` and `transcribe` already rely on it |
| Stills and strips from a video | **In** (`labs frames`) | Video Lab's `frame`. One ffmpeg call per time; what an agent uses to "look closely" |
| Captions from speech | **In**, as a flag (`labs cut --captions`) | `openagents transcribe --srt` exists. Labs calls it; it doesn't own it |
| Side-by-side compare of two outputs | **In** (`labs compare`) | Video Lab's compare and Game Lab's stats diff, generalised to any two outputs with labels |
| Game tuning checked by the bot | **In** (`labs playtest`) | Ice Cream Man's rules, tuning bounds and bot are Coax (`verse/icecream/rules.coax`, `tuning.coax`). Seeded runs are pure |
| A board of every page | **In**, for web pages only (`labs board`) | A list of URLs taken at two sizes and two themes is `shot` in a loop. The iOS, Android and desktop tours needed Xcode, emulators and app code; they stay with the apps |
| Agent suites scored offline | **In**, later (`labs suite`) | Agent Lab's replay scoring started no process and touched no network. A suite file plus saved replies is pure and repeatable |
| Agent trials on a suite | **Later**, with limits the user sets | `coax/lib/agent/claude` and `codex` can run a turn, but each case spends the user's model credit. It ships after `suite score`, with a cost cap flag the user sets |
| Realm builder evals and the Aethyrforge journey | **Out** | `realm-layout-eval` and `forge-journey` are our release checks against staging. They stay in `coax/web/tool.sh` |
| Design Lab's native app tours | **Out** | They need simulators and app source, so they are developer tooling inside `apps/` |
| CAD Lab | **Out** | Its kernel was Rust (Truck) and is gone. It returns only with a Coax kernel |
| Robot Lab | **Out** | MuJoCo, LeRobot and policy runs need GPUs and physical hosts |
| Ship Lab | **Out for now** | The space primitives (`verse/orbit.coax`, `thrust.coax`, `vehicle.coax`) have no spacecraft document yet. When one exists, `labs ship check` fits the pattern |
| Briefing Lab | **Out of labs** | It belongs with the project queue, as `project brief SLUG`, if anywhere |
| The hosted Labs platform (`/v1/labs`, cards, chat tools, enrolled hosts) | **Out** | The CLI v1 rule: a domain ships only when its routes exist and are tested. Those routes were Rust and are gone. The CLI's MCP server gives agents the same tools without a hosted store |
| MetaHuman, Fab and other private-asset renders | **Out** | Private assets never enter a user tool. They stay in the owner's private pipeline |

## The model: a lab file, an output, a label

Labs keeps the pre-Nuke loop and drops the server.

- **A lab file is the document.** It is plain text in the deck format's
  style (one word starts each line), so you can write it, diff it and keep
  it in Git. Kinds: `.deck` (exists), `.cut` (**new**), `.board`
  (**new**), `.suite` (**new**). Git is the version history; Labs adds no
  store of its own.
- **Every command checks its file before it makes anything.** A refusal names
  the line and the reason in plain words, exits 1, and makes nothing. This is
  Video Lab's `inspect`, run every time.
- **Every output gets a label.** Beside `OUT` Labs writes `OUT.json`, the
  label `docs/verse/clips.md` already asks for:

  ```json
  {"kind": "clip", "source": "staged", "url": "https://openagents.com/realms/belt-2200?stage=highlight",
   "commit": "a40dfd940a", "size": "1280x720", "fps": 30, "seconds": 6, "skip": 0,
   "input": "sha256:…", "output": "sha256:…", "made": "2026-11-01T15:04:05-05:00", "cli": "1.0.0"}
  ```

  `source` is one of `recorded`, `simulated`, `staged`, `page`, `file` or
  `generated`. `input` is the digest of everything the output depends on
  (the lab file, the flags, each source file's digest, and the page's
  served commit when the page reports one). `commit` is that served commit,
  read from the page, never the local checkout.
- **Outputs are cached by `input`.** When `OUT.json` already has the same
  `input`, the command prints the output and does nothing else. `--fresh`
  makes it again. A deck's `DECK.captures/` keeps working this way.
- **Nothing opens on screen and no sound plays.** Chrome is always headless
  and muted (`coax/lib/net/cdp.coax`). Labs never opens a browser window,
  even with `--open`; `--open` prints the path.

## Commands

All commands take the CLI's global flags (`--json`, `--profile`,
`--origin`, `--quiet`). Text output is `key value` lines. A path argument
may be a URL, or a path such as `/realms/SLUG`, which is joined to
`--origin` (default `https://openagents.com`).

Labs adds one exit code to the CLI's table:

| Code | Meaning |
| --- | --- |
| 8 | a program Labs needs is missing: Chrome, ffmpeg, or the transcribe backend. The message names it and the one command that installs it, for example `brew install ffmpeg` |

### `labs anime`

```sh
openagents labs anime catalogue --json
openagents labs anime direct "Draconia casts a fireball. Close up on Kai." --take scene.take
openagents labs anime play scene.take out.png --at 1.5 --card
openagents labs anime play scene.take out.mp4 --size 640x360 --fps 12
```

`catalogue` lists the fixed cast and every finite selector field. Catalogue
membership does not promise that a particular direction has a rendered effect;
slot 6 has no stock body and arbitrary `make` answers remain heard only.

`direct` signs one request to `/api/playtest/labs/anime/direct` with the
selected profile's NIP-98 identity. It sends the written words and a finite
context summary of the previous take's admitted events: cast visibility and
places, last directed action/emotion, camera framing, and up to eight recent
executable user answers. This is selector context, not a physical observation
of a running page. The complete public answer passes the same catalogue,
probability and render-support holders as the API before it can enter the
[recorded take](../verse/anime-takes-v1.md). The CLI prints the words, held
answers and their confidence dispositions. It performs no word matching,
speech recognition or asset generation.

`--take FILE` creates a take or appends one written line. A written line is
instantaneous: `said == done`, at zero for the first line and at the preceding
take's length for later lines. This records an editing sequence, without
inventing speech duration. Existing lines and answer metadata are retained;
the rebuilt take receives a new tail for playback. The 32 KiB, 16-line,
16-answer and 600-second take bounds still apply. Invalid input, a failed
request or an invalid answer leaves the take unchanged. Concurrent CLI writers
use an exclusive `FILE.anime.lock` directory and compare the original bytes
before a complete replacement is renamed into place. A held lock or changed
take exits 5. After an interrupted writer, inspect that lock before removing
it; the CLI never deletes a lock it did not acquire.

`play` validates the take locally, encodes it in the page's `take64` input,
and requires the page to report that it accepted the complete document before
writing media. PNG defaults to `--at 0`; MP4 defaults to the remaining take
duration at 30 fps. Anime playback accepts 10–120 fps so each encoded frame
fits the page's maximum 100 ms time step; lower rates refuse before capture.
`--at`, or `--skip` plus `--seconds`, must fit inside the
take. `--card` hides captions. Size presets, `--size`, PNG `--wait`, MP4
`--fps`/`--frames`, and `--fresh` work as for shot/clip. Chrome stays headless
and muted; captures retain the existing cache and sidecar behavior. The take,
output, output label and any generated frame must be different files.
The report names the files and label; the label retains the complete encoded
take URL and capture inputs.
`--frames` needs a new or empty directory and regenerates the frame collection
even when the video is cached. This keeps old frame files out of a new clip.

All three commands support `--json`, `--profile` and `--origin`. Unknown,
duplicate, missing-value and irrelevant flags are usage errors. `--` makes
later words literal operands; a flag-shaped option value stays a value.
Exit codes follow the CLI table: 2 usage, 3 missing identity, 4 forbidden,
5 conflict, 6 missing take, 7 quota, 8 missing media program, and 1 other
failures. API and page deployment are separate from source qualification;
`--origin` selects the API for `direct` and the page for `play`.

### `labs shot`

```text
openagents labs shot URL OUT.png [--size WxH | --phone] [--wait MS] [--eval JS]
                                 [--as-me] [--dark] [--fresh]
```

- Takes `URL` in headless Chrome with WebGPU once the page is ready, then
  `--wait` milliseconds more to settle (default 4000). Ready is what
  `media_ready_step` (`coax/lib/media/media.coax`) says: a Coaxial page's
  `globalThis.coaxial.ready` and `sceneReady` (host.js sets it when the first
  scene is drawn) with its loading card gone, or any other page loaded. A page
  not ready within 180 seconds, or whose loading card says it failed, is not
  taken: the command fails saying so, and writes no picture and no label, so
  a loading screen is never shot or cached. Labels from before this rule never
  match, so their pictures are taken again.
- `--size` defaults to 1440x900; `--phone` is 390x844. `--dark` asks the
  page for its dark theme (**new**: `shot.coax` takes no theme today).
- `--eval JS` runs once before the capture and prints what it returns as
  `eval VALUE`.
- `--as-me` signs the page in as the profile's account, so a private realm
  you own is taken as you see it. Without it the page is taken as a visitor.
  This is how `realm shot` from the CLI v1 spec works; `realm shot` becomes
  a short name for `labs shot /realms/SLUG --as-me`.
- Prints `out PATH`, `size WxH`, `label PATH.json`.
- Uses: `coax/web/shot.coax`'s `shot` and `coax/lib/net/cdp.coax`.

### `labs clip`

```text
openagents labs clip URL OUT.mp4 [--size WxH | --phone | --square | --vertical]
                                 [--seconds S] [--fps N] [--skip S] [--eval JS]
                                 [--stage NAME] [--frames DIR] [--fresh]
```

- Records `URL` in clip mode: each frame is one fixed step of game time, so
  a slow machine never makes it stutter and the same stage gives the same
  clip.
- `--stage NAME` adds `?stage=NAME&card=1` to the URL. Without it, the clip
  starts where the page starts.
- `--square` is 1080x1080 and `--vertical` is 1080x1920, from the same run
  (**new** in `clip.coax`: today it takes one size).
- `--frames DIR` keeps the PNG frames.
- Prints `out`, `frames N`, `seconds`, `label`.
- Uses: `coax/web/clip.coax`'s `record`, host.js clip mode, ffmpeg.

### `labs frames`

```text
openagents labs frames VIDEO.mp4 OUTDIR --at T[,T...] | --every S | --strip N
```

- Writes one PNG per time (`--at 1.5,12,1:26`), one every `S` seconds, or a
  strip of `N` evenly spaced frames joined into `OUTDIR/strip.png`.
- Prints one `frame T PATH` line each.
- Uses: ffmpeg through `coax/lib/sys/proc.coax`; `coax/lib/image/png.coax`
  for the strip (**new**: a horizontal join).

### `labs cut`

```text
openagents labs cut FILE.cut OUT.mp4 [--quality preview|review|master]
                                     [--captions] [--check] [--fresh]
```

A `.cut` file is Video Lab's `cut.v1` in line form (the format is in
[cut-format.md](cut-format.md)):

```text
cut EPISODE 291
size 1920x1080
fps 30
music audio/theme.m4a gain=-14 duck=-8
voice audio/vo.m4a

shot open   src=clips/belt.mp4      in=0    dur=4.5  xfade=0.5
shot portal src=clips/portal.mp4    in=2.2  dur=6    xfade=0.75 title="THE PORTAL"
shot grid   src=https://openagents.com/realms/orbit-league?stage=highlight  dur=5
```

- `--check` only checks the file and prints its problems, one per line,
  `problem LINE REASON`. Every render runs the same checks first.
- Checks (Video Lab's `inspect`, the ones that need no private tools): every
  source exists or is a URL; in point plus duration fits inside the source;
  a crossfade is shorter than both shots it joins; a title fits the safe
  area at the cut's size; the total length is printed. A shot whose `src` is
  a URL is recorded with `labs clip` first and cached by its label.
- `--quality preview` is quarter size and fast; `review` is the cut's size;
  `master` adds a ProRes file beside the MP4.
- `--captions` runs `openagents transcribe --srt` on the voice track and
  burns the captions in. It never edits the transcribe domain; it calls it.
- Prints `out`, `seconds`, `shots N`, `label`.
- Uses: `labs clip`, ffmpeg (concat, xfade, drawtext, subtitles),
  `openagents transcribe`, `apps/openagents-web/font.coax` for titles
  (**new**: titles drawn as PNG overlays in Filament, as the deck draws them).

### `labs deck`

```text
openagents labs deck FILE.deck OUT.pdf [--check] [--fresh]
```

- The deck tool as it is, with `--check` to parse the file without taking
  scenes.
- Prints `out`, `slides N`, `scenes N` (and how many were reused).
- Uses: `coax/web/deck.coax`, `docs/web/decks.md`.

### `labs board`

```text
openagents labs board FILE.board OUTDIR [--fresh]
```

A `.board` file lists pages:

```text
board OPENAGENTS WEB
base https://openagents.com
sizes desktop phone
themes light dark
page /                 HOME
page /realms           REALMS
page /realms/everglade EVERGLADE wait=9000
```

- Takes each page at each size and theme, then writes `OUTDIR/index.html`
  (a static grid of the pictures, no scripts) and `OUTDIR/board.json`.
- Prints one `page NAME SIZE THEME PATH` line each.
- Uses: `labs shot` in a loop. The pictures are what `coax/web/goldens`
  keeps for our own pages, but the board is for any site and checks nothing.

### `labs playtest`

```text
openagents labs playtest GAME [--set NAME=VALUE ...] [--seeds N] [--from SEED]
                              [--against FILE.json]
openagents labs playtest GAME --tunables
```

- `GAME` is `icecream` first. Each game that has rules and a bot in Coax
  registers here.
- `--tunables` lists every tunable with its default and bounds
  (`verse/icecream/tuning.coax`, `TUNABLE_BOUNDS`).
- `--set` changes tunables for this run. A value outside its bounds is
  refused with the bounds, exit 2.
- Runs the bot on `--seeds` seeded houses (default 24, as Game Lab did),
  each a fixed-step run of the rules with no window.
- Prints one line per seed (`seed N won|lost|timeout SECONDS`) and a summary:
  `won K of N`, median time, deaths, keys found. `--json` gives every run.
- `--against FILE.json` compares with a previous run's JSON and prints what
  changed (`won 21 → 17`).
- Uses: `verse/icecream/rules.coax` (`icecream_bot`, the rules step) and
  `tuning.coax`. **New**: a headless run loop that steps the rules and the
  bot to the end, and a seed list. `verse/icecream/replay.c` already steps
  the rules natively; the loop is that, without a trace.

### `labs compare`

```text
openagents labs compare A B [OUT.png|OUT.mp4] [--at T]
```

- Two outputs with labels: two PNGs side by side, two videos side by side
  with one clock, or two playtest JSON files as a table of what changed.
- Prints what differs in the labels (`input`, `commit`, flags) before
  anything else, so you can see why the outputs differ.
- Uses: ffmpeg `hstack`, `coax/lib/image/png.coax`.

### `labs suite` (phase 3)

```text
openagents labs suite FILE.suite list
openagents labs suite FILE.suite task CASE
openagents labs suite FILE.suite score CASE REPLY.json
openagents labs suite FILE.suite run --agent claude|codex [--model M] [--max-usd N] [--repeat N]
```

- A `.suite` is Agent Lab's `agent-suite.v1` in line form: cases, each with
  a task, an optional project snapshot, and checks.
- `list`, `task` and `score` are pure: they read files and print, with no
  process, file write or network. This is Agent Lab's replay harness.
- `run` gives each case to an agent through `coax/lib/agent/claude` or
  `coax/lib/agent/codex` with the user's own credential, in a fresh temporary
  directory, and scores the reply. `--max-usd` is a cap you set; the run
  stops before a case that would pass it. A run that did not finish is
  recorded as not finished, never as a failure or a pass.
- Prints `case ID passed|failed|unfinished CHECKS` lines and a summary.

## MCP and skills

Each `labs` verb is one row in the CLI's command table, so `openagents mcp`
offers it as a tool (`labs_shot`, `labs_clip`, `labs_playtest`, ...).
`labs_shot` and `labs_frames` return their PNGs as images, so an agent can
look at what it made. `openagents skill labs` writes a `SKILL.md` with the
loop: write the lab file, `--check`, make, look with `frames` or `shot`,
compare, commit the lab file.

## What is missing

| Piece | Where | Needed by |
| --- | --- | --- |
| `shot` and `record` as loadable functions, apart from each tool's `tool_main` | `coax/lib/media/shot.coax`, `clip.coax` (**new**, moved out of `coax/web/`) | every command |
| The CLI linking `cdp.coax` and its WebSocket client | `apps/openagents-cli/build.sh` | `shot`, `clip`, `deck`, `board` |
| Finding Chrome and ffmpeg, and exit 8 with the install command | `labs-rules.coax` (**new**, `#authority`) | every command |
| The output label and its `input` digest | `labs-rules.coax` | every command |
| Signing a headless page in as the profile | a NIP-98 login for `cdp.coax` (**new**) | `shot --as-me` |
| Several sizes from one clip run | `clip.coax` | `clip --square`, `--vertical` |
| The `.cut` and `.board` formats and their checks | `labs-cut.coax`, `labs-board.coax` (**new**) | `cut`, `board` |
| Titles drawn in Filament as PNG overlays | `font.coax` to PNG (**new**) | `cut` |
| A horizontal PNG join | `coax/lib/image/png.coax` | `frames --strip`, `compare` |
| A headless run loop and seed list for Ice Cream Man | `verse/icecream/run.coax` (**new**) | `playtest` |
| The `.suite` format and the replay scorer in Coax | `labs-suite.coax` (**new**) | `suite` |

## Plan

Each phase ships on its own and leaves `coax/web/tool.sh` working.

1. **Wrap what exists.** Move `shot` and `record` into `coax/lib/media/`,
   keep `tool.sh shot|clip` as callers, and add `labs shot`, `labs clip`
   and `labs deck` with labels, caching and exit 8. `realm shot` becomes
   the short name. Check: the same stage clipped twice gives the same
   output digest.
2. **Make things.** `labs frames`, `labs cut` with `--check` and
   `--captions`, and `labs compare` for images and videos. Check: a fixture
   cut of three public clips renders at preview quality, and each refusal
   has a test with its sentence.
3. **Play and score.** `labs playtest icecream` with `--tunables`, `--set`
   and `--against`, and `labs board`. Check: a run on a traced setup matches
   the traces in `verse/icecream/fixtures/*.trace.gz` step for step, as
   `verse/icecream/replay.c` already checks, and the same seeds give the same
   summary twice.
4. **Agents.** `labs suite list|task|score`, then `run` with `--max-usd`.
   Check: the 2026-10-17 replay suite, ported from
   `git show 0b401561a0^:crates/agent-lab/fixtures/replay-2026-10-17.json`,
   scores the same cases the same way.
5. **MCP and skill.** The command-table rows, `openagents skill labs`, and a
   run where Claude Code makes a clip of a realm, looks at its frames, and
   cuts it into a short with captions.

## What shipped

Phases 1 to 3, on 2026-11-01. The code is `apps/openagents-cli/labs.coax`
(the commands) and `labs-rules.coax` (formats, checks and the summary, with
tests) and `coax/lib/media/media.coax` (shot, clip and deck as loadable
code). `coax/web/tool.sh shot|clip|deck` call the same
code. Verified end to end against openagents.com: the same stage clipped
twice gives the same output digest; a fixture cut (a title card, a local
clip with a title, a recorded URL, a card over a clip, music, a voice and
`--captions`) renders at preview, review and master; the Video Lab cuts in
`~/work/film-out/cut` read and check.

Where the build differs from the text above:

- `labs-rules.coax` is not `#authority`; it has tests but no mutation run yet.
- Strips and side-by-sides are joined with ffmpeg's `hstack`, not
  `coax/lib/image/png.coax`.
- `--square` and `--vertical` are sizes for one run, not crops from one run.
- `--as-me` refuses: the site has no NIP-98 sign-in for a headless page yet.
  A shot is a visitor's, so a realm must be unlisted or public for one
  (`openagents realm visibility SLUG unlisted`); `realm shot` of your private
  realm says so before starting Chrome.
- `deck` remakes the PDF each run and reuses its scene pictures from
  `DECK.captures/`.
- Titles and captions are drawn in Filament as SVG, taken as transparent
  PNGs by headless Chrome and overlaid by ffmpeg (this ffmpeg has no
  `drawtext` or `subtitles`). `--captions` reads `openagents transcribe
  --json`, not `--srt`.
- The music duck is a constant level while there is a voice, not a
  sidechain.
- A cut.v1 title's lines are drawn at their sizes; their colour, weight,
  spacing and timing, and `over`, `push` and script captions, are printed as
  notes.
- To stay inside the compiler's 128-unit budget for one program, the
  commands are one file (`labs.coax`), shot, clip and deck are one library
  (`media.coax`), and Ice Cream Man's headless run is in `labs-rules.coax`
  rather than `verse/icecream/run.coax`.

Left: `labs suite` (phase 4), the MCP rows and `openagents skill labs`
(phase 5), `--as-me`, and drawing the cut.v1 keys above.

## Decisions

Decided by the owner, 2026-11-01:

1. **Name.** `openagents labs`.
2. **Cut format.** `labs cut` reads both the old `cut.v1` JSON (for example
   `~/work/film-out/cut`) and the new line format, and writes the line
   format.
3. **Agent runs.** Deferred: `labs suite run` is not built until the owner
   decides who may spend model credit on it.

The questions as first asked:

1. **Name.** `openagents labs` (this spec) or a plainer `openagents make`?
   "Labs" keeps the pre-Nuke name; "make" says what the verbs do.
2. **Cut format.** A new line format (this spec, like `.deck`) or the old
   `cut.v1` JSON unchanged, so your existing cuts in `~/work/film-out/cut`
   render as they are? Recommendation: read both, write the line format.
3. **Agent runs.** May `labs suite run` ship to users, spending their own
   model credit with a cap they set, or should it stay owner-only at first?

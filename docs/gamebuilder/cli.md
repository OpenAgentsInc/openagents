# The gamebuilder command line

`gamebuilder` (and `gb`) does from a terminal what the editor does: make
levels and games, add things, set their fields, take them out, and carry out
an agent's `world.edit.v1` batch. It is stage C1 of
[programmatic-access.md](programmatic-access.md). Every command is a row of
the command table, `coax/commands.coax`. What a command does, what it refuses
and the words it uses are the core's, the same Coax the app runs.

## Install

```bash
scripts/install-cli.sh
```

This builds `cli/main.coax` against the pinned openagents tree and copies it
to `~/.local/bin/gamebuilder` (set `GAMEBUILDER_BIN` for another folder). It
also links `gb` to it, unless `gb` already means something in your shell.

The common clash is oh-my-zsh's git plugin, which defines `alias gb='git
branch'`. An alias always wins over a program on `PATH`, so a `gb` there would
never run. The installer asks your login shell what `gb` is. If it is anything
but this CLI, the installer leaves `gb` alone and says why. Two flags change
this:

- `--gb` installs `gb` anyway. You still have to remove the alias.
- `--no-gb` installs only `gamebuilder`.

It also writes the Claude skill made from the same table to
`~/.claude/skills/gamebuilder/SKILL.md`, for every project on this computer.
`scripts/build.sh` runs it after every build of the app (`GB_NO_INSTALL=1`
skips it), so the CLI on `PATH` is never older than the app. Run the script
again after pulling to update. It rebuilds only when the core, `cli/`, the
compiler, `openagents.lock` or the commit changed.

### Which build: `gamebuilder version`

```bash
gamebuilder version              # gamebuilder 0.1.0+f3ece6bd335f.table-1a2b3c4d
gamebuilder --remote version     # and gamebuilder.org's, and whether the two tables are the same
```

The version is the release, the commit the build was made from (with
`.dirty` when it had changes not committed) and a digest of the command
table: every row, every column. Two builds that run the same commands the
same way have the same table digest. The app's About (the Mac's menu, the
account sheet on a phone), `gamebuilder commands`, `/api/capabilities`
(`version` and `table`), `/api/openapi.json`, MCP's `initialize` and every
answer of `gamebuilder serve` (`X-GameBuilder-Version`,
`X-GameBuilder-Table`) say the same words.

When the two ends differ, they say so. A call sent with `--attach` or
`--remote` carries the digest of its row. The app or the site carries it
out only when its own row is the same, and otherwise refuses it as
`version-mismatch`, naming both builds. When only other rows differ, the
call is carried out and `gamebuilder` notes on stderr that the app or the
site was built from another table. `gamebuilder --remote version` exits 0
when the site's table is this build's and 1 when it isn't;
`deploy/api/deploy.sh` runs it after a deploy, and
`.github/workflows/hosted.yml` every day.

## Log in

```bash
gamebuilder login
```

At a terminal this opens `https://gamebuilder.org/login/cli` in your browser.
Log in there with your email if you aren't already, press **make a code**, then
press **copy** and paste the code where the terminal asks (it isn't echoed).
This computer is then logged in. It is the same account as the app and
gamebuilder.org.

- `gamebuilder login --browser` asks for the browser's code even with no
  terminal, and `--browser --code CODE` passes it as a flag.
  `GAMEBUILDER_NO_BROWSER=1` names the page without opening a browser, for a
  computer you reach over SSH: open the page on any computer you are logged in
  on and paste the code here.
- `gamebuilder login --email YOU` is the app's own login instead: type your
  email, then the code we email you. A script with no one at the keyboard can
  run `login --email YOU`, then `login --code CODE` within ten minutes. With no
  terminal and neither flag, a bare `login` reads the email on stdin.
- `gamebuilder whoami` asks the server which account you are, with its email.
- `gamebuilder logout` ends this computer's login on the server and forgets
  it.
- `--profile NAME` (or `GAMEBUILDER_PROFILE`) keeps a second login, for
  example to test as someone who doesn't own a game. `--origin` (or
  `GAMEBUILDER_ORIGIN`) names another server.

The login is kept in `~/.openagents/gamebuilder/cli-login-PROFILE` (mode
0600). Its access token lasts an hour and is renewed with its refresh token
when needed.

If you never logged in here but the GameBuilder app is logged in on this Mac,
the CLI uses the app's login while the app's access token lasts. It only reads
the app's file. It never uses the app's refresh token, because that token is
bound to this Mac's Secure Enclave and using it twice would end the app's
login.

### Log in from a browser

The page is GameBuilder's, on gamebuilder.org, with a copy button, so nothing
is selected by hand. How it works and why it is safe:

1. `/login/cli` is the site's own email-code login (`web/login.coax`). Once
   you are logged in there, **make a code** asks `POST /api/auth/cli/code`.
   The service makes a one-time code of 32 hex digits (128 random bits), keeps
   only its SHA-256, and ties it to that browser session. The page shows it with
   **copy**.
2. The CLI sends the code to `POST /api/auth/cli/redeem` with no Origin, as an
   app. The service spends it once and answers with the CLI's own app login:
   an access and refresh token pair, kept in `cli-login-PROFILE` as before.

What keeps it safe (openagents `INVARIANTS.md`, "A command line's login from a
browser is the browser's email login, no more"):

- The CLI's login is exactly the browser's email login: level L1, via that
  same email, with the browser's own email-proof time. A code is not a fresh
  email proof, so it can't unfreeze, merge or delete an account. It links no
  key or device, so the rule that only an L2 login (a key or passkey) may add a
  way in is untouched. A browser that logged in with a key can't make a code.
- Only a page of gamebuilder.org or openagents.com, logged in by email with
  its cookie, can make a code. Only an app (no Origin) can spend one, so a page
  never holds app tokens.
- A code works once, for five minutes. Making another ends the one before. It
  is dead if the browser logs out first. Wrong guesses are rate-limited and
  128 bits can't be guessed anyway.
- The CLI never prints the code or the tokens, and reads the code with echo
  off.

Why this design: it is the smallest change that gives a copy button on
gamebuilder.org. Nothing in the URL ties the page to the terminal (no PKCE,
no loopback server), so the page can be opened on any computer and the code
pasted over SSH. The cost is that the code is a bearer secret for five
minutes: paste it only into a terminal you started. Its risk is the same as
the emailed code's, which a person can also be tricked into reading out.

Why the browser is the default at a terminal: the person is there to click
and paste, and the page is the login people already use on the site. With no
terminal (an agent or a script), a bare `login` keeps the email path, so
nothing opens a browser behind someone's back.

Before every command, the email your login proved goes to the core. That is
how a game's owners may change its own level (Scrappers Arena's level 0, for
example). Anyone else gets the core's refusal and can `copy-level` instead.

## Use

```text
gamebuilder [--dir DIR] [--attach] COMMAND [ARGS] [FLAGS]
gamebuilder help          every command, from the table
gamebuilder commands      the capability manifest (gamebuilder.commands.v1, JSON)
gamebuilder skill [--install [--global]]   the Claude skill, made from the table
gamebuilder mcp [--read-only] [--attach]    the commands as MCP tools on stdio
```

<!-- rows: cli -->
| Area | Commands |
| --- | --- |
| level | `list-levels`, `read-level LEVEL`, `read-state LEVEL`, `read-content LEVEL`, `new-level [--dry-run] [--key K]`, `copy-level LEVEL [--dry-run] [--key K]`, `write-content LEVEL REVISION FILE\|'world.doc.v1 ...' [--dry-run] [--key K]`, `delete-level LEVEL [--dry-run] [--key K]`, `rename-level LEVEL NAME [--dry-run] [--key K]`, `move-level LEVEL GAME [--dry-run] [--key K]` |
| game | `list-games`, `list-templates`, `new-game TEMPLATE [--dry-run] [--key K]`, `rename-game GAME NAME [--dry-run] [--key K]`, `read-game GAME`, `delete-game GAME [--dry-run] [--key K]`, `owners GAME`, `set-owners GAME EMAIL[,EMAIL...] [--dry-run] [--key K]` |
| edit | `apply-edits LEVEL BATCH.json\|'{...}' [--agent NAME] [--dry-run] [--key K]`, `add LEVEL KIND [--x CM --z CM] [--agent NAME] [--dry-run] [--key K]`, `set-field LEVEL THING FIELD VALUE [--agent NAME] [--dry-run] [--key K]`, `delete LEVEL THING [--agent NAME] [--dry-run] [--key K]`, `list-palette LEVEL`, `list-fields`, `claim LEVEL box:X0,Z0,X1,Z1\|things:ID,ID --agent NAME [--dry-run] [--key K]`, `claims LEVEL`, `release LEVEL [--agent NAME] [--dry-run] [--key K]` |
| play | `list-modes`, `play-test LEVEL [--mode MODE] [--size N] [--seconds S] [--seed N]`, `list-scrappers` |
| account | `whoami [--profile NAME]`, `login [--browser] [--email YOU] [--code CODE]`, `logout [--profile NAME]` |
| engine | `engine`, `engine-list`, `engine-read CHAPTER`, `triage REQUEST`, `engine-ready`, `engine-mine` |
| view | `changes LEVEL`, `show-changes LEVEL`, `dismiss-changes LEVEL`, `pictures LEVEL`, `picture LEVEL REVISION before\|after\|playtest` |
| agent | `agents`, `agent-read AGENT`, `agent-events AGENT AFTER`, `agent-new GAME ROLE JOB [--dry-run] [--key K]`, `agent-message AGENT MESSAGE [--dry-run] [--key K]`, `agent-answer AGENT ANSWER [--dry-run] [--key K]`, `agent-stop AGENT [--dry-run] [--key K]`, `agent-undo AGENT [--dry-run] [--key K]`, `agent-report REPORT.json\|'{...}' [--dry-run] [--key K]`, `agent-picture AGENT PICTURE.json\|'{...}' [--dry-run] [--key K]`, `computers`, `computer-report STATUS.json\|'{...}' [--dry-run] [--key K]`, `agent-new-on COMPUTER GAME ROLE JOB [--dry-run] [--key K]` |
| code | `code-brief REPO` |
| site | `funnel` |
| publish | `publish LEVEL NAME [--from CODE] [--dry-run] [--key K]`, `published`, `versions PUBLISHED`, `set-live PUBLISHED VERSION [--dry-run] [--key K]`, `unpublish PUBLISHED [--dry-run] [--key K]`, `remove-published PUBLISHED [--dry-run] [--key K]`, `feedback PUBLISHED`, `feedback-hide FEEDBACK [--dry-run] [--key K]`, `block FEEDBACK [--dry-run] [--key K]`, `report FEEDBACK [--dry-run] [--key K]` |
<!-- /rows -->

## Agents: the skill

`gamebuilder skill` prints a Claude skill (`SKILL.md`) made from the command
table: when to use it, how to edit a world, the `world.edit.v1` batch with
examples, the limits with their numbers, every command, and what to do when a
world needs more than the commands. It can't drift from the commands:
`gb_skill` writes it from the rows, and a check holds that it names every row.

- `gamebuilder skill --install` writes it to `.claude/skills/gamebuilder/SKILL.md`
  in the folder you are in. Claude Code there picks it up.
- `--install --global` writes it to `~/.claude/skills/gamebuilder/SKILL.md`,
  for every project on this computer.
- This repo keeps its own copy at `.claude/skills/gamebuilder/SKILL.md`.
  `cli/check.sh` fails when that copy differs from the table's; run
  `build/cli/gamebuilder skill --install` from the repo's root to update it.

The skill tells an agent never to log in for the person. Only the person logs
in, at gamebuilder.org/login/cli, and pastes the code.

## Agents: MCP

`gamebuilder mcp` is an MCP server on stdio: JSON-RPC 2.0, one request a line.
Its tools are the command table's rows, so any agent that speaks MCP can build
worlds with them. Add it to Claude Code with:

```bash
claude mcp add gamebuilder -- gamebuilder mcp
```

Add `--dir DIR` after `mcp` to work in another folder, as the CLI does.

- **Tools.** Each row offered on MCP is a tool named `gb_` and the verb, with
  `_` for `-` (`gb_read_level`, `gb_apply_edits`, `gb_engine`). That is 52
  tools. The account rows aren't tools: only the person logs in.
- **Schemas.** Each tool's input schema is made from its row's argument kinds
  and their bounds: a level is 0 to 9999, `x` and `z` are within 20000 cm, a
  template, kind or field is one of its words. `dry_run` is on every tool that
  changes something. Properties the schema doesn't list are refused.
- **Descriptions.** Each description is the row's help, then what it does and
  who must agree, such as `[edit; only on a level or game you own]`. Reads are
  marked `readOnlyHint`, and `gb_delete` is marked `destructiveHint`.
- **Instructions.** The server's instructions tell the agent how to edit a
  world, the order of a batch's fields, and when and how to propose an engine
  change.
- **Batches.** `gb_apply_edits` takes the batch as a JSON object or as a
  string, or a batch file's path.
- **Calls.** A call is read by the same `gb_args_read` as the CLI and carried
  out by the same `gb_run`, in the same process. The tool's text is exactly
  the JSON the CLI would print, and `isError` is true when the row refused it.
- **`--read-only`** offers only the reads.

The server is in `coax/mcp.coax`, and `cli/main.coax` runs it around the same
`cli_exec` as the CLI's commands. The login is the CLI's: an agent acts as
whoever is logged in on this computer, and owns what they own.

With 34 tools, every one is listed. A find tool and a call tool, as the
openagents CLI has, come when the table is long enough to need them.

### Each session is an agent

Each MCP session is an attached agent (`docs/agents.md`, "Built: attached
agents"). It gets a record beside the levels, named by the client's name, and
each call is logged to it. `gamebuilder agents` lists it. `agent-stop ID` cuts
it off, and its later calls are refused in words. GameBuilder can't stop its
process; you end that in the agent. With `--attach`, its batches are its own
turn, and it gets an `undo-turn` tool that takes back only its own batch.

- **Over HTTP**, `POST /mcp`'s `initialize` answers an `Mcp-Session-Id`
  header. Send it back with each call, and `DELETE /mcp` with it to end the
  session.
- **`--no-agent`** is a session with no record.
- **A one-off call** makes no record unless you name an agent:

```bash
gamebuilder --as-agent codex add 1 block          # logged to codex's record
gamebuilder --attach --as-agent codex undo-turn 1 # takes back codex's last batch
```

`NAME` is 1 to 16 letters, digits, `.`, `-` and `_`. Over HTTP, the
`GameBuilder-Agent: NAME` header does the same.

## Engine upgrades

GameBuilder is built on the OpenAgents Metaverse Engine and Coax. Both are open
source at [OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents).
`gamebuilder engine` answers which commit GameBuilder is built on
(`openagents.lock`, written into the CLI when it is built), with a link to the
source at that commit, the engine's parts, and the steps to propose a change.

`gamebuilder engine-list` answers what the engine can do: every chapter of
the engine's book at that commit (103 at the commit pinned on 2026-11-17),
each with its number, title, part, engine folder and status: implemented,
partly implemented or design draft. `gamebuilder engine-read N` answers one
chapter: a summary of up to 300 bytes, up to 4 source files, and what its
status means for a request. Both work with no network and no openagents
checkout: the list is `coax/engine-list.coax`, which
`tools/engine-list.coax` makes from the book and `scripts/pin.sh bump` makes
again. The list is the book. What the engine does with no chapter is not in
it.

`gamebuilder triage "REQUEST"` answers where a request in the person's words
belongs (`gamebuilder.triage.v1`):

| `fit` | What it means |
| --- | --- |
| `commands` | The commands do it. `command` names the one to start with |
| `gamebuilder` | It needs GameBuilder's own code: a new kind of thing, a template, a rule or the editor |
| `engine-has` | The engine does it. GameBuilder does not use it yet |
| `engine-partly` | The engine does part of it. Read the chapter it names |
| `engine-needs` | The engine can't do it yet. `offer` is what to say to the person |

It asks OpenAgents' decision model once a call, so it needs the network.
`"sure":false` means the model was under 75% sure: check `engine-list`
before you act on it. A request is up to 1,000 bytes. The call is signed
with a key the command line makes on first use and keeps in
`~/.openagents/gamebuilder/decide-key-PROFILE` (mode 0600); it is no login.
The decision door limits each key by the minute and the day.

`gamebuilder engine-ready` answers what this computer needs before an engine
pull request can be opened: git, gh, gh's login, push access or your own
fork, and Claude Code for a Code agent. Each one missing comes with the one
command that fixes it. It says whether your branch goes to the repository or
to your fork. It reads only: it never logs in or forks for you.

`gamebuilder engine-mine` lists your own pull requests at
OpenAgentsInc/openagents (the newest 20) and says, for a merged one, whether
the engine commit GameBuilder is built on holds it yet.

The design for what comes next (rewards, and what openagents needs for
outside pull requests) is [engine-upgrades.md](engine-upgrades.md).

When a world needs the engine to do something it can't, such as water that
reflects the sky, a new kind of collision or a camera move, the skill has an
agent run `triage` first, say the offer, and on a yes open a pull request
there:

1. Make sure it is the engine's to do. What is in a level is the commands'.
   New kinds of thing, templates and game rules are GameBuilder's own code.
   How worlds draw, move and collide, and the world formats, are the engine's.
2. Tell the person what you would change and why. An agent asks before it
   opens a pull request in the person's name.
3. Run `gamebuilder engine-ready`. Clone OpenAgentsInc/openagents and branch
   from `main`. Without push access, which is the usual case,
   `gh repo fork OpenAgentsInc/openagents --remote --remote-name fork` in the
   clone makes your fork and adds it as the remote `fork`.
4. Follow its `AGENTS.md`. Add the feature as an engine primitive: its own
   module, its own `#test` checks, and a section in the engine's docs, usable
   by any world.
5. Run the checks it names.
6. Push the branch (`git push fork BRANCH`, or to the repository if you
   may), then `gh pr create --repo OpenAgentsInc/openagents`, saying what a
   GameBuilder world needs it for.
7. Once it is merged, GameBuilder moves its pin (`scripts/pin.sh bump`) and
   uses it. `gamebuilder engine-mine` says whether it has yet.

The steps are one list in `coax/commands.coax` (`GB_ENGINE_STEP`), which both
`gamebuilder engine` and the skill print.

Arguments:

- Level 0 is Scrappers Arena's own level. It is not yours to change, so
  `copy-level 0` makes your own copy.
- A thing is its id, from `read-level`.
- A field is `kind`, `east`, `up`, `north`, `turn`, `scale`, `width`, `height`
  or `depth`.
- `BATCH` is a file, or the JSON itself. Read the level first: the batch
  carries the revision it read, and the batch is refused whole if the level
  changed since.
- Every command that changes something takes `--dry-run`. It carries the
  command out, answers, and writes nothing.
- Every command that changes something takes `--key K` too: 1 to 64 letters,
  digits, `.`, `_` or `-`, starting with a letter or digit. The answer is kept
  in `DIR/results/K`, and sending the same command with the same key again
  answers what was kept without doing it twice, so a call can be retried
  safely. A key is one call's: the same key with another command is refused.
  A dry run keeps nothing. MCP takes the key as a `key` argument.
- `apply-edits`, `add`, `set-field`, `delete`, `claim` and `release` take
  `--agent NAME` too (1 to 32 letters, digits, `-`, `_` or `.`, starting with
  a letter or digit; MCP's `agent`): the agent making the change. Without it
  the change is the person's, and a claim never refuses the person's own
  change.

### Several agents on one level: claims

An agent claims what it is working on before it changes it, so two agents on
one level don't undo each other's work:

```bash
gb claim 3 box:-500,-500,500,500 --agent ann   # a box on the ground, in cm
gb claim 3 things:12,31 --agent bob            # these things, wherever they go
gb claims 3                                    # every claim on the level (gamebuilder.claims.v1)
gb release 3 --agent ann                       # ends ann's claims; without --agent, everyone's
```

- Another agent's change that touches a thing inside a claim, or would put
  one there, is refused whole: `{"ok":false,"error":"claimed","why":...}`,
  the why naming whose claim it is (409 over HTTP). A claim that overlaps
  another agent's is refused the same way. Claiming the same region again
  renews it.
- A claim ends when its agent releases it, when its turn ends (in the app's
  agents pane: done, stopped or failed), when its work is undone, or 600
  seconds after the agent last claimed or changed something
  (`CLAIM_SECONDS`), so a crashed agent's claim lapses by itself.
- Limits: a level holds 8 claims at once (`CLAIMS_MAX`), an agent holds 4
  (`CLAIMS_EACH`), a things claim names at most 16 things (`CLAIM_THINGS`),
  and a box's corners are within 10000 cm (`CLAIM_SIDE`).
- They are kept beside the level as `level-N.claims` (`gamebuilder.claims.v1`),
  which the CLI, `serve`, `serve --accounts` and the app all read and write,
  and the app draws each claim's box on the ground in its agent's colour.
  The rules are `coax/claims.coax`.

```bash
gb new-game adventure
gb add 2 tree --x 400 --z -200
gb set-field 2 31 height 600
gb read-level 2
```

`play-test` plays a level headless and answers `gamebuilder.playtest.v1`. An
arena is played by a match with bots in every seat, yours too: whether the
match ended and how, the winner, the score, knockouts, bots that stood still
for five seconds and where (`stuck`), and the goals, hoops, ball and health
packs that no bot came within 3 m of (`unreached`). It runs in fixed steps of
1/60 s from a seed, for at most 600 seconds of match time, so the same call on
the same level answers the same bytes every time. The defaults are `--mode
knockouts --size 3 --seconds 600` and the app's own seed. Every other game
type is walked through: a bot walks from your start, at 4 m a second on the
engine's walking grid (cells of 0.5 m or more, 16,384 at most), to the
nearest thing it has not reached yet, until it has reached all it can or the
seconds run out. Its `score` is what it reached and what it looked for (64
things at most), its `stuck` places are where it came nearest a thing it
found no way to, and `unreached` is what it never came within 3 m of. It
walks the level's things, not the type's own world round them (the village's
houses, the crates' room, the course, the circuit, open space). Both answer
`rev` (the revision played), `type`, `bot` (`match` or `walk-through`) and
`paths`: where each bot went, a point every second at first, at most 40 a bot
(past that every other point goes and the spacing doubles), in centimetres.
In the app the same play-test is also a picture of the level with those
paths on it (`picture LEVEL REV playtest`).

```bash
gb play-test 1 --mode scrapball --size 2 --seconds 60 --seed 7
```

Every answer is one line of JSON: `{"ok":true,...}`, or
`{"ok":false,"error":CODE,"why":WORDS}`. A limit that refused it is named,
with its number (`Turn is from -180 to 180°`). The exit code is:

- 0: done, or a dry run;
- 1: refused;
- 3: no such command.

## Which files

It reads and writes the files the app keeps, as the app does
(`host/Sources/Levels.swift`): `level-N.world`, `scrappers-arena.world` and
`game-N.game`. On the Mac they are in `~/Library/Application
Support/com.openagents.gamebuilder/Levels`, or the same folder inside the
sandbox's container when the App Store build made one. On Linux they are in
`$XDG_DATA_HOME/gamebuilder/Levels`, or `~/.local/share/gamebuilder/Levels`.
`--dir` or `GAMEBUILDER_DIR` names another folder.

Close a level in the app before changing its file here. The app does not read
a level's file again while it is open, and its next save would write over the
change. Or use `--attach`.

## The open level: --attach

`--attach` sends the command to the running app, which carries it out on the
level open there, as a press of its own button is: an edit lands as one step
that ⌘Z takes back, and what it changed stays outlined in the terminal's
colour (yellow), with a ghost where a thing was, and is said under the
toolbar until the person dismisses it (`coax/changes.coax`). Attached, the
app's own commands work too: `undo`, `redo`, `set-tool`, `focus`, `copy`,
`cut`, `paste` and `duplicate` (each names the open level, and a thing where
it acts on one), and `changes`, `show-changes` (Show me: the view frames
what was changed) and `dismiss-changes` (takes the highlight off). On a
level's file there is no highlight: `changes` answers that nothing is
highlighted, and `show-changes` is refused with that reason. Every
`apply-edits` answer says what the batch changed, in words (`"summary":
"added 2 blocks, moved 1, removed 2"`) and by thing (`"things"`), so a dry
run says what a batch would do. A level that isn't the open one is refused. `--dry-run`
carries the command out on a copy and changes nothing in the app. `gamebuilder
mcp --attach` offers the same to an agent.

```bash
gb --attach add 3 block --x 400 --z 0
gb --attach undo 3
```

How it works (`coax/attach.coax`): the app listens on a Unix socket in its own
temporary folder (inside its sandbox on the Mac; `$XDG_RUNTIME_DIR` on Linux),
readable only by you, and answers only a caller running as you. It writes the
socket's path into the Levels folder as `attach`, which is how the CLI finds
it. A call is one line, `{"verb":VERB,"args":{...}}` with the arguments by
their MCP names, and the answer is one line, the exit code and the JSON. The
app polls for calls on a timer, so nothing in it waits on the terminal. When
the app isn't open the CLI says so. `tools/attach-check.coax` (run by
`cli/check.sh`) plays the app's part on a real socket and runs the CLI
against it.

## Over HTTP: serve

`gamebuilder serve` offers the same commands as HTTP routes on this computer
(127.0.0.1 only), on the folder's files and as your login, until you stop it.
Each answer is the same JSON the CLI prints; the status says what kind it is.

```bash
gamebuilder serve                      # port 8811; --port 0 picks a free one
curl -s localhost:8811/api/levels/3    # read-level 3
curl -s -X POST localhost:8811/api/levels/3 -d @ramps.json    # apply-edits: the body is the batch
curl -s -X POST localhost:8811/api/rows/add -d '{"level":3,"kind":"block","x":400,"z":0}'
curl -s localhost:8811/api/openapi.json   # every route, from the command table
```

| Route | What |
| --- | --- |
| `GET /api/capabilities` | Every command, as `gamebuilder commands` prints it |
| `GET /api/openapi.json` | OpenAPI 3.1 for every route |
| `GET /api/levels/N` | `read-level N` |
| `POST /api/levels/N` | `apply-edits N`, the body a `world.edit.v1` batch |
| `POST /api/rows/VERB` | Any command offered over HTTP; the body is its arguments by their MCP names, `{}` for none |
| `POST /mcp` | MCP over HTTP: one JSON-RPC message a request, answered as JSON (no event stream), the same tools as `gamebuilder mcp` |

200 is done. A refusal keeps the CLI's JSON and says its kind: 400 the
arguments don't read, 403 the level isn't yours (or `--read-only`), 404 no
such level or thing, 409 stale (read it again), 422 the edit can't be made,
503 a match or a drag is going on. One request a connection, one at a time;
a request has five seconds to arrive whole, with a `Content-Length` and a
body of at most 262,144 bytes. `--read-only` serves only the reads. It answers
only this computer: a request whose `Host` isn't 127.0.0.1, localhost or
[::1], or that a web page elsewhere sent (its `Origin`), is refused with 403,
so a page you open can't change your levels through it. An agent on this
computer can use it as an MCP server over HTTP:
`claude mcp add --transport http gamebuilder http://127.0.0.1:8811/mcp`. The routes
and statuses are `coax/api.coax`; the golden cases run through them too.

### A level's whole file: read-content and write-content

`read-content N` answers level N's file as the app writes it, with its
revision. `write-content N REVISION FILE` puts a file back whole: REVISION is
the one `read-content` gave (0 for a level that isn't there yet), and a level
that changed since is refused as stale (409 over HTTP). The file is a path, or
the text itself when it starts with `world.doc.v1 `. It is how a level moves
between your devices through your account, and it refuses level 0 (Scrappers
Arena's own), a file that isn't a level's, and `--attach` (it would take the
open level's history with it: close the level first).

```bash
gamebuilder read-content 3 | jq -r .content > level-3.world
gamebuilder write-content 7 0 level-3.world     # a new level 7 with level 3's things
```

### Agents: agents, agent-read, agent-events and the rest

An agent is a record with an event log, kept beside the levels as
`agent-ID.agent` and `agent-ID.events` (`docs/agents.md`, "The record and its
events"; `coax/agent-record.coax`). `agents` lists them, waiting for you
first, then the newest, each with the state it shows in words, how long it
has been in it and where it runs. `agent-read ID` is one of them.
`agent-events ID AFTER` is its events after event AFTER (0: from the first),
50 a read, with `next` to read on from.

`agent-new GAME ROLE JOB` queues a new agent on a game of yours on this
computer (ROLE is `build` or `playtest` today; the job is at most 240 bytes),
and GameBuilder runs it with your own Claude Code. `agent-message ID TEXT`
sends it a message (at most 1000 bytes), `agent-answer ID ANSWER` answers the
question it waits on with one of the answers it offered, and `agent-stop ID`
stops it. `agent-undo ID` is the app's: it takes back an agent's last edit on
the level open there, and from the folder's files it says which level and why
it can't. `agent-report REPORT` is how an agent's computer reports it to your
account: its record and the events after those kept there, at most 20 a
report, refused as stale when it doesn't go on from them.

```bash
gamebuilder agent-new 1 build "add a second floor"
gamebuilder agents | jq -r '.agents[] | "\(.agent.id) \(.words) \(.for) \(.where)"'
gamebuilder agent-events a19a91a37500-0 0
```

On the site's server (`serve --accounts`) your computers' agents are
reported and read (`agents`, `agent-read`, `agent-events`, `agent-report`),
and queued on a computer (`agent-new-on`), never told anything or stopped
there: they run on your computer. Cloud agents are started and told things
there (`docs/agents.md`, "Built: a cloud agent"). The app reports this Mac's
agents there itself while you are logged in and its Agents pane's **Kept on
your account** is on, so `gamebuilder.org/agents` shows them; the app and the
CLI keep the same files, so `gamebuilder agents` shows the agents the app
started, and a message sent with `agent-message` reaches the app's pane
within two seconds.

### The resident service: resident

`gamebuilder resident` is GameBuilder's resident service
(`docs/agents.md`, "Built: the resident service, computers and cost"): it
runs the agents queued on this computer with your own Claude Code, with the
app open or not, one at a time, at no charge from us. It says it is on in
its first line, and writes this computer beside the levels
(`computer-ID.computer`), which `computers` lists in words. `agent-new-on
COMPUTER GAME ROLE JOB` queues an agent on it by its name; the App Store
build's Agents pane and `gamebuilder.org/agents` do the same.

```bash
gamebuilder resident install          # a login item: it starts when you log in
gamebuilder computers | jq -r '.computers[] | "\(.computer.name): \(.words)"'
gamebuilder agent-new-on "Studio Mac" 1 build "add a second floor"
gamebuilder resident stop             # the agent it runs stops, and says you quit the service
```

Logged in (`gamebuilder login`), it puts this computer on your account every
10 seconds (`computer-report`), takes the agents queued for it there, and
reports its own; `--server URL` names a `serve --accounts` on this computer
instead of gamebuilder.org. It takes no key: the keys a coding agent would
bill are taken out of `claude`'s environment, and a session that would bill
one is stopped. One runs on a folder at a time; `resident status` says
whether it is on.

### Hosted: serve --accounts

`gamebuilder serve --accounts DIR` is the same server for many people: every
route but discovery (`/api/capabilities`, `/api/openapi.json`) wants
`Authorization: Bearer TOKEN`, a GameBuilder login's access token, which the
login service at `--origin` (gamebuilder.org unless named) names with
`GET /api/auth/account`. Each account's levels and games are the files in
`DIR/ACCOUNT`, and no other account's. No login, or one the service doesn't
know, is 401 with `WWW-Authenticate: Bearer`; the service not answering is
503. It answers any `Host` (it runs behind a load balancer), but a browser's
call only from `--origin`'s own pages. Such a page (gamebuilder.org/agents)
may send its login's cookie instead of a token: the server passes the cookie,
whole, to the login service the same way, and only with that page's Origin,
which a browser always sends on a POST. It still binds 127.0.0.1: in a
container a proxy beside it takes the outside's connections. Nothing is kept
of a caller but their levels and agents: the token or cookie is wiped once
the service has answered.
`tools/hosted-check.coax` holds all of it, standing in for the login service.
It runs at gamebuilder.org (since 2026-11-17): `deploy/api/deploy.sh` makes
its Cloud Run service with the accounts' files in a storage bucket, and the
site's load balancer sends `/api/rows/*`, `/api/levels/*`,
`/api/capabilities`, `/api/openapi.json`, `/api/count` (the page's counts of
its core loop: no login, totals a day in `DIR/.loop`; docs/web-world.md,
"Counting the loop"), `/api/play/*`, `/g/*` and `/mcp` to it.

**Published games.** On the hosted server the publish rows keep what they
publish for anyone to play (issue 17; docs/publishing.md, "What is built"):
`GET /g/CODE` is the link's page (the site's own `/play` page, fetched from
`--origin`, with the game's name and picture in its head),
`GET /api/play/CODE` what the link plays (`gamebuilder.play.v1`: the name,
the game type, the version and the level file; nothing of the account) and
`POST /api/play/CODE/report` a player's report (`{"reason":"offensive" |
"personal" | "spam" | "other"}`, one line in `DIR/.reports`) and
`POST /api/play/CODE/played` one count for the game (`{"fact":"opened" |
"minute" | "remix"}`: its link opened, played a minute, Remix pressed;
totals only, in `DIR/.plays`; docs/publishing.md, "Play counts and remix
lineage"). None of the four wants a login. `published` then says each
game's `plays`, and `from` for a remix published with `publish --from
CODE`; the operator's `funnel` says the site's in its `games` section. A publish row called for an agent (`GameBuilder-Agent`
or `/mcp`) is 403. The links it says start with the `GAMES_DOMAIN`
environment variable, or `--origin` when that is unset. The operator takes a
game down by writing `DIR/.published/CODE.down`. Without `--accounts` the
three routes are 404: a device's published games are not played from it.
`tools/web-publish-check.coax` holds all of it.

### The account's levels: --remote

`--remote` sends a call to the hosted server instead of the files here, signed
in with `gamebuilder login`'s login: `POST ORIGIN/api/rows/VERB` with the
call's arguments, and the server's answer is the answer. It works for every
row the API offers, and for `gamebuilder mcp --remote`, so an agent on any
computer edits the levels the person's phone syncs. `--origin URL` names
another site, whose login it then uses. `--server http://127.0.0.1:PORT` keeps
the site's login and sends the calls to a `serve --accounts` on this computer
instead: a check before the site has the service. It only takes this
computer's addresses, so the login never goes elsewhere by a slip.
`scripts/sync-check.sh` runs stage C7's check that way: a server here, the
iPhone app on the simulator syncing to it, and a block added to the level you
have open. `--remote` can't go with `--attach`.

```bash
gamebuilder --remote list-levels
gamebuilder --remote read-content 3 | jq -r .content > level-3.world
claude mcp add gamebuilder -- gamebuilder mcp --remote
```

The app on a phone keeps its levels the same as the account's while it is
logged in (a pass every 10 seconds, `coax/sync.coax`). A level changed on only
one side goes to the other; the level on screen reopens when the account's
copy changed. When both changed, the account's keeps the number and the
phone's becomes a new level on both. Games are kept the same too, as games of
their type, and a level or game deleted on one side is deleted on the other
rather than brought back (`docs/web-world.md`, "The app has the same games").

## Not yet

- Remote MCP sign-in by OAuth discovery, for MCP clients that need it: for
  now the bearer is the OpenAgents login.
- Undo across runs. Each run is one step; the app's history is per window.

On Linux it builds and passes every check (`.github/workflows/linux.yml`; run
by hand in an Ubuntu 24.04 container until the org's Actions run).

`cli/check.sh` runs the CLI end to end in a folder of its own, and
`scripts/check.sh` runs it.

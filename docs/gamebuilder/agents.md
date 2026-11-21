# Agents: the page, where they run, and your own agent in the desktop app

Written 2026-11-17. Status: specification, with slices built: a build agent
started from the chat ("Built: the first build agent", below) through the
resident service, computers and cost (issue 11).
Sections marked **Built** say what exists in the openagents tree today, with
paths; **Was built** means it existed before the reset of
2026-10-27 and is readable in git at revision `853417f290`.

The money side (what a cloud agent costs us, what we charge, why this is
likely the largest line) is in [business/agents.md](business/agents.md).
What already exists for an agent changing a level is audited in
[agent-mode-audit.md](agent-mode-audit.md); this spec builds on it and does
not repeat it.

What is built of this as of 2026-11-17, why the desktop pane feels rough and
what to build next are in [agents-audit.md](agents-audit.md).

The requirement, in the owner's words: a page at
`https://gamebuilder.org/agents` where a person sees all their agents at a
glance and makes new ones: three agents working on that game, one on this
one, click any of them to see what it is doing. People who bring their own
agent (Claude, Codex, whatever runs on their computer) manage it easily
from the desktop app. People who have none, or want more, use ours in the
cloud and pay for it.

## What an agent is here

An **agent** is one piece of work on one game, done by a model that can
change the game, with a person able to watch, answer and stop it.

| An agent has | |
| --- | --- |
| A game | The one game it may change. Never more than one |
| A job | What it was asked to do, in the person's words |
| A kind | Build, art, playtest, fix, watch, or code (below) |
| A place | Where it runs: one of the person's computers, or our cloud |
| A state | Working, waiting for you, queued, paused, done, stopped, failed, offline |
| A history | Everything it said and did, and each change it made as a revision that can be undone |
| A limit | The most it may spend, and how long it may run |

| Kind | What it does | Needs a computer? |
| --- | --- | --- |
| **Build** | Changes the level and its rules: places, moves, sizes, removes, sets | No. Level edits are data |
| **Art** | Makes models, textures, sound and voices and puts them in the game | No for generated assets; yes to run a modelling tool |
| **Playtest** | Plays the game and reports what broke, what was dull, how long it took | Yes: it runs the game |
| **Fix** | Reads the feedback inbox (`publishing.md`) and proposes changes | No |
| **Watch** | Stays on: plays each new version, reads new feedback, keeps a list | Sometimes |
| **Code** | Changes the engine (OpenAgentsInc/openagents) or the app (this repository) itself, when a world needs what neither can do yet. Below, "A Code agent" | Yes: it runs commands |

"Needs a computer" decides the cost. An agent that only sends typed edits
runs in our own runner for the price of the model. One that plays the game,
runs a tool or compiles something gets a machine of its own.

**Built.** The typed edits an agent sends (`world.edit.v1`), the level as a
model reads it (`world.turn.v1`) and the plan it returns (`world.plan.v1`)
are in the engine (`metaverse-engine/editor/world-edit.coax`,
`coax/lib/agent/world/turn.coax`). The driver a playtest agent uses to walk
and act in a running game is `metaverse-engine/agent/drive.coax`. A batch
of edits names the revision it was made against and is refused whole when
that revision has moved on.

## The record and its events

**Built** (issue 4). An agent is a stored record with an append-only event
log. Both are written and read by one Coax module, `coax/agent-record.coax`,
which the desktop core, the `gamebuilder` CLI and its servers, and the
website load; none of them has its own copy of a field, a state or a state's
words. Nothing in the module touches a file or the clock: the surface hands it
bytes and the time.

On a computer the record is `agent-ID.agent` and its log `agent-ID.events`,
beside the levels. On the account the hosted server (`gamebuilder serve
--accounts`) keeps the same two files in the account's folder.

### `gamebuilder.agent.v1`, the record

One JSON object, its fields always in this order:

| Field | Holds |
| --- | --- |
| `schema` | `"gamebuilder.agent.v1"` |
| `id` | 1 to 32 of `a`-`z`, `0`-`9` and `-`, starting with a letter: `a`, the time it was made in hex, `-`, how many agents its folder had (`a19a91a37500-0`) |
| `game` | The game of yours it works on (`list-games`' number); 0 for a level in no game of yours. An agent is bound to a game |
| `title` | That game's name, at most 48 bytes ("My levels" for none), so a list can group without the game's file |
| `level` | The level it is working on, a detail of the game; 0 when none |
| `kind` | `build`, `playtest`, `art`, `fix`, `watch` or `code` (today a runner takes `build`, `playtest` and `code`) |
| `repo` | Only a Code agent's, right after `kind`: `openagents` (the engine) or `gamebuilder` (the app) |
| `job` | What it was asked to do, in the person's words: 1 to 240 bytes on one line |
| `place` | `local` (one of the person's computers), `cloud` or `attached` |
| `computer` | The computer's name as the person sees it, at most 48 bytes: `this Mac` |
| `runner` | What runs it: `claude` (the person's own Claude Code) |
| `state` | One of the states below, by its id |
| `since` | When it entered that state, in milliseconds since 1970 |
| `made`, `updated` | When it was made, and its last event, in milliseconds |
| `heard` | When the account last heard from its computer; 0 on the computer itself |
| `step` | The one-line current step, at most 120 bytes ("" when none) |
| `why` | Why it stopped or failed, at most 240 bytes ("" otherwise) |
| `limits` | `{"turns":N,"cents":N}`: model turns, and cents of the person's plan, a message may take |
| `events` | How many events its log has |

### `gamebuilder.agent.event.v1`, one event

One JSON object a line of the log, at most 8,192 bytes, numbered from 1 with
no gaps. Every event has `schema`, `agent` (its record's `id`), `n`, `at`
(milliseconds) and `type`, then the fields its type has, in this order:

| `type` | Fields after `type` |
| --- | --- |
| `message` | `from` (`person` or `agent`), `text` (at most 4,096 bytes) |
| `tool` | `tool` (its name), `args` (the arguments as it sent them, as text, at most 2,048 bytes), `ok`, `result` (a short result, at most 240 bytes), and for a play-test `level` and `after` (the level and the revision it played, whose picture its card shows) |
| `edit` | `level`, `before` and `after` (the level's revisions), `things` (the ids it touched, at most 64), `text` when its writer gave it (what it changed in words, the same words as the change's summary: "added 2 blocks (Block 29, Block 30)") |
| `ask` | `choices` (2 to 4 answers of at most 40 bytes), `text` (the question) |
| `answer` | `text` (the answer given) |
| `state` | `state` (the new state's id), `text` (why, for `stopped` and `failed`) |
| `usage` | `turns`, `cents`, `ms` |
| `error` | `text` |
| `worktree` | A Code agent's: `repo`, `base` (the origin/main commit its worktree was made from) |
| `branch` | `repo`, `branch` (its name, at most 128 bytes) |
| `pr` | `repo`, `branch`, `number`, `url` (`https://github.com/OpenAgentsInc/openagents/pull/N`, or the app's; nothing else) |
| `files` | `count` (files changed since `base`, committed or not, and new), `names` (at most 32 of them) |
| `check` | `name` (the check's command, at most 64 bytes), `ok` (it passed), `text` (a line of what it said) |
| `pin` | `url` (the engine pull request it follows), `text` (the job offered: the pin bump) |

A text or arguments longer than kept are cut on a whole character and the
event says `"cut":true`. Appending an event moves the record: `events` and
`updated` always, and a `state` event its `state`, `since` and `why`.

The log is only ever appended to, and read by cursor: the events after `N`,
at most 50 at a time, answered as `{"events":[...],"next":N,"more":BOOL}`
where `next` is the cursor to read on from. A last line without its line
break (a write cut short) is not an event yet. A line that is not the
agent's event `n` stops a read there and the answer says `"broken":N`; the log
is never rewritten.

### States and their words

| Id | Words shown | Means |
| --- | --- | --- |
| `working` | working | a turn is running |
| `waiting` | waiting for you | it asked a question and waits for your answer |
| `queued` | queued | it is ready and waits for its computer to have room |
| `paused` | paused | your own plan hit its limit; it goes on when the plan does |
| `done` | done | it finished and said so |
| `stopped` | stopped | you, or its limit, stopped it; what it changed stays |
| `failed` | failed | it could not go on, and says why |
| `offline` | offline | its computer stopped answering; what it last did is shown with its age |
| `unknown` | unknown | the connection was lost mid-turn and we can't tell whether the turn ended |

A view never shows a local agent's record as it was left when its computer
has gone quiet: a working, waiting, queued or paused agent whose computer
has not been heard from for two minutes (`AGR_OFFLINE_MS`) is shown offline,
with the age of what is shown, and never as done.

## Where an agent runs

| Place | Whose model | Who pays for the model | Who pays for the machine | When it is right |
| --- | --- | --- | --- | --- |
| **Your computer** | Yours: the coding agent installed and signed in there | You, to its maker | Nobody | You already pay for an agent. Free with us |
| **Our cloud, our model** | Ours, chosen by the router | You, in Credits | You, in Credits, when it needs one | You have no agent, or want more at once, or want it to carry on with the laptop shut |
| **Our cloud, your model** | Yours: you sign in inside the cloud computer, or give a key | You, to its maker | You, in Credits | You want your own agent running while you are away |
| **Someone else's computer** (later) | Open models on a Pylon | You, in Credits; we pay the machine's owner in sats | The same | Cheap work that need not stay private |

Rules that hold in every place:

- **Nothing moves by itself.** An agent on your computer that goes offline
  says so. It is never quietly restarted on our model at your cost. The
  page offers "carry on in the cloud" and the person chooses.
- **You never pick a model.** The router decides which agent, model and
  effort a job gets, from what is connected and what has room, and the page
  shows what it chose (openagents `docs/router/README.md`, the owner's
  decision of 2026-11-14). What a person sets is which accounts are
  connected, each computer's rules, and the limit.
- **An agent changes only a game its owner may change.** The server checks
  the account against the game on every edit. The app's own rule
  (`coax/games.coax`) is not the protection.
- **Agents work on the draft.** Publishing a version to players is a
  person's tap, unless the person has said in so many words that this agent
  may publish.

## The page: `gamebuilder.org/agents`

Logged in only. It is another view of the one page (`web/page.coax`), in a module of its own that only /agents loads (`web/site-agents.coax`, issue 30), with
its own address object in the bucket as `/login` and `/play` have. Black,
white and grey like the rest of the site; a game's picture is the colour.

### At a glance

```
AGENTS                                   4 working · 1 waiting for you · 212 credits today
[ + new agent ]

SCRAPPERS ARENA                                                         3 agents
  ● build     "add a second floor to the east tower"      working   2m   this mac
  ● playtest  "play 20 rounds of hoops, 3v3"              working   9m   cloud      38 cr
  ◐ art       "a crate that looks like a fruit box"       waiting for you   cloud   60 cr

SPLICEBOTS                                                              1 agent
  ● build     "block out the garage ring"                 working   31s  coderos

DONE TODAY                                                              6  ▸
COMPUTERS      this mac ● 2 of 4   coderos ● 1 of 4   cloud ● 2 running   [ pair a computer ]
```

| Part | What it shows |
| --- | --- |
| The top line | How many are working, how many wait on the person, and Credits spent today against the day's limit |
| A game's group | The game's picture and name, how many agents it has, and one row for each |
| A row | Kind; the job in the person's words; state; how long in that state; where it runs; Credits spent (nothing shown for an agent on the person's own model) |
| Waiting for you | Sorted to the top of its group, with what it is asking in one line. This is the row a person most needs to see |
| Done today | Folded. Each finished job, what changed, what it cost |
| Computers | Each paired computer: on or off, slots in use, which agents are signed in there. Cloud machines running now and what they cost an hour |

A row's state, exactly:

| State | Means | What the person can do |
| --- | --- | --- |
| Working | A turn is running | Open, stop |
| Waiting for you | It asked a question or wants permission | Answer, allow, deny |
| Queued | It is ready and the place it runs has no free slot | Wait, move it, or add a cloud agent |
| Paused | The person's own plan hit its limit; resumes at the time shown | Wait, or carry on in the cloud |
| Done | It finished and said so | Open, undo its changes, start another from it |
| Stopped | The person, or its limit, stopped it | Open, continue |
| Failed | It could not go on, with the reason | Open, retry |
| Offline | Its computer is unreachable. What it last did is shown with its age | Wait, or carry on in the cloud |
| Unknown | The connection was lost mid-turn and we cannot tell whether the turn ended | Open. Never shown as done |

### One agent

Tapping a row opens the agent beside the list (below it on a phone).

| Part | What it shows |
| --- | --- |
| The game, now | A picture of the game at the agent's latest revision, and **open in the editor** |
| What it is doing | The current step in one line, live |
| The conversation | What was asked, what it said, each tool it used. Long output is folded |
| Changes | Each revision it made: "moved 3 blocks, added 1" with **undo this** and **undo all of this agent's changes** |
| Questions and permissions | Allow or deny, answer in a line |
| A box to type in | Send it another message. It reads it at its next turn |
| Cost | Credits so far, the limit, and what the limit is made of (model, machine). Or "your own agent: no charge from us" |
| Where | The computer and the agent the router chose, in plain text. **Carry on in the cloud** or **move to this computer** where that is possible |
| Stop | Always there, always works. Stopping keeps the changes made so far |

### A new agent

One sheet, three fields, one button.

| Field | |
| --- | --- |
| **Game** | A game of the person's, or "a new game" |
| **What should it do?** | A line or a paragraph. The kind is worked out from it; the person can change it |
| **Where** | **Auto** (the default: the person's own computer when one is on and has room, else the cloud), **my computer**, or **cloud** |
| Under "more" | A spending limit for this agent (default: a set amount, shown), a time limit, and "may publish" (off) |

Before **Start**, a cloud agent shows what it can cost at most: "up to 120
Credits; stops there". A local agent shows "runs on your Mac, on your own
plan". Starting reserves the limit; what is not used comes back when it
ends.

Shortcuts the page should offer when they fit:

- On a game with feedback waiting: **start a fix agent on this feedback**.
- On a game just published: **playtest this version**.
- On a queued row: **add a cloud agent** (when the reason is a full
  computer or a one-at-a-time plan).

### Several agents on one game

Three agents on one game is the picture the owner drew, so it has to be
safe.

- **Edits are checked against the revision they were made on.** An edit
  made against an old revision is refused and the agent reads the game
  again. This is built.
- **An agent claims what it is working on**: a box on the level or a set of
  things (`claim`, `claims`, `release`, every surface; `coax/claims.coax`).
  Another agent's edit that touches a thing inside a claim, or puts one
  there, is refused whole with error `claimed`, naming the claim's owner.
  The person's own edit is never refused by a claim. Claims end with the
  turn, when the agent stops or fails, when its work is undone, and 600
  seconds after it last claimed or changed something. A level holds 8, an
  agent 4, a things claim 16 things. They are kept beside the level as
  `level-N.claims`, which the app, the CLI and the hosted server all honour,
  and the editor draws each one's box on the ground in its agent's colour.
  This is built (2026-11-17; docs/cli.md says how).
- **Every turn names the agent that made it**, so one agent's work can be
  undone without touching another's: `undo_turn` takes back the agent's
  newest turn even when other agents changed the level since, as long as
  only agents did (a change of the person's since is refused, so their work
  is never undone underneath them). This is built for the app's agents
  pane.
- **A playtest agent plays a pinned revision**, not the draft as it moves.
- If claims prove too coarse, the next step is each agent on its own copy
  with a merge. Do not build that first.

### What the page asks of the server

Under this site's own origin, as the login does (the browser host talks
only to its own site).

| Route | Does |
| --- | --- |
| `GET /api/agents` | The account's agents with state, game, place, spend and the one-line step; and its computers |
| `POST /api/agents` | Makes an agent: game, job, where, limits. Answers its id |
| `GET /api/agents/ID` and `/events?after=N` | One agent, and its events after a cursor |
| `POST /api/agents/ID/message`, `/stop`, `/answer`, `/move` | Send a message, stop, answer a question or permission, change where it runs |
| `POST /api/agents/ID/undo` | Undo one of its revisions, or all of them |
| `GET /api/computers`, `POST /api/computers/pair` | The account's computers; start a pairing |

### Built: the page, read only (issue 4)

What is built of the above is the reading half, from the account's
records:

- **`gamebuilder.org/agents`** (`web/agents.coax`, `gs_agents`): logged in
  only (logged out it is the login, and back here once you are in). It is a
  chat with your agents since issue 29: see "Built: the page is a chat"
  below. The agents are down the left, grouped by game, a game with an
  agent waiting for you first, waiting first within it, then newest. A
  row: kind and job, then the state in the record's words, how long in it,
  and where it runs ("this Mac"). An agent whose computer has said nothing
  for two minutes (`AGR_OFFLINE_MS`) shows offline with how long, never
  done. Pressing a row brings its conversation up. A cloud agent is
  started, told things, stopped and its change taken back here ("Built: a
  cloud agent"); an agent on a computer is read here and told things
  there, and the page says so in the rows' own words (`AGR_THERE`).
- **Its routes are the rows**, not the planned `/api/agents`: `POST
  /api/rows/agents` and `/api/rows/agent-events` on the hosted server
  (`gamebuilder serve --accounts`), which reads the records that computers
  report. The page signs in with its login's cookie: the server passes the
  cookie, only from the site's own page (its Origin), to the login service,
  as it passes a bearer token.
- **The app reports in** (`coax/sync.coax`, after the levels): it lists the
  account's agents, reads the Levels folder's records, and for each with
  more events sends `agent-report` from the account's count, at most 20
  events a report (`AGR_REPORT`) and 64 reports a pass (`SYNC_REPORTS`),
  every 10 seconds while logged in. The Agents pane's **Kept on your
  account** switch (the `agents-account` row, on unless turned off) says
  whether it does, and the first agent started says so in words.
- **No drift**: `tools/agents-drift.coax` fails the check when the pane,
  the page, the CLI or any other source has its own copy of a state's,
  kind's or place's words, or a list of their ids; `coax/sync.coax`'s
  `#test` and `tools/hosted-check.coax` hold that events written on the
  computer and sent as the app sends them read the same from the account's
  `agent-events`.


### Built: the page is a chat (issue 29)

The page is laid out as Coder for desktop is (the pinned tree's
`apps/coder-desktop`): the chats down the left, the conversation, and a
message box at its foot. It is `web/agents.coax`, drawn with the component
kit's pieces (`vendor/openagents/components/kit/frames.coax`: the composer,
the person's message, the agent's message as Markdown, the tool line, the
select, the side and its rows). Those are the tree-drawn counterparts of the
shared app set (`components/app/`), which draws on the GPU and so cannot be
used by a page that is a DOM tree; their look here is the site's own
(`web/look.coax`, `gs_css_agents`, every rule under `.ag`).

- **A message box is always there**, the caret in it on arrival. Return
  sends, Shift and Return makes a line. It is there with no game and no
  agent: nothing on the page says you need one first.
- **Sending starts a Builder.** A control beside the box says which game:
  one of your games, or a new game and its type (the nine the site has,
  from `shared/game-types.coax`; third person until another is picked).
  The type is the person's pick, never read out of the sentence. A send in
  an empty conversation is two rows: `new-game` with the type's id and a
  key of that send's (so one sent twice is made once), then `agent-new`
  with the game it answered and the words as the job. On one of your games
  it is `agent-new` alone. A first message is a job, so at most 240 bytes
  (`AGR_JOB`), counted beside the box as it gets near and refused with its
  numbers past it. If the game is made and the Builder is refused, the
  page says so, keeps the words in the box and keeps the game picked, so
  the next send makes no second game.
- **The conversation** is the agent's log, its newest 200 events
  (`GS_AG_SHOWN`; an earlier one is counted in a line at the top). Your
  message; each command it ran as a row (the tool, and what it sent in
  plain words) that opens on a press onto what it sent and what came back,
  with copy; a refused one marked and saying "refused:" and the row's own
  reason; each change, the newest not yet taken back carrying "take back
  its last change" (`agent-undo`); its answer as Markdown. While it works
  its one-line step (how far against its limits, and the command) and how
  long it has been at it show under them, read again every three seconds;
  Stop (`agent-stop`) stands where Send does, and Return still sends it a
  message (`agent-message`), shown at once and kept until its log has it.
  What a row came to is said in one line over the box, a refusal marked
  (a Stop or a take back only until its log's own line says it). A refused
  row's reason wraps rather than being cut, at any width.
- **The list** is a drawer on a screen narrower than 860 pixels: the bar's
  button brings it out over the conversation, and a press beside it or on
  a row puts it away. "New agent" clears the conversation. Your computers
  are a fold at its foot: how many and how many are on, and their words
  (or how one is paired) only when it is opened. With a computer paired
  the game control also offers where a new agent runs.
- **Every conversation links to its game**: "open in the builder" in the
  bar opens that game there, as picking it under "your games" does.
- **The limits are said once**, in one line under the box before the first
  send, built from the constants (`AGR_CLOUD_STEPS` and the rest), with
  "free" beside them.
- **Checks**: `web/site-agents.coax`'s `#test`s (an empty account; a send that
  makes a game; the conversation's rows; a refusal; Stop; take back; the
  list and its drawer; your computers), and the golden case
  `tests/commands/agents-chat.case`, the two rows a send is.

### Built: the first build agent

Built 2026-11-17, the smallest real slice of the above: no page of its own
yet, and one agent at a time, started from the chat.

- **The offer.** When the chat routes a message to `make` (nothing does it
  yet), the answer may offer to start a build agent on it. The offer is
  pending until the next message, and that message's select step is told of
  it and has two routes more, `offer/accept` and `offer/decline`
  (`web/chat.coax`, `ch_select_read`). Yes starts the agent; no says "ok,
  i won't start it."; any other message takes its own route and the offer
  lapses. The brief tells the model never to say it is working on
  something: nothing happens unless the person accepts.
- **The agent.** One build agent, on this game, with the job in the
  person's words. Its brief is the chat's own build and set briefs, the
  registry's index and the world as it stands. It has no tools: its only
  effect is its answer's plan lines, which the page makes exactly as a chat
  answer's, checked against the registry and the level's limits.
- **Where it runs.** One durable account turn on OpenAgents' hosted runner
  (`oa-agent`), through three routes on this site's own origin. The load
  balancer sends exactly these paths to `oa-api`
  (`scripts/web-deploy.sh domain`); the contract is openagents
  `docs/protocol/gamebuilder-agent-v1.md`.

| Route (this site) | Does |
| --- | --- |
| `POST /api/gamebuilder/agents/v1` | Starts one: `{"schema":"gamebuilder.agent.start.v1","operation_id","game","job","brief"}`. Answers 202 with its `turn_id` |
| `GET /api/gamebuilder/agent/v1?id=ID` | It: `state` (queued, running, completed, failed, cancelled, unknown), its answer so far, usage |
| `GET /api/gamebuilder/agent/events/v1?id=ID&after=N` | Its events after a cursor |

(`/api/agents` is the OpenAgents API's agent accounts, so the routes are
named for GameBuilder.)

- **In the chat.** A row under the messages says its state in words
  ("starting", "waiting for a runner", "working", "done", "failed: why",
  "stopped", "lost: the runner stopped answering") and the job. The page
  reads it every two seconds while it works and gives up after seven
  minutes, saying so. Its cap is said when it starts: one model turn, at
  most 2,048 tokens of answer and about 6 minutes, and nothing is charged
  for now. Who may start one: any logged-in person, for their own account
  (since 2026-11-17, issue 15; a guest starts none, and the page makes no
  request for one). At most 32 turns queued or running at once on an
  account, and the account's day of 500 requests and $2.00 of model use:
  a start a limit refuses says so in the service's words, with its number.
- **Not yet**: this page, a list of agents, Stop and messages from the page,
  claims and per-agent undo for this hosted agent (both are built for the
  app's pane and the CLI/MCP agents), kinds other than build, and the agent reading the
  level from a server copy rather than its brief.

**Built, to stand on.**

| Piece | Where (openagents) | State |
| --- | --- | --- |
| A conversation with an agent on a paired computer: describe, list, read, open, send, stop, answer | `docs/protocol/environment-runtime-v1alpha1.md`, `coder/tui/host*.coax` | A first working slice. Four agents at once on a computer |
| What a computer reports: cores, disk, memory, load, which agents are installed and signed in, which are running | `DescribeStation`, `coder/tui/host-station.coax` | Built |
| A turn that outlives the client, with saved events, a cursor and Stop | `docs/protocol/account-chat-turn-v2alpha1.md` | Live on staging |
| The hosted runner that runs a turn on our side | `coax/web/agent/` | Running |
| Agent engines: Claude Code, Codex, Cursor, Antigravity, our own | `coax/lib/agent/engine/` | Built |
| The router's first piece | `docs/router/README.md` | Built; no app calls it yet |

**Was built, to rebuild.** One service that owned every computer's wanted
and actual state and settled them (`oa-computers`,
`853417f290:docs/cloud/computers.md`); a board of every agent on every
computer (`GET /v1/agents`). Both were Rust and went in the reset. The
designs hold; the code is to be written in Coax.

**Not built anywhere.** This page, and an agent bound to a game on it.
(Claims and per-agent undo are built in the app's pane and on the command
table: above.) (One build agent at a time from the chat is built:
above.)

## Your own agent, in the desktop app

Status: a first spec. The full one comes next and should start from
[agent-mode-audit.md](agent-mode-audit.md), section 6.

### What the person should experience

1. They open GameBuilder on their Mac. It says "Claude Code is installed
   and signed in on this Mac. Use it to build?" with one button.
2. They press it. From then on **Agent** in the editor builds with their
   own agent, on their own plan, and the app says so.
3. The same agents show on `gamebuilder.org/agents` and on their phone,
   marked "this Mac".
4. If the Mac sleeps, the agents show as offline. Nothing is charged and
   nothing moves.

No terminal, no key to paste, no settings file.

### The piece that makes it hard

The Mac app is sandboxed and may only make outbound connections
(`host/GameBuilder-macOS.entitlements`). A sandboxed app cannot usefully
start the person's `claude`: the child inherits the sandbox and cannot
read the person's login. Every OpenAgents app that starts an agent is
unsandboxed.

| Way round it | What it is | For | Against |
| --- | --- | --- | --- |
| **A. Pair with the resident service** (recommended) | A small program that runs outside the app, starts agents, and is reached over an encrypted relay or a local socket. It pairs with OpenAgents' `coder host serve`: GameBuilder's own is `gamebuilder resident` ("Built: the resident service", below) | Built. The app's entitlements stay as they are. The same code serves the iPhone and the web page | Something has to install it |
| B. A build of GameBuilder that is not sandboxed | Downloaded from gamebuilder.org, not the App Store | Can start agents itself. Also the build that can carry the whole economy ([business/strategy.md](business/strategy.md)) | A second build to ship and sign |
| C. The agent attaches itself | GameBuilder offers its level tools as an MCP server. The person adds it to whatever agent they use | Works with any agent, including ones we have never heard of | The person sets it up in their agent. We see less of what it does |

Do **A** for the managed experience and **C** for everything else. **B**
is likely to exist anyway and makes A's install step disappear.

### What "easy" needs

| Step | What happens | Who does it |
| --- | --- | --- |
| Find | The app asks the resident service what is on this computer: which agents are installed, which are signed in, on what plan. No credential is read, only "signed in: yes" | Built (`DescribeStation`) |
| Install | If the service is not there: one button that installs and starts it. From the unsandboxed build this is direct. From the App Store build it opens the OpenAgents desktop app or a small installer | `gamebuilder resident install` is built (a login item); the button is to build |
| Pair | The app and the service exchange keys. On the same Mac this is one confirmation, not a code | Built for phones (`apps/shared/app/device/seal.coax`); the same-Mac shortcut is to build |
| Allow a folder | The service is told which game it may work on. It gets that game and nothing else on the disk | Built (`environment-enable`); the per-game binding is to build |
| Sign in, when needed | The app opens a terminal running the agent's own sign-in. We never show a login form of our own and never see the credential | Was built for Claude (`853417f290:docs/cloud/claude-code-byo.md`) |

### In the app

- **An agents panel** in the editor: the same rows as the web page, for
  the game that is open, with the whole list one tap away.
- **Each computer's card**: on or off, the agents signed in there, slots
  in use, and for a plan that has hit its limit, when it comes back.
- **Permission prompts** come to the app as Allow and Deny, and to the
  phone.
- **The level tools** are the agent's only tools by default: read the
  level, apply a batch of edits, undo. No shell, no files outside the
  game. The two Coax exports this needs are named in the audit
  (section 6.3).
- **One at a time on a plan.** A person's Claude plan runs one automated
  turn at a time; a second agent queues, and the row says why. This is
  Anthropic's rule for plans, not ours. Several at once needs an API key,
  or cloud agents.

### Built: the Agents pane (step 1)

`coax/agents.coax`, beside the level when the toolbar's Agents is on, drawn
in Coax like the side panel. What it does today:

- **+ New agent** opens a sheet: Builder, Playtester or Code, "What should it
  do?", "Runs on: This Mac, with your Claude Code", Start. A Builder is
  refused on a level the person may not change, with the step in words. Code
  adds the repository and the approval (below, "A Code agent").
- **A row** per agent on the open level's game (All games: every one),
  those waiting for the person first on a gold edge, then the newest: its
  kind and job, then "this Mac - Claude - 2m - working", the state in the
  record's words (`AGR_STATE_WORDS`), then what it is doing now in one live
  line ("Used read_level", "Changed the level: added 2 blocks (Block 29,
  Block 30)"; its question while it waits).
- **An open agent** (issue 5) shows a meter, "Turn 3 of 20 - $0.42 of $2.00
  of your Claude plan" (the record's limits: 80 turns and $10 for a Code
  agent), over a bar as full as the nearer of the two limits, a Code agent's
  card (its folder, branch and pull request), then a timeline of cards built
  from its events (the record's types, `coax/agent-timeline.coax`): the
  person's messages and the agent's, each tool call folded to one line (tap
  it for its arguments and result), a change card with what it changed in
  words (the edit event's text), **Show me** (the view turns to frame that
  change's things) and **Undo this** (on its latest change still standing:
  `ed_agent_undo_turn`, so the person's own later edit stands and the
  refusal says so), an ask and its answer, usage, and how a turn ended.
  The pane keeps the last 96 events of all its agents (`ATL_CARDS`) and
  says when older ones are not shown; the whole is the agent's log. A type
  of event without a card yet is drawn as its word and its text, so a new
  one (the code card, issue 8) shows at once. Then the answers to an `ask`
  as buttons (and, when a Builder's ask names an engine change, "Start a
  Code agent on the engine..."), Stop while it works, Undo all of this
  message when it made more than one change, and the message field.
- **The message field** is a real text field drawn in Coax: a caret and a
  selection, arrows, Option for words, Command for a line's ends and the
  field's, Shift to select, Command-A, Command-Z, Return to send and
  Shift-Return for a new line; at most 1,000 bytes (AG_DRAFT). It takes
  typing while the agent works: Send says "Send when this turn ends", the
  message is in the agent's log and a card at once (agent-message), and its
  runner gets it when the turn ends (Stop puts it back in the field, unsent).
  While the agent waits on an ask, the words are the answer: one of its
  answers goes through agent-answer, the person's own words straight to the
  waiting turn, and either is an answer event in its log.
- **Side by side**: with an agent open the pane is 640 points wide and the
  list and the open agent sit side by side (from 600 units, AG_WIDE); with
  none open it is 340 wide and one column.
- **Each agent is a record with an event log** (issue 4,
  `coax/agent-record.coax`), kept beside the levels as `agent-ID.agent`
  and `agent-ID.events`: quit and reopen the app and the agent, its whole
  conversation and what it changed are still there, and its timeline is
  built again from its log. A turn cut off by quitting shows "unknown",
  with why, until the next message. What the person does in the pane is the
  agent rows (`coax/agent-rows.coax`): agent-new, agent-message,
  agent-answer, agent-stop. So `gamebuilder agents` and `gamebuilder
  agent-events ID` show the same agent, and a message sent from the CLI is
  taken by the pane within two seconds. The pane keeps up to 256 agents
  (`AGR_RECORDS`); a log holds 100000 events (`AGR_EVENTS`).
- **The runner** (`coax/runner.coax`) is the person's Claude Code: it
  reads the record, takes the person's words, and appends what it says
  and does to the log (`ag_call` carries a tool call out as the agent and
  appends its tool event, and an edit event with the revisions, the things
  and the words of what it changed). Another runner (an attached agent, a
  Code agent) writes the same events.
- **The computer's tile** says the plain next step: Install Claude Code
  (opens Anthropic's download page), or sign in (opens Terminal running
  `claude`). The App Store build says to turn on GameBuilder's resident
  service on this Mac, and the iPhone sends you to gamebuilder.org/agents.

It is way B above for now: the `GameBuilder-macOS-Dev` build has no App
Sandbox, starts the person's `claude` itself, and the App Store build is
unchanged. One session at a time, as the plan rule says; a second agent
queues. Quitting the app stops a running agent, since it is the app's
child; the resident service (way A, "Built: the resident service", below)
is what lets one outlive the app.

### Built: pictures, the replay and the play-test picture (issue 9)

An agent's work can be looked at as well as read. Every picture is of one
level at one revision (`ed_agent_rev`), drawn by the renderer that draws that
level in the app: the engine's page for a level of the website's types
(`coax/world.coax`), the app's box pass for the arena, adventure and open
field. Nothing draws a picture any other way.

- **Before and after.** A call that changed the level (from the pane or from
  the terminal with `--attach`) queues two pictures (`coax/pictures.coax`
  `pic_want_change`, from `ch_record_into`): the level as it stood and as it
  stands, framed alike from the south about 55 degrees down, the change lit
  and outlined in the agent's colour as the editor shows it. The change card
  (`coax/agent-timeline.coax`) shows them side by side, by the revisions its
  edit event names (`before`, `after`).
- **The replay.** An open agent that changed the level has a replay over its
  timeline (`coax/agent-replay.coax`): one picture and a slider along its
  changes. Step 0 is the level before its first change, step N after its
  N-th; dragging the slider shows each in turn, with that change's words.
- **The play-test picture.** A play-test's tool card shows the level with the
  bots' paths on it, in their side's colour (a walk-through's in green), a red
  cross where one stuck and a gold box round each thing nobody reached, and
  what it found in words. Its tool event names the revision it played.

How a picture is made: the core keeps a queue of what is wanted; the host
takes the first each frame (`ed_pic_due`), sets up an editor of its own with
that level (`ed_pic_stage`: the things, the lit change, the marks, the
camera), draws it offscreen with the level's renderer at twice the size, and
hands back the pixels (`ed_pic_put`), which the core halves and codes as a
JPEG (`coax/lib/image/jpeg_enc.coax`). The pane leaves each picture's place
and lists it (`ed_agents_pic`); `host/Sources/AgentsView.swift` lays the JPEG
over it, as New Game's picker does its cards', since the component kit draws
no images. A place whose picture isn't kept says why, in words.

The pictures are kept beside the level as `level-N.pictures`, one
`gamebuilder.picture.v1` a line:

```json
{"schema":"gamebuilder.picture.v1","level":3,"rev":685592831,"which":"after","made":1763380000000,
 "w":160,"h":96,"type":"image/jpeg","bytes":4210,"text":"added 2 blocks (Block 29, Block 30)","data":"/9j/..."}
```

The CLI, MCP, the skill and the hosted API read them through two rows:
`pictures LEVEL` (the list, without the bytes: `gamebuilder.pictures.v1`) and
`picture LEVEL REVISION before|after|playtest` (one, its JPEG in base64). The
command line draws none: a level that has never been open in the app beside
an agent's change has none, and `picture` says so in words.

| Limit | Number |
| --- | --- |
| A picture's size | 160 by 96 pixels (`PIC_W`, `PIC_H`), drawn at twice that and halved (`PIC_DRAW`) |
| A picture's JPEG | at most 8,192 bytes (`PIC_JPEG`); a busy picture is coded coarser until it fits |
| Pictures a level keeps | its last 32 (`PIC_KEEP`); the oldest goes first |
| Pictures waiting to be drawn | at most 4 (`PIC_QUEUE`); past that the oldest waiting is dropped |
| A play-test picture's marks | at most 384 lines (`PIC_SEGS`) |
| A picture's words | at most 240 bytes (`PIC_TEXT`) |
| Pictures the pane shows at once | 24 (`ATL_PLACES`) |
| Pictures an account keeps | one an agent, its newest, so at most 256 (`AGR_RECORDS`), each a JPEG of at most 8,192 bytes (`PIC_JPEG`) |
| Pictures the app sends a pass | at most 16 (`SYNC_PICTURES`) |

**A game's newest picture on its group at `gamebuilder.org/agents`.** The
website draws no pictures of its own: it shows the ones the app drew.

- **The app sends them** (`coax/sync.coax`, in the agents' half, after the
  reports): for each agent the account keeps, it reads its level's
  `level-N.pictures` and sends the newest picture, when it is newer than
  the one the account has, with the `agent-picture` row
  (`coax/agent-pictures.coax`): `{"agent":ID,"picture":LINE}`, the line as
  it is in the file. Only while the Agents pane's **Kept on your account**
  switch is on: pictures go only when agents' records do.
- **The account keeps one an agent**, its newest. The agent's record goes
  first (`agent-report`); a picture of another level, an older one, or one
  that is not a `gamebuilder.picture.v1` line is refused in words. The
  folder keeps `agent-pictures` (a line an agent: `ID MADE ADDRESS`) and
  `picture-ADDRESS.jpg`; a newer picture takes the old one's place and the
  old file goes (`cli/agent-pictures.coax`).
- **The address is the picture's own**: the first 16 bytes of the SHA-256
  of the agent's id and the JPEG, as hex (`agp_address`), so it says
  nothing about the account and can't be guessed. The `agents` row's answer
  gives each agent's `"picture":{"at":"/api/pictures/ADDRESS.jpg","made":MS}`,
  and the hosted server (`gamebuilder serve --accounts`) answers `GET
  /api/pictures/ADDRESS.jpg` with the JPEG (`image/jpeg`, kept a year:
  the address changes when the picture does). A server on this computer
  serves none. The deploy routes `/api/pictures/*` to the hosted server
  (`scripts/web-deploy.sh domain`).
- **The page shows it** (`web/agents.coax`, `gs_agents_side`): a game's group
  shows the newest picture of its agents above their rows, at 320 by 192.
  An account with none shows nothing there: no empty frame, no broken
  image.

### Built: a Code agent (issue 8)

`coax/code-agent.coax`. When a world needs what the engine or the app can't
do yet, a person hands that work to a **Code agent**: their own Claude Code
with its normal coding tools, which a Builder does not have.

- **Starting one.** The New agent sheet's Code tab, or File > New Code
  Agent... (the `code-agent` row, which only opens the sheet). The person
  picks the engine (`openagents`) or the app (`gamebuilder`), says what it
  should do, and reads the approval: it can run commands and change files in
  its own folder; where that folder is; how its work lands; what follows;
  that it runs on their own Claude plan; and "At most 80 turns and $10 of
  your plan a message." **Approve and start** is the approval: nothing
  starts without it. `gamebuilder code-brief REPO` answers the same words
  as JSON (`gamebuilder.code.v1`).
- **Where it works.** The app's own clones in `~/gamebuilder-code/repos/`
  (made the first time, `--filter=blob:none`), fetched, and a fresh detached
  worktree from `origin/main` at `~/gamebuilder-code/REPO-ID`: outside the
  app's own folders (refused there) and outside any main checkout. An app
  agent gets the engine's clone too, named by `GB_OPENAGENTS_MAIN`, so
  `scripts/pin.sh` works there. The git steps are child processes polled
  from the pane's clock.
- **How it lands.** Its brief says to follow the repository's `AGENTS.md`.
  The engine changes by pull request at OpenAgentsInc/openagents only: its
  clone has a pre-push hook that refuses `main`. The app is landed by this
  repository's own rules. After an engine pull request, the card offers the
  next job, **Bump the pin...**: the sheet of a Code agent on the app with
  the pin bump (`scripts/pin.sh bump`, then `scripts/check.sh`) as its job.
  It starts only when approved, like any other.
- **Its session.** `--setting-sources=project`, its worktree as the working
  folder, `--max-turns=80`, `--max-budget-usd=10`, the pane answering each
  tool's permission (`--permission-prompt-tool=stdio`): commands run; a
  file edit outside its worktree is refused. ANTHROPIC_API_KEY and its kin
  are taken out, and a session that reports a key source is stopped, as for
  every agent. It reports on its own server, `gamebuilder-code`:
  `report_branch`, `report_check` and `report_pull_request` (an address at
  the repository's own GitHub `pull/N`, nothing else).
- **Its card**, from its events: the repository, its folder and the commit
  it started from, its branch and pull request (with **Open pull request**),
  the files it changed (measured after each turn), its checks passed or
  failed, and the pin bump that follows.
- **A Builder** that hits an engine gap says so in an `ask` with `engine`
  set; the pane offers "Start a Code agent on the engine...", which opens
  the sheet with that job and tells the turn. A Builder never starts one.
- Only where the build may start `claude` (the Mac download, Linux); the
  App Store build and the iPhone say the plain next step.

Its record and events are kept beside the levels with every agent's (issue
4), so a Code agent, its conversation and its card are still there when the
app opens again. The pane runs at most 8 Code agents at once (`CA_SLOTS`),
each with its steps and its card; one that is over gives its slot to a new one.
`tools/code-agent-check.coax` makes a worktree for real from scratch
repositories in a temporary folder and checks the hook and the files.

### Other agents

| Agent | Managed by us (A) | Attaches itself (C) |
| --- | --- | --- |
| Claude Code | Yes: built | Yes |
| Codex | The engine exists; GameBuilder's resident service runs Claude Code only, and says whether Codex is installed and signed in | Yes |
| Cursor, Antigravity | Engines exist; not in the service yet | Yes |
| A desktop chat app with MCP | No | Yes |

An attached agent appears on the agents page as "attached", with its
changes as revisions and an undo. We can refuse its edits and cut it off.
We cannot stop its process, because it is not ours.

### Built: attached agents (issue 6)

An outside agent that builds through `gamebuilder mcp`, `mcp --attach`,
`mcp --remote` or `POST /mcp` gets a record of its own, made with the record
module (`coax/attached.coax`, `cli/attached.coax`):

- **Its record.** A session opens a `gamebuilder.agent.v1` record with kind
  `build` and place `attached`. Its runner is the client's name from
  `initialize` (`clientInfo.name`, cut to 16 letters, digits, `.`, `-` and
  `_`; `mcp` when it sends none). Its job says how it came in, such as "an
  outside agent, building through gamebuilder mcp --attach". It starts on "My
  levels" (game 0) and takes the game and level of the last level it called.
  The record and its log sit beside the levels as `agent-ID.agent` and
  `agent-ID.events`. On a hosted server they sit in the account's folder.
- **Its events.** The record starts with a state `working` event. Each tool
  call appends a `tool` event: the tool's name, its arguments as sent, whether
  it was refused, and its answer cut to 240 bytes. A call that changed a level
  also appends an `edit` event, with the level's revisions before and after
  and the things it touched. When the session ends (stdin closes, or `DELETE
  /mcp`), a state `done` event says "its session ended".
- **Its turns are its own.** Its edit batches carry a turn number made from
  its id (`gb_att_turn`, 1048576 and up, apart from the pane's agents). So
  `undo-turn` takes back only its own batch: through MCP with `--attach`, or
  `gamebuilder --attach --as-agent NAME undo-turn LEVEL`. The app's Undo turn
  takes it back the same way.
- **Sessions over HTTP.** `POST /mcp` with `initialize` answers an
  `Mcp-Session-Id` header. Calls that send it back are that agent's. An id
  the folder doesn't know gets 404 with words, and `DELETE /mcp` ends the
  session. `gamebuilder mcp --remote` keeps the site's session id for you.
- **Listed and cut off.** `agents` and `agent-read` list it as `attached`,
  with `"can"` saying what GameBuilder can do to it: refuse its calls and cut
  it off, but not stop its process. `agent-stop` cuts it off: its log says
  so, and every call it makes after that is refused as `not-permitted`, in
  words. It stays cut off; a new session is a new agent. Its process is the
  person's, so it is ended in the agent itself.
- **One-off calls.** A CLI call makes no record unless it names an agent:
  `gamebuilder --as-agent NAME VERB ...` logs the call to the newest record
  of that name that isn't done (a new one when there is none). Over HTTP, the
  `GameBuilder-Agent: NAME` header does the same for `POST /api/rows/VERB`.
  `gamebuilder mcp --no-agent` is a session that makes no record. The golden
  cases' MCP leg uses it, so the agent rows answer the same four ways.
- **Limits.** A folder keeps at most 256 agents (`AGR_RECORDS`). Past that,
  a session's calls are carried out but not recorded, and it says so on
  stderr.

The checks are the `#test`s in `coax/attached.coax`, `coax/attach.coax` and
`coax/api.coax`, the golden case `tests/commands/attached.case`, and the end
to end runs in `cli/check.sh`, `tools/attach-check.coax` and
`tools/hosted-check.coax`.

### Built: a cloud agent (issues 7 and 27)

A Builder your account runs, started from `gamebuilder.org/agents` or by
saying yes to the chat's offer, with no Mac and no Claude plan. It is bound
to a game of yours and works on that game's first level.

**The loop is Microcoder's (issue 27).** The hosted server (`gamebuilder
serve --accounts`) runs it, a step at a time, between the requests it
answers (`cli/cloud.coax`). The loop itself is the pinned openagents tree's
`coax/lib/microcoder/loop.coax`, loaded as it is: one model call a step,
answered with one `next_action` under Microcoder's strict schema, with its
own state, its own compaction of old steps, and its own endings. GameBuilder
holds no second loop: `coax/cloud.coax`, which built a Chat Completions
request with tools and read the tool calls, is gone.

A step asks OpenAgents' model, `POST https://openagents.com/api/v1/inference`
(`google/gemini-4-hydrogen`). The request is Microcoder's (`mc_request`: its
system prompt, its state as one message, its schema), written as a Chat
Completions body by the gateway library (`gw_translate`, not streamed) and
read back by it (`gw_parse_whole`), then fed to the loop (`mc_feed`).
Microcoder's system prompt is not changed. What makes it a Builder is the
state's environment, as openagents.com's own `/chat` does it for its
repository reader (`coax/web/agent/chat-micro.coax`).

**A command is a row.** Microcoder's action carries `commands`. On a coding
run those go to a shell. Here there is no shell, and its shell runner
(`tools.coax`) is not loaded: each command is one row of the command table,
written as the CLI takes it, for example `read-level 3` or `apply-edits 3
'{...}'`. `coax/cloud-rows.coax` splits the line into words as a command
line is split, finds the row by its verb (`gb_find`) and reads the words
with the table's own reader (`gb_flag_take`, `gb_args_read`), the one the
CLI uses. Nothing reads what the model means. The row is then carried out
the same as an attached agent's call (`cli_att_run`), on the account's files
and with the agent's own turn number. So claims hold, the record and log
are the same as any agent's, and the desktop app sees each change by sync
like any change to the account's level.

The rows it may write are the ones the table marks with the `cloud` surface
(`GB_CLOUD`): `read-level`, `apply-edits`, `list-palette`, `list-fields`,
`list-modes`, `claim`, `claims`, `release`, `list-scrappers`, and
`agent-undo` on itself. `play-test` is not one: a match would hold the one
server for minutes. What it is told about its rows (the environment's
`rows`) is made from those table rows, and a `#test` fails if what it is
told, what is carried out and what the table marks ever differ.

Refused in words, each as a result the model reads in its next step:

- A command that starts with anything but one of its rows' names (a shell
  command, another row, a program's name).
- Another level than its own, or `agent-undo` on another agent.
- `--agent` or `--key` (the server says who it is), and a batch named as a
  file (there are no files).
- Microcoder's `edits` and `view`, which have nothing to work on here.
- The ninth command of a step and on.

**What the environment carries**, at most 16,384 bytes (`CLD_ENV`; a run
that would pass it fails in words rather than being cut): what a command is
there, its level and game, its id, the rows, the batch's form
(`gb_batch_text`, the skill's own), the level's palette (`list-palette`'s
answer, asked as the run starts), its rules, and its limits in words. The
task is the conversation so far and the person's new words, in the form
Microcoder's chat writes it ("Me:", "You:", then "Now:"), from its log: the
newest 12 messages, the oldest let go first to fit 16,384 bytes (`CLD_TALK`,
`CLD_TASK`). Each message of the person's starts a fresh run of the loop.

**Where the loop's state lives (decided by issue 27's spike).** In no
process. Each step is a child, `gamebuilder cloud-step URL JOURNAL REPORT`.
It starts a fresh loop, gives it back the run's journal in order
(`agent-ID.loop`: what it was told, each model answer as it came, each
result it was given), takes one step, writes a report for the server, and
ends. The server reads the report, runs the commands, and adds the answer
and its results to the journal in one write. This was chosen over one child
kept for the whole run because:

- The server stays the only writer of the account's files, one request at a
  time, as before, and it keeps answering while the model is asked.
- A restart or a deploy between two steps loses nothing: the next child
  reads the same journal. `tools/cloud-check.coax` stops the server between
  two steps and starts it again to show it.
- Giving the journal back is exact and cheap. The loop is deterministic for
  the same inputs in the same order, a message has at most 12 steps, and one
  child holds about 2 MB while it runs on a 512 MB instance.
- Microcoder's state holds slices into its own buffers, so it can't be
  written to a file and read back without a change to the engine. Giving
  its inputs back needs none.

**How the signed route is reached.** The gateway library writes the request
and reads the answer, but the POST itself stays GameBuilder's: the child
reads the server's Nostr key from `GB_MODEL_KEY_FILE`, signs the request
under NIP-98 with the body bound (`n98s_header`), sends it (`http_do`), and
wipes the key. The gateway library's own transport was not used because it
starts `curl`, which the server's image doesn't have, and its signer for
our route does not bind the body. So the key, the image and the deploy are
as they were, and nothing in the engine changed.

**Its record.** Kind `build`, place `cloud`, runner `openagents`, computer
none. `turns` holds its model steps for each message and `cents` is 0. Its
files sit in the account's folder:

- `agent-ID.agent` and `.events`: the record and its log, as every agent's.
- `.loop`: its current message's journal.
- `.run`: where its current message stands.
- `.undo.N`: its last 16 changes, kept to take back.
- `.owner`: the login it acts as.
- `.reply`: one step's report while it is asked.
- `cloud-day`: the account's steps today.

**Its events.** The person's message, then `working`. Each command is a
`tool` event: the row's name (`gb_read_level`), its arguments, whether it
was refused, and the answer. A change also adds an `edit` event with the
revisions. A refused row is a `tool` event with `ok` false and the refusal's
words, which the person reads in its log: inside another agent's claim, a
stale batch, past a limit. A command that is no row of its own is a `tool`
event named `command`, with the line as written. Its finishing reply is a
`message` from the agent; a question it can't go on without is one too, and
the person answers with a message. Microcoder's `rationale` is for the
journal and is not shown. At the end of each message come `usage` (model
steps, 0 cents, time) and a state: `done` when it finishes, `stopped` at a
limit, `failed` when the route refused, when 3 answers in a row couldn't be
read, or when it took 3 steps in a row with no command and no answer
(Microcoder's own two limits). The record's step line says where it stands
while it runs, for example "step 3 of 12, 8,200 of 100,000 tokens, 2 of 30
changes: asking the model", and names the row while one runs.

**Its limits**, said in words with their numbers before it starts (in
agent-new's answer, on the page and in the chat) and shown as it runs
(`agr_cloud_words`, `agr_cloud_spent`):

- 12 model steps, 100,000 tokens, 30 changes and 10 minutes a message
  (`AGR_CLOUD_STEPS`, `AGR_CLOUD_TOKENS`, `AGR_CLOUD_EDITS`,
  `AGR_CLOUD_MINUTES`).
- A step is one model call, and it may run several rows: at most 8
  (`AGR_CLOUD_COMMANDS`). The limits text says so: "12 model steps (a step
  runs at most 8 commands)".
- 8,192 tokens for one answer (`AGR_CLOUD_ANSWER`).
- 60 model steps a day for an account (`AGR_CLOUD_DAY`).
- 2 of an account's agents working at once, and 4 on a server
  (`AGR_CLOUD_EACH`, `CLOUD_SLOTS`).
- 16 changes kept to take back (`AGR_CLOUD_UNDO`).
- A command line of at most 65,536 bytes and 12 words (`CLD_COMMAND`,
  `CLD_WORDS`).
- A row's answer longer than 11,000 bytes comes to the model as two
  results; past 22,000 bytes it is told how many bytes are not shown
  (`CLD_PART`, `CLD_PARTS`). One step's results take at most about 32 KB of
  the loop's state, and the loop says so itself when it holds one back.
- 2 MB of journal for one message (`CLOUD_JOURNAL`): past it the message
  ends and says so.

At a step, token, minute or day limit it stops, and says which limit and
that a message goes on. Past 30 changes its changes are refused in words. A
new message starts the message limits again. The free key's own caps
(OpenAgents' 500 requests and $2 a day for a key) come back as 429, which
fails the agent with those words.

**What the person does**, each a row of the table's (`agent-new`,
`agent-message`, `agent-stop`, `agent-undo`, on `GB_WEB` too):

- **Start:** on `/agents`, pick a game and say what it should do. In the
  chat, yes to the offer starts one on the game on your account that the
  world is (web/account.coax's link). A world not on the account yet keeps
  the earlier one-turn build agent.
- **Message:** a message to an agent that finished or stopped goes on from
  it. A message sent while it works reaches it as a note at its next step.
- **Stop:** ends the step under way; its answer is not used.
- **Take back its last change:** the level is put back as it was before that
  change, while the level is as the change left it. If the level changed since,
  it is refused as `stale`, and that change stands.

While a cloud agent of yours works, the page reads the list and the open
one's events every three seconds. A server without a model key
(`GB_MODEL_KEY_FILE`) runs no cloud agents, and `agent-new` says so.
`gamebuilder cloud-key FILE` makes that key once (64 hex, mode 0600) and
prints only its public key, the one openagents.com's `OA_INFERENCE_PUBKEYS`
must list before the model route takes the server's calls; run again, it
says the same public key and makes no new key.

**Not here yet:** sub-agents (Microcoder's `delegate`), and a live run
against the real route after this change, which is the owner's check after
a deploy.

**Checks:**

- The `#test`s in `coax/cloud-rows.coax`: the rows offered are the rows
  marked, a command line read into its row, the task, the parts and the
  journal's lines. `coax/agent-record.coax` has its limits in words.
- Two site `#test`s in `web/site-agents.coax`: the page's start, message, stop and
  take back, and the chat's offer.
- `tools/cloud-check.coax`, run by `cli/check.sh`, runs a real server with
  this tool standing in for the login service. First against Microcoder's
  scripted gateway (`serve --model script:FILE`): one block placed through
  a row. Then against a stand-in for the model route: the signed start with
  Microcoder's own request, several rows in one step, a change through
  `apply-edits` read back from the account and the log, the page's undo, a
  change refused inside another agent's claim, its own `agent-undo`, each
  refusal above, stop while asked, a server restarted between two steps,
  each limit, three answers that can't be read, and a refused route. It
  also holds that the two cloud files name no Chat Completions field.
- `tools/hosted-check.coax` checks that a server with no key says it runs
  none.

### Built: the resident service, computers and cost (issue 11)

**The resident service** is `gamebuilder resident` (`cli/resident.coax`): a
program of the command line's, outside the app and outside its sandbox,
that runs the agents queued on its computer, with the app open or not.

- **What it runs.** Local records whose runner is `resident` and whose
  computer is its own name (`GAMEBUILDER_COMPUTER`, else the Mac's name),
  one at a time (`AGR_SLOTS`, the plan rule). Each turn is the person's own
  `claude -p --output-format stream-json`, with `--max-turns 20` and
  `--max-budget-usd 2.00`, no settings of the person's
  (`--setting-sources=`), no tools of its own (`--tools=`) and only the
  rows' tools: an MCP server that is `gamebuilder mcp --resident ID` on the
  same folder, so each tool call and each change is the record's event as an
  attached agent's are. The keys `AG_NO_KEYS` names are taken out of its
  environment, and a session that says it would bill a key
  (`apiKeySource` not `none`) is stopped and fails in words: a plan login
  is never copied off its computer, and a key is never billed.
- **Who starts one.** `agent-new-on COMPUTER GAME ROLE JOB` queues an agent
  on a computer by its name. The App Store build's pane uses it when it
  can't start `claude` itself and this Mac's service is on; the page does
  when the person picks a computer; the iPhone goes to the page.
- **The app closed, the service quit.** The service is its own process, so
  closing the app stops nothing. `gamebuilder resident stop` (or the login
  item unloaded) ends its turn: the agent is stopped with
  `AGR_QUIT` ("you quit GameBuilder's resident service") and the computer
  says it is off. A service killed mid-turn leaves the turn `unknown`,
  said with `AGR_LOST` when it starts again; it never says done.
- **Paused.** A plan's limit (Claude's `rate_limit_event`, status
  `rejected`, with `resetsAt`) pauses the agent until that time. The record
  keeps `resumes`, the views say "it goes on in 2h", and the service (and
  the desktop pane, for its own) queues it again then. Nothing else moves
  it.
- **On the account.** Signed in (`gamebuilder login`), every 10 seconds it
  puts its computer on the account (`computer-report`), takes the agents
  the account has queued for it (`agents`, `agent-events`) and reports its
  own (`agent-report`). Its computer file names no login, token or key.
- **One per folder** (`resident.lock`). `gamebuilder resident status`
  says whether it is on; `gamebuilder resident install` writes a login
  item (`~/Library/LaunchAgents/org.gamebuilder.resident.plist`).

Why our own and not a call into `coder host serve`: that service's
environment runtime runs Claude with no GameBuilder tools per agent, and
the pairing it needs (keys sealed between app and service) is not in this
tree. The resident service takes the same facts `DescribeStation` does
(the program installed, `~/.claude.json`'s account, `~/.codex/auth.json`)
without reading a credential, and the rows are its tools. Pairing a
computer is: install the command, `gamebuilder login`, `gamebuilder
resident install`; the computer shows on the account once it reports.

**Computers** (`gamebuilder.computer.v1`, `coax/agent-record.coax`): its
name, on or off and since when, when it was last heard, its slots and the
agent in each, and for Claude and Codex whether each is installed and
signed in. The row `computers` lists them; a computer not heard from for
two minutes is `offline`, with the age. The desktop pane's computer tile
and `/agents`' "your computers" say the same words
(`agr_computer_words`): "on: 1 of 1 slot in use; Claude signed in, Codex
installed, not signed in". With none, the page says how to pair one.

**States the page lacked.** Paused, with when it goes on. Offline, with the
age of what is shown ("what is shown is as it was then"). Unknown, with why.
On a Builder of the person's computer that is paused, offline or unknown,
**carry on in the cloud** starts a new cloud Builder on the same game and
job (`agent-new`); only the person's press does it.

**Cost.** Every record's view carries `cost` (`agr_cost_words`). A local
agent's is "your own agent: no charge from us", said by its first event as
it starts, by `agent-new` and `agent-new-on`, and by the page before it is
queued. A cloud agent's is its Credits so far and its limit: today "0
Credits so far, of a limit of 0", since OpenAgents' free key and our
server carry every cloud agent and nothing is charged yet.

**Kinds.** Build and playtest run on the service (code runs in the pane,
issue 8). Art, fix and watch are not built, and not stubbed:

- **Art** needs an editor that makes or changes a model or texture from
  words. The editor places the engine's kinds and can't make new art, so an
  art agent would have no tool to call.
- **Fix** starts from a player's feedback on a game. Feedback is read
  (`feedback`), but only on the device that published the game, by
  published version, and nothing ties an item to the draft level it is
  about or marks it answered; a fix agent would be a Builder given the
  line as its job, which the person can already start. The editor work it
  needs is that link, and it isn't built.
- **Watch** is a standing agent woken by something that happens in a game.
  A record is one job, and whether an agent is one job or a standing helper
  is still the owner's question (below, 1).

**Limits**, in words with their numbers: one agent at a time on each
computer (`AGR_SLOTS`, 1); 8 computers an account keeps (`AGR_COMPUTERS`);
a computer's file of at most 1,024 bytes (`AGR_COMPUTER_MAX`); offline
after 2 minutes unheard (`AGR_OFFLINE_MS`); each turn at most 20 model
steps and $2.00 of the plan's budget; the account reached every 10 seconds.
From the web, an agent on a computer is not told things or stopped yet: the
rows refuse it in words, and GameBuilder on that computer does it. A
service killed by a signal may leave its `claude` running until that ends.

**Checks:** the `#test`s in `coax/agent-record.coax` (a computer, cost and a
pause) and `coax/brief.coax`; the golden case `tests/commands/computers.case`
through core, CLI, MCP and HTTP; a site `#test` in `web/site-agents.coax` (the
computers, a Builder queued on one, paused, offline, carry on in the cloud);
and `tools/resident-check.coax`, run by `cli/check.sh`, which runs the
service for real with `tools/claude-standin.coax` standing in for Claude
Code: a build through `mcp --resident` with its tool, edit, answer and
usage events, a stop that frees the slot, a pause that goes on by itself, a
key refused, `resident stop`, and a killed service's turn unknown.

### What must be true before it ships

- An agent on the person's computer never costs Credits, and the app says
  that at the moment it starts. Its first event, and every answer that
  starts one, says "your own agent: no charge from us" (above, "Cost").
- A plan login is never copied off the computer it was made on. The
  service reads only whether one is there; keys are taken out of
  `claude`'s environment, and a computer's report names none.
- Closing the app does not stop a running agent; quitting the resident
  service does, and says so (`AGR_QUIT`).
- An agent offline is shown as offline, with the age of what is shown.

## Open questions for the owner

1. **Is "agent" one job, or a standing helper?** This spec says one job,
   with "watch" as the standing kind. The alternative is named helpers
   that take jobs.
2. **Auto's default.** The person's own computer first, as proposed, or
   the cloud first for a new person with nothing paired?
3. **Does the App Store Mac build depend on the OpenAgents desktop app**
   to install the resident service, or does GameBuilder ship its own
   installer?
4. **May an agent publish?** Proposed: only with a per-agent switch, off
   by default.
5. **Agents on a child's account.** Proposed: local agents and the free
   allowance only; no cloud spending without a parent's limit.
6. **The iPhone.** The same list as a native screen. Cloud agents spend
   Credits there, which Apple's purchase can sell.

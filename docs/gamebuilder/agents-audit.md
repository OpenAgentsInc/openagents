# Agents audit: what is built, why the desktop pane feels rough, and what to build next

Written 2026-11-17, against commit `38ced7c`. Status: an audit with
recommendations. Nothing here is built by this document.

The owner's aim: delegate work on a game, on the whole game creation
process, and on the engine itself, from the desktop app, with a clear picture
of what each agent is doing, and with the CLI or MCP doing the work under the
hood. `https://gamebuilder.org/agents` should exist in a very basic form soon
so the same flow can be connected on the web.

Read with: [agents.md](agents.md) (the specification of the page and of your
own agent in the desktop app), [agent-mode-audit.md](agent-mode-audit.md)
(what existed in OpenAgents before any of this), and
[programmatic-access.md](programmatic-access.md) (the command table every
surface runs through). Issues 2 and 3 are in progress and this audit assumes
they land: one creation process for web and desktop, and the CLI, MCP and
HTTP API at parity with the interface.

## Summary

1. **There are three separate agent implementations and no shared idea of
   "an agent".** The desktop pane runs your own Claude Code in the app's
   process. The website's chat can start one hosted build agent with no
   tools. Any outside agent can attach through `gamebuilder mcp`. None of
   them knows about the others, and none writes a record anything else can
   read.
2. **`gamebuilder.org/agents` answers 404.** The page, its routes
   (`GET /api/agents` and the rest in agents.md) and the store behind them
   are not built. The only agent routes that exist are the three for the
   chat's build agent.
3. **The desktop pane works but is thin.** It is one file,
   `coax/agents.coax`, about 1,160 lines, that holds the interface, the
   session, the agent list and the tools together. Everything lives in the
   app's memory: quit the app and every agent, its conversation and its
   history are gone.
4. **An agent in the pane can only edit the one open level.** It has no
   files, no shell and no code tools, by design. It cannot work on the
   engine, on game rules, or on a level you do not have open.
5. **What you see of an agent's work is a line of text.** A tool call shows
   as `  > apply_edits`, changes show as "2 steps, 5 things", and the things
   it changed glowed for 1.5 seconds (since issue 5: they stay outlined in
   the agent's colour until dismissed, and the pane shows a timeline of
   cards with Show me and Undo this on each change; see "Since this audit").
   There was no timeline, no before and after, and no way to look at a
   change later.
6. **The pieces to do this properly mostly exist.** The command table, the
   golden cases, `--attach`, the hosted API with each account's levels, MCP
   over HTTP, the level sync and per-agent undo are built. What is missing
   is one agent record with an event log, runners behind it, and views
   drawn from it.

The recommendation, in one line: make an agent a stored record with an event
log, in shared Coax, with rows in the command table; make the desktop pane,
the web page and the CLI three views of that record; then build the
visualisations from the events.

## What is built today

### 1. The desktop Agents pane

| Part | Where | What it does |
| --- | --- | --- |
| The pane | `coax/agents.coax` | The list, the New agent sheet, one open agent, the composer, the computer's tile; all drawn in Coax with the component kit |
| Its carrier | `host/panel/agents-host.c`, `host/Sources/AgentsView.swift` | Places the view beside the level, runs a 60 Hz clock, hands it keys and text, opens Claude Code's download page or Terminal for sign-in |
| The session | The Coax Claude Agent SDK, `vendor/openagents/coax/lib/agent/claude` | Starts the person's `claude` as a child of the app and polls it from the clock (`ed_agents_tick`), never waiting |
| The tools | The command table's rows marked `GB_PANE` | One in-process MCP tool a row on one server, `gamebuilder`. Carried out by `gb_command_run_as` with the agent's id |
| The level seam | `coax/agent.coax` | `ed_agent_apply` (a `world.edit.v1` batch as one undo step, refused whole when stale, locked or mid-match), `ed_agent_undo_turn` |
| The toolbar button | `coax/controls.coax`, `toolbar.agents` | Opens and closes the pane |

How a run goes:

1. The person presses **+ New agent for this level**, picks Builder or
   Playtester, types a goal and presses Start (`ag_new`).
2. The pane looks for `claude` on the PATH, then in four usual places
   (`ag_program`), and starts it with: no built-in tools, no settings files,
   a folder of its own, at most 20 turns and $2 of the plan a message, and
   `ANTHROPIC_API_KEY` and its kin removed from the environment
   (`ag_options`). A session that reports any key source but the login is
   stopped.
3. The first message is the brief: the rules (`AG_RULES`), the form of an
   edit batch, the kind's line, the level number and the goal (`ag_brief`).
4. Each tool call is a row run on the open level. What it changed is
   highlighted in the agent's colour until dismissed (`ch_record`,
   `coax/changes.coax`) and is counted. `ask` waits on buttons.
5. A result ends the turn: done, or failed with the reason in words (the
   turn limit, the budget, signed out).

Limits that are deliberate: one session at a time, because a Claude plan
runs one automated turn at a time, so a second agent queues; and the
`GameBuilder-macOS-Dev` build only, because a sandboxed app cannot start the
person's `claude`. The App Store build and the iPhone say so in words.

### 2. The website's build agent

| Part | Where |
| --- | --- |
| The offer and the start | `web/chat.coax` (`ch_job_start`, `ch_job_tick`, `ch_job_end`) |
| The routes | `POST /api/gamebuilder/agents/v1`, `GET /api/gamebuilder/agent/v1?id=`, `GET /api/gamebuilder/agent/events/v1?id=&after=` |
| The contract | openagents `docs/protocol/gamebuilder-agent-v1.md` |
| Where it runs | One durable account turn on OpenAgents' hosted runner |

When the chat cannot do something, it may offer to start a build agent. Yes
starts one. It has **no tools**: its brief holds the world as text, and its
answer's plan lines are made by the page as a chat answer's are. It gets one
model turn, 2,048 tokens of answer and about six minutes. The page reads its
state every two seconds and shows a row under the chat. Only one at a time,
no Stop, no list, no messages to it. Any logged-in account may start one
(since 2026-11-17, issue 15; before, only OpenAgents' team could).

### 3. An outside agent, attached

| Way in | State |
| --- | --- |
| `gamebuilder mcp` (stdio) | Built. The table's rows as MCP tools on the level files |
| `gamebuilder mcp --attach` | Built. The same tools on the level open in the running app; changes glow and ⌘Z takes them back |
| `gamebuilder --remote`, `POST /mcp` on gamebuilder.org | Built and live. The account's levels, signed in with the OpenAgents login |
| The Claude skill | Built (`gamebuilder skill`), made from the table |

This is the most capable path today: any coding agent with its own tools can
build levels through it. But the app shows nothing of such an agent beyond
the glow. It has no row in the pane and no record.

### 4. What does not exist

- The page at `/agents`, its routes and a store of agents.
- An agent's record outside the process that runs it.
- Agents on a game rather than on one open level.
- Claims, so several agents can work on one game safely.
- The resident service route (way A in agents.md), which lets an agent
  outlive the app and serve the phone and the web.
- Kinds beyond Builder and Playtester. Play-test itself runs only arena
  levels.
- Any agent that can change game rules, tables or scripts (those are not
  built in the editor either), or the engine.

## Why the desktop flow feels rough

Each row is something read in the code, not a guess.

| What the person meets | Why | Where |
| --- | --- | --- |
| Quit the app and every agent is gone | The agents are an array in the pane's memory. Nothing is written | `AgPane.agents` |
| At most eight agents, ever | `AG_AGENTS :: 8`; the oldest finished one is overwritten | `ag_new` |
| The agent forgets the conversation | The session is not persisted. A message after the session ended starts a new one whose brief is the goal plus "you worked on this before", with none of the history | `persist_session = false`, `ag_brief` |
| Old messages vanish | The log is 16 KB of plain text; the older half is cut. Only the last 40 paragraphs are drawn. Since issue 5: events as cards, the last 96 of all agents kept in the pane (the record's own log is issue 4's) | `ATL_CARDS`, `AG_SHOWN` |
| No formatting | Markdown marks are stripped and everything is one text style | `ag_say` |
| You cannot see what a tool did | A call is logged as its name only: no arguments, no result, no time taken. Since issue 5: a folded card that opens to its arguments and result | `ag_tool`, `atl_card` |
| You cannot see what changed | "Changes this turn: 2 steps, 5 things", and a glow that faded in 1.5 seconds. No list of things, no before and after, nothing to click. Since issue 5: a lasting highlight, a summary in words, Show me | `ag_open`, `coax/changes.coax` |
| The counts reset | Batches and things lit are zeroed on each new message, so earlier work has no trace | `ag_message` |
| You cannot type while it works | The composer and Send are disabled while the agent is working, waiting or queued. Since issue 5: the message goes when the turn ends, and answers an ask while it waits | `ag_send` |
| Typing is crude | The caret is a `|` appended to the text. No cursor movement inside the text, no selection. Since issue 5: a field with a caret, a selection and the usual keys | `atl_field`, `atl_field_key` |
| Switch levels and the agent is stuck | Its tools are refused while another level is open | `ag_tool` |
| It stops at 20 turns or $2 | Fixed limits a message, said in words when hit, not shown as progress before. Since issue 5: a meter of both against their limits | `AG_TURNS`, `AG_BUDGET_USD`, `atl_meter` |
| "This Mac - Claude - 2m - working" tells you little | The row has no line for the current step. Since issue 5: one live line of what it is doing now | `ag_row`, `atl_step` |
| Only on a special build | The App Sandbox; way B of agents.md | `AgentsHost.here` |
| It cannot do most of game creation | Its tools are level rows: read, apply edits, add, set a field, delete, new and copy level, new game, lists, play-test, undo its turn, ask | The `GB_PANE` rows |
| The layout is one scrolling column | List, sheet, open agent and tile are stacked in one pane beside the level | `ag_layout` |

Since issue 4 (the agent record), the rows "Quit the app and every agent
is gone", "At most eight agents, ever", "Old messages vanish" and "The
counts reset" no longer hold: an agent is a record with an event log kept
beside the levels, up to 256 of them, its whole conversation is in the log
(the pane's timeline is built from it, the last 96 events of all its agents),
and nothing in the record is zeroed on a new message (only the meter and
Undo all of this message start again). Each tool call's event keeps its
arguments and a short result, and each change's event its words.

The root cause of most rows is the same: the pane is the agent. The record,
the session and the drawing are one struct, so nothing can be kept, shared
or shown another way.

## The gap to what the owner wants

### Delegating the whole of game creation

| Work | Can an agent do it today? | What is missing |
| --- | --- | --- |
| Lay out a level | Yes, through the rows | Better visibility only |
| Make a new game of a type and build on it | Partly: `new-game` is a row | Issue 2's game types, and an agent bound to a game rather than a level |
| Manage games and levels (rename, delete, move) | No | The rows of issue 3 |
| Rules, tuning tables, scripts | No | The editor work in `template-gap-analysis.md` (steps 3 to 5); then rows for them |
| Art, sound, models | No | Assets in the editor; an art kind |
| Play it and report | Arena only | Play-test for every game type |
| Publish | No | Publishing is a specification only |

### Delegating work on the engine

The engine is the OpenAgents Metaverse Engine in the `openagents` repository,
vendored here at a pinned commit (`openagents.lock`, `scripts/pin.sh`). The
`engine` row already tells an agent which commit that is and how to propose a
change as a pull request. But a pane agent has no files and no shell, so it
can only say that the engine lacks something.

Engine work is coding work. It needs a coding agent with its normal tools,
in a git worktree of `openagents`, ending in a pull request, followed by a
pin bump here. That is a different kind of agent from a Builder: its output
is a branch and a pull request, its checks are the repository's own, and it
runs for much longer. The pane has no place for it today.

### Seeing it

There is no event stream to draw from. A good picture needs, for each agent,
an ordered list of what happened with enough detail to show it again later.
Today only the live session has that, and it throws it away.

## Since this audit

- **Issue 5, the editor side.** What an agent (or a call from the terminal)
  changed stays outlined in its colour until dismissed: added things are lit
  and outlined, moved and changed ones outlined with a thin ghost where they
  were (moved with a line along the ground), removed ones a ghost alone. The
  words for a change ("added 3 blocks, moved 1, removed 2", the things by
  name and id) are `ch_words`/`ch_json` in `coax/changes.coax`, answered by
  every `apply-edits` and by the `changes` row, and said under the toolbar.
  Show me is `show-changes` (the toolbar's eye), and dismiss
  `dismiss-changes` (eye.slash). At most 64 things are highlighted at once
  (`CH_MARKS`); the oldest highlight goes first.
- **Issue 5, the pane.** `coax/agent-timeline.coax` draws an agent's events
  (the record's `gamebuilder.agent.event.v1` types) as a timeline of cards:
  messages, tool calls folded to one line, a change card with its summary,
  Show me (that change's things framed, `ch_frame_ids`) and Undo this, an
  ask and its answer, usage, how a turn ended. An edit event carries its
  summary as `text`. The row has a live line for the current step, the open
  agent a meter of turns and dollars against 20 and $2.00, and the message
  field is an EditBuf with a caret and a selection that takes a message
  while the agent works. With an agent open the pane is wide and the list
  and the agent sit side by side. The pane keeps its events in memory (the
  last 96 of all its agents); writing them to the record's files is issue
  4's.
- **Issue 9, pictures.** Each change card shows the level before and after
  the change, an open agent has a replay (a slider along its changes, the
  level at each revision), and a play-test's card is a picture of the level
  with the bots' paths, where they stuck and what nobody reached. Pictures are
  drawn by the level's own renderer in the app (the engine's page, or the box
  pass), kept beside the level as `level-N.pictures` (the last 32, each a
  JPEG of at most 8,192 bytes), and read by the `pictures` and `picture`
  rows. The play-test now plays every game type: an arena by a match, every
  other type by a walk-through on the engine's walking grid. See
  docs/agents.md, "Built: pictures, the replay and the play-test picture".

## Recommendations

### 1. Make an agent a record with an event log

One shared Coax module (not in the pane, not in `web/`) that owns:

| Thing | Holds |
| --- | --- |
| `gamebuilder.agent.v1` | Id, the game (and level, when it has one), kind, job in the person's words, where it runs, state, limits, times, the one-line current step |
| `gamebuilder.agent.event.v1` | A numbered event: a message, a tool call with its arguments and a short result, an edit batch with the revision before and after and the things it touched, an ask and its answer, a state change, usage, an error |

Stored as files beside the levels on a computer, and on the account by the
hosted server (`gamebuilder serve --accounts`), which already keeps each
account's levels. An event log is append-only with a cursor, the same shape
as the durable turn's events, so the web reads it the way it already reads a
build agent's.

Rows in the command table, so every surface gets them at once and issue 3's
checks cover them: `agents`, `agent-read`, `agent-events`, `agent-new`,
`agent-message`, `agent-answer`, `agent-stop`, `agent-undo`.

### 2. Put runners behind the record

A runner takes an agent record, does the work, and appends events. The
interface never talks to a runner directly.

| Runner | What it is | State |
| --- | --- | --- |
| Local Claude | The pane's session code, lifted out of the pane | Built, to move |
| Attached | An outside agent on `gamebuilder mcp`; each call it makes is appended to a record it opened | The transport is built; the record is new |
| Hosted | A durable turn on OpenAgents' runner, with the rows as real tools on the account's level through `/mcp`, in place of plan lines | The turn and `/mcp` are built; joining them is new |
| Resident service | `gamebuilder resident`, way A in agents.md, paired with `coder host serve`'s facts | Built (issue 11): `cli/resident.coax` |
| Code | Claude Code with its normal tools in a git worktree, for the engine and for this repository | New (recommendation 5) |

This also removes the three-implementations problem: the chat's build agent
becomes the hosted runner with a record, and shows up in the same list.

### 3. A very basic `/agents` page first

Smallest useful version, in order:

1. **The address.** A new view in `web/site.coax` (as `/login` and
   `/credits` are), its object in the bucket and its line in
   `scripts/web-deploy.sh`. Logged in only.
2. **Read only.** The account's agents grouped by game, one row each: kind,
   job, state, how long, where. Tap one to see its events as a plain list.
   This needs only `agents` and `agent-events` over the hosted API.
3. **The desktop app reports in.** While logged in, the app sends its local
   agents' records and events to the account, as `coax/sync.coax` already
   does for levels. They show on the page as "this Mac". Say plainly in the
   app that an agent's conversation is kept on the account, with a switch.
4. **Start and stop a hosted agent** from the page, on a level the account
   holds. This is where the web world needs to save its level to the account
   (issue 2 and stage 4 of web-world.md).
5. Everything else in agents.md (computers, pairing, cost, claims) after.

Steps 1 to 3 give a real page without any new agent capability, and they
prove the record and the events before anything is built on them.

### 4. Visualisations, drawn from events

All of these read the event log, so they work for a running agent, a
finished one, and one that ran on another computer.

| View | What it shows |
| --- | --- |
| A timeline | Each event as a card in order: message, tool call (folded, with its arguments and result), change, question. Replaces the plain text log |
| A change card | "Added 3 blocks, moved 1, removed 2", the things by name, and a small picture of the level before and after. **Show me** moves the camera there |
| A lasting highlight | What the open agent changed stays outlined in the agent's colour until dismissed, in place of a 1.5 second glow. Added, moved and removed drawn differently, with a ghost where a thing was |
| Scrub | Drag along an agent's changes to see the level at each revision. The revisions and per-agent undo exist; this is a view of them |
| The current step | One line on the row, live: "reading the level", "placing a wall", "waiting for you" |
| A meter | Turns and budget used against the limit, before the limit is hit |
| A play-test picture | The paths the bots took over the level, with stuck spots and unreached places marked. `gamebuilder.playtest.v1` already reports stuck and unreached |
| Several agents | Each agent a colour; their claims drawn as regions on the level |
| Code work | A card for the branch and pull request: files changed, checks passing or failing, screenshots the checks produced, and the pin bump that follows |

Pictures of a level should come from the engine's own renderer, so this
depends on issue 2's step of drawing with it on the desktop. Until then,
counts and names with the lasting highlight are enough to start.

### 5. A Code agent, for the engine and the app itself

- A new kind beside Builder and Playtester. It runs Claude Code with its
  normal tools, in a fresh git worktree from `origin/main` of the repository
  it is given (`openagents` for the engine, this one for the app), under that
  repository's `AGENTS.md`.
- It ends in a pull request or a push, as the repository's rules say, and
  for the engine, in a pin bump here with `scripts/pin.sh` and a passing
  `scripts/check.sh`.
- It needs the unsandboxed build or the resident service, the same as any
  local agent. It should never run inside the app's own process folder.
- The person approves before it starts: it can run commands and change
  files, which a Builder cannot. Say what it may touch, in words.
- Its record and events are the same shape, so the page and the pane show it
  with the code card above.

This is the piece that turns "the engine cannot do that yet" from a dead end
into a job that can be handed over from the same list.

**Built (issue 8):** `coax/code-agent.coax`, as above, with the owner's
answers to questions 3: a pull request for the engine (its clone's hook
refuses `main`), this repository's own rules for the app, and the person's
approval in the New agent sheet before it starts. The pin bump is offered on
its card as the next job, never done by itself. agents.md, "Built: a Code
agent", says what it does; its record and events live with the pane this run
until the record is kept (step 1).

### 6. Fix the pane, as a view

Once the record exists, the pane's rough edges are mostly small:

- Keep agents and their history across launches (the record).
- Carry the conversation into a new session (the event log as context, or a
  persisted session).
- Bind an agent to a game; let it open the level it needs, or work on a
  level that is not on screen through files mode.
- Let the person type while it works; the message is read at the next turn.
- A real text field (the component kit's), with a cursor and selection.
- Show limits as a meter.
- Split the layout: the list on one side, the open agent's timeline taking
  the room it needs.

### 7. Keep it from drifting

The same rule as issues 2 and 3. The record, the events, the states and
their words live in one shared Coax source that the pane, the page and the
CLI load. Checks hold that the pane and the page list the same states with
the same words, that every agent action is a row with a golden case, and
that an event written by one runner reads the same through every surface.

## Suggested order

| Step | What ships | Why here |
| --- | --- | --- |
| 1 | The agent record, the event log and their rows, with the local Claude runner writing to them. The pane reads from the record | Everything else stands on it. Fixes lost agents and lost history at once |
| 2 | `/agents` read only, and the app reporting in | The page the owner asked for, with real agents on it, for little work |
| 3 | The timeline, the change card and the lasting highlight in the pane | The first real visualisation; needs no renderer work |
| 4 | Attached agents get a record | Outside agents on MCP show up in the list |
| 5 | The hosted runner with real tools, started from the page | The chat's build agent becomes a proper agent |
| 6 | The Code agent | Engine and app work can be delegated |
| 7 | Pictures, scrub and the play-test picture | Once the engine's renderer draws on the desktop |
| 8 | The resident service, claims, cost and the rest of agents.md | Several agents at once, agents that outlive the app |

## Open questions for the owner

1. **Is an agent's conversation kept on the account?** The page needs it to
   show a local agent. Proposed: yes while logged in, with a switch, said in
   words when the first agent starts.
2. **Is an agent bound to a game or a level?** agents.md says a game. The
   pane says a level. Proposed: a game, with the level it is working on as a
   detail.
3. **May a Code agent push to `main`,** as this repository's rules have
   agents do, or must it stop at a pull request for a person to merge?
   Answered (issue 8): a pull request for the engine; this repository's own
   rule here; and the person approves before it starts.
4. **Which comes first after the page: better pictures or the Code agent?**
   The order above puts the timeline first because it is cheap, then the
   Code agent because it unblocks engine work.

## What I did not check

- I read the code and the documents; I did not run an agent in the Dev
  build. `programmatic-access.md` says the live run with a signed-in Claude
  Code is the owner's to do; I did not confirm whether it has been done.
- Whether the component kit's DOM backend (`components/backend/dom.coax`,
  which the website loads) could draw the pane's own layout in a browser, so
  the page and the pane share drawing as well as data. Worth an hour before
  step 2.
- The hosted runner's ability to call MCP tools on gamebuilder.org during a
  durable turn. The contract today says the agent has no tools.
- The resident service's current state in OpenAgents beyond what
  agent-mode-audit.md recorded.

# /chat before the zero-base rebuild: every system behind it

Date: 2026-11-11 (Chicago). Code read at `969b110f91`.

We are going to rebuild the chat UI on openagents.com from nothing. This audit
comes first. It covers every system the current `/chat` touches: the page, the
components it draws with, the browser host, the API, the database, the runner
and engine, the protocol, the book's plan for a conversation, and every other
chat surface in the repository. It says what each one does today, what limits
and defects it has, what a new UI can keep, and what a new UI can't have until
something under it changes.

It doesn't design the new UI.

It builds on these and doesn't repeat them:

- [docs/chat.md](../chat.md), the page's own description
- [Protocol review](../protocol/2026-11-11-protocol-review.md) and
  [compliance plan](../protocol/2026-11-11-compliance-plan.md) (tracker #13250)
- [Episode 289 port audit](2026-11-10-episode-289-port-audit.md)
- [Apps roadmap](../apps/2026-11-10-apps-roadmap.md) and
  [components spec](../../components/spec.md)
- The OpenAgents book, `openagents/book/`

## How this was done

Four reviewers read in parallel: the book; the protocol and `proto/`; the API,
database and runner; and the other chat surfaces plus the component system. I
read the page (`apps/openagents-web/chat.coax`, `chat-face.c`,
`chat/index.html`), the component headers and the host's contract myself, and
then checked the highest-stakes server claims in the SQL and the runner.
Claims marked **checked** are ones I read in the source. The rest come from a
reviewer's report with a file and line.

Nothing was built, run or deployed. No live state was read.

## Verdict

**The pixels are the strongest part, and the pipe under them is the weakest.**

- The drawing stack is good and shared: one Markdown parser, one edit buffer,
  one shader, four skins, and a render that matches the Mac app to a mean
  difference of 0.17 per channel.
- The page can't show an answer while it is written. The server keeps no
  partial text, no tool output and no thinking. The page reads a whole-state
  text snapshot every 2 seconds. No new UI fixes that; the API has to.
- None of `/chat`'s wire shapes is in the protocol. All of them sit in
  `proto/wire-allowlist.txt` as debt. A new page that reads the `Z C S U T`
  lines would be written against a format already scheduled for removal.
- The agent behind the page is a read-only Claude CLI over one repository
  checkout. The book and the Episode 289 audit describe a different thing: one
  agent with a typed front, members, approvals and a wallet, the same on every
  surface. None of that exists behind `/chat`.
- The same chat is written five times: web, desktop, the project page's agent
  panel, Aethyrforge, and Coder's terminal. Only two of them share the
  components, and those two each have their own sidebar, tabs and screen.
- The page is invisible to a screen reader, to browser find and to native
  text selection. **(checked)**

So "zero-base the chat UI" is really three decisions: what the page draws,
what stream it reads, and which agent answers. The second and third decide
most of what the first can be. They are listed under
[Decisions the rebuild needs](#decisions-the-rebuild-needs).

## The system, end to end

```
browser                                  Google Cloud
+----------------------------+           +-----------------------------------+
| chat/index.html            |           | oa-api (coax/web/api/chat.c)      |
|  host.js  (WebGPU, input)  |  GET      |  auth gate: admins only           |
|  gpui.js  (glyph atlas)    |  state    |  chat.api / chat.lines (plpgsql)  |
|  chat.wasm                 | --------> |  Postgres schema "chat"           |
|   chat.coax (page logic)   |  every    +----------------+------------------+
|   components/zeron/*       |  2 s/30 s                  | POST /wake
|   components/{core,layout, |                            v
|    paint,skin,backend,kit} |  POST     +-----------------------------------+
|  chat.wgsl (one shader)    |  send,    | runner (coax/web/agent/)          |
+----------------------------+  stop,    |  chat-runner.c, runner.c          |
                                model,   |  chat.coax -> Claude CLI          |
                                tab, key |  checkout of openagents main      |
                                         |  session files in a bucket        |
                                         +-----------------------------------+
```

| Layer | Where | Size |
| --- | --- | --- |
| Page shell | `apps/openagents-web/chat/index.html`, `index.md` | 1.6 KB |
| Page logic | `apps/openagents-web/chat.coax` | 1,536 lines |
| C face (Wasm exports) | `apps/openagents-web/chat-face.c` | 107 lines |
| Chat components | `components/zeron/` | 3,985 lines of Coax |
| Layers under them | `components/core`, `layout`, `paint`, `skin`, `backend`, `kit` | |
| Browser host | `coax/web/host.js` (4,050 lines), `gpui.js` (198) | |
| API | `coax/web/api/chat.c`, `migrations/0021_chat.sql`, `0139_chat_folders_tabs.sql` | |
| Runner | `coax/web/agent/chat-runner.c`, `runner.c` | |
| Engine | `coax/web/agent/chat.coax`, `coax/lib/agent/claude/`, `engine/` | |
| Session record | `coax/lib/agent/session/log.coax` | |
| Checks | `#test`s in `chat.coax`, `chat-frames.coax`, `components/zeron/*-checks.coax`, `coax/web/api/test_zchat.py` | |

## 1. The page

`chat.coax` is one file that holds the page's state, its wire reading, its
transcript building, its layout, its chrome and its input. **(checked)**

**What it draws.** A 260 px sidebar from 860 px wide (New chat, folders, up to
256 conversation rows, the credential line, Key and Theme); tabs across the
top; the transcript in a 736 px column; the composer, centered on an empty chat
and docked otherwise. Narrower than 860 px, Chats, New, Theme and Key move to
the titlebar and the list opens over the page.

**How it gets data.** `wc_ask` reads `/api/chat/state?c=ID`: every 2 s while a
turn waits or runs, every 30 s otherwise, every 5 s after a failed read
(`chat.coax:47-48, 1095-1099`). Each read replaces the whole state. `wc_build`
then throws the transcript away and builds it again from the `U` and `T` lines
(`:511-572`). One post can be in flight at a time, so a click on Stop, a model
pick or a tab close is dropped while another post waits (`:670, :680, :733`).

**What the page parses by hand.** All of it. `wc_cell` splits lines on `|`,
`wc_decode` undoes `\n`, `\\` and `\p`, `wc_value` scans a JSON answer for a
key, `wc_json` escapes a JSON string, and `wc_cents` reads money
(`:160-380`). This is the hand-written wire code the protocol rule forbids.

**Limits in the page, none shown to the reader:**

| Limit | Where | What the reader sees |
| --- | --- | --- |
| The transcript holds 256 rows, 64 tool groups, 256 tools, 64 notices, 65,536 bytes of Markdown, 256 Markdown blocks | `components/zeron/transcript.coax:24-25,39`, `core/markdown.coax:36-39` | When a chat outgrows any of them, the page drops the oldest turn and builds again (`chat.coax:1147`). Earlier turns vanish with no "earlier messages" marker and no way back |
| 16 tools per turn in the page; the server sends the last 12 | `chat.coax:528` | A turn that used 40 tools shows 12, and the summary counts 12 |
| 256 rows in the conversation list, 8 folders, 32 tabs | `chat.coax:68,71,867` | More are not drawn. Tabs that don't fit the width are skipped (`:928`) |
| A message of 24,600 bytes of JSON | `WC_BODY`, `:38` | `wc_json` stops writing at the buffer's end (`:312`); the server's own limit is 8,000 characters |
| The whole state must fit 1 MB | `WC_DATA`, `:33` | A longer state is cut mid-line |

These break the owner rule that every cap is shown on screen.

**What it does not have:**

- Streaming. A running turn shows the tool chips and a trailer with a flavour
  word and seconds. The answer appears whole when the turn ends.
- Accessibility. The canvas is `aria-hidden`. The page's `<main>` is one link
  to `index.md`. The host has an accessibility mirror (`coaxial_a11y`,
  `host.js:2893`), and `/chat` doesn't export it. **(checked)**
- Browser find, native selection, link right-click, text zoom and reader
  mode. The components do their own selection and copy.
- Links to a conversation. The shown conversation is page state. The URL
  never changes, so reload goes to the newest chat, and Back leaves the page.
- Rename, delete, search, edit, regenerate, attachments, approvals. The API
  has none of them (section 4).
- A fallback. Without WebGPU the page says so and links the Markdown page.

**Stale words in the page.** A comment says the cap is "$5.00"
(`chat.coax:816`); the cap is $10 and the text reads it from the state, so the
screen is right. The flavour words under a running turn ("Zeroning",
"Combobulating", `transcript.coax:45-47`) are the filler the owner rule on
status lines forbids.

**What is worth keeping from the page:** the optimistic send that returns the
message to the composer with the reason when it fails (`:428-433`); wiping a
credential from memory once posted (`:340-344`); remembering which tool groups
the reader opened across rebuilds (`:513-514`); the `#demo` and golden modes
for comparing renders.

## 2. The components

The layer system from `components/spec.md` exists as code: `core` (edit
buffer, Markdown parser, syntax, text, colour), `layout`, `paint` (the display
list), `skin` (Zeron, Everglade, Verse, Terminal), `backend` (GPU and its one
shader, terminal). Phases 1, 2 and 5 are done.

The chat set has not moved into `kit/`. It is still `components/zeron/`:

| File | Lines | What |
| --- | --- | --- |
| `transcript.coax` | 1,125 | Rows, bubbles, tool groups, notices, selection, the working trailer |
| `composer.coax` | 1,008 | The expanded and docked composer, chips, send and stop |
| `picker.coax` | 853 | The model menu and the access menu (Theme reuses it) |
| `attachments.coax` | 273 | Inline file chips, image blocks |
| `tools.coax` | 214 | Tool chips, call detail, output lines, diffs |
| `chrome.coax` | 127 | The page's buttons, field and failure notice |

`kit/` holds only `markdown`, `controls` and `hud`. The spec says `zeron/` is
"emptied as Phase 2 splits each file into the layers above"; that hasn't
happened, and no open issue tracks it.

Facts a rebuild should know:

- **The files load as one chain** (gpui, composer, markdown, tools,
  transcript, picker, chrome). You can't take the composer without the rest.
- **Fixed capacity everywhere.** Every component is arrays of a fixed size
  (the table in section 1). That is the Coax style, and it is why long chats
  lose their start. A new transcript needs windowing: lay out what is near the
  viewport and keep the rest as record lines.
- **The set already draws more than the web page uses.** Approval notices
  with Allow and Deny (`A_ALLOW`, `A_DENY`), thought chips, edit diffs, tool
  output (24 lines), attachments with image blocks, effort levels, an access
  chip and a plan toggle are all in the components. The web page turns them
  off because the API has nothing behind them (`chat.coax:640-642`).
- **The chat set does its own arithmetic**, not the flex layout (spec
  170-173), and the element tree's focus and accessibility don't read the
  element table yet (spec 114-115). Accessibility is unbuilt at the component
  level, not only on this page.
- **One layer breach:** `tools.coax` loads
  `coax/lib/agent/session/kinds.coax` from outside `components/`.
- **Sidebar and tabs are not components.** The desktop app has
  `dsk_sidebar_prims` and `dsk_tabs_prims` in
  `apps/openagents-desktop/zeron/desk.coax`; the web page paints its own in
  `chat.coax:839-945`.
- **`AGENTS.md` contradicts itself:** its second `components/` row says "the
  spec's layers and skins are not built yet". They are.

## 3. The browser host

`host.js` gives a Coaxial page a canvas, WebGPU, input, storage and three
ways to talk to the site (`host.js:68-91`): **(checked)**

- `coaxial_http`: one request, the whole answer back as one message. Eight
  slots. This is what `/chat` uses.
- `coaxial_http_bounded`: the same with a deadline; streamed bytes are
  gathered and delivered once.
- `coaxial_socket_open`: a WebSocket byte pipe, text frames only, four at
  most. Everglade's rooms use it. `/chat` doesn't.

There is no way today for a page to receive an HTTP answer in pieces: no
Server-Sent Events, no incremental fetch. A streaming chat needs either the
socket (text frames, so binary protocol frames need an encoding or a host
change) or a new streaming import.

Text entry goes through a hidden text field the host places at the composer's
caret, which is how IME, paste and the phone keyboard work. The atlas
(`gpui.js`) rasterizes Geist in the browser at four subpixel positions and
measures advances with the same font files, so line breaks match the Mac's.

Every frame repaints. "Draw when something changed" is built only as the Linux
host's frame comparison (apps audit, rec. 6). An idle chat tab draws at the
display's rate.

## 4. The API

`coax/web/api/chat.c` is a thin C file. The logic is plpgsql: `chat.api` and
`chat.lines`.

**Who gets in.** Admins only, by a NIP-98 signature, a `__Host-oa_session`
cookie, the `oa_admin` cookie or the legacy `oa_session` cookie; the key must
be in `admins.conf` or `access.admin_keys`. Cookie posts need the site's
`Origin`. Posts are limited to 60 a minute per key, 6 for `key`. Reads are
not limited.

**Routes.**

| Route | Body | Answer |
| --- | --- | --- |
| `GET state?c=ID` | | The text lines below |
| `POST new` | `{model}` | `{c}` |
| `POST send` | `{c?, text, model?}` | `{c, turn, model}`; 400 (1 to 8,000 characters), 409 a turn is running, 429 over the cap |
| `POST model` | `{c, model}` | `{c, model}` |
| `POST stop` | `{c}` | `{turn}`; 409 nothing running |
| `POST tab` | `{c, open}` | `{c, tab}` |
| `GET record?c=ID` | | `openagents.session.v1` JSON lines |
| `POST key`, `key_remove` | `{credential}` | `{kind, label, last4}` |
| `POST runner/next`, `event`, `end` | NIP-98, the runner's key | |

Any other `/api/chat/<name>` is the Verse room chat (`room.c`). The two share
a path prefix and nothing else.

**The state lines** (`0139_chat_folders_tabs.sql:96-124`). **(checked)**
Fields are split by `|`. Long text escapes `\` as `\\`, `|` as `\p` and a
newline as `\n`.

```
Z|spent|cap|credential kind|last4|expired|busy|shown id|2
C|id|title(80)|model|updated|busy|folder|tab         60 newest, plus open tabs
S|id|title|model|resumes
U|id|status|asked|model|credential|text(8000)|answer(24000)|error(400)|working(160)|cost
T|turn|tool name(40)|detail(200)                     last 12 per turn
```

Times are `Mon DD HH:MI` in Chicago with no year. The state holds the last 40
turns.

**What the state cuts without saying so:**

- An answer is stored to 60,000 characters and sent to 24,000. Nothing marks
  the cut.
- Turns before the last 40 are not sent, and no route pages back.
- Conversations past the newest 60 are not listed, and no route lists more.
- Tools past the last 12 of a turn are not sent.

## 5. The data

Schema `chat`:

- `conversations`: owner, title (the first message's first 80 characters),
  model, session id, status, folder, tab.
- `turns`: text, model, credential (`own` or `openagents`), status (queued,
  running, done, failed, stopped, refused), stop flag, times in ms, cost,
  working line, answer, error.
- `events`: turn, time, type, data. Types: `user.message` and
  `agent.message` (**only their length**), `user.interrupt`,
  `agent.tool_use` (name to 60, detail to 300), `session.status_running`,
  `session.status_idle`, `session.error`.
- `credentials`: owner, kind, last four, the sealed bytes, expired.

`chat.model_ok` accepts three Claude ids. `chat.folder_ok` accepts
`openagents`. `chat.spent()` sums today's cost on our key across everyone.
Deleting an account purges all four tables.

Nothing is ever pruned: conversations, turns, events and the session files in
the bucket grow without bound.

## 6. The turn: runner and engine

1. `send` inserts a queued turn. `chat.c` forks a child that wakes the runner
   (up to 20 s) and only then answers the browser (`chat.c:343-355`).
   **(checked)** Send latency includes the wake.
2. The runner runs one chat loop. It claims the oldest queued turn. All
   `/chat` turns, for every admin, run one at a time.
3. It refreshes a sparse checkout of main, opens the credential, restores the
   CLI session file from the bucket and starts the engine.
4. The engine (`coax/web/agent/chat.coax`) runs the Claude CLI through the
   Coax Claude Agent SDK: at most 30 model turns, a budget of what is left of
   the cap, Read, Glob and Grep confined to the checkout, WebSearch and
   WebFetch. Permissions are decided by code; nobody is asked.
5. While it runs, events go to oa-api in batches every 4 s or 40 events.
   oa-api keeps each tool's name and one detail, and the length of each text
   part. **It discards the text** (`0021_chat.sql:250`). **(checked)**
6. The answer is the CLI's final `result` text, posted at `end`. Text the
   model wrote between tool calls is not in it.
7. Stop sets a flag. The runner learns of it on its next event batch, tells
   the engine, and kills it 30 s later if needed. A stopped turn's answer is
   empty: what the model had written is thrown away (`chat.coax:311`).
   **(checked)**

Limits: 15 minutes per turn in the engine, a 20-minute stale sweep, 30 model
turns, $10 a day on our key for everyone together, no cap on your own key.

### Defects

| # | Defect | Where |
| --- | --- | --- |
| D1 | **A conversation can lock for good.** A turn left `running` (the runner died) is failed only by the sweep inside `next`, and `next` runs only after a `send` succeeds. A send into the stuck conversation gets 409 before any wake. It unlocks only when some other conversation sends. The sweep also leaves `conversations.status = 'running'` **(checked)** | `0021_chat.sql:177-178, 216` |
| D2 | A failed wake is only logged. `send` answers 200 and the turn sits queued | `chat.c:348-351` |
| D3 | A stopped or timed-out turn loses everything the model wrote | `chat.coax:311` |
| D4 | An `end` that arrives after the sweep gets 409 and the answer is lost | `0021_chat.sql:241-242` |
| D5 | One serial queue for every admin's turns | `chat-runner.c:361` |
| D6 | The checkout's git commands have no timeout. Open: #13252, the runner runs out of memory checking out the repository | `runner.c:295-310` |
| D7 | Spend is recorded at `end`, so turns in flight don't count against the cap | `0021_chat.sql:88-92` |
| D8 | `tab_open` takes `max(tab)+1` with no unique constraint; two opens can share a number | `0139:33` |
| D9 | The runner detects stop by matching the literal text `"stop": true` in Postgres's JSON output | `runner.c:423` |
| D10 | Reads of `state` are not rate limited, and every poll runs the full query | `chat.c` |
| D11 | The record's tool results are empty, because tool output is never kept | `0139:80` |

### What a UI cannot get from this server

| Wanted | State |
| --- | --- |
| The answer as it is written | Doesn't exist. Partial messages are off in the engine, and oa-api drops the text it is sent |
| Thinking | Not emitted |
| Tool output, full tool input | Not kept. It exists only in the CLI session file in the bucket |
| Approvals, questions | Don't exist. Permissions are automatic |
| Attachments, effort | Don't exist |
| Edit, regenerate, branch | Don't exist. Turns are immutable |
| Rename, delete, search, share | No routes. The schema would cascade a delete |
| Paging older turns or conversations | No routes |
| Exact times | Stored in ms; only `record` exposes them |
| Anyone who is not an admin | Refused at the gate |

## 7. The protocol

The rule (`docs/protocol/2026-11-11-protocol-review.md:93-94`, book ch. 52,
`docs/reuse.md`): a page or a route never invents a JSON shape, and
`coax/web/tool.sh wire-check` fails on an unregistered one.

`/chat` invents all of its shapes. They pass the check only because they are
on the allowlist (`proto/wire-allowlist.txt`):

| Shape | Allowlist line | Planned step |
| --- | --- | --- |
| The page lines `Z C S U T` | `shape chat-page-lines` | C9, wave 2 |
| Every `/api/chat` route body | `route coax/web/api/chat.c` | C9, after C5 |
| Runner lines (`turn`, `event`, `call`, `reply`, `done`, `stop`) | `shape hosted-runner-lines` | C5, #13242 |
| Runner to API event batches and end bodies | `shape runner-api-bodies` | C5 |
| `chat.record`'s output, built in SQL | `shape chat-record` | C2, #13239 |
| The sealed credential | `shape sealed-chat-credential` | a golden test |

The compliance plan notes that chat is fifth in its order, "though the owner
asked for chat first" (verdict 7).

**What exists that a new UI could read instead:**

- **The session log**, `coax/lib/agent/session/log.coax`: lines `session`,
  `user`, `thinking`, `reply`, `tool`, `result`, `routed`, `notice`, `ask`,
  `end` (with cost). Coder, the desktop app and Aethyrforge read it, and
  `GET /api/chat/record` writes it. It has no schema file yet (C1, #13238),
  and its id `openagents.session.v1` collides with the spec's NIP-SESS; the
  plan renames it `openagents.session.log.v1alpha1`. It is the strongest
  candidate for what a transcript is built from.
- **The reply stream**, `coder.reply-stream.v1` (`reply_delta`, `reply_end`,
  `segment`): a JSON Schema, written by `session/writer.coax`, read by
  Aethyrforge's chat. C3 replaces it with one codec.

**What is only a sketch:** `openagents.chat.v1` (spec 17.1: Thread,
ThreadMessage, ConversationRequest, ConversationFeedback with partial deltas,
cards and offers, ConversationResult) and `openagents.agent.managed.v1` (spec
18.2). `docs/chat.md` says the events follow the second; no `.proto` for
either exists. Spec 17.1's "Implemented" status cites crates deleted in the
Nuke.

**What the spec says the carriage is:** one message, three carriages (Nostr
kind 25900, HTTP, a sync frame); session events over Server-Sent Events
(spec 31.3); chats as append-only logs over one WebSocket per device at
`/api/sync` in binary `sync.v1alpha1` frames (`docs/sync/README.md`).
`proto/openagents/sync/` doesn't exist. `/chat` uses none of these.

**No schema at all for:** streaming deltas on the server side, approvals and
questions, tool calls and results as messages, attachments, cost and model
meta, the conversation list, folders, tabs, model choice, stop, credentials.

## 8. The book

The book is at `openagents/book/`. Every chapter that bears on chat is a
design draft or an outline; `README.md` says "the book's own code is not built
yet", and every planned path it names is absent. It is the plan of record, not
a description.

What it requires of a chat surface, most binding first:

1. "A page or a route never invents a JSON shape." (ch. 52; enforced)
2. "There is one conversation, with no switch between a chat mode and a work
   mode." (ch. 61, ch. 2; also an owner rule)
3. "A control that does nothing is not drawn." (ch. 61)
4. "It is 'Log in', and the path is `/login`." (ch. 50; built)
5. "No surface has its own agent logic." (ch. 2)
6. Our own apps use the public API: "There is no private API with more in
   it." (ch. 48)
7. The stream is "a typed stream of route, plan, approvals, steps and result".
   (ch. 48)
8. An approval is drawn the same on every surface: "what will happen, to
   what, and what it costs." (ch. 7)
9. "If the model saw it, it is in the log." (ch. 8)
10. The conversation "doesn't fill the screen with status lines or with
    warnings nobody asked for." OpenAgents says "we"; agents say "I". (ch. 9)
11. Each subagent has a card, its full history and the model it ran on.
    (ch. 16)
12. "Each step in a record carries what it cost." (ch. 54)

Where today's `/chat` contradicts it:

| Topic | Today | The book |
| --- | --- | --- |
| Transport | Polled text lines | A typed stream (ch. 9, 48, 61) or sync frames (ch. 51) |
| Model path | The Claude CLI on an Anthropic key, with a model picker | The gateway is "the only path to any model", Google first (ch. 54); a front routes each message (ch. 5). Whether a person picks the model is an open question (ch. 9) |
| Spending | Our $10 daily cap, 429 | "We do not set a limit for you" (ch. 20) |
| The record | Tool output not kept | Everything the model saw (ch. 8) |
| Scope | Admins, read-only, one repository | Every account; members, approvals, Coder, the wallet |

They agree on the record format, server-kept tabs, and showing the model and
cost under each answer.

## 9. Every other chat surface

| Surface | Draws with | Talks to | State |
| --- | --- | --- | --- |
| `/chat` (`apps/openagents-web/chat.coax`) | `components/zeron` | oa-api, the hosted runner | Live, admins |
| Desktop (`apps/openagents-desktop/zeron/`: `screen` 1,486, `desk` 1,362, `claude`, `modes`, `room`, `settings`, `store`) | `components/zeron` | Local engines in `coax/lib/agent/engine/` (Claude, Codex, Psionic, Lab) | Builds on Linux and the shared Mac host |
| Project agent panel (`apps/openagents-web/private.coax`, `/projects/SLUG`) | Hand-rolled | `/api/talk`, runner `turn.coax` | Live. Has diff cards, Stop and Undo turn |
| Aethyrforge (`apps/aethyrforge-desktop/chat.coax`, `Sources/chat.m`) | Hand-rolled HUD feed and AppKit text | The shared turn runner, in process | Linux checked |
| Coder's terminal (`coder/tui/`, `coax/lib/tui/`) | `core/markdown`, `core/edit`, `backend/terminal`, its own palettes | Shared engines | Live |
| WorldBuilder's box (`worldbuilder.coax`) | `kit/controls`, `core/edit` | `/api/worldbuilder/act` | Moved onto the kit; #13226 open |
| Verse room chat (`room.coax`) | Each realm's HUD | `/api/playtest/chat/<room>` | Live; unrelated to the agent |
| iOS | No text chat; voice captions only | | |
| Old HUD chat (`apps/openagents-web/chat-hud.coax`, 1,362 lines) | Hand-rolled | `/api/chat/state` | **Dead.** Only `apps/openagents-desktop/face.h` still names it, and nothing builds either **(checked)** |

What repeats:

- **Two session screens** (desktop `screen.coax` and the body of web
  `chat.coax`) and **two sidebars with tabs** (`desk.coax` and
  `chat.coax:839-945`).
- **Two transcript builders:** the desktop builds rows from the session
  record (`zeron/claude.coax`); the web builds them from `U` and `T` lines.
  #13239 is the one-reader issue.
- **Three hand-rolled conversation panels:** the project agent, Aethyrforge
  and Coder's terminal.
- **Two hosted agents:** `/chat`'s general mode and the project agent's
  `turn.coax`, on the same runner with different routes, tables and wire.

What the other documents ask of chat:

- **Apps roadmap, step 3.3:** next to the desktop app, `/chat` lacked
  folders, tabs, the shared record, and "access for people who are not
  admins". The first three are built. Access is the owner's call.
- **Episode 289 audit:** P1, one agent behind every surface, is "partial …
  no one shared agent runtime". Coax `/chat` lacks the typed front through
  the Decisions API, visible members with claim status, routes beyond a
  repository question, command-line members such as the wallet with money
  approval, knowledge-base answers, and building a missing plugin from chat.
- **Desktop doc:** not yet ported from Zeron: mentions, hover timestamp and
  copy, approvals in the transcript comparison, sub-agent frames.
- **Desktop `modes.coax`** puts Claude, Psionic and Lab tabs on the model
  list. Check it against the no-modes rule before copying it.

## What to keep, delete and replace

**Keep.**

- The layers: `components/core` (edit buffer, Markdown), `layout`, `paint`,
  `skin`, `backend`, and the atlas in `gpui.js`.
- The session log as the thing a transcript is built from.
- The credential path: checked before it is kept, sealed in the runner, never
  shown, one store for `/chat` and projects.
- The confinement of Read, Glob and Grep, and the per-turn environment
  allowlist.
- Golden mode and the `#demo` comparison against the Mac.

**Delete.**

- `apps/openagents-web/chat-hud.coax`, and the orphans beside it in
  `apps/openagents-desktop/` (`face.c`, `face.h`, `route-face.c`,
  `router.coax`). Confirm `ui.coax` has other users first.
- The page's hand-written wire code (`wc_cell`, `wc_decode`, `wc_value`,
  `wc_json`), with the line format.
- The flavour words in the working trailer.

**Replace.**

- `chat.lines` and the 2-second poll, with a registered stream.
- `chat.events` as lengths only, with the session log's lines kept in full.
- The page-private sidebar and tabs, with one component both the web and the
  desktop draw.
- Fixed-size transcript arrays that drop the oldest turn, with a windowed
  transcript and paging.

## Decisions the rebuild needs

These are the owner's. Each one changes what the page is.

1. **Which agent answers.** The read-only Claude CLI over one checkout, as
   now, or the book's one agent (front, members, approvals), which isn't
   built. A UI built for the first has no approvals, no members and no
   wallet to draw.
2. **Who gets in.** Admins only, or every logged-in account. Opening it needs
   a per-person spend rule in place of one shared $10, a queue that isn't
   serial, rate limits on reads, and D1 fixed.
3. **The stream.** Server-Sent Events of session-log lines (spec 31.3; needs
   a streaming import in `host.js`), the existing WebSocket pipe, or sync
   frames (`/api/sync`; not built). Whichever it is, register its schema
   first: the compliance plan has chat fifth, and a rebuild puts it first.
4. **Pixels or DOM.** Staying on the canvas keeps one component set for web,
   Mac and Linux and the Mac match. It costs accessibility, find, native
   selection and links until the element tree's accessibility is built
   (spec 114-115) and `/chat` exports the mirror. This audit doesn't pick;
   it notes that accessibility is at zero today.
5. **Where the chat set lives.** Rebuilding the UI is the natural moment to
   move `components/zeron/` into `kit/` as the spec says, with the sidebar
   and tabs as components, so the desktop gets the same rebuild.
6. **The model picker and the cap.** The book leaves the picker open (ch. 9)
   and says we set no limit for the user (ch. 20). Today's page has both.
7. **One hosted agent or two.** `/chat` and the project agent share a runner
   and a credential store and nothing else. A rebuild can make the project
   panel the same conversation with a project in scope.

## What the server must gain before any new UI can show it

In dependency order. None of this is UI work.

1. Keep every session-log line the engine writes (text, thinking, tool input
   and output), not lengths. Fixes D3, D11 and the record.
2. Turn on partial messages in the engine and carry deltas to oa-api.
3. A registered schema for the log and the stream (C1, C3, C5 pulled
   forward), and one codec the page loads.
4. A stream route, and a host import that delivers it in pieces.
5. Fix D1 and D2: sweep stale turns on `send` and `stop`, and fail a turn
   whose wake failed.
6. Paging for turns and conversations; rename and delete.
7. Then, as decided above: approvals, attachments, more than one turn at a
   time, access beyond admins.

## Stale documents found

- `docs/chat.md`: the refusals are "Log in first." and "Admins only.", not
  the strings it gives; it omits `__Host-oa_session` and
  `access.admin_keys`, the `session.status_idle` event, migration 0139 in
  Deploying, the 8,000-character message limit, the 24,000-character answer
  cut, the timeouts, the rate limits and the one-turn-at-a-time runner; its
  sparse-checkout list and sizes are from 2026-10-28 and no longer match the
  runner. It says `chat-hud.coax` is "kept for the Mac app's unshown HUD";
  nothing builds it.
- `coax/web/api/chat.c:22-23` still says `/signin`.
- `AGENTS.md`: the second `components/` row says the layers and skins are
  not built.
- Protocol spec 17.1: "Implemented" cites deleted crates.
- `docs/sync/README.md` says the sync frames are defined in
  `proto/openagents/sync/v1alpha1/`, which doesn't exist.

These are left as found. This change adds only this audit.

## Not read

The bodies of `components/zeron/*.coax` past their headers and constants; the
desktop `screen.coax` and `desk.coax` bodies; `coax/lib/agent/claude/`
internals; the auth gate's internals; the deploy scripts; `SUMMARY.md` and
the book chapters outside those listed in section 8; the protocol spec
outside sections 8, 17, 18 and 31.

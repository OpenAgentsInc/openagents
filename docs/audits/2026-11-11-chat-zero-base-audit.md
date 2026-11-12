# /chat before the zero-base rebuild: every system behind it

Date: 2026-11-11 (Chicago). Code read at `969b110f91`.

We are going to rebuild the chat UI on openagents.com from nothing. This audit
comes first. It covers every system the current `/chat` touches: the page, the
components it draws with, the browser host, the API, the database, the runner
and engine, the protocol, the book's plan for a conversation, and every other
chat surface in the repository. It says what each one does today, what limits
and defects it has, what a new UI can keep, and what a new UI can't have until
something under it changes.

It doesn't design the new UI's screens. It does settle how the web chat is
drawn (section 10) and its look and component set (section 11).

It builds on these and doesn't repeat them:

- [docs/chat.md](../chat.md), the page's own description
- [Protocol review](../protocol/2026-11-11-protocol-review.md) and
  [compliance plan](../protocol/2026-11-11-compliance-plan.md) (tracker #13250)
- [Episode 289 port audit](2026-11-10-episode-289-port-audit.md)
- [Apps roadmap](../apps/2026-11-10-apps-roadmap.md) and
  [components spec](../../components/spec.md)
- The OpenAgents book, `openagents/book/`
- [Exact research](../research/exact/README.md) and [coax/web.md](../../coax/web.md)
  (section 10, added later the same day)

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
- The web chat is behind the desktop chat on the agent loop. The desktop
  chat can be driven by id and compared as text; the web page can't, though
  both draw with the same components. **(checked)**
- Exact, which has our shape (a checked language, a compiled form, thin
  hosts), uses the real DOM on the web and its own painter only where there
  is no standard. Our research declined that for worlds. Chat was not in
  scope when the canvas-only decision was made (section 10).
- **Decided:** on the web the chat is drawn by the browser, from Coax
  components, through a new DOM backend. Worlds stay on the GPU
  (section 10).
- **Decided:** the chat gets a new, small component set with a grayscale
  ARWES look. The previous chat set is not carried over. We already ported
  ARWES's frame geometry and motion to Coax; its frames become SVG behind
  real text, which is how ARWES itself draws them (section 11).

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
| Chat components | `components/zeron/` | 3,938 lines of Coax |
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

## 10. Coaxial on the web, and what Exact does instead

Added 2026-11-11 after reading `coax/web.md`, `coax/web/HTTP.md`,
`coax/start-here.md`, `docs/verse/agent-clock.md`, the header of
`coax/web/drive.coax`, and all of
[docs/research/exact/](../research/exact/README.md) (five files). I also read
the description and file list of Exact's chat template in the reference
clone (`projects/repos/exact-templates/chat`), not its source.

### How a Coaxial page works

- **Build.** Coax compiles to one C11 file, clang compiles that to wasm32,
  and `wasm-ld` links it (`coax/web/wasm.sh`). There is no runtime in the
  output and no bundler. Shaders are emitted from Coax as WGSL.
- **Host.** `host.js` is "the browser's Swift": WebGPU, input, fetch,
  sockets, audio, the hidden text field, the accessibility mirror. Logic
  doesn't go there.
- **State.** Coax has no globals and no heap. All of a page's state is one
  struct in one `WebAssembly.Memory` the host owns (`WcHost` for `/chat`).
- **Frames.** Immediate mode. Every frame the host calls `coaxial_frame`,
  and the page writes the whole vertex buffer again. Nothing is retained
  between frames and nothing is diffed.
- **Hot reload.** `coax serve` rebuilds an engine in 118 to 135 ms and swaps
  it against the same memory. If the state's layout fingerprint is unchanged,
  all state survives. If one field changed, the whole state starts over.
- **Release.** `coax/web/release.sh`: build once from committed code, deploy
  to staging, compare staging's checks with production's, promote the same
  bytes, roll back on a new failure.
- **HTML** is used only where a canvas can't do the job: text input,
  the accessibility mirror, links and copying, and `index.md` for agents and
  crawlers.

The founding decisions (`coax/web.md`, 2026-10-28) are WebGPU only with a
text fallback, the GPU owns the screen, and "a DOM component system" is a
non-goal. Two things in that document bear on chat:

- The decision listed what openagents.com needed at the switch, and said
  "no chat, sign-in, API pages or payments in this site for now". The
  canvas-only rule was written for the splash and the worlds. Chat arrived
  the next day.
- Its open question 6 recommends chat in the canvas "with the text version
  kept for agents and no-WebGPU visitors", and question 5 recommends that a
  visitor without WebGPU gets "the text site (chat still works, with HTML
  forms)". Neither was built. `/chat/index.md` describes the page; it is
  not a chat, and a visitor without WebGPU can't use `/chat` at all.

`coax/web.md` also says the browser speaks protobuf, "one message per
WebSocket binary frame … No JSON on the hot path". The host's socket carries
text frames only (`host.js:68`). A chat stream in binary sync frames needs
that host change first.

### What Exact does on the web

Exact (Expo, released 2026-11-01) has the same shape as our stack: a
constrained, checked language, a compiled form, thin hosts. Its web host
makes the opposite choice from ours:

| | Exact on the web | Coaxial |
| --- | --- | --- |
| Output | The plan compiled to a JS module; about 20 KB of runtime, tree-shaken | Wasm with no runtime; `/chat` also loads four fonts and builds an atlas |
| Pixels | The real DOM. The browser lays out and draws | Ours, in one WebGPU canvas |
| First paint | HTML pre-rendered at build time, then "adopted in place instead of hydrated" | A blank canvas, then "Loading", then the first state read |
| Updates | Signals: one effect per dynamic binding, no tree diff | The whole vertex buffer, every frame |
| Text, selection, find, links, translate, zoom | The browser's | Ours, or absent |
| Accessibility | HTML roles and `aria-*`; `testId` serves the driver, tests and assistive tools at once | A mirror of buttons, checkboxes and one status line, which `/chat` doesn't export |
| Agents using the app | `tool` declarations registered through WebMCP, with schemas and consequence hints | None |

Exact draws its own pixels (its Ink painter) only "where there is no
standard": Linux and Windows. On the web it uses the platform. Its
principle is "write the web, ship the platform".

The research already ruled on this (`03-what-to-adapt.md`, "What not to
adopt"): no second UI language, no platform controls, no DOM rendering,
because they conflict with the 2026-10-28 decisions. It also names the
price: "we give up the free accessibility and crawler text that Exact gets
from the DOM." It set out to recover "the agent-facing half of that" with a
retained tree and declared tools. The half that faces people (a screen
reader, find, selection, translate, text zoom, reader mode) was left
unrecovered. For worlds that price is small. For a chat it is most of the
product: a chat is text.

### Where the research's nine phases stand for /chat

| Phase | State in the repository | State for web `/chat` |
| --- | --- | --- |
| 1. One driver, targets by id | Built for realms on the web (`coax/web/drive.coax`) and for the shared native hosts (`apps/shared/host/drive.c`) | **Not drivable by id.** The desktop chat fills the element table (`dsk_elements`, `desk.coax:1287`); `chat.coax` doesn't load `components/core/elements.coax` and has no tree to answer with **(checked)** |
| 2. An agent clock | Built for web realms and the native hosts | `/chat` says nothing about being busy, so `clock settle` can't wait for a turn. Our driver has no `clock data` (wait for pending answers), which a page that waits on a server needs |
| 3. Drive scripts | Not landed | The page's only checks are `#test`s on its helpers and frame goldens |
| 4. State kept field by field on reload | Not built | One new field in `WcState` resets the page |
| 5. Tools an agent can call | Not built | None |
| 6. The on-ramp | Built (`coax/start-here.md`, `coax vocab`) | |
| 7. Native drive hosts | Built | The desktop chat answers `tree`, `tap ID`, `type ID TEXT`, `state NAME` |
| 8. A retained tree with flex | The element table exists (`components/core/elements.coax`); flex exists and matches Chrome to 0.05 px (`components/layout/flex.coax`); only `kit/hud.coax` uses flex | The chat set uses neither on the web |
| 9. A baked first frame | Not started | The empty new-chat screen is the same for every visitor and is a natural first frame to bake |

So the desktop chat is ahead of the web chat on the agent loop, and both use
the same components. The gap is in `chat.coax`, not in the components.

Two more points from the apps audit (`04-apps-audit-2026-11-10.md`) apply
directly:

- **Draw when something changed.** The Linux host now presents 2 to 4 frames
  a second when idle where it presented 60. The web host has no such rule,
  and the `quiescent` export that would let a page skip its own frame is not
  built. A chat is still for minutes at a time.
- **Compare in text first.** The desktop chat's checks compare the element
  tree row by row before any picture. The web chat's compare pictures.

### What Exact's chat template has that ours doesn't

Exact's `chat` template is a person-to-person messenger, not an agent chat,
and it is 1,496 lines with its tests and seed data. From its description:

- A virtualized transcript "built only where it shows", which opens at its
  newest message and follows it.
- "240 earlier messages put above the reader without moving what they read."
- Routes: each conversation has a URL; the list sits beside the conversation
  from 900 px and is a pushed screen on a phone.
- Search over conversation names, then message text.
- A sent message shown from the send, before it is saved, until the answer
  has it.
- 217 lines of driver-script tests that run on the web and on iOS.

The first three are exactly the hidden limits in section 1: our transcript
drops its oldest turns, has no paging, and has no URL per conversation.

### What to take for the rebuild

Whatever is decided about pixels, these carry over and none conflicts with
a Coaxial decision:

1. **One id per row, used three ways.** Build the web chat on the element
   table. The driver's `tree`, the tests and the accessibility layer all read
   it, as Exact's `testId` does.
2. **The page says when it is busy.** One export answers "is anything
   scheduled?" The host stops drawing when it says no, and `clock settle`
   waits on it. Add `clock data` to the driver for pages that wait on a
   server.
3. **Drive scripts are the page's tests,** against a fixture API with the
   agent clock, compared as text first. Check the first screen and each
   test's end in every skin, at two widths and at the largest text size.
4. **A windowed transcript with paging,** following its end, and a route per
   conversation, as the template has.
5. **A first frame at build time** for the empty new-chat screen.
6. **Declared tools.** `send`, `new chat`, `stop` and `list conversations`
   declared once with a consequence word, checked against the effects
   inventory, and offered through WebMCP and the CLI's MCP server. The
   research's review says tools are deeds; for chat they are the session
   record's requests.
7. **State kept field by field on reload,** so working on the page doesn't
   reset it on every struct change.

### Decision: the web chat is drawn by the browser

Decided 2026-11-11. The owner set the earlier rulings aside and asked for the
best answer. It is this:

**On the web, a text surface is drawn by the browser. A world is drawn by
the GPU. Both come from the same Coax components.**

The chat is rebuilt on a third backend, `components/backend/dom.coax`,
beside `gpu` and `terminal`. The GPU backend keeps drawing the same chat on
the Mac, on Linux, on the phone and inside a realm.

**Why.** A chat is text that people read, select, search, copy, translate,
enlarge and have read aloud. The browser already does all of that, and
today's page does none of it. Getting it back on a canvas means writing a
screen-reader bridge, find, selection, translation and zoom ourselves, for
one platform, and never quite matching. Exact reached the same answer from
the same starting shape: its own painter where there is no standard, the
platform where there is one. The reasons for the canvas (one renderer, a
match with the Mac, 60 frames a second) are reasons for worlds. A page that
is still for minutes at a time gains nothing from them. The 2026-10-28
canvas-only decision was made for the splash and the worlds, with chat
explicitly out of scope, so this narrows it; it doesn't undo it.

**What stays ours.** This is not a JavaScript framework and not a second
component system.

- State, rules, the Markdown parser, the session record, the element table
  and every decision stay in Coax, in `components/core` and `kit`.
- The page is still one Wasm module with no runtime, no npm and no bundler.
- The host gains one small applier, as it has one for WebGPU. It holds no
  logic.

**How it works.**

1. **The element table is the source.** Every row, button and field has a
   stable id, a role and a label in `components/core/elements.coax`. The
   driver's `tree`, the tests, the DOM and the GPU all read that one table.
2. **Coax emits changes, not frames.** `backend/dom.coax` compares the table
   with the last one it sent and writes a short list of commands keyed by
   id: create, set text, set attribute, set class, move, remove. The host
   applies them. Nothing is sent when nothing changed, so an idle chat does
   no work. This is the retained, keyed update the research's phase 8 asked
   for, and the web chat is its first user.
3. **The browser lays out.** The layout layer already uses CSS's names for
   flex and matches Chrome to 0.05 px (`components/layout/flex.coax`). The
   DOM backend hands the same properties to the browser as CSS. The GPU
   backend computes them itself. One description, two ways to resolve it.
4. **The skin is custom properties.** Its tokens are written once as CSS
   variables. There is one skin, grayscale, and no Theme (section 11).
5. **The composer is a real text field.** IME, paste, spell check, the phone
   keyboard and password managers come from the browser. The hidden field
   and the page's own caret go away on the web. `core/edit.coax` stays the
   edit buffer for the GPU and terminal backends. The browser's field is the
   platform's, not a second one of ours.
6. **Markdown is elements.** `core/markdown.coax` still parses. Its blocks
   become headings, paragraphs, lists, tables, code blocks and links, so
   links are links and code can be selected and copied.
7. **The first screen is HTML.** The build runs the page's first frame and
   writes the empty new-chat screen into `index.html`. The module adopts
   those elements by id when it loads. No fonts, atlas or WebGPU are needed
   before the first paint, and a visitor without WebGPU gets the full chat.
8. **Each conversation has a URL** (`/chat/ID`), so reload, Back and a
   shared link work.
9. **The transcript is windowed in Coax.** It keeps rows near the viewport
   as elements and pages earlier turns from the server, following its end
   while a turn streams. No fixed array decides how long a chat may be.

**How we know the two backends agree.** Not by pictures. The check is the
element tree as text, row by row, the same for the DOM, the GPU and the
terminal: ids, roles, labels and order. Each backend then has its own small
set of pictures. A frame's SVG path is checked point for point against
what the GPU backend strokes (section 11).

**What it costs.**

- The web chat no longer matches the Mac pixel for pixel. Text is laid out
  and rasterized by the browser.
- A third backend to keep in step. The text comparison is what keeps it so.
- The browser's CSS becomes something the page depends on. Keep it to the
  flex subset the layout layer already defines, and to the skin's tokens.
  A style outside those is a defect, as a colour written into a component
  is today.

**What it doesn't change.** Everything in sections 4 to 7. The stream, the
schema, the stored record and the agent are the same work for any renderer,
and they come first.

**What else follows.**

- `/projects/SLUG`'s agent panel, `/settings/agents`, docs, the blog and the
  changelog are text surfaces too. They move to the same backend after the
  chat proves it. Worlds, labs and the splash stay on the GPU.
- The chat inside a realm is drawn by the GPU, from the same components.
- `coax/web.md` and `components/spec.md` now record this decision.

## 11. The look: a grayscale ARWES set, and a new component set

Added 2026-11-11 on the owner's direction: don't carry the previous chat
components over, and give the chat a black-and-white ARWES look with a few
components. For this I read our port (`aethyr/ui/arwes-*.coax`, its
[porting report](../verse/2026-11-01-arwes-coax.md) and
`aethyr/ui/arwes/SOURCES.md`) and the upstream source in
`projects/repos/arwes` at the same commit our port pins (`bdbaa03`).

### What ARWES is, and how it draws

ARWES is not a React library at its core. Ten plain packages do the work,
and the React and Solid packages only wrap them. On the web it draws three
ways:

- **Frames are SVG.** `createFrame` takes an `<svg>` laid behind an
  element's content. Each frame is a short list of paths in three named
  layers: `bg` (the fill), `line` (the border strokes) and `deco` (corner
  ticks and accents). The paths are computed from the element's width and
  height and recomputed by a `ResizeObserver`. Strokes don't scale
  (`vector-effect: non-scaling-stroke`). Colour comes from CSS: the paths
  read `currentcolor` and variables such as `--arwes-frames-bg-color`, and a
  page styles `[data-name='bg']` and `[data-name='line']`.
- **Animation is numbers over time.** An animator gives each node four
  states (entering, entered, exiting, exited) and schedules children in
  parallel, in sequence or staggered. A frame "assembles": the fill fades
  in, the lines draw along their length (a dash offset), and the
  decorations flicker in.
- **Backgrounds are a 2D canvas** (dots, puffs, grid lines, moving lines),
  and **text effects rewrite a hidden copy** of the element: the real text
  is hidden and a clone is revealed a character at a time.

There is no component library in ARWES. Its docs app builds its own Button,
Card, CodeBlock, FrameAlert, Header, Menu, Modal, Nav and Table from a frame
plus content (`apps/docs/src/ui/`). A button there is an octagon frame, an
octagon clip and uppercase text. That is the model to copy: a small set of
components, each a frame around content.

### What we already ported

All of the mathematics, none of the DOM (2026-11-01, 1,263 lines of Coax
with checks):

| File | What it holds |
| --- | --- |
| `aethyr/ui/arwes-frames.coax` | The nine frames (Underline, Lines, Corners, Octagon, Nero, Nefrex, Kranox, Header, Circle) as points with a layer, a fill flag, an alpha and a stroke; the shared assembly; each family's own transitions; the octagon and Kranox clips |
| `aethyr/ui/arwes-motion.coax` | The 31 easings, steps, draw progress, flicker, the animator's states and its five child schedules, the text sequence and decipher effects, text duration |
| `aethyr/ui/arwes-fx.coax` | Theme multipliers and colour math, the step, strip and separator patterns, the four backgrounds, the illuminator, keyed timers, the sound-cue model (no sound is played) |
| `aethyr/ui/arwes-paint.coax` | 25 specimens painted as the component system's display list; `/components` shows them on the GPU |

Time is supplied by the caller, randomness is seeded, and nothing allocates.
The MIT notice is kept in `aethyr/ui/arwes/LICENSE`.

So the frame geometry and the motion exist, are checked, and already draw on
the GPU. What we never built is the thing ARWES itself does on the web: the
frame as SVG behind real text. The browser backend decided in section 10 is
exactly where that fits.

### Decision: a new set, grayscale, frames as SVG

1. **A new, small component set.** The web chat does not use
   `components/zeron/` or its four skins. Theme goes away: there is one
   look. The set is about ten components, listed below, each a frame around
   content, written against the element table.
2. **One set, two backends.** A frame's geometry comes from
   `arwes-frames.coax` on both. The browser backend writes it as SVG paths;
   the GPU backend strokes the same points, as `/components` does today. The
   desktop app and the chat inside a realm can move to the new set later
   without a second design. `components/zeron/` stays for the desktop app
   until then and is deleted when it moves. It is not moved into `kit/`.
3. **Grayscale only.** Black ground, white ink, a ramp of greys between. No
   hue anywhere, including errors. State is carried by brightness, fill,
   line style and words, never by colour alone, which is also what
   accessibility rules ask for.
4. **Keep the shared base.** The Markdown parser, the element table, the
   flex layout and the session record stay. They are not the "component
   library"; they are what any set stands on.

### How a frame reaches the browser

1. A component marks its element as framed: a frame kind and its settings
   (corner size, stroke, padding).
2. The browser lays the element out. The host reports the element's size by
   id, once when it appears and again when it changes. This is the one
   thing the host tells Coax about layout.
3. Coax computes the frame's points for that size with the existing
   `af_frame_family` and sends them as path data for the three layers, into
   an `<svg>` the backend keeps behind the element's content.
4. While a frame is entering or exiting, Coax sends its changing alpha, dash
   offset and offsets each frame. When it has entered, nothing is sent. The
   page reports itself busy only while something is animating, so an idle
   chat still does no work.
5. Colour is CSS variables on the three layers, as upstream does it.
6. Clipped shapes (the octagon button) use the ported clip as a CSS
   `clip-path`.

The size report costs one frame: a frame's paths arrive the frame after its
element is laid out. The entrance starts from nothing, so it doesn't show.

### The tokens

One skin, `mono`, written as CSS variables from Coax:

| Role | Value | Use |
| --- | --- | --- |
| `ground` | #000 | The page |
| `surface` | #0a0a0a | A frame's `bg` |
| `raised` | #141414 | A hovered or pressed `bg` |
| `line-dim` | #2a2a2a | A resting frame's lines, rules |
| `line` | #6b6b6b | A frame's lines |
| `line-bright` | #ffffff | Focus, the active item, decorations |
| `ink` | #f2f2f2 | Body text |
| `ink-muted` | #9a9a9a | Labels, meta lines |
| `ink-faint` | #5c5c5c | Placeholders, disabled |
| `inverse` | #ffffff ground, #000 ink | The primary action, a selected row's marker |

Type: Geist for body text, Geist Mono in small capitals with wide tracking
for labels, headers and buttons. Both are already built for the site.
Strokes are 1 px; a frame's corner cut is 8 px on controls and 12 px on
panels. These values are a starting point to tune on the first screen, not
measured choices.

How state is shown without colour:

| State | How |
| --- | --- |
| Hover | `bg` goes to `raised`, lines to `line` |
| Focus | Lines to `line-bright`, decorations drawn |
| Active or selected | A bright underline or left bar; ink to white |
| Primary action | Inverse: white fill, black ink |
| Disabled | Faint ink, dashed lines |
| Error | The Lines frame, bright, with the word "Error" and what failed; never red |
| Running | The frame's decorations pulse; the answer streaming is the progress |

### The components

| Component | Frame | Notes |
| --- | --- | --- |
| Frame | any | The primitive: an element, its SVG, its state |
| Text | none | A label that types in with the sequence effect. Short text only |
| Button | Octagon, clipped | Uppercase label; a primary one is inverse |
| Field and Composer | Underline at rest, Corners when focused | A real text field; Send is a Button |
| Message | none for the agent; Corners, dim, for the person | The agent's answer is plain Markdown on the ground. Reading text gets no decoration |
| Tool call | Corners | A header row (what, on what) that opens onto input and output |
| Code block | Corners with a Header strip | Language and Copy in the strip |
| Notice | Lines | Errors, the cap, an approval with its two Buttons |
| Row | Underline when active | A conversation in the list; a menu item |
| Header | Header | The page title and the conversation's title |
| Menu | Corners | The model list, over the page |
| Background | none | Grid lines 40 px apart with one dot where they cross, as ARWES has them, faint and still, as one SVG tile |

Frames not used: Nero, Nefrex, Kranox and Circle. They stay in the port for
realms.

### What we take from ARWES, and what we leave

**Take:**

- The three-layer frame and its assembly on entrance.
- The animator: a new message, a tool call and the list's rows enter
  staggered, 20 to 40 ms apart, and the whole entrance is under 300 ms.
- The text sequence for short labels: the page title, a header, the empty
  screen's line.
- Grid lines and dots as a still background.
- The step and separator patterns for rules and dividers.

**Leave:**

- **Decipher and sequence on answers.** An answer streams in as it is
  written. Animating it again delays reading.
- **Flicker on anything a person reads.** Flicker stays on decorations.
- **Sound.** The cue model stays unused; nothing plays.
- **Moving backgrounds and puffs.** A chat is still. Motion behind text
  costs battery and attention.
- **The illuminator** (a glow that follows the pointer), at first. It needs
  an update on every pointer move. Add it later to Buttons if it is missed.
- **Colour.** Upstream's cyan and yellow, and our Verse skin's cyan.

**Rules that keep it usable:**

- With `prefers-reduced-motion`, every frame is entered at once and no text
  types in.
- The real text is always in the element. A text effect animates a copy
  marked `aria-hidden`, as upstream does, so a screen reader and Find see
  the whole text from the start.
- Motion is only on entrance, exit and a change of state. Nothing loops
  while the page is idle except a running turn's decoration.
- The driver's agent clock steps these animations, since Coax computes them
  from supplied time. `clock settle` waits for them.

### What is built (2026-11-11, later the same day)

- **The port is under `components/core/arwes/`** (`frames`, `motion`, `fx`,
  and `box` for the two shared types). The old paths in `aethyr/ui/` load
  the new ones.
- **The node tree** (`components/core/tree.coax`) and **the browser
  backend** (`components/backend/dom.coax`): the tree compared by id with
  what was last sent, and the changes written as commands. A framed node's
  size comes from the host, and the frame's points go back as runs the host
  draws as SVG paths. The points are sent as numbers, so no path writer in
  Coax was needed.
- **The host**, `coax/web/dom-host.js`, 170 lines, with no logic. It needs
  no WebGPU and draws only when asked.
- **The `mono` skin** (`components/skin/mono.coax`): the tokens above and
  the whole stylesheet, written from Coax. A check holds every colour in it
  to a grey.
- **The set** (`components/kit/frames.coax`): frame, text, button, link,
  field, composer, the person's message, the agent's message from Markdown,
  meta line, tool call, code block, notice, row, header, menu. Entrance,
  stagger and the typed label run on supplied time; reduced motion enters
  everything at once.
- **`/components`** shows every one of them, working
  (`components/gallery/web.coax`). The earlier page is `/components-gpu`.
- **`/chat`** is a composer under a transcript
  (`apps/openagents-web/chat-web.coax`). The earlier page is `/chat-gpu`.
- **Real answers, streamed.** A message is answered by one model call with
  the conversation so far: `POST /api/chat/complete`, a door for an admin's
  session onto the inference route's own ask (Gemini on Vertex, metered).
  The host has a streaming import (`dom_stream`), the request and the stream
  are read by the gateway's Chat Completions and SSE codecs
  (`coax/lib/gateway/types.coax` was split out so a page can load them), and
  Stop ends the stream and keeps what came.
- **Icons:** Apps SDK UI's 755, as SVG files and a Coax name table
  (`components/core/icons.coax`), found in the Rust tree at `coder-v1.0.0`.

What that door is and isn't: it is the old chat worker's serving path
(Episode 289's chat answered on Gemini Flash on Vertex first) with none of
the agent around it. Of the Rust `openagents-chat`, `route-contract`, `jev`
and `knowledge` crates (about 100,000 lines at `coder-v1.0.0`), nothing is
ported: no front, no routes, no members, no knowledge base, so the model
answers from what it knows and can be wrong about OpenAgents. The door's
wire is Chat Completions, a vendor's shape, inside the allowlisted
`chat.c`; `openagents.chat.v1alpha1` is still to be written.

Not built: stored conversations; the first screen as HTML at build time (the page is empty until
the module loads); exits (a node that goes is removed at once); the element
table for the driver, so `coax drive` can't name these nodes yet; a windowed
transcript; a route per conversation; tools for agents. Driven headless in
Chrome only, not in Safari or Firefox, and not on a phone.

### Work this needs that isn't UI

- **Move the port under `components/`.** A component may not load a file
  outside `components/`, and the port lives in `aethyr/ui/` and loads
  `hud.coax` for two small types (`HudBox`, `HudCoord`). Move
  `arwes-frames`, `arwes-motion` and `arwes-fx` to `components/core/arwes/`
  with their own box and point types, and have Aethyr load them from there.
  The licence and source ledger move with them.
- **A path writer** in the browser backend: frame points to SVG path data,
  with a check that the path's points equal the GPU's.
- **The size report** from the host, by element id.
- **The `mono` skin** and its generated variables.

## What to keep, delete and replace

**Keep.**

- The shared base: `components/core` (Markdown, the element table, the edit
  buffer for the GPU and terminal), `layout` (flex) and `paint`.
- The ARWES port (`aethyr/ui/arwes-*.coax`): frames, motion, patterns.
- The session log as the thing a transcript is built from.
- The credential path: checked before it is kept, sealed in the runner, never
  shown, one store for `/chat` and projects.
- The confinement of Read, Glob and Grep, and the per-turn environment
  allowlist.
- Golden mode.

**Not carried into the web chat.** `components/zeron/` (composer,
transcript, picker, tools, attachments, chrome), the four skins, Theme, the
glyph atlas in `gpui.js` and the `#demo` comparison against the Mac. They
stay for the desktop app until it moves.

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
4. **Pixels or DOM.** Decided 2026-11-11: the web chat is drawn by the
   browser, through a DOM backend under the same Coax components. The GPU
   backend draws it everywhere else. See
   [the decision](#decision-the-web-chat-is-drawn-by-the-browser).
5. **Where the chat set lives.** Decided 2026-11-11: a new, small set with
   a grayscale ARWES look, frames drawn as SVG (section 11).
   `components/zeron/` is not carried over and not moved into `kit/`; it
   stays for the desktop app until that moves to the new set.
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

For section 11: upstream ARWES beyond `createFrame`, the octagon settings,
the dots background, the text sequence and the docs app's Button and CSS;
the bodies of our port's procedures beyond their headers and signatures.
The token values are proposed, not tested on a screen.

For section 10: Exact's source (every statement about Exact comes from
`docs/research/exact/`, which read it at `94b8671`); the chat template's
`.contract` files; the bodies of `host.js` and `drive.coax` past their
contracts; `coax/web/STORAGE.md`, `fast-edit.md` and `PACK-GATE.md`.

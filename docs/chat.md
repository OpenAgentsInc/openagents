# /chat

**Changed 2026-11-11.** `/chat` is being rebuilt from nothing
([audit](audits/2026-11-11-chat-zero-base-audit.md)). The new page is
`apps/openagents-web/chat-web.coax`, drawn by the browser: a composer under
a transcript. A message is answered by one model call with the conversation
so far, streamed, through `POST /api/chat/complete`: a Chat Completions
request handed to the inference route's own ask (`inference.c`
`inference_ask`: Gemini on Vertex, its caps and its metering) for the admin
the cookie names. No agent is behind it yet, and nothing is stored.
To run it on this Mac with real answers: `OA_SERVE_INFERENCE_TOKEN=$(gcloud
auth print-access-token) coax/web/api/test.sh --serve http://127.0.0.1:PORT`,
then `coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:PORT --api
API_PORT`, and open the login link it prints (the token lasts an hour). Everything below describes the page before
it, which is now at `/chat-gpu` (`chat.coax`, `chat-gpu/index.html`) and still
talks to the same API. In production `/chat-gpu` reaches the site only after
`coax/web/api/lb.sh` has been run with its new path rule.

[openagents.com/chat](https://openagents.com/chat) is a direct chat with a
Claude agent for general questions about OpenAgents. It runs on the Coax
Claude Agent SDK through the same hosted runner as the project agent
([project-agent.md](project-agent.md)), in a general mode: the agent reads an
up-to-date copy of the public openagents repository and the web, and can't change
anything. Admins only for now.

## Who gets in

- An admin logs in with the NIP-98 admin login: `coax/web/api/admin.sh chat`
  opens `/api/admin/login?...&next=chat`, which sets the admin cookie (its
  third copy has `Path=/api/chat`) and goes to `/chat`.
- Anyone else gets the dashboard's answers: 401 `sign in as an admin` with
  no session, 403 `admins only` with one that isn't an admin's. The page
  then draws only a Log in button (to `/login`).
- Cookie writes need the site's `Origin` (a write from another site is 403).

## The pieces

| Piece | Where |
| --- | --- |
| The page, drawn in Coax | `apps/openagents-web/chat.coax`, `chat-face.c`, `chat.wgsl`, `chat/index.html` |
| The Zeron components it draws with | `components/zeron/` (shared with the Mac app) |
| The atlas host | `coax/web/gpui.js` (used by `coax/web/host.js`) |
| The API | `coax/web/api/chat.c`, `migrations/0021_chat.sql` (schema `chat`), `migrations/0139_chat_folders_tabs.sql` (folders, tabs, the record) |
| The runner's general mode | `coax/web/agent/chat-runner.c` (included by `runner.c`) |
| The engine | `coax/web/agent/chat.coax` (Coax Claude Agent SDK) |
| Checks | `coax/web/api/test_zchat.py` (with `test-chat-engine.py`), the `#test`s in `chat.coax`, `coax/web/agent/live-check.py chat` (a timed turn on a deployed runner) |

A conversation is a managed session (`openagents.agent.managed.v1`,
protocol spec §18.2): `chat.conversations` is the session (idle or running)
with the CLI session id its turns resume, `chat.turns` each message, and
`chat.events` what a turn did (`user.message`, `user.interrupt`,
`agent.message` (only its length), `agent.tool_use`,
`session.status_running`, `session.error`).

`/api/chat/state?c=ID` answers lines (`Z C S U T`, see the migration); the
page reads it every 2 s while a turn waits or runs, else every 30 s.
`send {c, text, model}` makes a turn and wakes the runner; `model {c, model}`
changes the model the next turn uses; `stop {c}` is the Stop button (or Escape); `key` and
`key_remove` are the credential. Other `/api/chat/<room>` paths stay the
Verse demo's room chat (`room.c`).

## Models and spend

The page offers Claude Haiku 5.5, Sonnet 5.5 and Opus 5.5 (default Opus).
The model is kept per conversation and can change mid-conversation: the
next turn resumes the same CLI session with `--model` set to the new one.
Each answer says which model wrote it. `chat.model_ok` accepts only those
three ids.

A turn on OpenAgents' key (Secret Manager `openagents-anthropic-api-key`)
counts against a daily cap for /chat of $10 across every model and person
(`OA_CHAT_DAILY_USD`, Chicago days, `chat.spent`). The turn's budget is
what's left; over it `send` answers 429 and the page says so. Tests use
Haiku.

## Theme

Theme, beside Key, lists every skin from `/components` (`components/skin/all.coax`):
Zeron (the default), Everglade, Verse and Terminal. The terminal skin is drawn in
pixels by `components/skin/terminal-pixels.coax`. The page keeps the choice in this
browser (`host.js` action 4, given back as message 4). Golden mode always draws Zeron.
Checks: `apps/openagents-web/chat-frames.coax --only Theme` and
`components/zeron/skins-checks.coax` (GitHub #13230).

## Your own key

Key on the page takes an Anthropic API key (`sk-ant-api...`) or a Claude
subscription token (`claude setup-token`, `sk-ant-oat...`).

- **Checked before it is kept.** oa-api hands it to the runner's
  `/credential` (identity token, the runner only): an API key with a
  models-list call, a token with a one-line Haiku CLI turn. A failure is
  "That key didn't work: REASON", and nothing is saved.
- **Sealed.** The runner seals it with AES-256-GCM (OpenSSL) under
  `OA_CHAT_SEAL_KEY`, 32 random bytes kept in Secret Manager
  (`oa-chat-credential-key`, `oa-chat-credential-key-staging`), with the
  owner and kind as associated data. oa-api stores only the sealed bytes, the
  kind and the last four characters (`chat.credentials`); it can't open them.
  We chose a Secret Manager key over Cloud KMS: one fewer service, and the
  key never leaves the runner, which reads it once at start and drops it from
  its environment.
- **Used alone.** A turn on a saved credential has no cap from us. The runner
  opens it for that turn only and passes it to the CLI child as
  `ANTHROPIC_API_KEY` (an API key) or `CLAUDE_CODE_OAUTH_TOKEN` (a token),
  never both. The child's environment is built from an allowlist (HOME,
  PATH, a fresh per-turn `CLAUDE_CONFIG_DIR`, the credential), so nothing of
  the runner's (cloud variables, `ANTHROPIC_AUTH_TOKEN`, our key) can win the
  CLI's credential order. `--bare` is never passed.
- **Expired tokens.** "Login expired" / "Please run /login" becomes "Your
  token expired. Make a new one with claude setup-token and save it under
  KEY." (the runner's wording), and the saved credential is marked expired; turns go back to our key
  until it is replaced.
- **Never shown.** The page sends it once and wipes it from its memory;
  answers carry only the kind and last four. It is never logged or put in an
  event. Remove deletes it. Each answer's meta line says "Your key" or "OpenAgents key".
- **One store for /chat and projects.** The credential is kept per signed-in
  pubkey, and the same KEY item sits on each project page under the agent's
  field (`/projects/SLUG/api/key`, `key_remove`; only a member can save one).
  A project turn runs on the credential of the member who sent the message,
  with no cap from us and no charge to the project's $10 a day; with none
  saved it runs on our key under that cap. Another member's credential is
  never used: `projects.api('next')` hands the runner only the asker's
  sealed credential, and the turn header says "CHRIS'S KEY" or
  "OPENAGENTS KEY". An expired token fails that turn with the same message
  and marks the credential expired.

## Folders, tabs and the record

The page lists conversations the way the desktop app's desk lists sessions
(GitHub #13156, `migrations/0139_chat_folders_tabs.sql`):

- **Folders.** A conversation's folder is the checkout its turns read. The
  runner has one checkout, the public openagents repository, so every
  conversation is in `openagents`. The sidebar shows the folder's name above
  its conversations. `chat.folder_ok` lists the folders the runner can read;
  a second folder needs a second checkout in the runner first.
- **Tabs.** A conversation's `tab` is its place among your open tabs, or
  empty when it isn't open. Sending a message or starting a new chat opens
  its tab, and so does picking a conversation in the list. On a wide window
  the tabs run across the top. Click a tab to show its conversation, or its
  **×** to close it. Closing the shown tab shows the tab after it, or the
  one before. The server keeps your tabs, so another browser shows the same
  ones.
- **The record.** `GET /api/chat/record?c=ID` returns the conversation in
  the shared session log format (`coax/lib/agent/session/log.coax`,
  `openagents.session.v1`), one JSON object a line. That is the same format
  the desktop app, Coder and the agent boards read. The tools' output is not
  kept on the server, so each tool's result line is empty.

API: `POST /api/chat/tab {c, open}` (open is `true` or `false`) and
`GET /api/chat/record?c=ID`. The state's `Z` line ends in `2` when the
server keeps folders and tabs. `C` lines then carry the folder and the tab.
An older server sends neither, and the page draws its old list.

## How the page is drawn

Since 2026-10-29 the page draws everything with the Zeron/GPUI components the
Mac app uses (`components/zeron/`: `gpui.coax`, `composer.coax`,
`markdown.coax`, `transcript.coax`, `picker.coax`, the icons and Geist under
its OFL), compiled to wasm and drawn on WebGPU. The old HUD chat is
`apps/openagents-web/chat-hud.coax`, kept for the Mac app's unshown HUD.

- **Shaders.** `chat.wgsl` is `components/backend/gpu-shader.coax` emitted as WGSL (the Mac apps emit the same file as Metal) (quads, shadows,
  underlines, mono and polychrome sprites).
- **Glyphs.** `coax/web/gpui.js` rasterizes Geist and Geist Mono in the
  browser (FontFace into an OffscreenCanvas) at the device pixel ratio, four
  quarter-pixel subpixel positions per glyph, with two weight levels, into an
  r8 atlas; color glyphs (emoji) go to a dynamic rgba cache measured on
  first use. Zeron's icons are drawn from their SVGs into the same atlas.
  Advances and kerning come from the browser's measurement of the same font
  files, so line breaks match the Mac's.
- **Layout.** At 860 px and up: a 260 px sidebar (New chat, the chat list,
  the credential and Key) beside the transcript. Narrower: Chats, New and Key
  sit in the titlebar, the list opens over the page, the composer docks at
  the bottom with 16 px sides and the page never scrolls sideways.
- **Transcript.** Each turn is a user bubble, a tool group (`T` lines:
  Read, Grep, Glob, WebSearch, WebFetch as Zeron tool chips, open while the
  turn runs), the answer as Markdown, then a meta line (model, whose key,
  cost, time). A running turn shows Zeron's thinking trailer; the composer's
  send button becomes Stop.
- **Composer.** Zeron's composer: selection, double and triple click,
  copy and cut, paste and IME through host.js's hidden text field. The model
  pill opens Zeron's picker with Haiku 5.5, Sonnet 5.5 and Opus 5.5. The
  paperclip is hidden and no effort levels are shown: the API takes no
  attachments and has no effort setting.
- **Against the Mac.** The `#demo` hash draws Zeron's demo conversation;
  against the Mac app's `--shot` of the same (1320x880 at scale 2), the mean
  per-channel difference is 0.17 over the page (0.66% of pixels differ by
  more than 8), 0.47 in the user bubble, 0.98 in the tool group header, 0.73
  in the Markdown answer, 0.29 in the code block and 0.46 in the composer.
  The rest is the browser's text rasterizer against CoreText's, at glyph
  edges.

## The checkout and the runner's size

The agent reads a checkout of `OpenAgentsInc/openagents` main at
`/opt/oa-chat/repo` (`OA_CHAT_DIR`). It is shallow (`--depth 1`), blobless
(`--filter=blob:none`) and sparse: images, video, sound, fonts, wasm, packed
and raw data (`.bin`, `.gz`, `.pack`, `.safetensors` and the rest of the list
in `chat_checkout`, `coax/web/agent/chat-runner.c`) are left out, so their
blobs are never fetched.

**The image holds the checkout.** `coax/web/agent/deploy.sh` builds it into
the runner's image: the Dockerfile runs `oa-agent --chat-checkout`, the same
code a turn runs. At the start of every turn the runner fetches main
(`fetch --depth 1`) and writes only the files main changed since
(`read-tree -m -u`, which looks at no other file), so a cold start doesn't
download the repository again. On Cloud Run a written file is held in
memory, so the longer since the last deploy, the more a refresh fetches and
holds. Deploy the runner again when the refresh grows slow: the log line
`chat checkout: refreshed in N s (fetch N s, files N s)` says how long each
one took. If the refresh can't be applied, the runner puts the whole checkout
back with `reset --hard` and `clean`.

Measured on 2026-11-11: the checkout is 707 MB (471 MB in 27,063 files and
236 MB of `.git`). Before that date every cold start made the whole checkout
in the instance's memory. On 2026-11-11 that took 103 seconds and then ran out of the
runner's 2 GiB, and the page said "the repository could not be reached"
(GitHub #13252). With 4 GiB the same cold start answered, in 29 seconds
on staging and 103 seconds on production (whose older image left less out).
With the checkout in the image, a first message on production after it had
scaled to zero was taken in 1.9 seconds, refreshed the checkout in 2.1
seconds, had the engine running at 8.4 seconds and its answer at 9.4 seconds.

Two things made a refresh of the image's copy slow, and both are handled in
`chat_checkout`:

- `reset --hard` and `clean` look at every file's entry (12 s on a cold
  instance), so a refresh uses `read-tree -m -u`.
- On Cloud Run the image's index doesn't read as newer than the files it
  lists. Git then takes every file as maybe changed in the index's last
  second and reads all 471 MB again before it writes the index (45 s of a
  47 s refresh). The runner sets the index's time to now before it fetches.

A git step that takes over 3 seconds logs where git spent the time
(`chat checkout: git STEP took N s: ...`, from git's trace2 timings).

**The size check.** `oa-agent --chat-checkout` fails when the checkout is
bigger than 1,200 MB (`OA_CHAT_REPO_MAX_MB`), which fails the image build and
so the deploy. When it fails, add what grew to the sparse list. The limit is
what a runner with 4 GiB can still hold whole in memory, beside the Claude
CLI's 300–500 MB a turn, if it ever has to make the checkout itself.

**When the checkout fails.** If main can't be fetched and a copy is there,
the turn reads that copy and the runner's log says so. With no usable copy
the turn fails, and the person reads which of these happened:

| What failed | The person reads |
| --- | --- |
| The kernel ended git for memory | We ran out of memory getting our copy of the code, so we couldn't answer. Send your message again. |
| A git step passed its time limit (fetch 60 s, reset 120 s) | Getting our copy of the code took too long, so we stopped. Send your message again. |
| GitHub didn't answer | We couldn't reach GitHub to get our copy of the code. Send your message again in a minute. |
| Anything else | We couldn't get our copy of the code. Send your message again. |

The runner logs `chat checkout: git STEP failed after N s (exit code, signal
or time limit, and the kind)`, then git's own output.

**Timing a cold start.** `coax/web/agent/live-check.py chat --cold
oa-agent-staging` makes a new revision (so no instance is running), logs in
as an admin, sends one Haiku message and prints when the runner took the
turn, when the engine started and when the answer came. It fails when the
engine starts later than 15 seconds after sending. For production:
`--origin https://openagents.com --cold oa-agent`.

Allowed tools: Read, Glob, Grep (only inside the checkout, as
`coax/web/agent/confine.coax` decides in a PreToolUse hook and the
permission callback), WebSearch and WebFetch. Bash, Edit, Write,
NotebookEdit, Task/Agent, TodoWrite and KillShell are refused by the
permission callback. A refusal is a `session.error` event.

Each conversation's CLI session file is kept in a Cloud Storage bucket
mounted at `/sessions` (`openagentsgemini-oa-chat-sessions`, `-staging`), so
a follow-up resumes the session after the runner scales to zero. If the file
is gone the turn starts a new session with the last ten exchanges as
history.

## Deploying

- oa-api: `coax/web/api/deploy.sh` (migration 0021 runs on start).
- The runner: `coax/web/agent/deploy.sh` (makes the sealing key and the
  session bucket once, and builds main's checkout into the image). Then time
  a cold start with `coax/web/agent/live-check.py chat --cold oa-agent-staging`.
- The page: `coax/web/release.sh`; `coax/web/api/lb.sh` sends `/chat` to the
  site's bucket in production.

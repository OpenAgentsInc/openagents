# /chat

[openagents.com/chat](https://openagents.com/chat) is a direct chat with a
Claude agent for general questions about OpenAgents. It runs on the Coax
Claude Agent SDK through the same hosted runner as the project agent
([project-agent.md](project-agent.md)), in a general mode: the agent reads a
fresh copy of the public openagents repository and the web, and can't change
anything. Admins only for now.

## Who gets in

- An admin signs in with the NIP-98 admin sign-in: `coax/web/api/admin.sh chat`
  opens `/api/admin/login?...&next=chat`, which sets the admin cookie (its
  third copy has `Path=/api/chat`) and goes to `/chat`. A phone sign-in
  (`/signin`, the `oa_session` cookie, [phone-sign-in.md](phone-sign-in.md))
  is accepted too when its key is in `admins.conf`.
- Anyone else gets the dashboard's answers: 401 `sign in as an admin` with
  no session, 403 `admins only` with one that isn't an admin's. The page
  then draws only SIGN IN (to `/signin`).
- Cookie writes need the site's `Origin` (a write from another site is 403).

## The pieces

| Piece | Where |
| --- | --- |
| The page, drawn in Coax | `apps/openagents-web/chat.coax`, `chat-face.c`, `chat/index.html` |
| The API | `coax/web/api/chat.c`, `migrations/0021_chat.sql` (schema `chat`) |
| The runner's general mode | `coax/web/agent/chat-runner.c` (included by `runner.c`) |
| The engine | `coax/web/agent/chat.coax` (Coax Claude Agent SDK) |
| Checks | `coax/web/api/test_zchat.py` (with `test-chat-engine.py`), the `#test`s in `chat.coax` |

A conversation is a managed session (`openagents.agent.managed.v1`,
protocol spec §18.2): `chat.conversations` is the session (idle or running)
with the CLI session id its turns resume, `chat.turns` each message, and
`chat.events` what a turn did (`user.message`, `user.interrupt`,
`agent.message` (only its length), `agent.tool_use`,
`session.status_running`, `session.error`).

`/api/chat/state?c=ID` answers lines (`Z C S U T`, see the migration); the
page reads it every 2 s while a turn waits or runs, else every 30 s.
`send {c, text, model}` makes a turn and wakes the runner; `model {c, model}`
changes the model the next turn uses; `stop {c}` is STOP; `key` and
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

## Your own key

KEY on the page takes an Anthropic API key (`sk-ant-api...`) or a Claude
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
  KEY.", and the saved credential is marked expired; turns go back to our key
  until it is replaced.
- **Never shown.** The page sends it once and wipes it from its memory;
  answers carry only the kind and last four. It is never logged or put in an
  event. REMOVE deletes it. Each answer says "YOUR KEY" or "OPENAGENTS KEY".

## The checkout and the runner's size

The agent reads `/tmp/oa-agent/chat-repo`, a checkout of
`OpenAgentsInc/openagents` main refreshed at the start of every turn
(`fetch --depth 1 --filter=blob:none`, `reset --hard`, `clean`). It is sparse:
images, video, wasm, `.bin`, `.vertices`, `.indices`, `.glb`, `.ktx2` and
`.zip` are left out, so their blobs are never fetched.

Measured on 2026-10-28: main's tree is 34 MB in 1,740 files (the largest a
3.5 MB Coax file and 2–3 MB meshes); the sparse checkout is 41 MB on disk
(29 MB of files, 12 MB of `.git`) in 1,733 files and takes about 5 s. The
runner keeps 2 GiB: the Claude CLI (Node) takes 300–500 MB per turn, the
checkout 41 MB of in-memory disk, and a project turn may run beside it.
Cloud Run's `/tmp` is memory, so 1 GiB would be tight with two turns.

Allowed tools: Read, Glob, Grep (only inside the checkout: no `..`, `~` or
absolute paths outside it), WebSearch and WebFetch. Bash, Edit, Write,
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
  session bucket once).
- The page: `coax/web/release.sh`; `coax/web/api/lb.sh` sends `/chat` to the
  site's bucket in production.

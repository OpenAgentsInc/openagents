# 39. Drive Antigravity

> **Status: implemented.** Coder runs tasks through Google's Antigravity
> CLI (`agy` 1.5.6) on its own sign-in: `coder run "TASK" --engine
> antigravity`, `coder fork`, `coder replay` and the terminal UI's
> Antigravity agent. One ACP session holds the conversation, on the same
> ACP engine and client as Cursor ([chapter 38](ch038-drive-cursor.md)).
> Checked against a stand-in `agy acp` and with real turns on Google Cloud
> credits on `coderos-4080` (GitHub #13192).

Antigravity is Google's coding agent CLI. Its binary is closed source; its
public repository holds a README, a changelog and examples. `agy acp`
serves the Agent Client Protocol over stdio, the protocol Coder already
speaks to Cursor. So Antigravity is not a new client: it is a second agent
on the shared ACP engine, with its own quirks as a parameter.

`agy -p PROMPT --output-format stream-json` is the other headless door. It
starts a process for every turn and has no per-tool approval channel, so
like Cursor's print mode it is not the driver.

## One client, two agents

[coax/lib/agent/cursor/](../../coax/lib/agent/cursor/) is the ACP client
(`cu_*`). A `CuAgent` names what differs between agents:

| Field | Cursor | Antigravity |
| --- | --- | --- |
| `name` (in errors) | `cursor agent` | `agy` |
| `program` (run with `acp`) | `agent` | `agy` |
| `login` (authenticate after initialize) | `cursor_login` | none |
| `resume` | `session/load` (replays history) | `session/resume` (replays nothing) |
| `unset` (never passed to the child) | `CURSOR_API_KEY`, `CURSOR_AUTH_TOKEN` | none |

[coax/lib/agent/engine/cursor.coax](../../coax/lib/agent/engine/cursor.coax)
is the ACP engine (`cue_*`). Its agent defaults to Cursor's, so a caller
that never names one gets the old behaviour.
[antigravity.coax](../../coax/lib/agent/engine/antigravity.coax) (`agy_*`)
is everything Antigravity adds:

- `agy_program`: `agy` on `PATH`, else `~/.local/bin/agy`, where the
  installer puts it.
- `agy_launch`: the agent started with the credentials file's variables.
- `agy_configure`: the model, effort and mode for the session that just
  opened.
- `agy_signed_out`, `agy_provider`: how a refusal reads.
- `agy_outside`: the paths Antigravity's tools name in `rawInput` (`Path`,
  `Cwd`, `TargetFile`, `AbsolutePath`, `DirectoryPath`).

Coder's terminal (`coder/tui/cursor.coax`) and the turn runner
(`coax/lib/agent/engine/run.coax`) keep one ACP engine. Switching between
Cursor and Antigravity closes the other agent's process.

## Sign-in

`agy acp` does not need `authenticate`: it signs in on its own (Google,
a Gemini API key, or Application Default Credentials). A signed-out `agy`
refuses `session/new` with `Authentication required`. Coder then ends the
turn with what to do:

```text
Antigravity is not signed in. Run agy once to sign in, or name Google Cloud credentials in ~/.openagents/coder/antigravity.env (AGY_ADC_AUTH=1, GOOGLE_APPLICATION_CREDENTIALS=PATH, GOOGLE_CLOUD_PROJECT=ID).
```

Coder passes its own environment through. Besides, it reads
`~/.openagents/coder/antigravity.env`, one `NAME=value` a line, `#` for
comments. Only four names are read: `AGY_ADC_AUTH`,
`GOOGLE_APPLICATION_CREDENTIALS` (the path to a key, never the key),
`GOOGLE_CLOUD_PROJECT` and `GOOGLE_CLOUD_LOCATION`. A variable already set
in Coder's environment wins over the file. On `coderos-4080` the file names
the `openagentsgemini` service account's key in `~/work/.secrets`, so
`coder --engine antigravity` runs on Google Cloud credits with nothing
exported.

With no `agy` installed, the turn fails with the install line:

```text
The Antigravity CLI (agy) was not found. Install it with: curl -fsSL https://antigravity.google/cli/install.sh | bash
```

## Models, effort and mode

`session/new` lists models (`models.availableModels`, such as
`gemini-3.8-flash-low`) and config options (`model` with base names such as
`gemini-3.8-flash`, and `thought_level` low, medium or high).

- `--model` that agy lists goes by `session/set_model`. A base model name
  goes by `session/set_config_option` on the `model` option. Anything else
  is not sent, and the session log notes that it was not applied.
- `--effort` is the `thought_level` option's value, sent the same way.
- With no `--effort`, a turn thinks low (`AGY_EFFORT`, GitHub #13193). A
  model named with its level (`gemini-4-argon-medium`) is set as given. A
  base model (`gemini-4-argon`) is set as its `-low` variant when agy lists
  one, else as named with `thought_level` low. No model means agy's own at
  `thought_level` low. At medium, Flash and Carbon spent 7 to 15 s thinking
  before a call's first byte, and Carbon's longest calls thought for 5 to 6
  minutes.
- Each setting holds the first message until it is answered, as Cursor's
  do. A refused setting is a note, and the message then goes.

Modes: Coder answers every permission itself (its policy, `--ask`, or the
session's trust). So the session runs in agy's `default` mode, where edits
and commands are asked. A resumed session found in another mode
(`accept-edits`, `auto`, `plan`) is set back with `session/set_mode`.
agy's own permission settings still apply before it asks: reads in the
workspace and writes to the system temporary folder are allowed without a
request.

## Approvals

Permission requests come with string ids and two options, `allow`
(allow once) and `deny` (reject once). Chapter 5's policy and `--ask` work
as for Cursor. `y` allows once and `n` denies. `a` (from now on) allows
once, because agy offers no allow-always option. The tool kind is the
effect (`execute` runs, `edit` writes, `read` and `search` read, `fetch`
uses the network).

## The session log

Antigravity's turns write the same lines as Cursor's: `engine_session` with
`"engine":"antigravity"`, `approval`, `ask` and `answer`, `result`, `diff`,
`reply_delta`, `end`. Each turn adds a usage line from `session/prompt`'s
answer, under ACP's names. agy reports no cost.

```json
{"kind":"usage","schema":"coder.observability.v1","engine":"antigravity","scope":"turn","totalTokens":30212,"inputTokens":19269,"cachedReadTokens":10759,"cachedWriteTokens":0,"outputTokens":184,"thoughtTokens":0,"cost_usd":null}
```

`stopReason` `end_turn` finishes the turn and `cancelled` stops it.
Anything else fails it as `Antigravity ended the turn: REASON`. A refused
`session/prompt` fails the turn with agy's message. When the message names
the provider (quota, rate, overload, unavailable), the end line's cause is
`provider` and routing's cooldown is recorded.

### A model call agy gave up on

agy does not refuse `session/prompt` when a model call fails for good (a
location that does not serve the model, or a 429 still there after its own
retries). It sends the failure as the turn's words, `\n⚠ MESSAGE\n`, answers
`end_turn`, and reports no usage. Coder used to log that as a finished turn
with no steps. Now a turn whose words are such a notice and whose answer has
no usage fails as `Antigravity: MESSAGE`, with the cause `provider` when the
message names quota, rate or capacity (GitHub #13227). A notice followed by
other words, or an answer with usage, is an ordinary turn.

### Where the model runs

agy calls Vertex AI in `GOOGLE_CLOUD_LOCATION`, `global` when the
credentials file names none. Gemini 4 Carbon and Argon are served in
`global` and in the `us` and `eu` multi-regions, not in single regions such
as `us-central1` or `us-east5`. On 2026-11-11, `us` answered Carbon's
requests with a steady 5 to 8 seconds to the first token and kept its
prompt cache. `global` sometimes waited minutes and dropped the cache.
`coderos-4080`'s credentials file names `GOOGLE_CLOUD_LOCATION=us`. The
measurements are in
[docs/research/2026-11-11-antigravity-task-fit.md](../../docs/research/2026-11-11-antigravity-task-fit.md),
"Why Carbon was slow".

### A reply that stops arriving

agy waits a fixed 5 minutes on a model reply that went silent, then retries.
Two Carbon runs lost 5 minutes each that way. The shared ACP engine has a
stall watchdog (`cue_watch`, off for Cursor). While a turn is in flight and
no tool call runs, if agy sends nothing at all for `CODER_AGY_STALL_S`
seconds (default 120), Coder sends `session/cancel`. When the cancelled
turn ends, it sends one message in the same session: "Your last reply
stopped arriving, so the turn was cancelled. Continue where you left off.
Don't repeat steps that already finished." It does this at most
`CODER_AGY_RECOVER` times a turn (default 2). After that a silence cancels
the turn, which fails as `Antigravity stopped answering: ...`, and a cancel
that goes unanswered for another limit closes agy. Each step is a note line
in the session log, and a line in the Workshop's chat.

The limit sits above agy's quiet times, measured over ACP: a thought chunk
every 2 to 3 s while the model thinks, but nothing while it writes a tool
call's arguments (27 s for a 1,500-line file on Flash at low) or waits for
a first token (up to 83 s in `us`). A tool call counts as running from its
`pending` or `in_progress` status to `completed`, `failed` or `cancelled`,
so a long command never trips the watchdog, and neither does a permission
waiting on the person. `CODER_AGY_STALL_S=0` turns it off.

## Resume and fork

`coder fork ID "TASK"` of an Antigravity session sends `session/resume`
with the parent's `engine_session` id. Nothing replays, and only the new
message is sent. If agy cannot read the session, Coder notes it and opens a
new one. `coder replay` knows such a fork sent no earlier turns, for
Cursor's `session/load` forks as well.

## Routing

System One still chooses among its four engines (chapter 13). Antigravity is
not a route label. It is a candidate for automatic routing only when
`CODER_ROUTE_ENGINES` names it (bit 16 of the masks), only when `agy` is
installed and not cooling down, and only when none of the other four is
eligible. `--engine antigravity` always runs it.

## Checks

```sh
coax test coax/lib/agent/engine/antigravity.coax --library   # the agent, settings, resume, credentials file
coax test coax/lib/agent/cursor/tests.coax --library         # the shared client
coax test coder/tui/coder.coax                               # Coder, Shift-Tab, the routing number
coax mutate coax/lib/agent/engine/availability-policy.coax   # routing eligibility (#authority)
coax/web/tool.sh coder/tui/agy_standin.coax                  # the stand-in agy, on PATH as agy
```

The stand-in covers a plain turn, a command allowed and denied, an edit
allowed and denied, a refusal, a refused turn, agy's failure notice, a
signed-out agy, model and effort, resume, a resume refused, and no `agy` on
`PATH`, a reply that stops arriving (`stall`, `stall always`) and a slow
command (`slow tool`). Runs of
`coder run --stream-json` against the Claude Code, Codex and Cursor
stand-ins, Microcoder's recorded replay and the echo engine printed the
same stdout, stderr, session JSONL and protocol lines before and after this
chapter's change.

## Dependencies

- [Chapter 5](PLAN.md#i-the-loop), [chapter 13](ch013-choose-an-engine.md)
  and [chapter 14](ch014-forks-subagents-and-jobs.md).
- [Chapter 38](ch038-drive-cursor.md): the ACP client and engine.

## Done means

- [x] `coder run "TASK" --engine antigravity`, `coder fork` and the
  terminal UI's Antigravity agent start `agy acp` on the installed CLI.
- [x] Approvals are answered by chapter 5's policy and `--ask`.
- [x] A real turn ran a command and made an edit on Google Cloud credits,
  allowed and denied, and a fork resumed its session.
- [x] Cursor's behaviour did not change.

## Sources

- `coax/lib/agent/engine/antigravity.coax`, `coax/lib/agent/engine/cursor.coax`,
  `coax/lib/agent/cursor/`.
- `coder/tui/cursor.coax`, `coder/tui/agy_standin.coax`.
- google-antigravity/antigravity-cli `CHANGELOG.md` for 1.5.6, read
  2026-11-11, and `agy acp` as it answered on `coderos-4080`.

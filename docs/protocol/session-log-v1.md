# Session log v1

The session log is one conversation on disk:
`~/.openagents/coder/sessions/ID.jsonl` (under `$CODER_HOME` when it is set,
else `$HOME`). Each line is one compact JSON object ended by `\n`. Coder's
terminal and runs, `coder import`, the turn runner that Aethyrforge calls, the
desktop app and the agent boards write and read it. `GET /api/chat/record`
returns a `/chat` conversation in the same format.

The header's `schema` is `openagents.session.v1`. That id names this log. The
protocol spec's `openagents.session.v1` (section 18.3, NIP-SESS) is a different
thing; the schema package here is `openagents.session.log.v1alpha1`, and the id
on disk does not change.

| | |
| --- | --- |
| Writer | [`coax/lib/agent/session/log.coax`](../../coax/lib/agent/session/log.coax): every line of every kind is built there |
| Reader | The same file: `sl_next` walks the lines and names each kind from its first bytes, `sl_read` gives a line's fields typed (`SlLine`), `sl_load` rebuilds the desktop app's record. Every program that reads the log reads it there |
| SQL | `chat.record` ([0139](../../coax/web/api/migrations/0139_chat_folders_tabs.sql), [0162](../../coax/web/api/migrations/0162_chat_record_log.sql)) writes a `/chat` conversation; [`test_zchat.py`](../../coax/web/api/test_zchat.py) reads every line it returns through the reader and requires it back byte for byte (`log-profile.coax --read LOG`) |
| Schema | [`proto/openagents/session/log/v1alpha1/log.proto`](../../proto/openagents/session/log/v1alpha1/log.proto), profile `session-log` |
| Fixtures | [`proto/coder/session-log-v1.json`](../../proto/coder/session-log-v1.json): each kind's values and its exact line |
| Check | `coax/web/tool.sh coax/lib/agent/session/log-profile.coax proto/coder/session-log-v1.json` |

## Lines

A field in brackets is left out when it is empty or 0. Keys come in the order
shown; a reader looks a key up by name and skips a kind it does not know.

| Kind | Fields | What it is |
| --- | --- | --- |
| `session` | `schema`, `id`, `version`, `engine`, `cwd`, `started_us`, [`title`, `model`, `imported`, `parent` and `at`, `pid`, `report_to`, `resume`, `allow` and `ceiling`, `deadline_s`] | The first line. Coder always writes `version`, `engine`, `cwd` and `started_us`; the desktop app leaves them out when empty |
| `user` | `text` | A message the person sent |
| `thinking` | `text` | The model's thinking |
| `reply` | `text` | The answer's text, Markdown; one per stretch of text |
| `tool` | `label`, [`tool`, `name`, `id`, `arg`, `detail`, `old`, `new`, `input`] | A tool call. Coder writes only its `label` (`Bash(ls)`) |
| `result` | `text`, `error`, [`bytes`, `output`, `id`, `open`] | The call's result: the first 4,000 bytes, the whole size when cut |
| `routed` | `text`, [`tool`, `id`, `arg`, `detail`, `output`, `error`, `resolved`] | A routing or loading step |
| `notice` | `notice`, `text` | `error`, `signed_out`, `limit`, `install`, `stopped`, `busy`, `offline`, `meta`, `codex_install` |
| `ask`, `answer` | `text` | An approval waiting on the person, and their answer |
| `approval` | `tool`, `effect`, `inside`, `verdict`, `why` | A policy verdict on a call |
| `agent` | `engine` | The person switched agent |
| `context`, `keys`, `note` | `text` | What a turn added to its prompt, keys typed, a note |
| `memory` | `text`, [`withheld`] | The remembered facts a first message carried |
| `request` | `sha256`, `bytes` | The SHA-256 and length of what a turn sent its engine. `coder replay` rebuilds the prompt and compares both |
| `engine_session` | `engine`, `id` | The engine's own session, for a fork that resumes it |
| `diff` | `path`, `adds`, `dels`, [`old`, `new`] | An edit's counts and text. Legacy: Coder still writes it; wave 2 (#13250) moves an edit's text to the `tool` line |
| `hook` | `on`, `plugin`, `run`, [`tool`], `ok`, `text` | A plugin hook ran |
| `goal` | `check`, `turn`, `met`, `text` | A goal's check ran |
| `job` | `id`, `ending`, `text` | A job this session started ended (`--report-to`) |
| `end` | `ending`, `ms`, `steps`, [`cost_usd_e6`, `text`, `cause`] | The turn's end: `finished`, `stopped`, `failed`, `expired`, `redirected` or `lost` |

Stream records also ride in the file under their own schemas:
[`coder.reply-stream.v1`](coder-reply-stream-v1.md) (stdout only),
[`coder.observability.v1`](coder-observability-v1.md) and
[v2](coder-observability-v2.md), and `coder.route.v2`. They are not this log's
kinds.

Older lines still read: a header with no `schema`, a tool line with only its
label, a result with only text, a `diff` line, an `end` whose `text` is the
failure, and the desktop app's first format (`{"k":"user","t":T}`). The fixture
file holds one of each.

## Files beside the log

| File | What it holds |
| --- | --- |
| `ID.pid` | A detached run's process ID |
| `ID.stop` | Left by `coder stop`: the run stops at its next step |
| `ID.turnstop` | Stops the current turn only (the Router writes `redirect`) |
| `ID.answer` | The answer to a waiting approval, from another process (an ACP client, the terminal) |
| `ID.inbox/*.msg` | Messages queued for the session's next turn |
| `ID.acp` | For an ACP session (`acp-...`), the ID of the Coder session its turn runs in |
| `ID.remote` | Where a session started with `--on` lives: `ssh` or `cloud`, its host and repository |
| `ID.jsonl.cut` | A partial last line, moved aside before another process appends |

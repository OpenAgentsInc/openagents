# Codex app-server client (Coax)

A client of `codex app-server`, the Codex CLI's JSON-RPC 2.0 server over
stdio, written the way `../claude` ports the Claude Agent SDK: pull-based,
no callbacks, caller storage. It tracks **codex-cli 0.163.0**
(`CX_UPSTREAM_CODEX_VERSION` in `codex.coax`) and follows the requests
Zeron's Codex harness sends (`~/work/zeron/crates/harness/src/codex/mod.rs`,
`approval.rs`), checked against recordings of 0.163.0 on a ChatGPT login.

| File | What |
| --- | --- |
| `client.coax` | The client as a state machine over lines: requests, answers to server requests, `cx_feed` |
| `transport.coax` | `codex app-server` as a child process, through `coax/platform/process-native.c` (the Claude SDK's `cxproc_*`) |
| `codex.coax` | The entry: load it and link `process-native.c` |
| `fixture.coax`, `fixtures/` | Two recorded sessions, scrubbed (`fixtures/fixture.py` from `fixtures/record.py` logs) |
| `tests.coax` | The requests byte for byte, the recorded sessions replayed, approvals, refusals |
| `smoke.coax`, `smoke.sh` | The live smoke against the installed CLI |

## The wire

- Requests: `initialize` (clientInfo, `experimentalApi`, and
  `optOutNotificationMethods` for `turn/diff/updated` and the command and
  patch output deltas, as Zeron's `OPTED_OUT_NOTIFICATIONS`), then the
  `initialized` notification once it is answered; `thread/start` and
  `thread/resume` (cwd, approvalPolicy, sandbox, `approvalsReviewer: "user"`
  when the person is asked, model); `turn/start` (text and `localImage`
  input items, approvalPolicy, sandboxPolicy with `networkAccess` on
  workspace-write, `summary: "auto"`, model, effort, and the
  `collaborationMode` once plan mode was asked for); `turn/interrupt`;
  `model/list` (one page of 50, hidden models left out); `cx_request` for
  any other method.
- The server's lines: a notification is an event with its method and
  params; an answer reaches the request that asked (`ev.of`); the thread id
  (`thread/start` / `thread/resume` answers) and the turn in flight
  (`turn/start`'s answer, `turn/started`) are kept.
- Server requests: command and file-change approvals (v2 and the v1
  `execCommandApproval` / `applyPatchApproval`) and permission requests are
  events the host answers with `cx_approve` (accept / decline; approved /
  denied; the asked permissions for the turn or none). Unanswered before
  the next line, they are declined, never allowed silently.
  `item/tool/requestUserInput` gets no answers, an MCP elicitation is
  declined, anything else gets -32601, as Zeron does when nothing can ask.
- On 0.163.0 an interrupted turn sends no `turn/completed`: the answer to
  `turn/interrupt` (and `thread/status/changed` to idle) ends it.

## Process

The executable is a path the host found (the app probes the login shell,
the nvm versions, `~/.local/bin`, `~/.codex/bin`, `~/.npm-global/bin` and
Homebrew), run with its own folder first on PATH (an npm install is a node
script). `OPENAI_API_KEY` and `CODEX_API_KEY` are removed from the child so
its own login is used; `BROWSER=/usr/bin/true` and `NO_OPEN_BROWSER=1` keep
it from opening a sign-in page. The child runs in its own process group,
killed as one on `cx_close_process`. `cx_poll` never blocks longer than its
timeout; initialize unanswered for 60 s fails the client.

## Verification

- `coax/bin/coax test --library coax/lib/agent/codex/tests.coax`.
- `coax/lib/agent/codex/smoke.sh`: on 2026-10-29 with codex-cli 0.163.0 on
  the owner's ChatGPT login: initialize, 8 models, a thread, `ls` run with
  full access (exit 0), a supervised `touch smoke-denied.txt` declined by
  the host (the file not made), and a story interrupted after 21 deltas.

## Not done

- Steering (`turn/steer`), `/compact` and `/review`, subagent threads
  (their notifications reach the host untouched), realtime voice, skills,
  service tiers, MCP elicitation forms and user-input questions.
- Paging `model/list` past its first 50.

# The shared app

The shared app is the Coax side of the lean compiled iOS app (GitHub #13645):
one state record, one entry point, and one effect outbox the host drains. It
connects the chat page to the #13620 task system: log in, send a request, see
the task's state, messages and tool rows, answer a permission prompt, Stop,
and rejoin after relaunch.

| Directory | Owner | What it is |
| --- | --- | --- |
| `core/` | `claude-app-core` | State, dispatch, the effect outbox, the task client adapter, the host ABI |
| `login/` | `claude-login-flow` | The login flow and its card |
| `view/` | `claude-chat-view` | The chat page |
| `history/` | `gpu-native-identity` | Saved account chats through the existing index codec, visible-only GPU rows, sleeping automatic sync |
| `settings/` | `gpu-native-identity` | Public Nostr identity state; private custody stays in the native SDK |
| `environment/` | `claude-env-select` | Environment and repository selection |
| `cases/` | `claude-drive-cases` | `coax drive` cases and fake services |
| `preview/` | `claude-dev-loop` | Running the app on the Mac without Xcode |
| `device/` | `claude-device-submit` | Your own computers with no account: pairing, the device lane, its seal |

### Your own computers need no account

1. The phone pairs with your computer as its own NIP-HOST device: `openagents-environment pair --workspace DIR` prints an `openagents://pair?` link with a one-time code, and opening it on the phone asks to pair.
2. The phone's secp256k1 key is made once and kept in the Keychain (this device only, never synced); the 7-day grant sits beside it. Neither enters Coax, the outbox or a log.
3. Every request and reply is sealed, opened and checked in `device/seal.coax`; Swift (`native/AppDeviceLink.swift`) only holds the key and moves bytes over the relay's WebSocket.
4. The device lane (`device/lane.coax`) drives the environment runtime (`DescribeEnvironment` … `StopTurn`) with the same EnvironmentApp reducer, and keeps "host TAB session" for rejoin.
5. Login gates only Default (where we choose); a paired computer works logged out, and nothing is shared with the server.

## The core

| File | What it is |
| --- | --- |
| `core/core.coax` | Load this file once. It loads everything below and its checks |
| `core/state.coax` | `AppCore` and its accessors: login, the current conversation and task, drafts, selections, what Send, Stop and a permission prompt may do |
| `core/dispatch.coax` | `app_dispatch(c, AppEvent)`, the `EV_` and `ACT_` codes, answers, the guest route, `app_busy`, `app_next_wake`, `app_dirty_take` |
| `core/effects.coax` | The effect outbox: `FX_` kinds, states and outcomes, generations and fencing |
| `core/task-client.coax` | The #13620 client driven from the core: one request in flight, scheduled reads, no repeated writes |
| `core/url-policy.coax` | The installed policy: which HTTP paths, links and copies the core may ask for |
| `core/host.coax` | The C entry points (`app_host_*`) and the service table |
| `core/core-checks.coax`, `core/effects-checks.coax` | Checks |

The core adopts the #13620 client as it is: `apps/shared/durable-chat.coax`,
`chat-task-app.coax`, `chat-task-protocol.coax` and `coax/lib/agent/environment/`
own the records, codecs and control rules. The core never edits them; it
decides when to make a request and hands each result to them.

### The rules the outbox keeps

- A request carries an operation ID that is never reused, its owner and the
  owner's generation. The host taking a request means it accepted it, not
  that the effect ran.
- A completion is a later event. It is delivered once; a duplicate or an
  unknown operation changes nothing (`APP_R_STALE`).
- Outcomes stay apart: done, failed (nothing left the device), unknown (it
  may have run). The core never retries an unknown outcome by itself. The
  person asks again (`ACT_RETRY` sends the same bytes) or a read settles it.
- Cancelling an owner (a new chat, a log out) advances its generation. A late
  result of the old generation is dropped (`APP_R_LATE`).
- A refused event changes nothing and counts in `AppCore.refusals`.
- Tokens, keys and the email attempt never enter Coax.
- Closing the login flow (`ACT_LOGIN_SHOW` with flag false) ends the attempt:
  an email or verify in flight is fenced, so its late result cannot log in,
  and the host gets `FX_AUTH_CANCEL`.
- A log out forgets the drafts of the account's saved conversations and keeps
  the new chat's draft.

## Integration

### Views (login, chat page, environment)

Read the record through the accessors in `core/state.coax`; never write it.
Turn a tap into an action:

```coax
#load "../core/core.coax";

e := AppEvent.{kind = EV_ACTION, code = ACT_SEND, mono_ms = now_ms, wall_ms = wall_ms};
answer := app_dispatch(c, e);     // APP_OK, APP_LOGIN (show the login flow), or APP_R_*
if app_send_state(c) == SEND_STOP { /* the round button stops the reply */ }
```

Redraw the surfaces `app_dirty_take(c, SURF_TRANSCRIPT | SURF_COMPOSER)`
returns. The task records are `app_task(c)` (`NativeTaskApp`) and
`app_chat(c)` (`DurableChat`); read them with the shared `nta_` and `dca_`
procedures.

### Environment picker (a guest)

The picker makes its own reads through the core and routes its own results:

```coax
op := app_guest_http(c, OWN_ENVIRONMENT, MY_PURPOSE, UP_GET, "/api/chat/...", "");
// later: app_dispatch answers APP_GUEST; read c[0].guest (purpose, outcome,
// status) and the event's text, then call app_guest_done(c).
```

Paths must pass `up_path` (under `/api/chat/` or `/api/auth/account`). The
picker records a choice with `ACT_ENVIRONMENT` or `ACT_REPOSITORY` (text: the
value, more: the label).

### Host (iOS, the Mac preview, `coax drive` fakes)

Allocate one zeroed `coax_t_AppCore` on the heap (about 19 MiB, mostly task
transcript buffers) and follow the loop at the top of `core/host.coax`:
`app_host_init`, `app_host_launch(rejoin)`, then after every call drain
`app_host_next` / `app_host_request` / `app_host_path` / `app_host_text` /
`app_host_body` / `app_host_taken`, tell `app_host_cancel_next`, deliver
results with `app_host_done` in a later turn, and sleep until
`app_host_wake`. Keep `app_host_keep` for the next launch's rejoin.

The services the host implements:

| Kind | Request | Result |
| --- | --- | --- |
| `FX_HTTP` | method, relative path, body, response limit, timeout | status, body; the host adds credentials |
| `FX_AUTH_STATE` | - | flag: logged in |
| `FX_AUTH_EMAIL` | text: email address | on failure, text: words to show |
| `FX_AUTH_VERIFY` | text: the code | flag: verified; on failure, text: words |
| `FX_AUTH_CANCEL` | - | forget the email attempt |
| `FX_AUTH_LOGOUT` | - | clear the native session |
| `FX_IDS` | - | text, more: two random 128-bit IDs (32 lowercase hex); value: wall ms |
| `FX_CLIPBOARD` | body: text | - |
| `FX_LINK` | body: http(s) URL | - |
| `FX_DEVICE_LIST` | - | text: one paired computer a line, "host TAB name" |
| `FX_DEVICE_PAIR` | body: the `openagents://pair?` link | flag: paired; text: the name, or words on failure |
| `FX_DEVICE_CALL` | text: the computer's key (64 hex); body: the environment request | text: its view; unknown when no answer came |

The device rows need a `DeviceLane` attached with `app_host_device(c, d)` before
launch (about 1.6 MiB; `apps/openagents-ios/core-host.c` callocs it). With no
lane attached they are never asked for.

Lifecycle facts come in through `app_host_lifecycle` (`LIFE_FOREGROUND`,
`LIFE_BACKGROUND`); a foreground asks for the login state again and resumes
reads. The table is salvaged from the installed service table of the old
runtime (`coax/native/README.md` at `269227265a^`) without its JavaScript,
Updates and Nostr rows.

## Checks

```sh
coax/remote/remote.sh run -- coax/bin/coax test apps/shared/app/core/core.coax --library --traversal 1000000000
```

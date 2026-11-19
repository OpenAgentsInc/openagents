# Foreman control API

Date: 2026-11-19
Status: Implemented (issue #13708)
Scope: `apps/coder-desktop/ForemanOperator.swift`, `Foreman.swift`, `App.swift`, `foreman-ctl`

Coder Desktop runs a small HTTP server on a loopback port for Foreman
(`docs/coder/foreman-spec.md`). This page describes how a program outside the
Foreman chat, such as a script, a test or another agent, finds that server and
manages Foreman through it.

## Find the server

When Coder starts, it writes a control file:

```
~/Library/Application Support/Coder/foreman-control.json
```

```json
{
  "api_version": 1,
  "url": "http://127.0.0.1:52811",
  "token": "<48 hex characters>",
  "principal": "operator",
  "pid": 4242,
  "started": 1763572000.0,
  "audit_log": "/Users/you/Library/Application Support/Coder/foreman-audit.jsonl"
}
```

- Only your user can read the file (mode 0600). Coder does not use the Keychain for it.
- The port and the token change at every launch. Read the file again after Coder restarts.
- Coder removes the file when you quit it. If Coder is killed, the file stays, so
  check that `pid` is alive before you call. `foreman-ctl` does this for you.
- If two Coders run, the newest one owns the file.
- Set `CODER_FOREMAN_CONTROL_FILE` to put the file somewhere else. Coder and
  `foreman-ctl` both read that variable.

Send the token in the `X-Foreman-Token` header. Bodies and replies are JSON.

```sh
f="$HOME/Library/Application Support/Coder/foreman-control.json"
url=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["url"])' "$f")
token=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["token"])' "$f")
curl -s -H "X-Foreman-Token: $token" "$url/v1"
```

## Two credentials

| Principal | Who holds it | How it gets it |
|---|---|---|
| `foreman` | The Foreman chat's worker | `CODER_FOREMAN_URL` and `CODER_FOREMAN_TOKEN` in its environment |
| `operator` | A program outside the Foreman chat | The control file |

Routes marked **operator** below return 403 to the `foreman` token. This keeps
the specification's rule that Foreman cannot change its own tier, and adds
that it cannot prompt, stop or reset itself.

A request with no token, or a wrong one, gets 401.

## Routes

`GET /v1` returns this table, the API version and the caller's principal.

### Manage Foreman (operator)

| Route | Body | Result |
|---|---|---|
| `GET /v1/foreman` | | `shown`, `busy`, `status`, `has_history`, `folder`, `tier`, `managed_chats`, `managed_running`, `event_cursor` |
| `POST /v1/foreman/open` | | Shows the Foreman chat in the window |
| `POST /v1/foreman/prompt` | `{prompt, steer?}` | Shows Foreman and sends it the message. 409 while Foreman replies, unless `steer` is true, which stops the reply first |
| `POST /v1/foreman/stop` | | Stops Foreman's own reply. Managed chats keep running |
| `POST /v1/foreman/reset` | | Clears Foreman's history with no question asked. Managed chats are kept |
| `POST /v1/foreman/wait` | `{timeout_seconds?}` | Returns when Foreman's reply ends, or with `timed_out: true` |
| `POST /v1/tier` | `{tier}` | Sets the tier: `observe`, `managed` or `autonomous` |

A message sent with `prompt` starts with the line `[From the operator API]`, so
the transcript shows that a program sent it.

### Configuration

| Route | Result |
|---|---|
| `GET /v1/config` | `tier`, `operator_tier_raise`, `workspace`, `audit_log`, `control_file`, `api_version` |
| `GET /v1/tier` | The tier |

A program can always lower the tier. It can raise the tier only after a person
turns that on: right-click **Foreman** in the sidebar and choose **Let Programs
Raise the Tier**. The setting is off by default, and no route changes it. While
it is off, a raise returns 403 and the tier stays as it was.

### Chats, events and search (either principal)

These routes act as Foreman, within the current tier.

| Route | Body |
|---|---|
| `GET /v1/status` | |
| `POST /v1/chats/list` | `{filter?, managed_only?, folder?, limit?}` |
| `POST /v1/chats/read` | `{chat_id, tail?, include_tools?}` |
| `POST /v1/chats/create` | `{prompt, title?, folder?, budget_usd?, goal?, pin?}` |
| `POST /v1/chats/send` | `{chat_id, prompt, mode?}` |
| `POST /v1/chats/control` | `{chat_id, action, ...}` |
| `POST /v1/chats/stop-all` | `{all?}` |
| `POST /v1/wait` | `{chat_ids?, mode?, timeout_seconds?}` |
| `POST /v1/events` | `{since_id?, chat_id?, limit?}` |
| `POST /v1/audit` | `{limit?}` |
| `POST /v1/search/chats` | `{query, limit?, folder?, date_from?, date_to?}` |
| `POST /v1/search/repo` | `{query, folder?, glob?, max?}` |

## Command line

`foreman-ctl` is in the app at `Coder.app/Contents/Resources/foreman-ctl`.
Inside the Foreman chat it uses Foreman's token. Anywhere else it reads the
control file and uses the operator's.

```sh
ctl=/Applications/Coder.app/Contents/Resources/foreman-ctl
$ctl info                          # version, principal, routes
$ctl config                        # tier and settings
$ctl set-tier observe
$ctl foreman status
$ctl foreman open
$ctl foreman prompt "Summarize what the managed chats did today."
$ctl foreman wait --timeout 300
$ctl foreman stop
$ctl foreman reset
$ctl list-chats --managed-only
```

## Audit

Every operator action that changes something is appended to
`foreman-audit.jsonl` as `operator:<action>` with the principal. A request
refused for using the wrong credential is recorded too, as is a person turning
the tier-raise setting on or off.

## Limits

- One Coder window, one Foreman. The API manages the Foreman of the Coder that wrote the control file.
- `prompt` and `open` change which chat the window shows.
- Any program running as you can read the control file. That includes a command Foreman runs in its own shell. The tier-raise setting exists for this reason: with it off, the operator token cannot give Foreman more authority than a person chose.
- Events are kept in memory, 500 at most, and start again at each launch.

## Check

```sh
apps/coder-desktop/foreman-control-test.sh          # credentials, route policy, control file, server
apps/coder-desktop/foreman-control-test.sh --live   # also drives a built Coder.app through foreman-ctl
```

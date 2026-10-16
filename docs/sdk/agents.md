# Custom agents

Account-owned definitions and durable assignments are deployed through the
public website handlers and `openagents-sdk`. The iOS app's agent picker uses
these same APIs for creation, editing, and delegation. Execution requires an
updated app/headless host with the selected runtime installed and signed in.
See the [delivery plan](../custom-agents.md) and
[qualification evidence](agents-evidence.json) for scope and remaining device checks.

Use `client.agents(&session)` with an app session or supported sign-in key.
The server derives ownership from the authenticated account. Cookies, a supplied
owner, and a definition ID cannot grant access to another account's definitions.

| Request | Result |
| --- | --- |
| `POST /v1/agent-definitions` | Save `{request_id, spec}` and return the first revision. |
| `GET /v1/agent-definitions?limit=100&after=ID` | `{data, next}`, ordered by ID, excluding deleted definitions. Follow `next` until null. |
| `GET /v1/agent-definitions/{id}?revision=N` | Read the requested immutable revision, or the latest when omitted. Deleted definitions remain readable to their owner. |
| `PUT /v1/agent-definitions/{id}` | Save `{request_id, expected_revision, spec}` as a new revision. |
| `DELETE /v1/agent-definitions/{id}` | Save `{request_id, expected_revision}` as a deletion revision. |

`spec` contains `name`, `description`, `instructions`, `runtime`, optional
`model`, and `access` (`host_default`, `read_only`, or `workspace_write`). Limits are 80 UTF-8
bytes for the nonblank name, 512 for the description, 16,384 for nonblank
instructions, 128 ASCII bytes for the runtime ID, and 256 bytes for a model ID.
Names/descriptions/models reject control characters; instructions reject NUL.
Runtime IDs contain only ASCII letters, digits, `.`, `_`, `:`, and `-`.
They identify a host-supported implementation and are never commands or URLs.
Unknown request fields are refused. Saving a definition executes no code.

IDs and `request_id` are random 64-character lowercase hexadecimal values.
Create uses its request ID as the definition ID. Every mutation retains its
request ID and exact typed inputs durably. Identical retries return the original
result, including after updates or deletion. Changed inputs with the same request
ID return `409 idempotency_conflict`. A stale expected revision returns
`409 version_conflict`. Reload the latest state before proposing another edit.
A replayed result describes that original operation; read again for current state.

SDK request records bind mutations to an account and deployment. Save them
privately before sending, serialize them across restart, and reuse the original
record after an uncertain response. The SDK sends once, supports cancellation,
and never retries an uncertain mutation with a new identity. Canceling the local
request cannot undo a completed server write. Account/deployment mismatch refuses
before transmission. Clear app views and ignore late responses after sign-out.

Lists accept limits 1–100 and continue after an ID even if that definition was
deleted. They are not snapshots; restart discovery to find new earlier IDs.
The initial account capacity is 128 active definitions, 4,096 successful mutations,
and 4 MiB including all revisions and retry records. Deletion frees an active slot but does not free
historical capacity. Capacity returns `429 quota_exhausted`; it never discards
unresolved retry records. The HTTP body limit is 24 KiB, and SDK definition
responses are bounded to 4 MiB. Responses are private and must not be cached.

The API refuses missing/foreign IDs with `404 not_found`, invalid fields with
`400 invalid_agent`, invalid credentials with `401`, and unavailable stores with
`503`. A failed read is not an empty list. Current account checks inherit the
existing successful authorization cache of at most 30 seconds. Definition
permission settings are requests; only the executing host can enforce them.

## Assign work

Create a bound request with `agents.assignment_request(AssignmentSpec { agent,
agent_revision, host, project, task })`, save it privately, and send it with
`agents.assign`. Retry the same record after an uncertain response. The response
pins the definition and supplies a stable `chat` ID. Saving this request starts
no cloud computer; an authenticated app host must be online with the runtime
installed and enabled. Use `assignments`, `assignment`, `command_request`,
`command`, `report_request`, and `report` to observe and control work.

The endpoints are `GET/POST /v1/agent-assignments`,
`GET /v1/agent-assignments/{id}`, and
`POST /v1/agent-assignments/{id}/commands` or `/report`.
Commands are `follow_up` with text, `stop`, and `archive`. A stop is pending
until the host reports it stopped. Archive requires a terminal assignment.
Reports require the current revision and selected host ID. They are account
claims, not hardware attestation. A report pins the active follow-up and its
starting session sequence; completion must come from a later session event.
A stale report cannot complete a later follow-up. An uncertain run is `unknown`,
never automatically retried as new execution. Inspect its conversation first.

The existing app session owns live output and native conversation controls.
Assignment `result` is only a summary of at most 4,096 UTF-8 bytes. Task and
follow-up text are bounded to 64 KiB. Lists have at most eight entries per page;
follow `next` until null. The account book permits 256 unarchived assignments,
128 commands each, 4,096 mutations, and 4 MiB including retry records.

## Run an independent Rust agent

The [definition example](../../crates/openagents-sdk/examples/create_agent.rs)
creates an account-owned agent through the public SDK without host dependencies.
Build the [runner example](../../crates/openagents-sdk/examples/agent_runner.rs):

```sh
cargo build -p openagents-sdk --example agent_runner
```

On the app host, create `sdk-runners.json` inside that host's device data
folder. Restart the host after changing the file. The host alone owns this list:

```json
{
  "reviewer": {
    "label": "My Rust reviewer",
    "executable": "/absolute/path/to/agent_runner",
    "sha256": "<64 lowercase hexadecimal characters from shasum -a 256>",
    "args": []
  }
}
```

Use the executable’s actual SHA-256 hash. The host checks it at registration
and before each launch; a changed binary requires an operator update.

Save a definition with runtime `sdk`, model `reviewer`, and access
`host_default`. The mobile form calls this **SDK runner** and asks for the
installed runner name. The host lists installed runner names through its model
catalog and refuses missing names. The definition never contains a command.
The runner executes under the host user's configured access; scoped access is
refused for runners until an enforcing adapter exists.

`agents::runner::serve_once` accepts one bounded JSON `Turn` and returns a typed
result. A turn includes instructions, task, assignment, conversation, and working
folder. The wire schema is `openagents.agent.runner.v1`; stdin contains one JSON
line. Stdout contains `text`, then exactly one `completed` or `failed` event.
Logs go to stderr. Maximum frame: 96 KiB; output: 1 MiB; wall time: five minutes.
A fresh child handles each follow-up. The host stops and reaps its child when
the user stops work or closes the event stream. It refuses malformed output,
missing results, and unsuccessful process exits.

Private Nostr export uses [NIP-AGENT](../../nips/openagents/NIP-AGENT.md) and
`nostr::agent_profile`; exporting configuration does not execute it.

## Test an agent in Agent Lab

Use [`Client::labs(session)`](../../crates/openagents-sdk/src/labs.rs) to save
an experiment and run a finite set of tasks through the same assignment service.
The [Agent Lab guide](../agent-lab/README.md) covers the shared mobile/browser
workbench, checks, comparisons, supported references, and explicit promotion.
Saving a Lab document or opening its report does not start an agent.

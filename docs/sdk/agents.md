# Private agent definitions

The implementation adds account-owned definitions to `openagents-sdk` and the
public website handlers. Production qualification is tracked in
[#11903](https://github.com/OpenAgentsInc/openagents/issues/11903); do not infer
availability from the existence of a schema. The [custom-agent plan](../custom-agents.md)
tracks assignment, mobile UI, runners, and protocol work separately.

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
`model`, and `access` (`read_only` or `workspace_write`). Limits are 80 UTF-8
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

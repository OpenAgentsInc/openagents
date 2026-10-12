# Managed agent operations

2026-10-11. Proposed. Nothing on this page is a live route.

This page is the workspace around a session: webhooks, cron deployments, the files you keep, and what a session spends. Paths and objects are in [Managed Agents](managed-agents.md). Steering a live session is in [Managed agent events](managed-agents-events.md).

## Webhooks

A webhook tells your server that a resource changed. The body is thin. Fetch the resource for its current state. The signature matches [decision-job webhooks](../decision-models/service/durable-jobs.md): one header for the event id, one HMAC over that id and the raw body.

These notices are OpenAgents calling you. A GitHub push that should start a session is your own handler, which then calls `POST /v1/sessions`.

### Register an endpoint

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/v1/webhook_endpoints` | Create an endpoint. The response includes `secret` once. |
| `GET` | `/v1/webhook_endpoints` | List endpoints. |
| `GET` | `/v1/webhook_endpoints/{webhook_endpoint_id}` | Read one. No secret. |
| `POST` | `/v1/webhook_endpoints/{webhook_endpoint_id}` | Replace `url` or `event_types`. |
| `POST` | `/v1/webhook_endpoints/{webhook_endpoint_id}/rotate_secret` | A new secret, once. Later deliveries use it. |
| `POST` | `/v1/webhook_endpoints/{webhook_endpoint_id}/disable` | Stop delivery. `disabled_reason` is `manual`. |
| `POST` | `/v1/webhook_endpoints/{webhook_endpoint_id}/enable` | Deliver again. Events emitted while it was disabled are not replayed. |
| `DELETE` | `/v1/webhook_endpoints/{webhook_endpoint_id}` | Delete. Returns `204`. |

The `agents` scope covers these routes. `agents:read` covers the reads.

```json
{
  "url": "https://example.com/hooks/agents",
  "event_types": ["session.status_idled", "deployment_run.failed"]
}
```

`url` is `https` on port 443 and a public hostname, or `http` to a loopback address. A URL with a username or password is `400`. An address that is not public, other than loopback, is `400`. `event_types` has at least one entry from the table below. An unknown type is `400`.

The id is `whk_`. `secret` is opaque, shown once, and stored as a digest. `status` is `active` or `disabled`.

An event is offered to endpoints subscribed to its `data.type` at the moment it is emitted. Subscribing later does not backfill.

### Verify

Every delivery is `POST` with:

| Header | Value |
| --- | --- |
| `x-openagents-event` | The event id (`whe_`). The same value on every retry. Dedupe on it. |
| `x-openagents-signature` | `sha256=<hex>`. Hex is HMAC-SHA256 of `id + "." + raw body`, with the endpoint secret as the key. |

Compare the hex in constant time over the raw body bytes. A framework that re-parses JSON and serializes it again changes the bytes and fails the check. A bad signature is your `400`. A delivery you have already handled is your `2xx`.

### Body

```json
{
  "v": "openagents.agent-event.v1",
  "id": "whe_9d5c1f7e",
  "created_at": "2026-03-18T14:05:22Z",
  "data": {
    "type": "session.status_idled",
    "id": "sesn_01XYZ",
    "workspace_id": "ws_01abc"
  }
}
```

`id` matches `x-openagents-event`. `created_at` is when the event happened. Switch on `data.type`, fetch `data.id`, and return any `2xx`.

The webhook `data.type` values are a different set from the session stream. `session.status_idled` here is `session.status_idle` on the stream.

| `data.type` | When it fires |
| --- | --- |
| `session.status_scheduled` | The session exists and can take events. |
| `session.status_run_started` | The agent started working, on every transition to `running`. |
| `session.status_idled` | The agent is waiting, or the budget paused it. The body has no `stop_reason`. List events and read the latest `session.status_idle`. |
| `session.status_rescheduled` | A transient error. The session will retry. |
| `session.status_terminated` | The session ended, on completion or on error. |
| `session.thread_created` | A thread was opened, including an advisor consultation. |
| `session.thread_idled` | A child thread is waiting, or paused at the budget. The primary thread's end is `session.status_terminated`. |
| `session.thread_terminated` | A child thread ended. |
| `session.outcome_evaluation_ended` | The grader finished one iteration. |
| `session.updated` | A session field changed. |
| `session.deleted` | The session was deleted. There is nothing to fetch. |
| `vault.created`, `vault.archived`, `vault.deleted` | Vault lifecycle. Delete also emits `vault_credential.deleted` for each credential. |
| `vault_credential.created`, `vault_credential.archived`, `vault_credential.deleted` | Credential lifecycle. Archive and delete include credentials closed because the vault was. |
| `vault_credential.refresh_failed` | An MCP OAuth refresh failed. |
| `agent.created` | An agent was created. |
| `agent.updated` | A new agent version was stored. An update that does not create a version emits nothing. |
| `agent.archived` | The agent was archived. |
| `deployment.created`, `deployment.updated` | Deployment lifecycle. |
| `deployment.paused` | Paused by request, or because a fire failed in a way that will not recover (the agent or the environment is archived). A rate cap, an empty balance, or `service_unavailable` does not pause the deployment. |
| `deployment.unpaused` | The schedule resumes. |
| `deployment.archived` | Archived directly, or because the agent was archived. |
| `deployment_run.started`, `deployment_run.succeeded`, `deployment_run.failed` | A scheduled fire. `data.id` is the `drun_` id. Manual runs emit none of these. |
| `environment.created` | An environment was created. |
| `environment.updated` | An update changed at least one field. |
| `environment.archived` | Archived. Archiving an archived environment emits nothing. |
| `environment.deleted` | Deleted. There is nothing to fetch. |
| `memory_store.created`, `memory_store.archived`, `memory_store.deleted` | Store lifecycle. Delete removes memories and versions and emits no per-memory event. |

There is no `memory_store.updated`, no per-memory event, and no work-item event. Poll memory versions when you need that history. Agents have no delete, and deployments have no delete, so there is no `agent.deleted` or `deployment.deleted`.

### Delivery

Attempts are the same rule as decision-job webhooks: the first POST plus up to three retries, so four tries, with backoff between 5 and 120 seconds. A `2xx` completes the delivery. After the last failure the event is dropped. Webhooks are not a log. List or fetch the resource when you have to see every transition.

The same `id` and the same body are retried, so the signature does not change between attempts.

Three cases disable the endpoint on the first attempt and do not retry that event:

| `disabled_reason` | Cause |
| --- | --- |
| `redirect` | The endpoint returned a `3xx`. Redirects are not followed. |
| `invalid_address` | At connect time the host is not a public address, and it is not loopback. |
| `sustained_failure` | Delivery has been failing for a sustained period. One `2xx` resets that window. A single failed event does not disable the endpoint. |

Events are not ordered. `session.status_idled` can arrive before `session.outcome_evaluation_ended`, and a delete can arrive before an archive. Drive state from the resource you fetch.

A scheduled run has no client attached. Answer a `requires_action` idle from the webhook handler, or the run waits. If the idle is `budget_reached`, change or remove that session's budget. A `user.message` there is `400`.

## Schedules

A deployment (`depl_`) creates a session each time it fires. The catalog has the routes. This section is how the clock and the failures work.

`initial_events` has at least one of `user.message`, `user.define_outcome`, and `system.message`. A session's `initial_events` does not accept `system.message`. Use an outcome when the fire produces one checkable artifact. Use a message when it answers a question or changes records in another system.

`schedule.expression` is a POSIX cron expression with minute resolution: `minute hour day-of-month month day-of-week`. `schedule.timezone` is an IANA name. The expression is wall-clock time in that zone. `0 20 * * *` in `America/New_York` fires at 20:00 local in both standard time and daylight time.

A local time that does not exist on a spring-forward day is skipped. A local time that occurs twice on a fall-back day fires twice. Use UTC, or a time outside 01:00–03:00 local, when a skip or a double fire matters.

`schedule.upcoming_runs_at` lists the exact next fire times. Execution is delayed by up to 15 percent of the interval between fires, at least 5 seconds and at most 9 minutes. An hourly deployment can start 9 minutes after the listed time.

A workspace holds at most 1,000 deployments. That is a resource ceiling, not a request-rate limit.

`budget` on a deployment is copied onto each session at fire time. You can set it on create and on update. `budget: null` clears it, and a later update can set it again. A change applies to the next session. A session that is already running keeps the cap it was created with. Change that cap on the session.

### Runs

Every trigger writes a deployment run (`drun_`), except when the agent is archived. Then the deployment is archived in the same operation, no run is written, and no further session is created.

```json
{
  "type": "deployment_run",
  "id": "drun_01abc",
  "deployment_id": "depl_01xyz",
  "trigger_context": { "type": "schedule", "scheduled_at": "2026-05-09T00:00:00Z" },
  "session_id": null,
  "error": { "type": "environment_archived", "message": "environment env_01abc is archived" },
  "agent": { "type": "agent", "id": "agent_01ghi", "version": 3 },
  "created_at": "2026-05-09T00:00:01Z"
}
```

A successful run has `session_id` and no `error`. Follow that session on its stream or with `session.*` webhooks.

| `error.type` | Meaning | Next fire |
| --- | --- | --- |
| `environment_archived` | The environment is archived. The deployment is paused. | Fix it and unpause, or archive the deployment. |
| `agent_archived` | Recorded only if a run was already opening. Archiving the agent archives the deployment with no run. | None. |
| `vault_not_found` | A `vault_id` on the deployment is gone. | Fix the deployment. The schedule tries again next time. |
| `computer_not_granted` | A self-hosted environment's computer is not granted to the key. | Grant the computer. The schedule tries again next time. |
| `session_rate_limited` | A cap you set on the key refused the fire. OpenAgents adds no house rate limit. | The schedule tries again next time. This fire is not retried. |
| `insufficient_balance` | The workspace balance could not cover the session. | Top up. The schedule tries again next time. |
| `service_unavailable` | The service could not create the session. | The schedule tries again next time. |

`trigger_context.type` `manual` is `POST /v1/deployments/{deployment_id}/run`. It has no `scheduled_at`. It works while the deployment is paused. It writes a run and emits no `deployment_run.*` webhook.

Pause sets `paused_reason` to `{ "type": "manual" }` and leaves sessions that are already running. It does not fill in fires that were missed. Unpause clears `paused_reason` and waits for the next occurrence. An automatic pause sets `paused_reason` to `{ "type": "error", "error_type": "<the run error>" }`.

Archive is terminal. The schedule stops and the deployment cannot be updated.

## Files you keep

The HTTP API is the control plane. A directory is a convenient way to store the JSON you POST. Create each resource once, keep the id, and send that id on later session creates. Creating an agent on every request leaves unused agents and skips versions.

```text
agents/<name>/agent.json          POST /v1/agents
agents/<name>/environment.json    POST /v1/environments
agents/<name>/vault.json          POST /v1/vaults, with no secret fields
agents/<name>/session.json        POST /v1/sessions, ids filled in after create
agents/<name>/deployment.json     POST /v1/deployments
agents/<name>/outcome.json        a user.define_outcome event, not its own resource
```

`outcome.json` is the event you put in `initial_events` or send later. It is not a route.

Skills live where the rest of the workspace already looks:

```text
.agents/skills/<name>/SKILL.md
.agents/skills/<name>/references/
.agents/skills/<name>/examples/
.agents/skills/<name>/scripts/
```

`SKILL.md` starts with YAML front matter: `name` (the directory name, lowercase and hyphens), `description` (when to use the skill, in the third person, with the phrases that should trigger it), and optional `metadata`. The body is instructions. Keep it short and put long material under `references/`. The agent sees the name and description first and reads the body when the task matches. Publish a skill with `POST /v1/skills`. A payable package stays `/v1/plugins`.

This API does not run a hooks file. Who may call a tool is `permission_policy` on the agent. A check that has to run in your process answers `user.tool_confirmation`, or you wrap the action in a custom tool.

A mounted file is a session copy. `session.resources[].file_id` is not the `file_id` you uploaded. You can delete the upload. The copy is removed with the session. `mount_path` is absolute and lands under `/mnt/session/uploads/`.

## Spend

Set a session budget when a run can keep going on its own, and when a workflow run can spawn many agents. The gate, the rounding, and the pause order are in [events](managed-agents-events.md#budget).

What you can change without a new agent:

| Lever | Where |
| --- | --- |
| Prompt cache | Automatic on a session. Reported as `cache_read_input_tokens` and `cache_creation_input_tokens`. Stable tools and system text belong before text that changes every turn. |
| How much context the model sees | Skills load on demand. The session summarizes model context when it grows, and emits `agent.thread_context_compacted`. `POST /v1/responses/compact` is the same idea for a stored response you hold yourself. |
| How hard the model tries | `model.effort` on the agent. A session `model` override replaces the whole model object, so set `effort` on the override when you want it. |
| How much it may spend | `budget.max_list_cost.amount_sats`. |
| Which model | `openagents/fast` for a narrow worker. `openagents/reason` or `openagents/code` for the coordinator. A roster entry is priced at its own model. |

A task budget on a single `/v1/responses` call is advisory and token-denominated. A session budget is a hard cap in sats, enforced by the service.

## One model call

Use `POST /v1/responses` when you want one answer, a function tool, or a short loop you run yourself. The [inference gateway](../inference/gateway.md) is that contract:

| You want | On `/v1/responses` |
| --- | --- |
| A function the model calls, and you run | A function tool, `tool_choice` (`auto`, `required`, `none`, a named function, or `allowed_tools`), and `parallel_tool_calls`. Return every output before the next call. The caller steps are in [The tool loop](model-calls.md#the-tool-loop). |
| Web search without a session | The hosted `openagents:web_search` tool. |
| Reasoning you can send back | A reasoning item. Pass `encrypted_content` through unchanged when the response includes it. A session keeps that prefix. On `/v1/responses`, follow [Keeping reasoning valid](model-calls.md#keeping-reasoning-valid). |
| A shorter stored conversation | `POST /v1/responses/compact`. |
| JSON that matches a schema | The response text format. |
| How many input tokens a body will use | `POST /v1/responses/count`. The count is not billed. |
| Many calls at half the rate-card price | `POST /v1/responses/batches`. |
| A file, citations, a cache breakpoint, a refusal, or a context edit | [Model calls](model-calls.md). |

A session is that loop plus a container, permissions, memory, files, and a schedule. Bash and the file tools in the container are how a session runs code. Screen control is not a session tool. Wrap a host-side action in a custom tool.

Documents and images on a response, including citations, are in [Model calls](model-calls.md). On a session, upload a file and attach it as a `file` resource. A session's outputs are the files under `/mnt/session/outputs/`.

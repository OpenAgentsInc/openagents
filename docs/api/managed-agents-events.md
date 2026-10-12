# Managed agent events

2026-10-11. Proposed. Nothing on this page is a live route.

This page is how you steer a session and how you read it. The resource paths, object fields, and error codes are in [Managed Agents](managed-agents.md). Schedules, webhooks, and the files you keep are in [Managed agent operations](managed-agents-operations.md). A single model call, with tools and no session, is [`POST /v1/responses`](../inference/gateway.md).

Auth, cursors, sats, and the error object are the same as the rest of the API.

## Send events

`POST /v1/sessions/{session_id}/events` takes `{ "events": [ ... ] }`. The server validates the whole list, stores each event, and assigns `sevt_` ids before it answers. The create response for a session does not echo `initial_events`. Read them back from the event list.

| Type | When you send it |
| --- | --- |
| `user.message` | A turn from you. On the primary thread, this text counts as your intent when a tool policy is `auto`. |
| `system.message` | Context appended for this turn and later turns. It does not replace the agent's `system`. |
| `user.define_outcome` | A goal and a rubric. See [Outcomes](#outcomes). |
| `user.tool_confirmation` | Allow or deny a tool call that paused for you. |
| `user.custom_tool_result` | Your application's answer to `agent.custom_tool_use`. |
| `user.tool_result` | A settle event, processed on receipt. Permission asks use `user.tool_confirmation`. Custom tools use `user.custom_tool_result`. |
| `user.interrupt` | Stop work. With no `session_thread_id`, every non-archived thread stops, including the primary. |

You can send events while the session is `running` or `idle`. The server queues them and handles them in order. You do not wait for a reply before sending the next message.

Two states accept only settle events: `user.tool_confirmation`, `user.tool_result`, `user.custom_tool_result`, and `user.interrupt`. Anything that starts new work, including `user.message` and `user.define_outcome`, is `400`.

| State | Settle events only |
| --- | --- |
| `stop_reason.type` is `requires_action` | A tool is waiting for you. |
| `stop_reason.type` is `budget_reached` | The session budget is spent. Changing or removing the budget is what resumes it. |

`stop_reason` is a field of `session.status_idle` and `session.thread_status_idle`. The session object has `status` and `usage`. It does not carry `stop_reason`. Read the latest idle event.

### System context

`system.message` content is 1 to 1,000 text blocks:

```json
{
  "type": "system.message",
  "content": [{ "type": "text", "text": "The caller's timezone is America/New_York." }]
}
```

The check uses the session's primary model. `openagents/classify` rejects the event with `400`. Every other public model id accepts it. The event lands on the primary thread.

While the session is idle with `requires_action`, a `system.message` is accepted only as a later event in the same request as a settle event. On its own, or beside a `user.message`, it is `400` until the pending tool events are answered.

A session's `initial_events` accepts `user.message` and `user.define_outcome` only. A deployment's `initial_events` also accepts `system.message`.

### Confirm a tool

`tool_use_id` is the `sevt_` id of the `agent.tool_use` or `agent.mcp_tool_use` event. `result` is `allow` or `deny`. `deny_message` is allowed only with `deny`, and the agent sees it as the tool result. Several confirmations can go in one request.

Send a confirmation only when `evaluated_permission` is `ask`. A call the server denied under `auto` (`reason_code` `high_risk`) already has an error result. Confirming it is `400`.

### Custom tool result

```json
{
  "type": "user.custom_tool_result",
  "custom_tool_use_id": "sevt_abc123",
  "content": [{ "type": "text", "text": "{\"temp_c\": 18}" }],
  "is_error": false
}
```

`custom_tool_use_id` is the `sevt_` id of `agent.custom_tool_use`. The container never sees a secret you use while computing this result. Run that call in your process.

### Interrupt

An interrupt jumps the queue, ahead of queued user messages. The agent does not see it as a message. The turn ends with `stop_reason.type` `end_turn`, the same value a finished turn uses. Remember that you sent the interrupt if you need to tell the two apart.

At `budget_reached`, an interrupt is accepted, ignored, and not stored. Raise or remove the budget.

Against a session that is already idle, an interrupt changes nothing, with one exception. A self-hosted session whose worker failed the claim sits idle with `requires_action` and no tool event to answer. `user.interrupt` queues that work again.

In a multiagent session, omit `session_thread_id` to interrupt every non-archived thread. Pass it to interrupt one thread. Naming a terminated thread is `400`. Naming a workflow run's thread while the run is open is `400` `workflow_run_open`. An interrupt does not end a workflow run. Ask the agent to stop the run.

An active outcome ends with result `interrupted`, except at the budget cap, where the interrupt is ignored and the outcome stays active.

## Read events

Three ways:

| Way | Call | What you get |
| --- | --- | --- |
| Stream | `GET /v1/sessions/{session_id}/events/stream` | Events emitted after the connection opens, as server-sent events. The server sends `: ping` comment lines to keep the connection open. |
| List | `GET /v1/sessions/{session_id}/events` | Stored events. `limit` and `after` follow the API's cursor rules. `order` is `asc` (default, oldest first) or `desc`. `types[]` filters. |
| Webhook | A workspace endpoint | A thin notice that something changed. Fetch the resource. See [operations](managed-agents-operations.md). |

The stream delivers events emitted after you open it. Open the stream, then send. A list is how you read events from before the connection, or events you missed while it was down.

A thread has its own list and its own stream: `GET /v1/sessions/{session_id}/threads/{thread_id}/events` and `GET /v1/sessions/{session_id}/threads/{thread_id}/stream`. The thread stream path has no `/events` segment.

The list returns immediately. It is not a long poll. Bound the wall clock of each request in your client. A per-chunk read timeout resets whenever a byte arrives, and a stalled body can hold that call open.

### Fields on a stored event

Every stored event has `id` (`sevt_`), `type`, and `processed_at`. `processed_at` is set when the server finishes the event.

Events you send are echoed on the stream. `processed_at` is `null` while the event is still queued, and set when it is processed, so the same id can appear twice. `user.define_outcome`, `user.custom_tool_result`, and `user.tool_result` are processed on receipt. Their first echo already has `processed_at` set.

An interrupt sent at the budget cap is not echoed.

Tolerate a `type` you do not recognize.

### Events the session emits

| Type | Meaning |
| --- | --- |
| `agent.message` | The agent's text. This is the record of what it said. |
| `agent.thinking` | The agent is reasoning. The event carries no reasoning text. |
| `agent.tool_use` | A built-in tool call, with `evaluated_permission` and usually `evaluation`. |
| `agent.tool_result` | The built-in tool's result. |
| `agent.mcp_tool_use` | An MCP tool call. Same permission fields as `agent.tool_use`. |
| `agent.mcp_tool_result` | The MCP tool's result. |
| `agent.custom_tool_use` | Your tool. The session goes idle until `user.custom_tool_result`. |
| `agent.thread_context_compacted` | The model context was summarized so the session can continue. `pre_compaction_tokens` is the size before the summary. The event list you read is unchanged. |
| `session.status_running` | The agent is working. |
| `session.status_idle` | The agent is waiting. `stop_reason` says why. |
| `session.status_rescheduled` | A retryable error. The server will try again. |
| `session.status_terminated` | The session is over, on completion or on an unrecoverable error. Fetch the session to tell the two apart. |
| `session.updated` | An update changed at least one field. The event carries the changed fields. Clearing the budget sends `budget: null`. |
| `session.usage` | Cumulative tokens, list cost, and active time. See [Usage](#usage). |
| `session.error` | A processing error. An empty balance is `error.type` `insufficient_balance`, then the session goes idle. Top up, then send a `user.message`. |
| `span.model_request_start` | A model call started. |
| `span.model_request_end` | A model call finished. `model_request_start_id` matches the start. `model_usage` has `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, and `cache_read_input_tokens`. `is_error` is a boolean. |
| `span.outcome_evaluation_start` | The grader started an iteration. |
| `span.outcome_evaluation_ongoing` | The grader is still running. The reasoning is not in the event. |
| `span.outcome_evaluation_end` | The grader finished an iteration. |
| `session.thread_*` and `agent.thread_message_*` | Multiagent threads. See the catalog. |
| `workflow_run.*` | A workflow run. See the catalog. |

`session.status_idle` looks like this:

```json
{
  "id": "sevt_456",
  "type": "session.status_idle",
  "processed_at": "2026-04-07T04:27:43Z",
  "stop_reason": { "type": "requires_action", "event_ids": ["sevt_123"] }
}
```

`stop_reason.type` is `end_turn`, `requires_action`, `retries_exhausted`, or `budget_reached`. `event_ids` is present for `requires_action`.

## Live previews

By default, `agent.message` arrives after the model call that produced it. Opt in on one stream connection with a repeated query parameter:

```text
GET /v1/sessions/sesn_abc/events/stream?event_deltas[]=agent.message&event_deltas[]=agent.thinking
```

Accepted values are `agent.message` and `agent.thinking`. Any other value is `400`. More than 100 values is `400`. The session stream and each thread stream accept the parameter. Percent-encode the brackets in a shell.

Previews are scoped to the thread that connection is reading. A child thread's preview is on that thread's stream. A reply to the coordinator arrives later as `agent.thread_message_sent` and is not previewed.

```text
event: event_start
data: {"type":"event_start","event":{"type":"agent.message","id":"sevt_01abc"}}

event: event_delta
data: {"type":"event_delta","event_id":"sevt_01abc","delta":{"type":"content_delta","index":0,"content":{"type":"text","text":"Here is the summary"}}}
```

`event_start` and `event_delta` have no id of their own. The id they carry is the id of the event they preview. For `agent.thinking`, the stream emits `event_start` and no deltas. The stored `agent.thinking` event also has no reasoning text.

Keep a buffer keyed by `(event_id, index)`. On `event_start`, start an empty buffer. On each `event_delta`, append `delta.content.text`. When the stored `agent.message` with that id arrives, discard the buffer and render the stored content. The stored message is the record.

On a normal turn the order is `session.status_running`, `span.model_request_start`, `event_start`, zero or more `event_delta`, the stored `agent.message`, then `span.model_request_end`. If the turn errors or is interrupted, the stored message may never arrive. `span.model_request_end` still does. Close any open preview when you see it.

Deltas are a prefix of the stored text, and under load the server can stop sending them. They are not stored, they are not on the list endpoint, and a connection opened after the model call started receives none for that call. A reconnect follows [Read across a dropped stream](#read-across-a-dropped-stream).

One connection emits at most one `event_start` per event id. Tool calls, tool results, and other threads are not previewed on that connection.

## Read across a dropped stream

Open the stream first, so events that arrive while you list history are buffered. Then list history. Then read the stream, and skip ids you already handled.

Terminal checks still run for an event you have already seen. A `session.status_terminated` or a finishing `session.status_idle` that was in the history has to end the loop, or a duplicate on the live stream is ignored and the loop never exits.

```text
seen = empty
open the stream
for event in list (order=asc, follow next):
  seen.add(event.id)
  handle(event)
  if the event says the session is done: stop
for event in stream:
  if event.id is not in seen:
    seen.add(event.id)
    handle(event)
  if the event says the session is done: stop
```

`event_start` and `event_delta` have no stored id. Dedupe applies to stored events.

## When to stop reading

Leave the loop when you see `session.status_terminated`, or `session.status_idle` whose `stop_reason.type` is not `requires_action`.

| `stop_reason.type` | What you do |
| --- | --- |
| `requires_action` | Stay in the loop and answer the tool. If the session is self-hosted, idle, and there is no `agent.tool_use`, `agent.mcp_tool_use`, or `agent.custom_tool_use` waiting, the worker failed the claim. Fix the computer and send `user.interrupt`. |
| `end_turn` | The turn finished. |
| `retries_exhausted` | The agent stopped after repeated errors. Fetch the session. |
| `budget_reached` | The cap was hit. A `session.usage` event immediately precedes this idle. Change or remove the budget, or treat the session as done. |

`span.outcome_evaluation_end` is not the end of the read. On `needs_revision` the session keeps running.

The stream can emit `session.status_idle` slightly before a fetch shows `idle`. Archive and delete return `400` while the status is still `running`. Fetch the session and wait until `status` is no longer `running` (a short poll, on the order of two seconds), then archive. Archiving a session is ordinary cleanup. Archiving an agent or an environment is permanent.

## Budget

The pause arrives in this order:

1. `session.thread_status_idle` for each thread as it pauses. When a thread's last request both crosses the cap and finishes the turn, that thread's `stop_reason.type` can be `end_turn` while the session's is `budget_reached`. Use the session event.
2. `session.usage`.
3. `session.status_idle` with `budget_reached`.

One thread waiting on a tool outranks the cap. The session's `stop_reason.type` is then `requires_action`. Answer the tool. Settle events do not start a model call.

No event you send resumes the cap. `POST /v1/sessions/{session_id}` with a new `budget`, or `budget: null`, does. The new `amount_sats` must be greater than the sats already consumed. A removed session budget cannot be added again. A deployment's budget can. See [operations](managed-agents-operations.md).

The gate runs before each model call. The call that crosses the cap finishes, so the total can pass the cap by at most one model call per running thread. `usage.list_cost.amount_sats` is rounded to the nearest sat. The gate uses the exact amount. Trust `budget_reached`, not an equal reported total.

## Usage

`session.usage`, and the session object's `usage`, carry:

| Field | Meaning |
| --- | --- |
| `input_tokens`, `output_tokens` | Tokens billed at the model's input and output rates. |
| `cache_creation_input_tokens` | Tokens written into the prompt cache. |
| `cache_read_input_tokens` | Tokens read from the prompt cache. |
| `list_cost.amount_sats` | List-priced spend so far, rounded to the nearest sat. This is the figure a budget uses. |
| `active_seconds` | Time with at least one thread running. Overlapping threads count once. |
| `server_tool_use.web_search_requests` | Web searches, priced on the rate card. |
| `server_tool_use.web_fetch_requests` | Reported. A fetch is not a separate charge. The page enters context as input tokens. |
| `budget` | The cap, when one is set. |

`stats.active_seconds` sums each thread's time and can be larger. Per-thread `list_cost` values do not add up to the session total: the session figure includes cloud container time, and each figure is rounded on its own.

The session caches the stable prefix of the model call (tools, then system, then earlier turns) and reports the counts above. You do not send cache breakpoints on a session event. A byte that changes in that prefix misses the cache. Prices for input, cached input, cache writes, output, and reasoning are on the model's rate card.

On a cloud environment, list cost is model tokens, web searches, and container time (`active_seconds`) at the rate card. On a self-hosted environment, list cost is model tokens and web searches. Time on the granted computer is that computer's, and it is not in `list_cost`.

A model with no list price cannot be used in a budgeted session (`400`). If a running budgeted session's usage later includes one, a budget change is rejected. Remove the budget to continue.

Reasoning text is not on the session stream. `agent.thinking` is a progress signal. Effort on the agent model is how you ask for more or less reasoning. On [`POST /v1/responses`](../inference/gateway.md), a reasoning item can carry `encrypted_content`. Send that item back unchanged on the next call. A session does not ask you to do that.

## Outcomes

An outcome is a `user.define_outcome` event, not a field on session create. The agent works from `description`. You do not also send a `user.message` to start it. The exception is a general description that needs the detail in an earlier message: put the `user.message` first and the outcome second, in the same `events` array.

One outcome in `initial_events`. A second, or a missing `rubric`, rejects the create with `400` and creates no session. A non-empty `initial_events` creates the session already `running`.

```json
{
  "type": "user.define_outcome",
  "description": "Build a discounted-cash-flow workbook.",
  "rubric": { "type": "text", "content": "The workbook has a sources sheet and a summary that ties out." },
  "max_iterations": 5
}
```

`rubric` is `{ "type": "text", "content" }` or `{ "type": "file", "file_id" }`. `max_iterations` defaults to 3 and is at most 20. Write criteria a grader can check one by one. The echo adds `outcome_id` (`outc_`) and `processed_at`.

The grader has its own context and does not have `web_search` or `web_fetch`. With an outcome defined, no evaluation starts while a workflow run is `running`. The agent's turn on the run's ending notice can start one.

| Event | What it carries |
| --- | --- |
| `span.outcome_evaluation_start` | `outcome_id`, `iteration` (starts at 0). |
| `span.outcome_evaluation_ongoing` | `outcome_id`. A heartbeat. |
| `span.outcome_evaluation_end` | `outcome_evaluation_start_id`, `outcome_id`, `iteration`, `result`, `explanation`, `usage`. |

| `result` | What happens next |
| --- | --- |
| `satisfied` | The session goes idle. This outcome is done. |
| `needs_revision` | The agent starts another iteration. |
| `max_iterations_reached` | The grader stops. The agent may run one more revision, then the session goes idle. |
| `failed` | The session goes idle. The rubric does not match the task. |
| `interrupted` | A `user.interrupt` arrived while the outcome was active, including before the grader started. `outcome_evaluation_start_id` can be an empty string. Do not look it up until you check. |

`GET /v1/sessions/{session_id}` includes `outcome_evaluations`, each with `outcome_id` and the latest `result`. Deliverables are files the agent wrote under `/mnt/session/outputs/`. List them with `GET /v1/files?scope_id=sesn_...` after the session is idle. Indexing can lag one to three seconds.

One outcome at a time. Send the next `user.define_outcome` after a terminal result (`satisfied`, `max_iterations_reached`, `failed`, or `interrupted`). The session keeps its history. A `user.message` during an outcome can steer it. The agent already continues until a terminal result. At `requires_action` or `budget_reached`, that message is `400`.

After a terminal result the session is idle and can take another outcome or a plain message.

Use an outcome when the session produces one checkable thing: a file, a report, a pull request, a dataset. Use `user.message` when the session answers a series of questions, or when the work is a change in some other system and there is no artifact to grade.

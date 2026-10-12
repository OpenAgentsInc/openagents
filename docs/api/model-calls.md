# Model calls

2026-10-12. Proposed. Nothing on this page is a live route.

A model call is [`POST /v1/responses`](../inference/gateway.md). This page is the rest of that call: counting input tokens, running many calls as a batch, sending files, asking for citations, placing cache breakpoints, running function tools yourself, keeping reasoning valid across turns, reading a refusal, and dropping old tool results from the context the model sees. Streaming, function-tool items, `tool_choice`, hosted web search, reasoning items, compaction, stored responses, the model list, the rate card, usage, and the key's limits are already specified on the gateway. Chat Completions stays the translation layer in that spec.

It is a PUBLIC surface on `https://api.openagents.com/v1` (alias `https://openagents.com/api/v1`). It follows [the API design](design.md): one error object, cursor lists, `oak_` keys, sats, and no house rate limit on your own keys. A container, a shell, files on disk, memory across turns, skills, and MCP servers are a [session](managed-agents.md), not a field on this call. Where a name on this page also names a protocol record, [Names shared with a protocol](design.md#names-shared-with-a-protocol) says which object a caller sends.

## Conventions

| Topic | Rule |
| --- | --- |
| Auth | `Authorization: Bearer oak_<id>.<secret>`. The key's workspace owns every row. Another workspace's id is `404`. |
| Scope | `responses` creates a call, a count, and a batch, and reads them. |
| Idempotency | `Idempotency-Key` on `POST /v1/responses`, `POST /v1/responses/batches`, and `POST /v1/responses/batches/{id}/cancel`. The same key and body within 24 hours returns the first response. The same key and a different body is `409` `idempotency_conflict`. A count is not stored, so it takes no idempotency key. |
| Lists | `?limit=` (default 20, maximum 100) and `?after=<cursor>`. The body is `{"data": [...], "next": "<cursor>" or null}`. An expired cursor is `410` `cursor_expired`. |
| Errors | `{"error": {"type", "code", "message", "param", "request_id"}}`. `type` is the category in the [gateway error list](../inference/gateway.md). A `409` uses `type` `conflict`. `code` is the reason. Statuses are in [Errors](#errors). Every response carries `x-request-id` and `openagents-version`. |
| Money | A create and a batch draw the workspace balance. The response `openagents.cost` object names `price_usd` and `price_sats`. A count is free. A failed create that produced no output tokens is not charged. |
| Files | One workspace file object, `file_`, shared with [sessions](managed-agents.md#files). |
| Times | RFC 3339. |

OpenAgents does not impose a request-per-minute cap on your keys ([decision D16](2026-10-02-openagents-api.md)). A cap you set on the key returns `429` `too_many_requests` with `Retry-After`, or `403` `limit_reached` when the cap is a spend or model limit. `param` names that limit.

## Count input tokens

`POST /v1/responses/count` takes the same JSON body as `POST /v1/responses`. `max_output_tokens` and `stream` are ignored. The response is the input the model would see, after `openagents.context_edits` and after file text is extracted:

```json
{ "input_tokens": 1842 }
```

The count is for the `model` you send. A different model can count the same text differently. The call does not create a response, does not read or write the prompt cache, and does not reserve balance. A missing `file_id` is `404`.

```sh
curl -s https://api.openagents.com/v1/responses/count \
  -H "Authorization: Bearer $KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"openagents","input":"Summarize the notes."}'
```

## Batches

`POST /v1/responses/batches` runs many creates asynchronously. Each request's tokens are priced at half the [rate card](../inference/gateway.md) row for that model, including cached input and cache writes. The `openagents.cost` object on each result uses those halved prices.

| Limit | Value |
| --- | --- |
| Requests in one batch | 100,000 |
| Request body | 256 MB. Larger is `413` `request_too_large`. |
| How long it runs | `expires_at` is 24 hours after `created_at`. Most batches end within an hour. Requests still running at `expires_at` finish as `expired`. |
| How long results stay | 29 days after `created_at`. A later read is `404` `batch_not_found`. |

`stream: true` inside a request is `400` `stream_not_allowed`. A batch request may use the other fields on this page: files, citations, cache breakpoints, strict tools, reasoning, tool changes, fallbacks, and context edits. The [tool loop](#the-tool-loop) still runs in your process, between calls.

### Create

`custom_id` is required, unique in the batch, 1 to 64 characters, and may contain letters, digits, `_`, and `-`. A duplicate is `400` `duplicate_custom_id`. `params` is a create body.

```sh
curl -s https://api.openagents.com/v1/responses/batches \
  -H "Authorization: Bearer $KEY" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: 6f1a9c2e-4b70-4d18-9c33-1a2b3c4d5e6f" \
  -d '{
    "requests": [
      {
        "custom_id": "notes-1",
        "params": {
          "model": "openagents",
          "input": "Summarize the notes."
        }
      }
    ]
  }'
```

```json
{
  "id": "batch_01HZY2QK8M7N6P5R4S3T2U1V0W",
  "object": "response.batch",
  "status": "in_progress",
  "request_counts": {
    "processing": 1,
    "succeeded": 0,
    "errored": 0,
    "canceled": 0,
    "expired": 0
  },
  "created_at": "2026-10-12T18:00:00Z",
  "expires_at": "2026-10-13T18:00:00Z",
  "ended_at": null
}
```

`request_counts` always sums to the number of requests.

| Status | Meaning |
| --- | --- |
| `in_progress` | The batch is running. |
| `canceling` | Cancel was accepted. Running requests can still succeed. |
| `ended` | Every request succeeded, errored, or expired. |
| `canceled` | Cancel finished. Requests that had not started are `canceled`. Requests that already succeeded stay `succeeded`. |

### Read, list, results, cancel

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/v1/responses/batches` | Create a batch. |
| `GET` | `/v1/responses/batches` | List batches, newest first. Filter with `status`. |
| `GET` | `/v1/responses/batches/{id}` | One batch. |
| `GET` | `/v1/responses/batches/{id}/results` | One row per request, cursor list. Rows stay in `custom_id` order. |
| `POST` | `/v1/responses/batches/{id}/cancel` | Move `in_progress` to `canceling`. Canceling or `canceled` returns the batch again. `ended` is `409` `batch_ended`. |

A result row is one of four types. `succeeded` carries the response object, with a `resp_` id, usage, and `openagents.cost`. `errored` carries the same error object as a failed create. `canceled` and `expired` carry no response.

```json
{
  "custom_id": "notes-1",
  "result": {
    "type": "succeeded",
    "response": { "id": "resp_…", "status": "completed" }
  }
}
```

Poll `GET /v1/responses/batches/{id}` until `status` is `ended` or `canceled`, then read results. There is no webhook on this page. A session's webhooks are in [operations](managed-agents-operations.md).

## Files, images, and documents

Upload with the [Files API](managed-agents.md#files), then reference `file_id` from a response. Upload, list, metadata, download, and delete stay those routes. One workspace file. Chat attachments and session mounts use it too.

A file is at most 500 MB. Over that, upload returns `400` `file_too_large`. A request body the server will not read inline is `413` `request_too_large`: upload the file and send `file_id`. Upload, list, and delete are free. Tokens extracted from the file on a create or a count are input tokens.

| Part | How you send it |
| --- | --- |
| Image | `input_image` with `image_url` set to an `https` URL or a `data:` URL. Media types: `image/png`, `image/jpeg`, `image/gif`, `image/webp`. |
| Document or other file | `input_file` with `file_id`, or `file_data` (base64) and `filename` for a small file. Media types include `application/pdf`, `text/plain`, `text/markdown`, and `text/csv`. |
| Text | `input_text`. |

```json
{
  "type": "input_file",
  "file_id": "file_01HZY2QK8M7N6P5R4S3T2U1V0W",
  "filename": "q4.pdf"
}
```

The file must belong to the workspace. Deleting it does not change a stored response that already used it. A later call that references the deleted id is `404` `file_not_found`.

The model list already reports `images` on each provider. This page adds `files` beside it. `images: false` with an `input_image`, or `files: false` with an `input_file`, is `400` `media_unsupported`.

## Citations

Set `citations` on an `input_file` when you want the answer tied to spans in that file. Every `input_file` in the request enables citations, or none of them do. A mix is `400` `citations_mixed`. An `input_image` does not take the field.

```json
{
  "type": "input_file",
  "file_id": "file_01HZY2QK8M7N6P5R4S3T2U1V0W",
  "citations": { "enabled": true }
}
```

Cited spans come back as annotations on `output_text`. `start` and `end` are character offsets into the extracted text. `page` is present when the file has pages.

```json
{
  "type": "openagents:citation",
  "file_id": "file_01HZY2QK8M7N6P5R4S3T2U1V0W",
  "filename": "q4.pdf",
  "cited_text": "Revenue grew 12 percent.",
  "start": 140,
  "end": 164,
  "page": 2
}
```

A model with `citations: false` that receives `citations.enabled` is `400` `citations_unsupported`. The Chat Completions translation omits annotations, as the [gateway fidelity table](../inference/gateway.md) says.

## Cache breakpoints

The service may cache a stable prefix when the model's provider has `cache: true`. You do not have to mark anything. A model that cannot cache still answers, and `cached_tokens` is 0.

To choose the prefix, set `cache_control` on a content part, or set `openagents.cache` on the request. `openagents.cache` places one breakpoint on the last cacheable part. `ttl` is `5m` (default) or `1h`.

```json
{ "type": "ephemeral", "ttl": "1h" }
```

At most four `cache_control` markers in one request. A fifth is `400` `cache_breakpoint_limit`. A marker on a model with `cache: false` is `400` `cache_unsupported`. Omitting every marker never returns that error.

A byte change before a breakpoint misses the cache from that breakpoint on. Put stable instructions, tool definitions, and files ahead of the text that changes each turn. A `system` or `developer` message anywhere in `input` applies from that point forward. Leave the earlier items as you sent them so the prefix can still match.

Usage on the response reports the split the meter already stores:

| Field | What it counts |
| --- | --- |
| `usage.input_tokens` | Input tokens, including cached reads and cache writes. |
| `usage.input_tokens_details.cached_tokens` | Tokens read from the cache. |
| `usage.input_tokens_details.cache_write_tokens` | Tokens written to the cache. |
| `usage.output_tokens_details.reasoning_tokens` | Reasoning tokens. |

A session event names the same two cache counts `cache_read_input_tokens` and `cache_creation_input_tokens`. Prices for cached input and cache writes are on `GET /v1/rates`. A batch uses half of those rows.

## The tool loop

You run every function tool. The gateway runs the hosted `openagents:web_search` tool.

A `function_call` carries `call_id`, `name`, and `arguments`. `arguments` is a string of JSON. On a stream, the argument deltas for that item are partial JSON. Run the tool after the stream emits `response.output_item.done` for that item.

When `status` is `incomplete` and `incomplete_details.reason` is `max_output_tokens`, skip that turn's tools. When `openagents.stop.type` is `refusal`, skip them too. A cut-off call has no complete arguments.

Check `arguments` against the tool schema before you run the tool. On the next request, send the same `arguments` string you received.

When one response contains several `function_call` items, return one `function_call_output` for each `call_id` before the next model call. When the arguments fail the schema, send the failure as the output and set `error` to `true`, so the model can correct the call:

```json
{
  "type": "function_call_output",
  "call_id": "call_01",
  "output": "The date is not a calendar day.",
  "error": true
}
```

`error` is a field on `/v1/responses`. A Chat Completions caller writes the failure in the tool message content. The [gateway fidelity table](../inference/gateway.md) lists what that translation drops.

With `previous_response_id`, the new input follows the stored context. Send the outputs as new items. The stored items stay as the service saved them. The stream events are the ones the gateway already emits.

## Strict tools and JSON

A function tool may set `strict: true`. `parameters` then has `additionalProperties: false`, and `required` lists every key in `properties`. Anything else is `400` `strict_schema`. The model's `function_call` arguments match that schema. `strict: true` on a provider with `json_schema: false` is `400` `strict_unsupported`.

`text.format` of `json_schema` uses the same schema rules for the answer text. `json_object` asks for JSON without a schema. Both are already on the gateway. `tool_choice` stays `auto`, `required`, `none`, a named function, or `allowed_tools`.

## Reasoning and effort

### Effort

`reasoning.effort` is `low`, `medium`, `high`, `xhigh`, or `max`, the same levels as `model.effort` on an [agent](managed-agents.md#models). A level the provider does not list under `effort` is `400` `effort_unsupported`. When `effort` is absent from the provider, the field is ignored.

### Sending reasoning back

When the response includes a reasoning item, send `encrypted_content` back unchanged on the next call. Raw reasoning text is included only when you ask for it and the provider allows it, as the gateway specifies. The Chat Completions translation drops `encrypted_content`. Send reasoning back on `/v1/responses`.

### Keeping reasoning valid

`encrypted_content` stays valid for four things together: the `instructions` string, the tool definitions, every input item before that reasoning item, and the model id that produced it.

Editing any of the first three and sending the old `encrypted_content` returns `400` `reasoning_prefix_mismatch`. Changing `encrypted_content`, the item's `summary`, or its `content` returns `400` `reasoning_modified`.

Drop reasoning items from the start of the input, or from the end, as one contiguous span. Omitting an item from the middle returns `400` `reasoning_order`. After you change an earlier item, omit reasoning items from that point forward and continue.

### Changes that leave the prefix valid

The prefix check reads the conversation you send, before the service applies `openagents.context_edits` or opens a compaction item.

These changes leave a replayed `encrypted_content` valid:

- Reordering `tools` while each definition stays the same.
- Adding or removing `cache_control`.
- Changing `model`, `max_output_tokens`, `temperature`, `top_p`, `tool_choice`, `metadata`, `stream`, or `reasoning.effort`.
- Sending the original items while the service applies `openagents.context_edits`.
- Appending a new `developer` or `system` message.

These changes return `400` `reasoning_prefix_mismatch` when the old `encrypted_content` is still on the request:

- Rewriting an earlier message, `instructions`, a tool description, or a tool schema.
- Adding or removing an entry in `tools`.
- Rewriting an earlier `function_call` `arguments` string, or an earlier `function_call_output`.
- Rebuilding items so the JSON differs from the JSON you stored.

Put a fact that changes each turn in a new user or developer message, and send the same `instructions` string on every call. Replay each `input_image` and `input_file` as the same object. Store the request JSON you sent.

### Adding or removing a tool

Keep the `tools` array the same JSON the first call sent. To change which tools apply from one message on, append a developer message whose content part is `openagents:tool_change`:

```json
{
  "type": "message",
  "role": "developer",
  "content": [
    {
      "type": "openagents:tool_change",
      "change": "add",
      "tool": {
        "type": "function",
        "name": "lookup_order",
        "description": "Look up an order by id.",
        "parameters": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "order_id": { "type": "string" }
          },
          "required": ["order_id"]
        }
      }
    }
  ]
}
```

`change` is `add` or `remove`. A `remove` part sends `name` and omits `tool`. An `add` whose `name` is already in `tools` replaces that definition from this message forward. The earlier definition stays in `tools`. Changing the `tools` array on a request that replays reasoning returns `400` `reasoning_prefix_mismatch`. Rewriting an earlier `openagents:tool_change` part is the same error. A part with an unknown `change`, or an `add` that omits `tool`, is `400` `tool_change_invalid`.

### Switching models

A reasoning item is readable by the model id that produced it. Another id skips the item, and the call succeeds. The response lists each skipped item on `openagents`:

```json
{
  "reasoning_dropped": [
    { "id": "rs_01", "reason": "model" }
  ]
}
```

Keep the items in the history you store. The next call on the producing model id reads them again. A router id (`openagents`, `openagents/auto`, and the other `openagents/…` ids) reads an item when the selected underlying model id is the one that produced it. Otherwise the reason is `model`.

## Refusals

When the model declines, the response `status` is `incomplete` and `incomplete_details.reason` is `content_filter`. The gateway's `openagents` object also carries why:

```json
{
  "stop": {
    "type": "refusal",
    "category": "cyber",
    "explanation": "The request asks for an attack procedure."
  }
}
```

`category` is a string or `null`. The set is open. Categories you can receive today include `cyber`, `bio`, `reasoning_extraction`, and `frontier_llm`.

`openagents.fallbacks` is the list of model ids already on the create body. On a refusal the gateway tries the next id. `openagents.attempts` names each model that declined, and `openagents.model` is the model that answered. If every model declines, `status` stays `incomplete` and `stop` describes the last decline. With no `fallbacks`, the call stops on the first refusal.

A decline that produced no output tokens is not charged. A decline after output tokens is charged like any other incomplete response. Each attempt's cost is on that attempt.

## Context edits

`openagents.context_edits` changes what the model sees. A stored response (`store: true`) keeps the original items. `GET /v1/responses/{id}` returns those items.

| `type` | Effect |
| --- | --- |
| `clear_tool_uses` | `keep` (default 1) is how many recent `function_call_output` items stay in the model context. Older tool outputs are replaced, for the model only, with the text `(cleared)`. |
| `clear_reasoning` | Reasoning items other than the latest are left out of the model context. Send the latest item's `encrypted_content` back unchanged. |

```json
{
  "context_edits": [
    { "type": "clear_tool_uses", "keep": 3 },
    { "type": "clear_reasoning" }
  ]
}
```

A provider with `context_edits: false` that receives the field is `400` `context_edits_unsupported`. Omitting the field leaves the context as you sent it.

To summarize a long conversation into one compaction item, call [`POST /v1/responses/compact`](../inference/gateway.md) and send that item back as input. That summary pass is metered like any create.

The next input starts with the compaction item, followed by the items the summary does not cover. Sending an item the summary already covers returns `400` `compaction_overlap`. On the request that first includes the compaction item, leave `openagents.context_edits` unset. Setting it on that request returns `400` `compaction_context_edits`. A later turn may set `context_edits`.

When developer instructions were inside the summarized span, append those instructions again after the first new user message. Rewriting an earlier `function_call_output` yourself, while its reasoning item is still on the request, returns `400` `reasoning_prefix_mismatch`. `clear_tool_uses` hides that output from the model and leaves the item you stored as you sent it.

## Model capabilities

`GET /v1/models` is the live catalog. Each provider under `openagents.providers` already has `context`, `max_output`, and boolean `capabilities` for `tools`, `json_schema`, `images`, and `reasoning`. This page adds the booleans `files`, `citations`, `cache`, and `context_edits`, and `effort` as the list of levels that provider accepts. Absent `effort` means the request's `reasoning.effort` is ignored.

`GET /v1/models/{id}` returns that one list entry. An id the key cannot call is `404` `model_not_found`.

Check the provider you will actually hit before you send a file, a citation, a cache marker, strict tools, an effort level, or a context edit. A router id (`openagents`, `openagents/auto`, and the other `openagents/…` ids) accepts the field when the route it selects accepts it. A field the selected provider rejects is the `400` from the sections above, and `openagents.attempts` names the provider.

## Usage and keys

These reads are already on the gateway. They are the caller's view of spend and limits.

| Method | Path | What you get |
| --- | --- | --- |
| `GET` | `/v1/key` | The calling key's balance, spend, and the limits its owner set. |
| `GET` | `/v1/usage/{request_id}` | Tokens, cost, upstream, attempts, and timings for one finished request. |
| `GET` | `/v1/rates` | The rate card, one row per model and upstream. |
| `GET` | `/v1/models` | Capabilities, context, price, and live latency. |

Workspace usage pages are in [the API design](design.md). A batch result's response id is the `request_id` you pass to `GET /v1/usage/{request_id}`.

## Errors

| Status | `type` | `code` |
| --- | --- | --- |
| `400` | `invalid_request` | `cache_unsupported`, `cache_breakpoint_limit`, `citations_mixed`, `citations_unsupported`, `compaction_context_edits`, `compaction_overlap`, `context_edits_unsupported`, `duplicate_custom_id`, `effort_unsupported`, `file_too_large`, `media_unsupported`, `reasoning_modified`, `reasoning_order`, `reasoning_prefix_mismatch`, `stream_not_allowed`, `strict_schema`, `strict_unsupported`, `tool_change_invalid` |
| `401` | `unauthorized` | `unauthorized` |
| `402` | `insufficient_balance` | `insufficient_balance` |
| `403` | `limit_reached` | `limit_reached` |
| `404` | `not_found` | `model_not_found`, `file_not_found`, `batch_not_found` |
| `409` | `conflict` | `idempotency_conflict`, `batch_ended` |
| `410` | `invalid_request` | `cursor_expired` |
| `413` | `invalid_request` | `request_too_large` |
| `429` | `too_many_requests` | `too_many_requests` |
| `500` | `server_error` or `model_error` | the same |
| `502` | `upstream_failed` | `upstream_failed` |
| `503` | `no_route` | `no_route` |

An item inside a batch uses this object and does not change the batch's HTTP status. The batch stays `in_progress` until every item has a result.

## What is not built

These routes are not mounted. `POST /v1/responses`, Chat Completions, compaction, stored responses, web search, the model list, the rate card, usage, and `GET /v1/key` are. Files, as a workspace object, are proposed with [sessions](managed-agents.md). A shell, on-disk files, memory, skills, and MCP servers run in a session. `error` on a tool output, `openagents:tool_change`, the reasoning-prefix checks, `openagents.reasoning_dropped`, and the two compaction checks on the next request are proposed on the live create route. They are not enforced yet.

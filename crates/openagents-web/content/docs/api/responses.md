# Open Responses

`POST /v1/responses` is our main API. It follows the
[Open Responses specification](https://www.openresponses.org/specification)
(version 2026-04-24), the open form of OpenAI's Responses API.

**Beta.**

## Request

```json
{
  "model": "openagents/chat",
  "instructions": "Answer in one sentence.",
  "input": [
    {"type": "message", "role": "user", "content": "What is Lightning?"}
  ],
  "stream": false,
  "max_output_tokens": 500
}
```

| Field | What it does |
| --- | --- |
| `model` | A model name or a task name ([Models](/docs/api/models)) |
| `input` | A string, or a list of items: messages, function calls, and their outputs |
| `instructions` | The system prompt |
| `tools`, `tool_choice` | Function tools; `tool_choice` is `auto`, `required`, `none`, one named function, or `allowed_tools` |
| `text.format` | `text`, `json_object`, or `json_schema` for structured output |
| `reasoning.effort` | How hard a reasoning model thinks |
| `temperature`, `top_p` | Sampling, where the model supports it |
| `max_output_tokens` | The longest answer |
| `stream` | `true` for server-sent events |
| `openagents` | Our options: [routing](/docs/api/routing), privacy, and who pays |

The API keeps nothing between requests. `store: true` and
`previous_response_id` answer `400`; send the whole conversation each time.

## Response

```json
{
  "id": "resp_...",
  "object": "response",
  "status": "completed",
  "model": "google/gemini-3.8-flash",
  "output": [
    {"type": "message", "role": "assistant",
     "content": [{"type": "output_text", "text": "Lightning is ..."}]}
  ],
  "usage": {"input_tokens": 24, "output_tokens": 18, "total_tokens": 42},
  "openagents": {
    "model": "google/gemini-3.8-flash",
    "upstream": "vertex",
    "attempts": [{"upstream": "vertex", "outcome": "ok", "ms": 840}],
    "cost": {"upstream_usd": "0.0000855", "margin_usd": "0.0000043",
             "price_usd": "0.0000898", "price_sats": 0}
  }
}
```

`openagents` says which model and provider answered, every provider tried,
and what the request cost: the provider's price, our margin, and the sum,
in dollars and sats. Amounts are decimal strings, so no rounding sneaks in.

Every answer also carries these headers:

```text
x-request-id
x-openagents-model
x-openagents-upstream
x-openagents-cost-usd   (answers without a stream)
```

## Output items

| Item | Holds |
| --- | --- |
| `message` | The answer: `output_text` and `refusal` parts |
| `function_call` | A call to one of your tools: `name`, `call_id`, `arguments` |
| `reasoning` | A reasoning model's summary, when the model gives one |

To answer a tool call, send the conversation back with a
`function_call_output` item carrying the same `call_id`.

## Streaming

With `stream: true` the answer arrives as server-sent events, in this
order, and ends with `data: [DONE]`:

```text
response.created
response.in_progress
response.output_item.added
response.content_part.added
response.output_text.delta        (many)
response.output_text.done
response.content_part.done
response.output_item.done
openagents:cost
response.completed
```

Two events are ours: `openagents:route` (which model and provider took the
request, before the first output) and `openagents:cost` (the cost, before
the last event). Clients that don't know them can skip them, as the
specification asks. If something breaks after the answer has started, the
stream ends with `response.failed`.

Errors are on their own page: [Errors](/docs/api/errors).

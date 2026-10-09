# Open Responses acceptance and OpenAI SDK runs (2026-10-09, #11068)

Against a local gateway (`scripts/dev/inference-local.sh --gateway-only`,
port 8795) built from `main` plus the fixes below, with a service key.
Upstream keys came from `~/work/.secrets`; this run had OpenRouter and the
Vercel AI Gateway configured (no Vertex, Z.ai, or Pro door keys), and the
router sent every attempt through OpenRouter. The whole run cost about
$0.02 of OpenRouter credit. Nothing here touched staging or production.

## How to run it again

```sh
scripts/dev/inference-local.sh --gateway-only          # or a staging URL
OPENAGENTS_API_KEY=oak_... scripts/dev/openresponses-acceptance.sh \
    http://127.0.0.1:8790/v1 google/gemini-3.8-flash [--json out.json]
OPENAGENTS_API_KEY=oak_... scripts/dev/openai-sdk-conformance.py \
    --base-url http://127.0.0.1:8790/v1 --model google/gemini-3.8-flash
# JavaScript: see the header of scripts/dev/openai-sdk-conformance.mjs
```

The acceptance suite is the upstream runner (`bin/compliance-test.ts` in
[openresponses/openresponses](https://github.com/openresponses/openresponses),
Apache-2.0), cloned at commit `1e33c10fb3c1` (2026-10-08) and run with bun.
It is driven as an external runner; nothing of it is vendored.

## Open Responses acceptance suite: 17 of 17

| Test | `google/gemini-3.8-flash` | `zai/glm-5.3-flash` |
| --- | --- | --- |
| Basic Text Response | pass | pass |
| Assistant Message Phase | pass | pass |
| Response Output Phase Schema | pass | pass |
| Streaming Response | pass | pass |
| System Prompt | pass | pass |
| Tool Calling | pass | pass |
| Image Input | pass | `503 no_route` (GLM takes no images: correct) |
| Multi-turn Conversation | pass | pass |
| Compaction Endpoint | pass | pass |
| Compaction Missing Required Model | pass | pass |
| WebSocket Response | pass | pass |
| WebSocket Sequential Responses | pass | pass |
| WebSocket Continuation | pass | pass |
| WebSocket Store False Reconnect Recovery | pass | pass |
| WebSocket Missing Previous Response | pass | pass |
| WebSocket Failed Continuation Evicts Cache | pass | pass |
| WebSocket Compact New Chain | pass | pass |

The first run failed 9 of 17 on Gemini. Fixes, all in this change:

- **Our stream events.** `openagents:route` and `openagents:cost` failed
  the suite's event validation on every streaming and WebSocket test. The
  spec allows prefixed events and says clients must ignore ones they don't
  know, but the suite (and strict clients built like it) refuse unknown
  event types. They are now sent only when the caller asks
  (`x-openagents-events: route,cost`), with sequence numbers kept
  consecutive; the same facts ride in every answer's `openagents` object
  and headers. The chat worker asks for them.
- **Function tools in the response.** `ResponseResource.tools[]` requires
  `description`, `parameters`, and `strict` (nullable). A response now
  fills missing ones with `null`; the request still goes upstream as the
  caller wrote it.
- **Reasoning settings in the response.** The Vercel GLM lane answers
  `reasoning.effort: "max"`, which the spec does not name. Every response
  we send now carries the caller's own `reasoning` settings.

## OpenAI SDK runs

Python, `openai` 2.44.0, `scripts/dev/openai-sdk-conformance.py`, model
`google/gemini-3.8-flash`: 15 of 15, one call per row of section 3's Chat
Completions table.

| Row | Result |
| --- | --- |
| `system` message, `user` text, `temperature`, `top_p`, `seed`, `user`, `max_tokens`, `usage`, `finish_reason: stop` | pass |
| `developer` message, `max_completion_tokens`, `stop` | pass |
| `stream` with `stream_options.include_usage` (deltas, finish, usage chunk) | pass |
| `tools`, `tool_choice: required`, `parallel_tool_calls`, `finish_reason: tool_calls` | pass |
| Named `tool_choice`, streamed tool call deltas | pass |
| `assistant` `tool_calls` and `tool` messages in history | pass |
| Multi-turn assistant history | pass |
| `response_format: json_object` | pass |
| `response_format: json_schema` (strict) | pass |
| `user` `image_url` part | pass |
| `reasoning_effort` | pass |
| `max_tokens` reached: `finish_reason: length` | pass |
| `n > 1` is `400` | pass |
| Unknown model is `404 model_not_found` | pass |
| `client.models.list()` | pass |

JavaScript, `openai` 7.31.0, `scripts/dev/openai-sdk-conformance.mjs`:
7 of 7 (`chat.completions.create` plain, streamed with usage, streamed tool
calls; `responses.create` plain, streamed, function tool; `models.list`).

Found on the way: Gemini 3.8 Flash through the router refuses
`reasoning_effort: "none"` with a `400` (it can't turn thinking off), which
reaches the caller as `400 invalid_request`; without a reasoning setting,
a 16-token answer can be spent entirely on thinking and come back empty
with `finish_reason: length`.

Still to do: run both against staging before release and against
production once deployed (the issue's standing requirement), and with a
public key through `https://openagents.com/api/v1` once the website
passes `/api/v1` to the gateway there.

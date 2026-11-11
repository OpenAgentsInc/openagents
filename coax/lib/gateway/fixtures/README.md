# Recorded gateway streams

Real streams (but two), byte for byte, carried over from
`crates/coder/fixtures/gateway/` at `0b401561a0^`. `gen.py` turns them into
`../fixtures.coax`, which the tests in `../gateway.coax`, `../chat.coax` and
the gateways replay.

| File | Wire | Where it came from |
| --- | --- | --- |
| `google-gemini-3.8-flash.sse` | Open Responses | Vercel AI Gateway `/v1/responses`, `google/gemini-3.8-flash`, 2026-09-19 |
| `zai-glm-5.3-flash.sse` | Open Responses | Vercel AI Gateway `/v1/responses`, `zai/glm-5.3-flash`, 2026-09-19 (34 reasoning deltas before the answer) |
| `openrouter-gemini-3.8-flash.sse` | Open Responses | OpenRouter `/v1/responses`, `google/gemini-3.8-flash` (no `event:` lines; the type is in the data) |
| `vertex-gemini-3.8-flash.sse` | Gemini native | Vertex AI `streamGenerateContent?alt=sse`, `gemini-3.8-flash`, 2026-10-10 |
| `vertex-openai-gemini-3.8-flash.sse` | Chat Completions | Vertex AI's OpenAI endpoint (`endpoints/openapi/chat/completions`), `google/gemini-3.8-flash`, 2026-10-30 |
| `openrouter-chat-gemini-3.8-flash.sse` | Chat Completions | OpenRouter `/api/v1/chat/completions`, `google/gemini-3.8-flash`, streamed with usage, 2026-10-30 |
| `openrouter-chat-gemini-3.8-flash.json` | Chat Completions | the same request unstreamed: one `chat.completion`, after OpenRouter's keep-alive whitespace |
| `vertex-gemini-4-carbon.sse` | Gemini native | Vertex AI `streamGenerateContent?alt=sse`, `gemini-4-carbon`, thinking level low, 2026-11-11 |
| `vertex-gemini-4-carbon-high.sse` | Gemini native | the same at thinking level high: 256 reasoning tokens counted, no thought text streamed (Carbon sends only a signature), 2026-11-11 |
| `vertex-openai-gemini-4-carbon.sse` | Chat Completions | Vertex AI's OpenAI endpoint, `google/gemini-4-carbon`, `reasoning_effort` low, 2026-11-11 |
| `vertex-gemini-4-argon-action.sse` | Gemini native | Vertex AI (location `us`), `gemini-4-argon`, thinking level low: Microcoder's first next_action for a one-file task, recorded through Coder's microcoder engine with `GW_RECORD`, 2026-11-11 (#13231) |
| `vertex-gemini-3.8-flash-action.sse` | Gemini native | the same with `gemini-3.8-flash`: 28 reasoning tokens counted, no thought text |
| `vertex-gemini-4-carbon-action.sse` | Gemini native | the same with `gemini-4-carbon` |
| `anthropic-text.sse` | Anthropic Messages | hand-written from the documented stream, not a recording |
| `anthropic-tool.sse` | Anthropic Messages | hand-written from the documented stream, not a recording |

Two are not recordings: no Anthropic key exists here, so
`anthropic-text.sse` (thinking, then text, with cache usage) and
`anthropic-tool.sse` (a forced tool's input_json_delta) are written by hand
in the event shapes Anthropic's streaming docs give. Replace them with
recordings once a key is in `~/work/.secrets/`.

The Carbon recordings' request bodies are the gateway's own translation (`gw_translate` for `gw_vertex` and `gw_vertex_native`), sent with curl. No Gemini Developer API key exists here, so Carbon has no `google-`/Gemini API recording yet.

Each recording answered "Count from one to five, one word per line." under a terse
system instruction. None carries a credential. A re-recording is a new
measurement: replace the file, run `gen.py`, and say in the commit what
changed.

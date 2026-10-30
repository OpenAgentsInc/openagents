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
| `anthropic-text.sse` | Anthropic Messages | hand-written from the documented stream, not a recording |
| `anthropic-tool.sse` | Anthropic Messages | hand-written from the documented stream, not a recording |

Two are not recordings: no Anthropic key exists here, so
`anthropic-text.sse` (thinking, then text, with cache usage) and
`anthropic-tool.sse` (a forced tool's input_json_delta) are written by hand
in the event shapes Anthropic's streaming docs give. Replace them with
recordings once a key is in `~/work/.secrets/`.

Each recording answered "Count from one to five, one word per line." under a terse
system instruction. None carries a credential. A re-recording is a new
measurement: replace the file, run `gen.py`, and say in the commit what
changed.

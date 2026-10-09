# Upstream streams

One stream per adapter and model, as the upstream sent it, for
`tests/upstreams.rs`. Each answered the same request as the gateway
streams in `crates/coder/fixtures/gateway/`: `Count from one to five, one
word per line.` under `You are terse. Answer with the words asked for and
nothing else.`

| File | Adapter | Model | Source |
| --- | --- | --- | --- |
| `vertex-gemini-3.8-flash.sse` | `vertex` | `gemini-3.8-flash` | Recorded 2026-10-09 from `streamGenerateContent?alt=sse` (project `openagentsgemini`, location `global`), thoughts included |
| `vertex-gemini-3.8-flash-tools.sse` | `vertex` | `gemini-3.8-flash` | Recorded 2026-10-09: `What is the weather in Paris? Use the tool.` with one `get_weather` declaration and `thinkingLevel: low`; a `functionCall` with its `thoughtSignature` |
| `pro-gpt-5.6-luna.sse` | `pro` | `openai/gpt-5.6-luna` | Recorded 2026-10-09 from the Pro door's proxy with `reasoning_effort: low` and `stream_options.include_usage` |
| `zai-glm-5.3-flash.sse` | `zai` | `glm-5.3-flash` | Constructed, not recorded: no Z.ai key is configured yet. Z.ai's documented stream shape (`reasoning_content` deltas, then `content`, usage on the finishing chunk), with the token counts of the Vercel-recorded GLM stream |

The OpenRouter and Vercel adapters run on the recorded streams in
`crates/coder/fixtures/gateway/`, which are already this shape.

Bytes are as the upstream sent them. No header or field carries a key.

## Re-recording

Send the request above to the upstream with its key, write the body
unchanged, and check that nothing in it carries the key. Replace the
constructed Z.ai stream with a recording once `zai-api-key` exists. A
re-recording is a new measurement: say in the commit what changed.

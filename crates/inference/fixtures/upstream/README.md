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
| `vertex-gemini-4-argon.sse` | `vertex` | `gemini-4-argon` | Recorded 2026-10-13 from `streamGenerateContent?alt=sse` (project `openagentsgemini`, location `global`; `us-central1` answers 404), the count request with `includeThoughts` |
| `vertex-gemini-4-argon-tools.sse` | `vertex` | `gemini-4-argon` | Recorded 2026-10-13: the `get_weather` request above with `thinkingLevel: low`; a `functionCall` with its `thoughtSignature` |
| `vertex-gemini-4-argon-thoughts.sse` | `vertex` | `gemini-4-argon` | Recorded 2026-10-14: `Is 391 prime? Think it through briefly, then answer in one line.` (the harness gate's thinking prompt), no system instruction, `includeThoughts` with `thinkingLevel: high`; a `thought: true` summary, then the answer (#11577: at Argon's own default level it sends no thought part) |
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

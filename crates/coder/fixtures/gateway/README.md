# Recorded gateway streams

Two real `POST /v1/responses` streams from `https://ai-gateway.vercel.sh`,
one per lane, recorded on 2026-09-19 with the same request:

| File | Model | Lane |
| --- | --- | --- |
| `google-gemini-3.8-flash.sse` | `google/gemini-3.8-flash` | `gemini` |
| `zai-glm-5.3-flash.sse` | `zai/glm-5.3-flash` | `glm` |

Both answered `Count from one to five, one word per line.` under the
instructions `You are terse. Answer with the words asked for and nothing
else.`, with `stream: true`, `store: false`, `tools: []`, and
`tool_choice: "none"` — the body `ResponsesDoor::body` sends.

They are byte-for-byte what the gateway sent, headers and blank lines
included. Nothing has been trimmed, because the point of a recorded
fixture is that the reader meets the wire rather than a tidied version of
it.

## What they are for

The gateway serves one event shape for every model in its catalog, which
is what makes a second model a configuration change rather than a second
client. `Reader` in `crates/coder/src/generate.rs` is the one place that
claim is relied on, and `crates/coder/tests/gateway_stream.rs` checks it
against both files: without them, a change in the gateway's event shape
reaches a person as a broken turn rather than a failing test.

The two lanes differ where it matters. The `glm` stream carries 34
`response.reasoning.delta` events that are not the answer, and the test
asserts none of them reaches the answer text.

## Re-recording

Send the request above to `/v1/responses` with `CODER_AI_GATEWAY_KEY` as
the bearer, write the response body to the matching file unchanged, and
check that no header or field carries the key. A re-recording is a new
measurement: say in the commit message what the gateway changed.

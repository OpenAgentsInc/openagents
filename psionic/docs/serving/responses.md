# Responses-native original GPT-OSS serving

`psionic/serve-gptoss.sh` serves `POST /v1/responses` from our resident original
20b CUDA engine. Coax owns the wire, authentication, JSON admission, tool schema
checks, Harmony recipe, reasoning state, item lifecycle and SSE serialization.
The C shell performs sockets, clocks, threads and device effects. There is one
active request and no automatic CPU fallback. This is an early usable edge;
continuous batching, storage and production ingress remain later chapters.

The public contract is
[`psionic-gptoss-responses-v1`](../../../proto/openagents/psionic/responses/v1alpha1/README.md).
The new alpha protobuf projection leaves preserved inference identities alone.
The [book chapter](../../book/ch048b-serve-responses.md) explains the code and
[verification record](../../../docs/verification/psionic-responses-2026-10-29.md)
records the actual qualification.

## Run it

Use our Coax compiler with `coax cuda` support, a CUDA driver/NVRTC installation,
the admitted original 20b cache and the pinned Harmony table. The model/profile
is the same as chapters 45a–45c: original MXFP4/BF16 bytes, 2048 context positions,
f32 activations with ordered wide reductions, greedy generation. Weights load
once before the listener becomes ready. Startup authentication can take about a
minute on the qualified worker. Do not restart the process for each request.

```shell
export PSIONIC_API_KEY="$(openssl rand -hex 24)"
psionic/serve-gptoss.sh "$HOME/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee" 8080
```

The process binds **127.0.0.1 only**. Reach a separate machine with an SSH tunnel;
public TLS, proxy authorization, rate limits and deployment are separate work.
`GET /healthz` and `/readyz` report process/model readiness. Authenticated
`GET /v1/models` lists `gpt-oss-20b`. The API key needs 16–128 visible ASCII bytes;
never commit it or place it in a URL. Request contents and reasoning are not logged.

```shell
curl --no-buffer http://127.0.0.1:8080/v1/responses \
  -H "Authorization: Bearer $PSIONIC_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-oss-20b","input":"Say hello.","max_output_tokens":128,"stream":true}'
```

The OpenAI SDK works with `base_url="http://127.0.0.1:8080/v1"` and your own
key. Use `client.responses.create(...)` or `client.responses.stream(...)`.
There is no dependency on that SDK, Python, an OpenAI account or OpenAI services
in the server. The SDK is a client compatibility check only.

## Supported profile

| Boundary | Implemented behavior |
| --- | --- |
| Generation | Original 20b only; low/medium/high reasoning; temperature 0; output limit 1–512, default 256; prompt plus reserved decode positions must fit 2048. No silent truncation. |
| Input | A string or up to 48 typed text/message/reasoning/function items; 65536-byte JSON body; each text body at most 16384 bytes. Ordinary text cannot insert Harmony controls. |
| Tools | Up to 8 named functions, flat object schemas with at most 16 string/number/literal-integer/boolean fields; string enums up to 16 alternatives. `auto` or `none`; one pending call; matched outputs; no execution in this process. |
| Reasoning | Typed raw `reasoning_text` content, separate from final text. Preserve current tool-turn analysis; remove completed prior-turn analysis. No invented summaries. |
| Streaming | Incremental semantic SSE with stable item/call IDs, ordered sequence numbers, typed parts/deltas/done events and one terminal response. Hold incomplete UTF-8 until it is valid. |
| Lifecycle | One active worker, 8 connections; busy returns 503; ingress/drain 5 s; execution 120 s checked between tokens and prefill chunks; disconnect releases the worker. A device failure makes readiness fail. |
| History | `store:false`, full stateless replay. Non-null `previous_response_id` refuses rather than pretending a stored response exists. |
| Errors | Explicit 4xx for unsupported/invalid requests before device mutation. Arithmetic/framing/device failure is an error, never a successful answer. A stream already started ends with `response.failed` when transport remains usable. |

Default reasoning event names match the upstream GPT-OSS reference:
`response.reasoning_text.delta` / `.done`. Send
`X-Psionic-Responses-Profile: openresponses-2026-04-24` for the baseline's
`response.reasoning.delta` / `.done` names. Raw reasoning input arrays are a
stated GPT-OSS extension to the baseline's null-only input content field.
This is a bounded compatibility profile, not full Open Responses compliance.

Strict/schema-constrained decoding, parallel tools, built-in tools, nonzero
sampling, summaries, encrypted reasoning, images/audio/video, compaction,
background jobs and stored-response endpoints are unsupported. Tool arguments
are checked against the admitted schema at a complete handoff; generation is
not masked to guarantee that schema. Invalid arguments fail the response. Clients
must execute only a completed authorized function call, never an argument delta
or an incomplete item. A successful API handoff does not authorize any payment,
network action or other application effect.

Replay the complete output item array, then a `function_call_output` whose
`call_id` matches the completed function call. SDK output items can be converted
with `model_dump()`; raw analysis stays API state. For a completed final answer,
display the assistant message's `output_text`; do not concatenate reasoning into
it. A next user turn can replay prior output, but prior completed analysis is
removed before rendering the new model prompt.

## Checks

```shell
psionic/tests/check_responses.sh
RESPONSES_PYTHON=/path/to/sdk-test-env/bin/python psionic/tests/check_responses.sh
python3 psionic/tests/book.py
```

The default harness uses owned continuations, real pinned tokenization, actual
sockets and the same Coax wire code in debug/release with plain, ASan and UBSan
native builds. Its fixture generator is absent from the production import root.
No full mutation runs or original CPU inference are involved. SDK/schema checks
are opt-in verification tooling:

```shell
/path/to/sdk-test-env/bin/python psionic/tests/responses_api.py \
  --binary /path/to/responses-fixture \
  --table psionic/fixtures/harmony/gptoss.hst --sdk \
  --schema /path/to/openresponses-2026-04-24.json
```

For an already running real GPU server, use `--url http://127.0.0.1:8080`
instead of `--binary`/`--table`; supply its key through `PSIONIC_API_KEY`. Add `--sdk --tools` to qualify a real
model function handoff and full typed replay, using only an owned weather fixture
as the externally supplied result.
Run only against a test instance: the checks deliberately exercise limits and
failures. Do not treat a fixture-backed passing result as model qualification.

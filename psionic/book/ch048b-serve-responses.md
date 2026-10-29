# Chapter 48b: Serve Responses

A useful model service must retain more than its final string. GPT-OSS emits
analysis, assistant text and function handoffs through Harmony. A Responses
endpoint can transport those as distinct typed items, continue a function call
with its reasoning intact, and expose progress without mistaking it for a
completed answer. We now put that interface around our resident GPU worker.

Read [Harmony](ch044-the-harmony-format.md), the
[resident decoder](ch045b-a-complete-cuda-decoder.md),
[batched prefill](ch045c-prefill-together.md) and
[tool grants](ch048-call-a-tool.md) first. This chapter's HTTP tool profile
checks flat schemas after generation; chapter 48's finite masked-decoding
profile remains separate. There is no hidden CPU fallback.

## Choose the contract before writing the listener

OpenAI's [GPT-OSS repository](https://github.com/openai/gpt-oss/tree/5681a6b6552635e7b29cde94a61ca6a7aee90ebd/gpt_oss/responses_api)
contains a partial Responses server. Its backend-independent token callback,
typed function items and raw reasoning replay fit our own engine. We read that
code as a reference; we run our own implementation. The
[Open Responses specification](https://www.openresponses.org/specification)
and [schema](https://www.openresponses.org/openapi/openapi.json) supply an
independent external shape. Their reasoning input/event details differ from the
GPT-OSS reference, so we name the compatibility profile explicitly instead of
claiming universal support.

Our [versioned profile](../../proto/openagents/psionic/responses/v1alpha1/README.md)
starts in `proto/`. It admits text, raw reasoning replay, sequential function
calls and semantic SSE. Storage, strict structured decoding, multimodal input,
parallel tools and nonzero sampling return explicit errors. The engine reserves
at most 2048 positions and generates at most 512 tokens. A request that cannot
fit is refused before its first model effect.

## Refuse ambiguous bytes first

[http.coax](../lib/responses/http.coax) checks HTTP framing, bearer authorization,
content length and supported routes. Duplicate lengths and transfer encodings
refuse; request bodies are bounded to 65536 bytes. The C shell does not decide
what an HTTP header means. It reads bytes and asks Coax whether a request is
complete, needs more data or must be refused.

[json.coax](../lib/responses/json.coax) builds a bounded tree using the shared
JSON parser. Every string/key is unescaped, and escaped duplicate keys refuse.
There is no approximate numeric admission: a tiny nonzero temperature does not
underflow into permission for an unsupported sampler.

[request.coax](../lib/responses/request.coax) then validates the Responses
profile, including declared tools and their schemas. It matches every tool
output to a unique pending `call_id`. A result with an unknown ID cannot become
a tool message in the model prompt.

## Preserve the right reasoning

The guidance in [Handle raw chain of thought](https://developers.openai.com/cookbook/articles/gpt-oss/handle-raw-cot)
requires retaining analysis across a current tool sequence and discarding it
from completed earlier turns. Our input mapping implements that rule:

```jai
completed prior analysis  -> remove from the next model prompt
current tool analysis    -> assistant analysis message
function_call            -> assistant commentary to=functions.NAME, handoff
function_call_output     -> functions.NAME to=assistant, commentary
final answer             -> assistant final, return
```

The caller replays the output items and appends the matched function result.
There is no in-memory response-ID store to accidentally lose on restart.
Non-null `previous_response_id` returns `stateless_replay_required`. Reasoning
is API state, separately typed from anything a UI displays. The existing
complete-final display gate still applies to the local CLI.

## Stream typed progress

[output.coax](../lib/responses/output.coax) inspects only the bounded generated
continuation. An unfinished header emits no speculative item. A completed header
admits a provisional typed body; incomplete UTF-8 remains held until a later
token finishes the scalar. The real model can emit `<|constrain|>json` on a
function header. A narrow Responses adapter accepts that label, trims its
framing whitespace and hands the remaining header to the existing strict
Harmony parser; other format labels refuse. A synthetic parser closing control lets us inspect
a partial body, but cannot become a claim that the model completed it.

[wire.coax](../lib/responses/wire.coax) assigns stable response, message,
reasoning and call IDs from host-provided random bytes. It emits creation and
progress, item/part addition, body deltas, item closure and a terminal response.
State advances only when the complete event batch fits in its private buffer.
A disconnect cancels the request instead of replaying a partially sent batch.

```jai
response.created
response.in_progress
response.output_item.added       // reasoning stays a reasoning item
response.content_part.added
response.reasoning_text.delta
response.reasoning_text.done
response.content_part.done
response.output_item.done
response.output_item.added       // final text or a function call
// typed deltas and closures
response.completed              // or response.incomplete / response.failed
```

The default reasoning event names follow the GPT-OSS reference. The explicit
Open Responses profile header selects its baseline names. A completed function
call still ends this API response: the application executes a permitted tool
and starts the next request with its result. This process executes no tool.

Output usage counts actual generated token IDs, including framing, with
analysis body tokens separately counted. A budget stop returns `incomplete`
with `max_output_tokens`, even if no complete header was generated. A malformed
model output or invalid function argument is an error rather than fabricated
success. Argument deltas and incomplete calls grant nothing.

## Keep effects small and state resident

[responses-server.c](../platform/responses-server.c) maps admitted artifacts,
opens one resident GPU session and exposes a loopback listener. Connections
have bounded storage. Coax admits at most eight; only one can hold the worker.
A concurrent model request receives 503 instead of sharing a mutable KV cache.
Each admitted request resets model state while retaining the uploaded weights.

The shell observes disconnects and monotonic time. Coax decides the 120-second
execution deadline and cancellation. `pg_prefill_poll` checks that decision
between committed prompt chunks, and decode checks between tokens. Previously
committed chunks remain valid; a cancelled request releases the worker, and the
next request resets it. A device effect failure makes readiness fail.

Ingress and connection drain have a five-second deadline. Before closing a
refused request, the host sends FIN and drains remaining bytes so an unread
body cannot erase its error response with a TCP reset. No request content or
reasoning is logged. The local server requires an API key and binds loopback;
production TLS, tenant admission, continuous batching and stored responses are
later work.

## Run and check

Use the [serving guide](../docs/serving/responses.md) for prerequisites and full
client examples. Build our compiler with CUDA emission support, then:

```shell
export PSIONIC_API_KEY="$(openssl rand -hex 24)"
psionic/serve-gptoss.sh "$HOME/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee" 8080
```

In another terminal, supply that same key and call the endpoint:

```shell
curl --no-buffer http://127.0.0.1:8080/v1/responses \
  -H "Authorization: Bearer $PSIONIC_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-oss-20b","input":"Say hello.","max_output_tokens":128,"stream":true}'
```

The owned fixture server exercises real HTTP framing, the original tokenizer,
SDK clients and Coax Responses serialization without loading a full model.
Its generator is imported only by the test root. Production has no fixture
switch or CPU fallback. Run changed-code checks:

```shell
psionic/tests/check_responses.sh
python3 psionic/tests/book.py
```

The harness runs debug/release plain, ASan and UBSan variants and checks typed
tool replay, incremental SSE reconstruction, errors, busy admission and client
disconnect. The optional SDK check also exercises `responses.stream()`'s final
response accumulator. The [dated record](../../docs/verification/psionic-responses-2026-10-29.md)
separates those owned checks from real original-model evidence and states the
remaining compatibility and throughput limits.

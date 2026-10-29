# Psionic Responses-native serving — 2026-10-29

Scope: `psionic-gptoss-responses-v1`, a new alpha contract, loopback HTTP over
our resident original gpt-oss-20b CUDA engine. See
[the profile](../../proto/openagents/psionic/responses/v1alpha1/README.md),
[the chapter](../../psionic/book/ch048b-serve-responses.md) and
[operating guide](../../psionic/docs/serving/responses.md).

## Verification

- **266 Coax checks** (including imported JSON/Harmony suites) pass; authored
  checks cover exact-zero admission, escaped duplicate JSON keys,
  HTTP framing/authentication, tool schema/arguments, matched history, reasoning
  pruning, UTF-8 token splits, optional JSON function-header metadata, typed
  snapshots, serialization and deadlines.
- Owned real-socket harness: text, sequential function replay, semantic SSE
  reconstruction and stable IDs/order, token budget, explicit unsupported errors,
  busy admission and disconnect/reuse. Debug/release, plain/ASan/UBSan native
  builds pass. Test continuations are Coax-owned; production imports no fixture.
- OpenAI Python SDK **3.41.0**: Responses create, streaming create, accumulated
  `responses.stream().get_final_response()`, typed function output replay.
  This SDK runs only as verification tooling against our local endpoint.
- Open Responses **2026-04-24**: response resources and supported baseline
  semantic events validate against the downloaded OpenAPI schema using
  jsonschema **4.26.0**. Raw-reasoning input arrays/default reasoning event names
  remain the documented GPT-OSS extension, not baseline-wide compliance.
- Pinned protoc **29.3**, Python protobuf **5.29.3**: generated owned descriptor,
  published compatibility, migration and consumer-profile checks pass.
- Existing device tiny-owned native checks pass with a new callback assertion:
  cancelled prefill performs no next chunk and retains committed cache/control.
  Original full-model CPU inference and full mutation roots were not run.

Local host compiler: trusted Coax 0.22 binary
`c5acf492faaef7ea8450245c44b454e69b208f83d9594e874be4d06eb3d3ae1a`.
Native compilation uses C11, `-ffp-contract=off -fno-fast-math`; sanitizers cover
socket/native and generated-Coax boundaries. CUDA source is our previously
qualified Coax emission; this change adds no numerical kernel/profile.

## Real GPU evidence

Owned worker `coderos-4080`, RTX 4080 (16 GiB), original 20b revision
`6cee5e81ee83917806bbde320786a8fb61efebee`, pinned Harmony table
`713aefbe74715ac78e32ad789ff6e6ad2d6d65372515462b76ab5ecaef8a0e7a`.
The server authenticates/uploads once before readiness; independent HTTP requests
reset KV state while retaining the weights. Model/device numerical qualification
remains the [45a–45c record](psionic-gptoss-device-2026-10-29.md).

Actual text create, streamed create, SDK stream accumulation, output limit and
unsupported-parameter checks pass through the resident GPU server. One short
stream on the final build used 63 input / 16 output IDs: creation arrived at
0.019 s and the full request completed at 0.706 s. Creation latency is not time to first generated
text. These are HTTP smoke measurements, not a new throughput benchmark.

A real streamed `weather` handoff produced the declared Paris JSON argument
(118 input / 29 output IDs). The SDK stream accumulator's full output dictionaries
were replayed with a matched owned weather result; the model completed a final
answer (172 input / 38 output IDs). Current analysis remained typed replay state.
The model's optional `<|constrain|>json` header is explicitly handled; nullable
SDK-derived projection fields do not become model instructions.

Disconnect during a longer owned prefill prefix released the worker for a later
request. Two overlapping real requests yielded one 200 and one 503. No CPU model
execution was used. The test server and its ephemeral key were removed afterward;
this is verified serving code, not a persistent/public deployment.

Qualified final GPU artifacts (SHA-256):

| Artifact | Identity |
| --- | --- |
| Native server | `320462a1b059d90e9bf9102dda3984d4372c83355809319f33071fd9856afaeb` |
| Generated host C | `b0bffb8ca59e66dce1567cc40dbd09ad703562ecd6ba81c772c6517b525aa45d` |
| Generated CUDA | `0a9ff413608cc328063407d68e2e40ec9dd1381e7bf5973ea5d253210d75f6c8` |

## Limits

Single worker, no continuous batching. Greedy original 20b only; 2048 context,
1–512 output tokens; checked flat schemas rather than masked GPU generation.
No response store, previous-response lookup, background work, multimodal input,
strict structured output, parallel/built-in tools, public deployment or universal
Open Responses compliance. Tool arguments must complete and validate before any
caller grant; analysis stays separate from UI text. No request content is logged.

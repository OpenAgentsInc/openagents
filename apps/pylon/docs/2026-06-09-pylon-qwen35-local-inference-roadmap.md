# Pylon Qwen3.5 Local Inference Roadmap

Date: 2026-06-09

Status: audit and first-pass roadmap for adding Psionic-backed Qwen3.5 local
inference as an optional Pylon coding-agent backend. The attach-only backend
profile and doctor surface are implemented for issue #13. The
OpenAI-compatible chat/completions client, tool-call loop, streaming
`delta.tool_calls` parser, round-trip guard, and redacted transcript/tool-call
receipts are implemented for issue #11. The 0.8B/2B model-row admission and
selection gates are implemented for issue #12. The #4665 pass adds
assignment-runner routing for `psionic_qwen35`, typed unattached refusal, and
the committed live smoke/runbook in `docs/psionic-qwen-live-smoke.md`.

## Source Material Read

Pylon:

- `README.md`
- `src/inventory.ts`
- `packages/runtime/src/backends/backend-profile.ts`
- `packages/runtime/src/backends/registry.ts`
- `packages/runtime/src/backends/gemini/client.ts`
- `packages/runtime/src/backends/gemini/protocol.ts`
- `packages/runtime/src/fleet/backend-capability.ts`
- `packages/runtime/src/llm/request.ts`
- `packages/runtime/src/llm/tool.ts`
- `packages/runtime/src/llm/tool-runtime.ts`
- `packages/runtime/src/benchmark/closeout-writer.ts`
- `docs/2026-06-09-pylon-psionic-ml-connection-audit.md`
- `docs/probe-port/probe-llm-core.md`
- `docs/probe-port/2026-06-07-blueprint-signature-lookup-apple-fm-tool-use-audit.md`

Psionic:

- `docs/NON_GPT_OSS_QWEN35_PILOT.md`
- `docs/HERMES_QWEN35_COMPATIBILITY.md`
- `docs/HERMES_BACKEND_BENCHMARK.md`
- `docs/HERMES_QWEN35_REUSE_BENCHMARK.md`
- `docs/QWEN35_RESPONSES_TOOL_LOOP_PILOT.md`
- `docs/PSION_RVLLM_SAMPLING_LOOP.md`
- `docs/PSION_RVLLM_CUDA_GRAPH_POOL.md`
- `crates/psionic-serve/src/bin/psionic-openai-server.rs`
- `crates/psionic-serve/src/openai_http.rs`

## Verdict

The first Pylon pass should support both documented small Qwen3.5 rows:

- `qwen3.5:0.8b` as the lowest-footprint local smoke and fallback row;
- `qwen3.5:2b` as the first coding-agent/tool-loop quality row.

Pylon should expose them through an optional `psionic_qwen35` backend profile
that attaches to a local or remote Psionic OpenAI-compatible server. Pylon
should not bundle model weights, should not download model artifacts on normal
startup, and should not claim training or paid capacity from this work.

Psionic support should be an optional ML workload install path, not part of
every Pylon install. A normal Pylon install should remain small and should
report `blocker.psionic_qwen35.connector_unconfigured` when Psionic is absent.
Only users with a compatible machine and an explicit desire to run local ML
workloads should download the Psionic binary and Qwen artifacts.

The smallest useful first pass is an attach-only backend:

1. operator starts or installs Psionic;
2. operator points Pylon at the Psionic base URL;
3. Pylon checks `/health` and `/v1/models`;
4. Pylon advertises local Qwen capability refs only for admitted model rows;
5. Pylon lowers provider-neutral LLM requests and tool definitions into
   OpenAI-compatible chat/responses calls;
6. Pylon records redacted transcript, tool-call, and capability receipts.

## What Psionic Already Proves

### Qwen3.5 0.8B

Psionic records the first explicit `qwen35` pilot for the Ollama
`qwen3.5:0.8b` GGUF in `docs/NON_GPT_OSS_QWEN35_PILOT.md`.

The documented artifact identity is:

- default path:
  `/home/christopherdavid/models/qwen3.5/qwen3.5-0.8b-q8_0.gguf`
- model digest:
  `afb707b6b8fac6e475acc42bc8380fc0b8d2e0e4190be5a969fbf62fcc897db5`
- chat-template digest:
  `273d8e0e683b885071fb17e08d71e5f2a5ddfb5309756181681de4f5a1822d80`

That row proves:

- real GGUF artifact detection as `qwen35`;
- real tokenizer/template facts;
- Qwen multimodal prompt-projection facts for image/video markers;
- deterministic tiny qwen35 native CUDA execution through Psionic;
- generic server publication and request execution;
- `/v1/chat/completions` and `/v1/responses` surface coverage;
- bounded qwen35 tool-loop continuation evidence through the responses pilot;
- explicit refusal boundaries for unsupported multimodal and structured-output
  paths.

The 0.8B row is the right first row for low-footprint local smoke, fast
operator validation, and fallback coding-agent tasks such as short grounded
answers, simple routing, and local tool-call plumbing checks. It should not be
treated as the default high-quality coding-agent model until Pylon retains its
own acceptance evidence.

### Qwen3.5 2B

Psionic's strongest retained coding-agent compatibility evidence is currently
on the `qwen3.5:2b` row.

Relevant retained evidence:

- `docs/HERMES_QWEN35_COMPATIBILITY.md` records a full `6/6` Hermes
  compatibility proof on native Psionic `qwen35` for the local 2B row.
- `docs/HERMES_BACKEND_BENCHMARK.md` records the same-host Hermes backend
  benchmark with Psionic model path
  `/home/christopherdavid/models/qwen3.5/qwen3.5-2b-q8_0-registry.gguf`.
- `docs/HERMES_QWEN35_REUSE_BENCHMARK.md` records repeated-loop prefix-cache
  evidence on `qwen3.5-2b-q8_0-registry.gguf`.

The 2B row is the right first row for Pylon's optional coding-agent backend
because it already has retained evidence for:

- required tool turns;
- auto plain-text turns;
- multi-turn tool loops;
- same-turn parallel tool calls;
- invalid-argument truthful refusal;
- streamed tool turns;
- serialized two-city tool loop behavior;
- repeated exact-hit warm-path reuse.

## Psionic Server Shape Pylon Should Attach To

Psionic already ships `psionic-openai-server`:

```sh
psionic-openai-server \
  -m /path/to/qwen3.5-0.8b-q8_0.gguf \
  -m /path/to/qwen3.5-2b-q8_0-registry.gguf \
  --backend cuda \
  --host 127.0.0.1 \
  --port 8080
```

The binary supports multiple `-m` model artifacts, `--backend cpu|cuda|metal`,
`--host`, `--port`, `--reasoning-budget`, and
`--mesh-coordination enabled|disabled`.

The Pylon first pass should assume attach mode:

- default base URL: `http://127.0.0.1:8080`;
- readiness path: `/health`;
- model list path: `/v1/models`;
- chat path: `/v1/chat/completions`;
- responses path: `/v1/responses`;
- management status path, when present:
  `/psionic/management/status`.

Pylon should not start Psionic implicitly until the signed sidecar release and
process-supervision gates exist. For the first pass,
`pylon backend psionic doctor` and its alias `pylon psionic doctor` explain how
to verify the configured base URL.

## Optional Download Policy

The default `@openagentsinc/pylon` install should not include Psionic binaries
or Qwen model weights.

The explicit opt-in flow is now scaffolded and guarded:

```sh
pylon psionic doctor --json
pylon psionic install --channel rc --manifest-url <release-manifest-url> --yes
pylon psionic models install qwen35-0_8b-q8_0 --manifest-url <model-manifest-url> --yes
pylon psionic models install qwen35-2b-q8_0 --manifest-url <model-manifest-url> --yes
```

The install flow runs machine checks before downloading anything:

- supported platform: macOS or Linux;
- supported architecture: `darwin-arm64`, `linux-x64`, or `linux-arm64`;
- backend viability: Metal on `darwin-arm64`, admitted CPU fallback on Linux;
- memory and disk budgets;
- no competing model workload if the installer is about to run a local smoke;
- Psionic release manifest verification;
- model artifact manifest verification;
- SHA-256 verification before binary/model placement;
- digest-addressed cache placement under the Pylon cache;
- explicit operator consent for each model artifact.

Current manifest URLs are operator- or env-supplied. Psionic now has
Pylon-consumable sidecar/model manifest fixtures for the first release lane,
but Pylon intentionally does not bundle live default manifest URLs until those
fixtures are promoted to the public release channel.

If a machine cannot run local ML workloads, Pylon should stay usable. It should
keep the Psionic backend blocked with precise blocker refs and continue using
other configured backends such as OpenCode, Apple FM, or Gemini. Psionic absence
must not break registration, wallet readiness, GEPA no-spend work, or normal
coding-agent operation.

This keeps Qwen local inference available to capable machines without turning
the v0.3 package into a heavy ML distribution.

## Pylon Backend Design

Add a third runtime backend family alongside Apple FM and Gemini:

- backend kind: `psionic_qwen35`;
- profile id: `psionic-qwen35-local`;
- capability ref: `probe.backend.psionic_qwen35`;
- default base URL: `http://127.0.0.1:8080`;
- env override order: explicit `--base-url`, `PYLON_PSIONIC_BASE_URL`,
  `PROBE_PSIONIC_BASE_URL`, default;
- auth mode: `none` for local attach;
- attach mode: `attach_existing`;
- stream mode: OpenAI-compatible SSE when streaming is requested;
- supported endpoints: `/v1/chat/completions`, `/v1/responses`.

Implemented attach-only surfaces:

```sh
pylon backend psionic doctor --json
pylon backend psionic smoke --json
pylon psionic doctor --json
pylon psionic smoke --json
```

The doctor checks `/health` and `/v1/models`, requires
`execution_engine = psionic` when that field is present, admits only the
0.8B/2B Qwen3.5 refs, and returns redacted availability receipts with blocker
refs including `connector_unconfigured`, `health_unreachable`,
`execution_engine_not_psionic`, and `qwen35_model_missing`.

The smoke command runs one bounded OpenAI-compatible chat completion through a
local Psionic server. It may pass even while `doctor` is not launch-ready, and
when that happens it reports the model-admission blockers separately as
`admissionBlockerRefs`. This is deliberate: real local inference can be proven
without pretending that an unverified GGUF artifact is safe to advertise as an
admitted launch model.

Live evidence captured on June 9, 2026:

- command:
  `PYLON_PSIONIC_BASE_URL=http://127.0.0.1:18080 bun src/index.ts psionic smoke --json`;
- Psionic server:
  `psionic-openai-server` with `execution_engine = psionic`, Metal backend, and
  `Qwen_Qwen3.5-0.8B-Q4_K_M.gguf`;
- Pylon result: `state = passed`, `inference = real_psionic_openai_compatible`,
  text `psionic pylon live`, usage `input=21 output=5 total=26`;
- admission result: blocked from launch advertisement by
  `blocker.psionic_qwen35.artifact_digest_unverified` and
  `blocker.psionic_qwen35.qwen35_model_missing` because the local Q4 artifact is
  not one of the first-pass Q8 manifest rows.

The backend should reuse Pylon's provider-neutral LLM core:

- `ProbeLlmRequest`;
- `ProbeLlmMessage`;
- `ProbeLlmToolDefinition`;
- `ProbeLlmToolChoice`;
- `dispatchProbeLlmTool`;
- `ProbeLlmUsage`.

It should not create a separate Qwen-only tool runtime. Tool planning should
continue through Blueprint signature lookup and `ProbeToolMenuPlanner`, then
the Psionic backend should lower that menu into OpenAI-compatible tool schemas.

## Model Row Admission

Pylon should admit these two rows in the first pass:

| Model ref | Role | Required proof before advertisement |
| --- | --- | --- |
| `model.psionic.qwen35.0_8b.q8_0` | lowest-footprint smoke/fallback row | `/health`, `/v1/models`, artifact digest match or model manifest ref, chat completion smoke, one required-tool smoke |
| `model.psionic.qwen35.2b.q8_0` | coding-agent/tool-loop row | all 0.8B checks plus multi-turn tool-loop smoke, same-turn parallel tool-call smoke, and transcript receipt |

Implemented model-row gate:

- `/v1/models` rows are decoded into observed public-safe model refs first;
- a Qwen-looking row is not admitted unless it carries either a verified
  artifact digest or a public-safe artifact/model manifest ref;
- the retained 0.8B digest
  `afb707b6b8fac6e475acc42bc8380fc0b8d2e0e4190be5a969fbf62fcc897db5`
  admits `model.psionic.qwen35.0_8b.q8_0`;
- 2B admission currently requires a public-safe manifest ref because this repo
  does not yet carry a retained 2B digest authority;
- path-like model IDs are hashed in doctor output and are not used as public
  refs;
- coding-agent selection prefers `model.psionic.qwen35.2b.q8_0` when both rows
  are admitted;
- 2B-required work is refused with
  `blocker.psionic_qwen35.model_2b_missing` when only 0.8B is admitted.

Pylon should prefer the 2B row for coding-agent tasks when both are ready. It
should use 0.8B for:

- install smoke;
- health and latency probes;
- simple local answer tasks;
- fallback when 2B is absent but local Qwen is still useful.

It should refuse tasks that require the 2B row when only 0.8B is available:

- multi-step coding-agent work;
- paid inference tasks;
- benchmark claims that name Hermes compatibility;
- launch copy that says Pylon has a high-quality local coding model.

## Capability Projection

`src/inventory.ts` currently has a generic `backend.local_model` row with
`blocker.backend.local_model_inventory_unproven`. Replace or refine it with
Psionic-specific rows:

- `backend.psionic.openai_server`;
- `backend.psionic.qwen35`;
- `model.psionic.qwen35.0_8b.q8_0`;
- `model.psionic.qwen35.2b.q8_0`.

Projected health should include only public-safe refs:

- backend ready/configured/missing;
- model refs;
- supported endpoint refs;
- model-cache state;
- release identity ref, if a signed Psionic sidecar is used;
- artifact digest refs, not local paths;
- blocker refs.

Projected health must not expose:

- raw GGUF paths;
- environment dumps;
- bearer tokens;
- provider secrets;
- private network topology;
- local benchmark transcripts.

## Tool-Call Integration

The Psionic Qwen backend should support two tool-call loops.

### Chat Completions Loop

Use `/v1/chat/completions` for the first coding-agent backend because Psionic's
Hermes compatibility proof is retained there.

Implemented surface:

- `makePsionicQwenClient().complete(...)` accepts `ProbeLlmRequest` and
  `ProbeLlmTools`;
- lowers system/user/assistant/tool turns to OpenAI-compatible chat messages;
- lowers `ProbeLlmToolDefinition` to OpenAI-compatible function tools;
- maps `auto`, `none`, `required`, and named tool choices to `tool_choice`;
- parses non-streaming `message.tool_calls`;
- parses streaming `delta.tool_calls` with chunked argument assembly;
- dispatches local tools through `dispatchProbeLlmTool`;
- appends assistant tool-call turns and tool-result turns;
- enforces a max model round-trip count;
- emits content-redacted transcript and tool-call receipts.

Required behavior:

- keep the chat/completions client attach-only;
- keep prompts, local paths, model files, and provider secrets out of receipts;
- preserve provider-neutral request/tool/usage contracts so assignment routing
  can select Psionic without a Qwen-only tool runtime.

### Responses Loop

Use `/v1/responses` when Pylon needs response-state replay.

Required behavior:

- preserve `previous_response_id`;
- preserve tool replay as `role = tool` with the tool name;
- surface Psionic response-state refs in private receipts;
- public projection should only carry redacted refs and status.

The responses loop is not required for the first useful `chat.completions`
integration, but it should be part of the roadmap because Psionic already has
a dedicated qwen35 responses tool-loop pilot.

## Acceptance Gates

Before Pylon advertises `psionic_qwen35` as ready:

1. `pylon backend psionic doctor` reaches `/health`.
2. `/health` reports `execution_engine = psionic`.
3. `/health` reports supported endpoints including `/v1/chat/completions`.
4. `/v1/models` includes at least one admitted qwen35 model row.
5. Artifact manifest or model digest matches the admitted row.
6. Plain text chat completion succeeds.
7. Required single-tool call succeeds.
8. Round-trip limit refuses infinite tool loops.
9. Receipts redact content and raw local paths.
10. `pylon inventory --json` projects only safe backend/model refs.

Before Pylon prefers 2B for coding-agent work:

1. 2B row is present in `/v1/models`.
2. Required tool turn passes.
3. Multi-turn tool loop passes.
4. Same-turn parallel tool-call smoke passes.
5. Invalid argument refusal is truthful.
6. Streamed tool-call event parsing is covered.
7. Local transcript receipts are retained and redacted.

Before any paid inference claim:

1. OpenAgents assignment lease supports local inference work class.
2. Pricing, budget, and timeout are in the lease.
3. Wallet/payout readiness is fresh.
4. Settlement path is proven.
5. Psionic artifact and backend receipts are attached to closeout.

Paid inference is not part of the first pass.

Implemented assignment/launch gate surface:

- assignment leases may carry `psionicQwenRequirements` for
  `workClass = local_inference`;
- admission selects from admitted Psionic Qwen model refs and distinguishes
  0.8B fallback from 2B-required work;
- 2B-required work emits `blocker.psionic_qwen35.model_2b_missing` when only
  0.8B is admitted;
- no-spend closeouts attach only public Psionic backend/model/receipt refs;
- launch copy may use bounded optional-local-inference language only;
- Qwen training and paid Qwen inference remain blocked launch claims.

## Blocker Refs

Add specific blockers:

- `blocker.psionic_qwen35.connector_unconfigured`
- `blocker.psionic_qwen35.health_unreachable`
- `blocker.psionic_qwen35.execution_engine_not_psionic`
- `blocker.psionic_qwen35.qwen35_model_missing`
- `blocker.psionic_qwen35.model_0_8b_missing`
- `blocker.psionic_qwen35.model_2b_missing`
- `blocker.psionic_qwen35.artifact_digest_unverified`
- `blocker.psionic_qwen35.chat_completion_failed`
- `blocker.psionic_qwen35.tool_call_failed`
- `blocker.psionic_qwen35.parallel_tool_call_unproven`
- `blocker.psionic_qwen35.responses_state_unproven`
- `blocker.psionic_qwen35.paid_inference_not_admitted`

## Implementation Roadmap

### Phase 1: Attach-Only Backend

- Add `psionic_qwen35` backend profile and registry entry.
- Add env/config resolution:
  - `PYLON_PSIONIC_BASE_URL`;
  - `PROBE_PSIONIC_BASE_URL` as a compatibility alias.
- Add `pylon backend psionic doctor`.
- Add health/model-list client.
- Add public-safe availability receipt.
- Update inventory to project Psionic Qwen model refs.

### Phase 2: OpenAI-Compatible Chat Client

- Implemented: request lowering from `ProbeLlmRequest` to OpenAI-compatible
  `/v1/chat/completions`.
- Implemented: SSE and non-stream parser for text deltas, finish events, usage,
  and
  tool calls.
- Implemented: reuse `dispatchProbeLlmTool`.
- Implemented: redacted transcript/tool-call receipts.
- Implemented: fake Psionic server tests for plain text, required tool,
  malformed
  response, and round-trip limit.
- Implemented: `pylon backend psionic smoke` and `pylon psionic smoke` for a
  bounded live completion proof.

### Phase 3: First-Pass Model Gates

- Implemented: 0.8B smoke case, including the observed live Q4 smoke path as
  non-admitted evidence.
- Implemented: 2B tool-loop case in the client test harness.
- Implemented: model preference policy:
  - prefer 2B for coding-agent mode;
  - allow 0.8B for smoke/fallback/simple local tasks;
  - refuse 2B-required assignments when only 0.8B is present.
- Implemented: launch gate copy allows "optional local Qwen inference backend"
  only after attach and smoke gates pass, and model advertisement still requires
  artifact admission.

### Phase 4: Responses State

- Add `/v1/responses` request lowering and parser.
- Preserve response-state refs privately.
- Add tool-result replay test.
- Keep public projection redacted.

### Phase 5: Sidecar And Artifact Installer

- Reuse the Psionic sidecar plan in
  `docs/2026-06-09-pylon-psionic-ml-connection-audit.md`.
- Implemented: opt-in Psionic binary/model installer scaffold, never startup
  auto-download.
- Implemented: release/model manifest verification and SHA-256 verification
  before placement.
- Implemented: digest-addressed binary/model cache layout.
- Implemented upstream in Psionic: Pylon-consumable release/model manifest
  fixtures and docs.
- Remaining: promote Psionic-owned signed release/model manifests to public
  release URLs.
- Remaining: wire default manifest discovery once Psionic publishes those
  manifests.
- Remaining: add sidecar process supervision after signed release identity
  exists.

## Copy Rules

Allowed after Phase 1 and Phase 2 pass:

- "Pylon can attach to a local Psionic Qwen3.5 server as an optional backend."
- "Pylon supports Qwen3.5 0.8B and 2B local inference rows when Psionic and
  verified model artifacts are present."
- "Pylon can route bounded local tool-call smokes through Psionic Qwen."

Blocked until later gates:

- "Pylon trains Qwen."
- "Pylon bundles Qwen models."
- "Pylon downloads Qwen on startup."
- "Pylon local Qwen inference is paid capacity."
- "0.8B is equivalent to the 2B coding-agent proof row."
- "Every Pylon can run Qwen."

## First-Pass Definition Of Done

The first pass is done when:

- `pylon backend psionic doctor --json` reports health and model rows;
- `pylon inventory --json` shows safe Psionic backend/model refs;
- a fake Psionic server test covers 0.8B and 2B model-list admission;
- a fake Psionic server test covers required tool calls;
- a fixture or local smoke proves 0.8B plain text generation;
- a fixture or local smoke proves 2B required-tool and multi-turn tool loops;
- docs and launch gates distinguish optional inference from training and paid
  work.

That is the smallest honest path to Qwen in Pylons: attach to Psionic,
support both small rows, prefer 2B for coding-agent work, keep 0.8B as the low
footprint row, and block all training or paid-capacity claims until their own
receipts exist.

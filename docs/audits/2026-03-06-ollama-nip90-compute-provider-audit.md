# 2026-03-06 Ollama NIP-90 Compute Provider Audit

## Scope

- Product/spec alignment:
  - `README.md`
  - `docs/MVP.md`
  - `docs/OWNERSHIP.md`
  - `docs/kernel/*`
- Current OpenAgents NIP-90/provider implementation:
  - `crates/nostr/core/src/nip90/*`
  - `crates/nostr/client/src/dvm.rs`
  - `apps/autopilot-desktop/src/provider_nip90_lane.rs`
  - `apps/autopilot-desktop/src/input/reducers/jobs.rs`
  - `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`
  - `apps/autopilot-desktop/src/state/job_inbox.rs`
  - `apps/autopilot-desktop/src/state/provider_runtime.rs`
- Relevant OpenAgents issues reviewed with `gh`:
  - `#2982` provider auto-accept / MVP concurrency defaults
  - `#2983` automated provider execution
  - `#2990` NIP-89 capability publication
  - `#2998` future backend-kernel authority direction
- DVM / NIP-90 protocol references:
  - `/Users/christopherdavid/code/data-vending-machines/README.md`
  - `/Users/christopherdavid/code/data-vending-machines/nip89.md`
  - `/Users/christopherdavid/code/data-vending-machines/kinds/5050.md`
  - adjacent kind docs for `5900`, `5905`, `5970`
- Ollama surface and implementation references:
  - `/Users/christopherdavid/code/ollama/ollama/README.md`
  - `/Users/christopherdavid/code/ollama/ollama/docs/api.md`
  - `/Users/christopherdavid/code/ollama/ollama/docs/api/*`
  - `/Users/christopherdavid/code/ollama/ollama/docs/openapi.yaml`
  - `/Users/christopherdavid/code/ollama/ollama/api/types.go`
  - `/Users/christopherdavid/code/ollama/ollama/server/routes.go`
  - `/Users/christopherdavid/code/ollama/ollama/server/sched.go`
  - `/Users/christopherdavid/code/ollama/ollama/envconfig/config.go`
  - relevant open/closed Ollama issues via `gh`

## Executive Verdict

OpenAgents can support "sell local inference through Ollama for NIP-90 jobs" without a repo-wide redesign, but the current implementation is not yet protocol-correct or product-truthful for that role.

The smallest correct MVP is:

1. Support only NIP-90 text generation first:
   - request kind `5050`
   - result kind `6050`
   - feedback kind `7000`
   - NIP-89 handler kind `31990`
2. Execute accepted `5050` jobs through a local Ollama backend, not through the current Codex execution lane.
3. Publish only capabilities the desktop can actually serve at that moment.
4. Return the generated text itself in the result `content`, not the current JSON execution envelope.
5. Keep money authority exactly where the kernel docs say it belongs: wallet-confirmed settlement and explicit receipts, not Ollama metrics and not Nostr event publication alone.

This should be implemented as an app-owned backend inside `apps/autopilot-desktop`, with only a small interop fix in `crates/nostr/core`. It should not be generalized into a broad multi-provider framework yet. A narrow execution-backend seam is enough for MVP.

## Kernel And MVP Alignment

The docs are internally consistent on the key product constraint:

- MVP is the provider/earn loop, not a generalized autonomous economy build-out yet.
- Nostr is the coordination surface, not the money authority.
- Provider runtime must remain deterministic and replay-safe where state is projected locally.
- Wallet and payout truth must remain explicit and honest in the UI and receipts.
- Ownership boundaries say product/provider behavior belongs in `apps/autopilot-desktop`, while reusable protocol parsing belongs in `crates/nostr/*`.

That means:

- Ollama execution should live in the desktop app layer.
- Shared NIP-90 parsing fixes should live in `crates/nostr/core`.
- `crates/wgpui/` should not become the home for provider execution policy or Ollama integration.
- Settlement must continue to depend on wallet evidence, not "Ollama says it ran."

## Canonical Protocol Baseline For Text Generation

From the DVM repo, the first compute lane should target the standard text-generation path:

- Request kind: `5050`
- Result kind: `6050`
- Feedback kind: `7000`
- Capability advertisement: `kind:31990` with `k` tags containing supported request kinds

For `5050`, the relevant request surface is:

- Input comes from `content` and/or `i` tags
- The example uses `["i", "<prompt>", "prompt"]`
- Optional params include:
  - `model`
  - `max_tokens`
  - `temperature`
  - `top_k`
  - `top_p`
  - `frequency_penalty`
- The example payload actually uses hyphenated spellings:
  - `top-k`
  - `top-p`
- Expected outputs include:
  - `text/plain`
  - `text/markdown`

Important protocol implication: for the first Ollama lane, the correct event kinds are `5050`, `6050`, `7000`, and `31990`. The adjacent DVM docs for `5900`, `5905`, and `5970` are not the text-generation lane and should not shape the initial implementation beyond reminding us that encrypted NIP-90 request/feedback flows exist in the broader ecosystem.

## Current OpenAgents State

### What Already Exists

- `crates/nostr/core/src/nip90/kinds.rs` already defines:
  - `KIND_JOB_TEXT_GENERATION = 5050`
  - `KIND_JOB_FEEDBACK = 7000`
  - request-result mapping as `request kind + 1000`
- `crates/nostr/core/src/nip90/model.rs` already models:
  - `JobRequest`
  - `JobResult`
  - `JobFeedback`
  - `i`, `param`, `bid`, `output`, `p`, `relays`, and encryption-related tags
- `apps/autopilot-desktop/src/provider_nip90_lane.rs` already:
  - parses inbound NIP-90 requests
  - maps `5050` to capability `"text.generation"`
  - publishes a NIP-89 handler event
- `apps/autopilot-desktop/src/input/reducers/jobs.rs` already:
  - auto-accepts valid requests
  - enforces provider runtime sequencing
  - publishes NIP-90 feedback and results
- Current wallet/receipt architecture already preserves the kernel rule that settlement truth is external to model execution.

### What Is Missing Or Wrong For An Ollama Provider

#### 1. The parser rejects the DVM repo's canonical `prompt` input type.

`crates/nostr/core/src/nip90/model.rs` only accepts:

- `url`
- `event`
- `job`
- `text`

The DVM `5050` example uses `["i", "...", "prompt"]`. Today OpenAgents will reject that request as `InvalidInputType`.

This is a real interop bug and the highest-priority shared protocol fix.

#### 2. The provider execution path is still Codex-specific.

`apps/autopilot-desktop/src/input/reducers/jobs.rs` currently:

- starts a Codex thread
- starts a Codex turn
- feeds a synthetic execution prompt
- captures final agent text

That is not "sell local inference via Ollama." It is a different compute substrate.

#### 3. The current result payload shape is not DVM-friendly for `5050`.

`queue_nip90_result_publish_for_active_job` currently serializes a JSON envelope into result `content`, including fields like:

- `request_id`
- `job_id`
- `capability`
- `input`
- `output`
- `provider_thread_id`
- `provider_turn_id`

For a text-generation DVM, the result `content` should be the generated text itself. If a requester asks "what is the capital of France?", the `6050` content should be "Paris" or a fuller text answer, not a JSON wrapper.

This is the second biggest protocol-correctness gap after the `prompt` alias bug.

#### 4. Capability advertisement is currently over-claiming.

`supported_handler_kinds()` in `apps/autopilot-desktop/src/provider_nip90_lane.rs` currently advertises:

- `5050`
- summarization
- translation
- extraction
- patch generation
- code review
- repo index
- sandbox run
- RLM subquery

An Ollama-first MVP provider cannot truthfully advertise that full set unless those kinds are actually implemented against Ollama and policy-validated. For the first cut it should publish only `5050`.

#### 5. There is no Ollama integration at all.

A repo-wide search shows no existing `ollama` integration in the retained code. There is no:

- Ollama client
- model discovery
- provider-side model selection
- warm/unload logic
- mapping from DVM params to Ollama options
- Ollama-specific health state in the UI/runtime

#### 6. Current provider prompts and outputs are wrong for a local inference provider.

`provider_execution_prompt_for_active_job()` constructs a Codex-agent instruction block. That is appropriate for the current Codex lane, not for a one-shot `5050` completion.

For Ollama `5050`, the job input should be normalized into a prompt and sent directly to `/api/generate`.

## Ollama Readout

### Recommended API Surface

For the first `5050` implementation, use Ollama's native local API, not the OpenAI-compatibility layer.

Reason:

- Native `/api/generate` matches the one-shot prompt shape of DVM `5050`.
- Native endpoints expose the load/unload behavior directly.
- Native responses expose timing and token metrics directly.
- Native docs/source make the scheduler and queue semantics clearer.
- The OpenAI-compatibility layer adds no value for this first integration.

#### Relevant Endpoints

- `POST /api/generate`
- `GET /api/tags`
- `POST /api/show`
- `GET /api/ps`
- `POST /api/chat` is useful later, but not required for prompt-only `5050`

#### Important Request/Response Fields

Ollama supports the knobs we need for a first text-generation lane:

- Request:
  - `model`
  - `stream`
  - `keep_alive`
  - `options`
  - `format`
  - `think`
- Options:
  - `num_predict`
  - `top_k`
  - `top_p`
  - `temperature`
  - `frequency_penalty`
  - `presence_penalty`
  - `stop`
  - `seed`
  - `num_ctx`
  - other runner controls
- Response metrics:
  - `total_duration`
  - `load_duration`
  - `prompt_eval_count`
  - `prompt_eval_duration`
  - `eval_count`
  - `eval_duration`

#### Operational Behaviors That Matter For Product Design

- Default server host is local loopback (`localhost:11434`)
- No auth is required for the local server
- Default keep-alive is `5m`
- Default per-model parallelism is `1`
- Empty `/api/generate` or `/api/chat` requests can preload a model
- `keep_alive: 0` can unload a model
- `/api/ps` can show what is currently loaded
- `/api/tags` can list installed models
- `/api/show` can expose model details/capabilities

#### Upstream Constraints That Must Shape The MVP

From source and issue review:

- Ollama only loads one model at a time during certain scheduler operations.
- Queue behavior is not a sufficient admission-control layer on its own.
- Concurrency plus model reloads can deadlock or behave poorly.
- Higher parallelism increases memory pressure materially.
- Structured output and reasoning/thinking paths still have open edge cases.

This strongly argues for:

- OpenAgents `max_inflight = 1`
- stable per-model runtime options
- no dynamic option churn that forces runner reloads during concurrent work
- plain text generation only in the first cut
- `stream: false` for the initial implementation

## Required Implementation Work

### 1. Shared Protocol Interop Fixes In `crates/nostr/core`

Files:

- `crates/nostr/core/src/nip90/model.rs`
- `crates/nostr/core/src/nip90/tests.rs`

Required work:

- Accept `prompt` as an alias for text input in `InputType::from_str`.
- Keep serialization canonical. The cleanest MVP choice is:
  - parse both `text` and `prompt`
  - internally normalize both to `InputType::Text`
  - continue emitting `text` unless and until there is a reason to preserve the original lexical form
- Add tests proving that a `5050` event using `["i", "...", "prompt"]` parses successfully.

Why this belongs here:

- It is a reusable NIP-90 interoperability fix, not app-specific policy.

### 2. Introduce A Narrow Provider Execution Backend In `apps/autopilot-desktop`

Do not build a large provider abstraction framework. Add only the seam needed to swap the current Codex execution lane for an Ollama-backed `5050` executor.

Recommended shape:

- A small app-owned trait or enum-driven dispatcher, for example:
  - `supports_kind(kind, job)`
  - `warm()`
  - `execute_text_generation(job)`
  - `unload()`
  - `health_snapshot()`

Recommended placement:

- New app-local module such as:
  - `apps/autopilot-desktop/src/provider_execution/mod.rs`
  - `apps/autopilot-desktop/src/provider_execution/ollama.rs`

Why app-owned:

- `docs/OWNERSHIP.md` places provider product behavior in the app.
- This is not a generic Nostr concern.
- This is not a `wgpui` concern.

### 3. Replace The `5050` Execution Path With Ollama

Primary file:

- `apps/autopilot-desktop/src/input/reducers/jobs.rs`

Required change:

- Accepted `5050` jobs should no longer start a Codex thread/turn.
- They should instead:
  - validate Ollama readiness
  - normalize the prompt
  - call the Ollama backend
  - capture the generated text
  - publish feedback/result events
  - record local receipts/metrics

The existing Codex flow can remain for other future/internal capabilities if needed, but it must not be the execution engine for the advertised Ollama `5050` lane.

### 4. Add An Ollama Client And Local Provider Configuration

Recommended new app-owned concerns:

- base URL, defaulting to local loopback only
- selected serving model
- model inventory
- keep-alive policy
- health / warm / loaded state
- last successful generation metrics
- last Ollama error

MVP recommendation:

- Treat remote Ollama hosts as out of scope.
- Assume the user has Ollama installed.
- Assume the user has already pulled at least one model.
- Require explicit selection of a local model before advertising the provider.

#### DVM To Ollama Param Mapping

Required normalization:

- `model` -> top-level Ollama `model`
- `max_tokens` -> `options.num_predict`
- `temperature` -> `options.temperature`
- `top_k` or `top-k` -> `options.top_k`
- `top_p` or `top-p` -> `options.top_p`
- `frequency_penalty` -> `options.frequency_penalty`

Reasonable MVP extras if already present in request params:

- `presence_penalty` -> `options.presence_penalty`
- `seed` -> `options.seed`
- `stop` -> `options.stop`

Important non-mapping note:

- DVM `output` is a MIME hint, not a direct Ollama parameter for this MVP.
- For `text/plain` and `text/markdown`, just return the generated text.
- Do not use Ollama structured-output or JSON modes in the first cut.

#### Prompt Normalization

For `5050`, normalize input in this order:

1. If there are `i` tags of type `prompt` or `text`, use them as the primary prompt material.
2. If `content` is non-empty, include it.
3. If multiple prompt-bearing inputs exist, join them deterministically.
4. If there is no usable prompt after normalization, reject the request truthfully.

This normalization should be deterministic and recorded in local receipts for replay/debugging.

### 5. Make NIP-89 Capability Publication Truthful And Dynamic

Primary file:

- `apps/autopilot-desktop/src/provider_nip90_lane.rs`

Required change:

- Replace the current static `supported_handler_kinds()` list with backend-driven capability reporting.
- In the initial Ollama MVP, the handler should publish only:
  - `k=5050`

Publishing rules:

- Do not publish a handler until:
  - a Nostr identity is ready
  - Ollama is reachable
  - a configured local model exists
  - the model has been validated as usable for text generation
- If Ollama becomes unavailable, stop claiming the provider is healthy and stop auto-accepting new jobs.

Residual protocol reality:

- NIP-89 handler events can linger on relays after the device goes offline.
- That is not introduced by Ollama; it is a general limitation of the current product shape.
- MVP can tolerate this if the runtime rejects/ignores jobs while offline, but it should be called out as a follow-on product truth problem.

### 6. Fix NIP-90 Result And Feedback Semantics For `5050`

Primary file:

- `apps/autopilot-desktop/src/input/reducers/jobs.rs`

Required change:

- For a successful `5050`, `JobResult.content` should be the generated text itself.
- Do not publish the current JSON execution envelope as the user-visible result.

Recommended feedback/result behavior:

- On start:
  - optionally publish `7000` with `status=processing`
- On success:
  - publish `6050` with plain generated text in `content`
  - include `amount` if there is a quoted price
- On failure:
  - publish `7000` with `status=error` and a truthful reason

Keep metrics and provenance out of the visible result text. Store them in:

- local receipts
- UI state
- possibly extra tags later if a stable interoperable pattern is chosen

Do not make payment settlement depend on result publication alone.

### 7. Expand Validation And Admission Control For Ollama

Current job-inbox validation and runtime sequencing are useful, but the Ollama lane needs provider-specific rejection reasons.

Add truthful rejection/preflight checks for:

- unsupported request kind
- malformed request payload
- missing prompt after normalization
- unsupported output MIME type
- encrypted request that cannot be decrypted
- Ollama not reachable
- selected provider model not installed locally
- requested `model` param not installed locally
- requested `model` param not allowed by local policy
- provider already at max inflight work
- bid below provider minimum
- TTL too short for sane execution

Important policy point:

- Do not rely on Ollama queueing as the provider's concurrency control.
- OpenAgents should continue to own admission and keep `max_inflight = 1` for MVP.

### 8. Add Truthful UI/Runtime State For The Compute Backend

Likely touch points:

- `apps/autopilot-desktop/src/state/provider_runtime.rs`
- provider-related UI panes in `apps/autopilot-desktop`

The provider UI should explicitly show:

- whether Ollama is reachable
- selected serving model
- whether the model is currently warm/loaded
- installed models discovered from `/api/tags`
- last load/generation timings
- token counts and derived token/sec if desired
- last backend error
- whether the provider is online but degraded

`Go Online` should mean something real:

- provider is connected to relays
- provider has a selected local model
- Ollama is reachable
- capability publication has succeeded or is actively retrying

Do not show the provider as a healthy compute seller if those conditions are not true.

### 9. Record Ollama-Specific Provenance In Local Receipts

Current receipt and earnings truth model is good and should remain.

Add local provenance fields for execution evidence such as:

- requested model
- actual served model
- normalized prompt digest
- chosen sampling/options
- host used
- total duration
- load duration
- prompt token count
- generated token count
- whether the model was already warm

Why:

- debugging and replayability
- truthful operator visibility
- later pricing and verification work

Why not more:

- these are evidence and local observability
- they are not the authority for economic settlement

## Recommended MVP Architecture

### Request Flow

1. NIP-90 request arrives on the current provider lane.
2. OpenAgents parses the request via shared `nip90` model types.
3. Preflight validation classifies the request.
4. If valid and within local policy, the runtime accepts it.
5. For `5050`, execution is dispatched to the Ollama backend.
6. Ollama returns generated text plus metrics.
7. OpenAgents publishes:
   - optional `7000 processing`
   - `6050` success result or `7000 error`
8. Local receipts are updated.
9. Wallet-based settlement remains the authoritative payout truth.

### Backend Choice For `5050`

Use `POST /api/generate` with:

- `stream: false`
- explicit `model`
- normalized prompt
- normalized options
- conservative `keep_alive`

Why `/api/generate` over `/api/chat` for the first cut:

- DVM `5050` is prompt/completion shaped
- request normalization is simpler
- result extraction is simpler
- it avoids inventing conversation semantics that the request kind does not require

`/api/chat` can be revisited later for richer compute kinds.

### Model Warm-Up / Cool-Down

Recommended behavior:

- On provider start or `Go Online`:
  - verify Ollama reachability
  - fetch installed models with `/api/tags`
  - validate selected model
  - optionally inspect capabilities with `/api/show`
  - warm the selected model with an empty generate request and nonzero `keep_alive`
- On provider stop or `Go Offline`:
  - stop accepting work
  - optionally unload with `keep_alive: 0`

This makes "online" and "offline" more truthful from the operator's point of view.

### Non-Goals For The First Ollama Cut

- No arbitrary remote Ollama server support
- No multi-kind capability advertisement beyond `5050`
- No tool-calling
- No structured-output / JSON-schema mode
- No thinking/reasoning-specific UX
- No image generation or multimodal lanes
- No automatic model pulling/downloading from OpenAgents
- No provider-side parallelism greater than `1`
- No attempt to make Ollama metrics the economic authority

## Concrete File-Level Change Map

### Shared Protocol

- `crates/nostr/core/src/nip90/model.rs`
  - parse `prompt` as a text-input alias
- `crates/nostr/core/src/nip90/tests.rs`
  - add interop coverage for DVM `5050` examples

### Provider Lane

- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
  - make capability publication backend-driven
  - advertise only `5050` for Ollama MVP
  - gate capability publication on real backend health

### Job Execution

- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
  - route `5050` jobs to Ollama instead of Codex
  - fix result `content` to be generated text
  - publish truthful feedback on failure/processing
  - record Ollama execution metadata locally

### Provider Validation / Ingress

- `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`
  - extend invalid/pending reasons for Ollama-specific preflight failures
- `apps/autopilot-desktop/src/state/job_inbox.rs`
  - no model change required in principle, but new validation reasons will surface here

### Provider Runtime State

- `apps/autopilot-desktop/src/state/provider_runtime.rs`
  - add backend-health and model-state fields needed for truthful UI

### New App-Owned Ollama Module(s)

- likely new files under `apps/autopilot-desktop/src/`
  - `provider_execution/*`
  - `ollama_client/*`
  - exact layout is flexible as long as ownership stays app-local

## Risk Register

### P0

- `prompt` alias missing in shared parser blocks interop with the DVM `5050` example.
- Result `content` currently carries JSON metadata instead of the generated text.
- Static NIP-89 advertisement currently over-claims unsupported kinds.

### P1

- No existing Ollama health/model-selection state means the UI cannot be truthful yet.
- Ollama scheduler behavior makes provider-side concurrency above `1` risky.
- Requested model names can diverge from locally installed model names and need explicit validation.
- `top-k` / `top-p` versus `top_k` / `top_p` spelling mismatch will cause silent option loss unless normalized.

### P2

- Handler events may linger on relays after the device goes offline.
- Encrypted request support may exist in the current lane but still needs explicit verification in the Ollama execution path.
- Structured outputs and advanced generation modes should stay out of scope until upstream Ollama behavior is more stable.

## Recommended Implementation Sequence

1. Fix shared NIP-90 `prompt` parsing.
2. Add a narrow app-owned provider execution backend seam.
3. Implement the local Ollama client and model inventory/health checks.
4. Route `5050` execution through `/api/generate`.
5. Change `6050` result publication to return raw generated text.
6. Make NIP-89 capability publication dynamic and truthful.
7. Add provider UI/runtime state for Ollama health and selected model.
8. Add receipts/tests for normalized prompts, option mapping, and result publication.

## Test And Validation Plan

Minimum new automated coverage:

- shared unit tests for `prompt` alias parsing
- shared unit tests for DVM `5050` example parsing
- app tests for param normalization:
  - `top-k` -> `top_k`
  - `top-p` -> `top_p`
  - `max_tokens` -> `num_predict`
- app tests for capability advertisement:
  - healthy Ollama backend advertises only `5050`
  - unhealthy backend advertises nothing
- app tests for result publication:
  - `6050.content` is generated text, not JSON envelope
- app tests for rejection cases:
  - missing model
  - missing prompt
  - unsupported MIME
  - full concurrency

Relevant repo gates after code implementation:

- `scripts/lint/workspace-dependency-drift-check.sh`
- `scripts/lint/ownership-boundary-check.sh`
- `scripts/lint/touched-clippy-gate.sh`
- repo-managed test coverage already called out in `AGENTS.md` where applicable

## Upstream Issue Watchlist

These issues should inform implementation choices, but they do not block the first cut:

- OpenAgents:
  - `#2982`
  - `#2983`
  - `#2990`
  - `#2998`
- Ollama:
  - `#7758` queue limits are not sufficient admission control
  - `#14407` reload/concurrency deadlock risk
  - `#14116` context-length and VRAM pressure risk
  - `#9174` keep-alive policy caveats
  - `#11660` lack of richer per-model server-side policy
  - structured-output/thinking issues such as `#10538`, `#11691`, `#14196`, `#14288`

## Bottom Line

The path is clear:

- Start with `5050` only.
- Serve it with local Ollama via `/api/generate`.
- Fix the shared `prompt` parsing bug.
- Stop advertising unsupported kinds.
- Publish text results as text.
- Keep provider truth grounded in wallet-confirmed settlement and explicit local receipts.

If those changes are made, OpenAgents can truthfully let a user with Ollama installed sell local text-generation compute over NIP-90 without drifting outside the MVP and kernel constraints documented in this repo.

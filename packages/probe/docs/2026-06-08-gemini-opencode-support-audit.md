# Probe Gemini / Opencode Support Audit

Date: 2026-06-08

## Summary

Probe does not currently have a Gemini backend, a provider-neutral LLM request
contract, or direct API-key inference. Its live backend surface is the
Apple Foundation Models bridge, plus OpenAgents product surface-managed ChatGPT Codex account
materialization for runner assignments. Probe can already model backend
capability, receipts, redacted provider evidence, and a backend-specific tool
callback loop, but those pieces are not enough for standard Gemini tool calls.

The local Opencode reference at `../projects/repos/opencode` has the missing
pieces in a shape that can be ported quickly:

- `packages/llm/src/providers/google.ts` resolves a direct `apiKey`, falls
  back to `GOOGLE_GENERATIVE_AI_API_KEY`, and applies it as the
  `x-goog-api-key` header.
- `packages/llm/src/protocols/gemini.ts` lowers a provider-neutral request into
  Gemini `streamGenerateContent` bodies, including system text, multimodal user
  parts, assistant tool-call history, tool-result history, generation options,
  Gemini thinking options, and native function declarations.
- `packages/llm/src/protocols/utils/gemini-tool-schema.ts` sanitizes common
  JSON Schema shapes that Gemini rejects before projecting them into Gemini's
  tool schema dialect.
- `packages/llm/src/schema/*`, `packages/llm/src/tool.ts`,
  `packages/llm/src/tool-runtime.ts`, and `packages/llm/src/route/*` provide the
  standard request, event, auth, route, streaming, and tool dispatch contracts.
- `packages/llm/test/provider/gemini.test.ts` and
  `packages/llm/test/provider/gemini-cache.recorded.test.ts` pin the important
  request-lowering, tool, reasoning, usage, cache, and auth behaviors.

The fastest path is not to wire the whole Opencode application into Probe. It
is to port the focused `@opencode-ai/llm` concepts into a Probe-owned LLM
backend layer, then register a `gemini_api` backend profile next to
`apple_fm_bridge`.

## What Probe Has Now

### Backend Registry

Probe's backend registry is currently single-backend:

- `packages/runtime/src/backends/backend-profile.ts`
- `packages/runtime/src/backends/registry.ts`
- `packages/runtime/src/backends/apple-fm/contract.ts`

`ProbeBackendKind` is only `apple_fm_bridge`. `ProbeBackendAuthMode` is only
`none`. `ProbeBackendStreamMode` is only `snapshot`. `DEFAULT_BACKEND_PROFILES`
contains only `APPLE_FM_LOCAL_PROFILE`.

That means there is no place today to say:

- backend kind is `gemini_api`;
- auth mode is `api_key`;
- secret source is env or OpenAgents product surface materialization;
- stream mode is SSE provider events rather than Apple FM snapshots;
- model id is `gemini-3.5-flash`, `gemini-3.5-pro`, or another Gemini model.

### Apple FM Client

`packages/runtime/src/backends/apple-fm/client.ts` owns Probe's only inference
client. It can:

- check `/health`;
- call `/v1/chat/completions`;
- stream Apple FM snapshot events;
- run `streamSessionWithTools` by starting a local callback server and passing a
  callback descriptor into the Apple FM bridge prompt.

This is useful as a local bridge pattern, but it is not provider-neutral. It is
hardwired to Apple FM request shape, failure receipts, and transcript receipts.
Gemini should not reuse the callback URL protocol for normal tool calls because
Gemini already supports native function declarations and function-response
turns.

### Tool Callback Loop

`packages/runtime/src/backends/apple-fm/tools.ts` defines
`AppleFmToolDefinition`, `AppleFmToolCallbackSession`, callback receipts, and a
small HTTP server. It supports bounded tools:

- `read_file`
- `list_files`
- `code_search`
- `shell`
- `apply_patch`
- `consult_oracle`
- `analyze_repository`
- `propose_action_submission`

This gives Probe an owned tool policy vocabulary, but only inside the
Apple-FM-specific callback model. The call/result transcript is not the same as
standard provider tool-call events:

- Apple FM emits callback HTTP requests into Probe.
- Gemini emits `functionCall` parts in the model stream.
- Probe must execute the tool locally, then continue Gemini with a
  `functionResponse` part.

### Auth And Provider Account Materialization

Probe already has careful auth handling for ChatGPT Codex/OpenAgents product surface:

- `packages/runtime/src/contracts/provider-account.ts`
- `packages/runtime/src/contracts/assignment.ts`
- `packages/runtime/src/openagents/grant-client.ts`
- `packages/runtime/src/auth/materializer.ts`
- `packages/runtime/src/runner/identity.ts`

That machinery is provider-account oriented and currently specialized around
`chatgpt_codex` materialization. The materializer can put secret content into a
file or env var and scrub it afterward, while receipts remain redacted.

For Gemini ASAP support, Probe can start with local environment resolution
without touching OpenAgents product surface:

- `GOOGLE_GENERATIVE_AI_API_KEY`
- optionally `GEMINI_API_KEY` as a compatibility fallback

Later, OpenAgents product surface/provider-account support can add a `google_gemini` provider and
materialize an API key into `GOOGLE_GENERATIVE_AI_API_KEY`, using the same
redaction and runner authorization rules.

### Public Projection / Evidence Policy

Probe's Blueprint contracts already reject raw provider credentials and raw
provider payloads in public projections. That matters for Gemini support:

- request/response fixtures must not include real API keys;
- raw Gemini payloads should be fixtures or redacted evidence, not public
  runtime receipts;
- usage metadata can be preserved under a provider metadata key when sanitized.

No invariant ledger update is required for this audit because no production
contract is changed here. The eventual implementation will likely change the
backend contract and should update docs/tests alongside that change.

## What Opencode Has

### Provider Facade And API-Key Resolution

Opencode's Gemini provider is small and directly reusable as a pattern:

- `packages/llm/src/providers/google.ts`

It exports `configure(input).model(modelID)`. Its auth precedence is:

1. explicit `auth` override;
2. explicit `apiKey`;
3. `GOOGLE_GENERATIVE_AI_API_KEY`;
4. missing credential error.

The key is applied as `x-goog-api-key`. It also accepts an optional `baseURL`.

For Probe, the equivalent should be:

- `makeGeminiClient({ apiKey?, env?, baseUrl?, model? })`;
- `apiKey` explicit option first;
- `GOOGLE_GENERATIVE_AI_API_KEY` env second;
- `GEMINI_API_KEY` env third if we want compatibility with common local setups;
- redacted receipt fields only.

### Provider-Neutral LLM Request Contract

Opencode's LLM package separates model requests from providers:

- `packages/llm/src/llm.ts`
- `packages/llm/src/schema/messages.ts`
- `packages/llm/src/schema/events.ts`
- `packages/llm/src/schema/options.ts`
- `packages/llm/src/schema/errors.ts`

The contract includes:

- `system`;
- chronological `messages`;
- `prompt`;
- text, media, reasoning, tool-call, and tool-result parts;
- tool definitions and tool choice;
- generation options;
- HTTP/provider options;
- normalized stream events such as `text-delta`, `reasoning-delta`,
  `tool-call`, `tool-result`, `step-finish`, and `finish`;
- usage with inclusive input/output totals and separate cache/reasoning
  breakdowns.

Probe does not need every field on day one, but it needs the shape. Without a
provider-neutral event stream, Gemini support will become another isolated
backend like Apple FM and will not compose with future OpenAI-compatible,
Qwen/Psionic, or Pylon routes.

### Gemini Request Lowering

Opencode's Gemini protocol:

- uses `https://generativelanguage.googleapis.com/v1beta`;
- calls `/models/{model}:streamGenerateContent?alt=sse`;
- converts system parts into `systemInstruction`;
- maps user messages to Gemini `contents` with role `user`;
- maps assistant text/reasoning/tool calls to role `model`;
- maps tool results to role `user` with `functionResponse`;
- maps generation options to `generationConfig`;
- maps provider-specific `gemini.thinkingConfig` into Gemini thinking config;
- emits Gemini `tools.functionDeclarations`;
- maps tool choice to `functionCallingConfig` modes:
  - auto -> `AUTO`;
  - none -> `NONE`;
  - required -> `ANY`;
  - named tool -> `ANY` with `allowedFunctionNames`.

This is the most important code to port because it avoids prompt-based
tool-calling hacks and speaks Gemini's native function-call protocol.

### Gemini Tool Schema Conversion

Gemini rejects several JSON Schema shapes that ordinary tool schemas often
contain. Opencode isolates that in
`packages/llm/src/protocols/utils/gemini-tool-schema.ts`.

The sanitizer/projector handles:

- integer or number enums by converting enum values to strings;
- dangling `required` fields by removing names that are not in `properties`;
- arrays without `items` by adding an item schema;
- scalar schemas with stray `properties` or `required` by dropping those keys;
- `const` by projecting to an enum;
- nullable union types by projecting `nullable: true`;
- dropping unsupported JSON Schema keys such as `$ref` and
  `additionalProperties`.

Probe should port this before exposing `apply_patch`, `shell`, or MCP-style
dynamic tools to Gemini. Otherwise the first real tool menu is likely to fail
at request time.

### Gemini Stream Parsing

Opencode parses Gemini SSE JSON events into normalized events. Important
behaviors:

- text parts become text lifecycle events;
- `thought: true` parts become reasoning events;
- `thoughtSignature` is preserved under provider metadata for continuation;
- `functionCall` parts become normalized tool-call events;
- `finishReason` maps into shared finish reasons;
- `usageMetadata` becomes normalized usage;
- Gemini `candidatesTokenCount` is visible output, so it is summed with
  `thoughtsTokenCount` to produce inclusive `outputTokens`;
- `cachedContentTokenCount` is surfaced as cache-read input tokens.

Probe should not start with a blind `response.text()` client. Tool calls and
reasoning signatures are stream events, and usage accounting is provider
specific.

### Tool Runtime

Opencode's `packages/llm/src/tool.ts` and
`packages/llm/src/tool-runtime.ts` provide the missing local execution bridge:

- define named tools with JSON Schema or Effect Schema input;
- decode model tool input;
- execute a local handler;
- encode the result;
- project structured/content output back into provider-neutral tool results;
- produce tool-result or tool-error events.

Probe can keep its existing Apple FM policy statuses, but Gemini needs this
standard dispatch loop:

1. send request with native Gemini tools;
2. parse `functionCall` events;
3. dispatch matching Probe tools locally;
4. append assistant tool-call and user tool-result history;
5. call Gemini again until finish, denial, failure, or round-trip limit.

### Tests And Fixtures

Opencode has the test cases Probe should copy conceptually:

- request target and body preparation;
- chronological system update lowering;
- multimodal input and tool history;
- tool omission for `toolChoice: none`;
- Gemini tool schema sanitization;
- text/reasoning/usage stream parsing;
- thought signature preservation;
- malformed or unsupported provider output;
- recorded Gemini cache behavior keyed by `GOOGLE_GENERATIVE_AI_API_KEY`.

Probe should add fixture tests first, then one opt-in recorded/live smoke that
requires a real key.

## Gap Analysis

### Missing In Probe

1. Provider-neutral LLM request, message, event, usage, and tool-call schema.
2. Gemini backend profile kind and API-key auth mode.
3. Gemini API-key resolver with Opencode-compatible env behavior.
4. Gemini request lowering for text/system/tool/tool-result history.
5. Gemini tool schema sanitizer.
6. Gemini SSE parser.
7. Provider-neutral tool dispatch and continuation loop.
8. CLI/runtime entrypoint to select Gemini by profile/model.
9. Tests for request lowering, stream parsing, tool schema conversion, auth
   resolution, and tool-round-trip behavior.
10. Redacted receipts/evidence for Gemini inference and tool calls.

### Partially Present In Probe

1. Backend capability reporting can grow to include Gemini.
2. Apple FM receipts provide a pattern for availability, transcript, and failure
   receipts.
3. Apple FM tool definitions provide a starter tool menu and policy vocabulary.
4. Auth materialization already supports secret-to-env patterns, but it is
   ChatGPT Codex specific today.
5. Public projection validators already defend against raw credentials and raw
   provider payload leakage.

### Should Not Be Ported Wholesale

1. Opencode's full app/provider catalog/UI.
2. Opencode's complete multi-provider router before Gemini works.
3. Opencode's generated SDK/openapi surface.
4. Opencode's provider account or console product code.
5. Lockfile-level `@ai-sdk/google` dependency choices. The reusable code here
   is Opencode's protocol/router package, not the AI SDK adapter.

## ASAP Implementation Plan

### Milestone 1: Minimal Probe LLM Core

Add a small Probe-owned provider-neutral layer under
`packages/runtime/src/llm/`:

- `messages.ts`: system, user, assistant, tool messages and content parts.
- `events.ts`: text, reasoning, tool-call, tool-result, provider-error,
  step-finish, finish.
- `usage.ts`: normalized token accounting with provider metadata.
- `tool.ts`: named tool definition and JSON Schema input.
- `tool-runtime.ts`: decode/execute/project local tool results.
- `request.ts`: model id, provider id, system/messages/tools/toolChoice,
  generation options, provider options.

Keep the first version smaller than Opencode, but preserve the same semantics
where Gemini depends on them.

### Milestone 2: Gemini Protocol Backend

Add `packages/runtime/src/backends/gemini/`:

- `contract.ts`
- `auth.ts`
- `client.ts`
- `protocol.ts`
- `tool-schema.ts`
- `receipts.ts`

Register:

- `ProbeBackendKind`: add `gemini_api`;
- `ProbeBackendAuthMode`: add `api_key`;
- `ProbeBackendStreamMode`: add `sse`;
- default base URL:
  `https://generativelanguage.googleapis.com/v1beta`;
- default model: start with `gemini-3.5-flash` for speed and cost, while
  allowing explicit model override.

Auth resolution should match Opencode:

1. explicit API key option;
2. `GOOGLE_GENERATIVE_AI_API_KEY`;
3. `GEMINI_API_KEY` compatibility fallback;
4. fail with a typed missing-credential error.

Do not store the key in receipts. Receipts should only record source labels such
as `explicit`, `GOOGLE_GENERATIVE_AI_API_KEY`, or `GEMINI_API_KEY`.

### Milestone 3: Native Tool Loop

Build `completeWithTools` for Gemini:

1. Lower current transcript into Gemini `contents`.
2. Include tool declarations after schema sanitation.
3. Stream Gemini response.
4. Collect text/reasoning/tool-call events.
5. If tool calls exist, dispatch them through Probe's local tool runtime.
6. Append assistant tool-call parts and user tool-result parts.
7. Repeat until a non-tool finish or `maxModelRoundTrips`.

Use existing Apple FM tool policies as the source policy layer, but record
Gemini-native tool-call receipts separately. Tool refusal and approval-required
states should become tool result/error events that are safe to feed back to the
model.

### Milestone 4: Tests

Add fixture tests before live tests:

- Gemini auth source precedence.
- Gemini body preparation for plain prompt.
- Gemini body preparation for tool declarations.
- Gemini body preparation for assistant tool-call and tool-result history.
- Tool schema sanitation.
- SSE text/reasoning/tool-call/usage parsing.
- End-to-end fake Gemini tool round trip.
- Missing API key failure does not print or persist secrets.

Then add opt-in live smoke:

- skipped unless `GOOGLE_GENERATIVE_AI_API_KEY` or `GEMINI_API_KEY` is set;
- one plain prompt against `gemini-3.5-flash`;
- one forced tiny tool call;
- redacted output only.

### Milestone 5: CLI / Runner Wiring

Expose a narrow path first:

- `probe backend gemini smoke`
- `probe backend gemini complete --model gemini-3.5-flash`
- optional `PROBE_BACKEND_PROFILE=gemini-api`

Then wire assignments:

- allow assignments to select `backendProfileId: gemini-api`;
- leave OpenAgents product surface provider-account grant support for a later `google_gemini`
  provider unless the product needs remote managed keys immediately.

## Recommended First Patch Set

For the first implementation PR/commit after this audit, keep scope tight:

1. Add Probe LLM core schemas and tests.
2. Add Gemini auth and protocol lowering with tests.
3. Add Gemini SSE parser with fixture tests.
4. Add Gemini client plain streaming smoke with env-key resolution.
5. Add standard tool-loop fixture test with a fake fetch.

Do not start by changing OpenAgents product surface or broad benchmark routing. Gemini must work
locally with an API key first, using Opencode's env convention.

## Source Map

Probe files reviewed:

- `packages/runtime/src/backends/backend-profile.ts`
- `packages/runtime/src/backends/registry.ts`
- `packages/runtime/src/backends/apple-fm/client.ts`
- `packages/runtime/src/backends/apple-fm/tools.ts`
- `packages/runtime/src/auth/materializer.ts`
- `packages/runtime/src/contracts/assignment.ts`
- `packages/runtime/src/contracts/provider-account.ts`
- `packages/runtime/tests/apple-fm-streaming.test.ts`
- `packages/runtime/tests/apple-fm-tools.test.ts`
- `packages/runtime/tests/materializer.test.ts`

Opencode files reviewed:

- `../projects/repos/opencode/packages/llm/src/providers/google.ts`
- `../projects/repos/opencode/packages/llm/src/protocols/gemini.ts`
- `../projects/repos/opencode/packages/llm/src/protocols/utils/gemini-tool-schema.ts`
- `../projects/repos/opencode/packages/llm/src/llm.ts`
- `../projects/repos/opencode/packages/llm/src/provider.ts`
- `../projects/repos/opencode/packages/llm/src/route/auth.ts`
- `../projects/repos/opencode/packages/llm/src/route/auth-options.ts`
- `../projects/repos/opencode/packages/llm/src/schema/messages.ts`
- `../projects/repos/opencode/packages/llm/src/schema/events.ts`
- `../projects/repos/opencode/packages/llm/src/schema/options.ts`
- `../projects/repos/opencode/packages/llm/src/tool.ts`
- `../projects/repos/opencode/packages/llm/src/tool-runtime.ts`
- `../projects/repos/opencode/packages/llm/test/provider/gemini.test.ts`
- `../projects/repos/opencode/packages/llm/test/provider/gemini-cache.recorded.test.ts`

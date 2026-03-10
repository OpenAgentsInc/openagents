# Apple FM API Coverage Matrix

Status: `FM-1`, `FM-2`, `FM-3`, `FM-4`, `FM-5`, `FM-6`, `FM-7`, and `FM-8` landed matrix, updated 2026-03-10 from the retained
Apple FM audit plus a direct scan of `~/code/python-apple-fm-sdk`, after
moving the current bridge contract and reusable client into
`psionic-apple-fm`, after landing typed system-model availability, use-case,
and guardrail coverage, after landing explicit session handles and
transcript-backed restore via raw transcript JSON, and after landing typed
generation-options coverage plus truthful estimated usage detail, and after
landing SSE session streaming with snapshot semantics, and after landing typed
transcript export/import and `from_transcript`-style restore coverage, and
after landing structured generation with schema-guided typed decode plus a real
live-bridge receipt, and after landing tool calling with a reusable Rust tool
trait, loopback callback transport, and a real multi-tool live receipt.

This is the living coverage matrix for the Psionic Apple Foundation Models
lane. It maps the exported Python SDK surface and its major behavioral families
to the Rust/Psionic roadmap.

Legend:

- `landed` = implemented on `main`
- `planned` = explicitly queued in `crates/psionic/docs/ROADMAP_FM.md`
- `n/a` = truly not applicable in Rust; avoid using this unless the difference
  is real and documented

## Public Surface

| Python SDK surface | Rust / Psionic target | Status | Roadmap issue | Notes |
| --- | --- | --- | --- | --- |
| `SystemLanguageModel` | `psionic-apple-fm::AppleFmSystemLanguageModel` | landed | `FM-2` / `#3347` | Reusable Rust wrapper now carries model id, use-case, and guardrails. |
| `SystemLanguageModelUseCase` | `psionic-apple-fm::AppleFmSystemLanguageModelUseCase` | landed | `FM-2` / `#3347` | Includes typed bridge decode plus unknown-value fallback. |
| `SystemLanguageModelGuardrails` | `psionic-apple-fm::AppleFmSystemLanguageModelGuardrails` | landed | `FM-2` / `#3347` | Includes typed bridge decode plus unknown-value fallback. |
| `SystemLanguageModelUnavailableReason` | `psionic-apple-fm::AppleFmSystemLanguageModelUnavailableReason` | landed | `FM-2` / `#3347` | Bridge and desktop now carry typed reason codes instead of only strings. |
| `LanguageModelSession` | `psionic-apple-fm::AppleFmSession` + session client APIs | landed | `FM-3` / `#3348` | Reusable bridge client now creates, inspects, resets, deletes, and responds through explicit session IDs. |
| `Transcript` | `psionic-apple-fm::AppleFmTranscript` + transcript export/import helpers | landed | `FM-6` / `#3351` | Typed transcript envelope, entry/content preservation, export, import, and restore are now landed. |
| `GenerationOptions` | `psionic-apple-fm::AppleFmGenerationOptions` | landed | `FM-4` / `#3349` | Includes local validation for non-negative temperature and positive maximum response tokens. |
| `SamplingMode` | `psionic-apple-fm::AppleFmSamplingMode` | landed | `FM-4` / `#3349` | Includes local validation for greedy-vs-random semantics and `top` versus `probability_threshold`. |
| `SamplingModeType` | `psionic-apple-fm::AppleFmSamplingModeType` | landed | `FM-4` / `#3349` | Typed greedy/random discriminator now exposed in reusable Rust code. |
| `GenerationSchema` | `psionic-apple-fm::AppleFmGenerationSchema` | landed | `FM-7` / `#3352` | Supports raw JSON input plus `schemars`-derived typed schema generation. |
| `GeneratedContent` | `psionic-apple-fm::AppleFmGeneratedContent` | landed | `FM-7` / `#3352` | Supports JSON export, typed decode, and per-property decode helpers. |
| `GenerationID` | `psionic-apple-fm::AppleFmGenerationId` | landed | `FM-7` / `#3352` | Bridge payloads and Rust generated content now carry stable generation IDs. |
| `Generable` | `psionic-apple-fm::AppleFmStructuredType` + `schemars::JsonSchema` | landed | `FM-7` / `#3352` | Rust maps typed structured generation to `JsonSchema`-derived types instead of Python decorators. |
| `GenerationGuide` / `guide` | `schemars` constraint annotations in typed Rust schemas | landed | `FM-7` / `#3352` | Enum/choice, numeric-range, list-count, and regex guidance now map through schema metadata instead of prompt hacks. |
| `generable` | Rust-native typed-schema derive path | landed | `FM-7` / `#3352` | Syntax differs from Python, but the typed-schema semantics are now covered. |
| `Tool` | `psionic-apple-fm::AppleFmTool` + typed tool definitions | landed | `FM-8` / `#3353` | Rust now registers typed tool definitions and executes real Apple FM tool callbacks through the reusable client runtime. |
| `ToolCallError` | `psionic-apple-fm::AppleFmToolCallError` | landed | `FM-8` / `#3353` | Explicit tool-call failures now carry typed tool name + underlying error detail. |
| Foundation Models error family | Rust typed error hierarchy | planned | `FM-9` / `#3354` | Current retained lane collapses most failures into strings. |

## Bridge Contract Ownership

| Contract family | Rust / Psionic owner | Status | Roadmap issue | Notes |
| --- | --- | --- | --- | --- |
| `/health` response shape | `psionic-apple-fm::contract::AppleFmHealthResponse` | landed | `FM-1` / `#3346` | Reusable typed contract moved out of desktop app code. |
| `/v1/models` response shape | `psionic-apple-fm::contract::AppleFmModelsResponse` | landed | `FM-1` / `#3346` | Includes typed model-list envelope + entry types. |
| `/v1/chat/completions` request shape | `psionic-apple-fm::contract::AppleFmChatCompletionRequest` | landed | `FM-4` / `#3349` | Now carries typed Apple FM generation options in addition to the compatibility fields. |
| `/v1/chat/completions` response shape | `psionic-apple-fm::contract::AppleFmChatCompletionResponse` | landed | `FM-1` / `#3346` | Includes choice/message/usage shapes. |
| Reusable current bridge client | `psionic-apple-fm::AppleFmBridgeClient` | landed | `FM-1` / `#3346` | Desktop now consumes the shared client instead of owning the transport types. |
| Typed system-model availability/configuration contract | `psionic-apple-fm::AppleFmSystemLanguageModelAvailability` | landed | `FM-2` / `#3347` | Health payload now reconstructs typed availability + configuration truth. |
| Typed model listing config/availability fields | `psionic-apple-fm::AppleFmModelInfo` | landed | `FM-2` / `#3347` | `/v1/models` now carries typed use-case/guardrail fields and availability detail. |
| Session create/get/delete/reset/respond contract | `psionic-apple-fm::{AppleFmSession*, AppleFmBridgeClient}` | landed | `FM-3` / `#3348` | Bridge now owns explicit session IDs instead of one hidden shared `LanguageModelSession`. |
| Session-aware bridge protocol | Reusable bridge/session contract | landed | `FM-3` / `#3348` | The bridge now uses explicit session handles; the old one-shot path remains only as a compatibility wrapper. |
| Plain-text generation request/response | `psionic-apple-fm::{AppleFmTextGenerationRequest, AppleFmTextGenerationResponse}` | landed | `FM-4` / `#3349` | Reusable Rust lane now exposes first-class text generation without forcing callers through the OpenAI chat envelope. |
| Generation-options bridge protocol | Reusable typed options contract | landed | `FM-4` / `#3349` | Chat and session-response endpoints now carry typed options and validate them before execution. |
| Streaming bridge protocol | `psionic-apple-fm::{AppleFmAsyncBridgeClient, AppleFmTextResponseStream}` + SSE session stream contract | landed | `FM-5` / `#3350` | Session streaming now uses `/v1/sessions/{id}/responses/stream` with snapshot events and terminal completion payloads. |
| Transcript bridge protocol | `psionic-apple-fm::{AppleFmTranscript, AppleFmBridgeClient}` + `/v1/sessions/{id}/transcript` | landed | `FM-6` / `#3351` | Bridge now exports typed transcripts and accepts either typed transcript objects or raw transcript JSON on restore. |
| Structured-generation bridge protocol | `psionic-apple-fm::{AppleFmStructuredGenerationRequest, AppleFmSessionStructuredGenerationRequest}` + `/v1/sessions/{id}/responses/structured` | landed | `FM-7` / `#3352` | The bridge now exposes a real structured-generation route backed by Apple FM schema responses. |
| Tool-calling bridge protocol | `AppleFmToolDefinition` + session loopback callback contract | landed | `FM-8` / `#3353` | The bridge now builds real Apple FM `Tool` objects and calls back into the Rust-side runtime for tool execution. |

## Behavioral Contract

| Behavior family | Status | Roadmap issue | Notes |
| --- | --- | --- | --- |
| Current minimal bridge contract captured in reusable Rust types and client | landed | `FM-1` / `#3346` | The desktop no longer owns the transport contract types for the current retained endpoints. |
| Typed model availability/use-case/guardrail truth | landed | `FM-2` / `#3347` | Desktop/provider runtime now carries typed system-model status instead of relying only on free-form strings. |
| Session-scoped request serialization semantics | landed | `FM-3` / `#3348` | Each session now has an explicit in-flight contract and rejects overlapping same-session requests with `concurrent_requests`; separate sessions remain independent. |
| Reset-after-failure semantics | landed | `FM-3` / `#3348` | Sessions expose a reusable reset hook without clearing transcript history; typed cancellation transport is still future work. |
| Plain-text generation with typed options | landed | `FM-4` / `#3349` | One-shot and sessioned responses now honor typed temperature, sampling, and maximum-response-token options. |
| Unsupported stream flag fails explicitly | landed | `FM-4` / `#3349` | The compatibility chat endpoint now rejects `stream: true` instead of silently ignoring it before `FM-5`. |
| Usage truth distinguishes exact from estimated | landed | `FM-4` / `#3349` | Estimated bridge counts now live in usage detail with `truth: estimated`; raw exact counts remain unset when the bridge cannot report them truthfully. |
| Streaming snapshot semantics | landed | `FM-5` / `#3350` | The stream yields full response snapshots, not deltas, and terminal completion includes final session state plus usage detail. |
| Transcript update timing | landed | `FM-5` / `#3350` | Session transcript snapshots stay stable while a stream is in flight and update only after successful completion. |
| Raw transcript-backed restore semantics | landed | `FM-3` / `#3348` | Sessions can still be recreated from bridge transcript JSON for compatibility. |
| Typed transcript export/import and restore semantics | landed | `FM-6` / `#3351` | The Rust lane now exports typed transcript snapshots, restores from typed transcript objects or raw transcript JSON, and preserves the rule that transcript tool history does not enable new tools by itself. |
| Typed, structured-generation behavior | landed | `FM-7` / `#3352` | Structured generation now uses the Apple FM schema path, with nested/list coverage and a real ignored live-bridge receipt. |
| Real tool-calling flow | landed | `FM-8` / `#3353` | Tool-enabled sessions now use a session-aware callback contract, with unit coverage and a real ignored multi-tool live receipt. |
| Typed error mapping | planned | `FM-9` / `#3354` | Must replace generic string failures. |
| Desktop/macOS Mission Control Apple FM truth | planned | `FM-10` / `#3355` | Mission Control is still GPT-OSS-first on `main`. |

## FM-1 Through FM-8 Landed Scope

The following is explicitly landed by `FM-1` through `FM-8` and should remain the
starting point for later issues:

- `crates/psionic/psionic-apple-fm` exists as the reusable crate for the Apple
  FM bridge contract and client
- the current bridge request/response types are reusable Psionic-owned types
- the current bridge has a reusable blocking client in Psionic
- the desktop Apple FM worker uses those shared types instead of owning its own
  transport contract
- the reusable crate now exposes typed system-model enums for use case,
  guardrails, and unavailable reason
- bridge health/model listing responses now carry typed system-model
  configuration and availability truth
- desktop/provider runtime state now carries typed Apple FM system-model
  readiness fields instead of only free-form availability text
- the bridge now owns explicit Apple FM session IDs and session lifecycle
  endpoints for create/get/respond/reset/delete
- session restore from raw transcript JSON is now real in the bridge and client
- the hidden shared Swift `LanguageModelSession` assumption is gone from the
  bridge session path
- the reusable crate now exposes typed generation-options and sampling-mode
  surface area with local validation mirroring the Python SDK rules
- the bridge now carries typed generation options for both one-shot and
  sessioned text generation
- the reusable crate now exposes first-class plain-text generation request and
  response types
- usage detail can now mark counts as `exact` versus `estimated`, and the
  bridge currently marks its derived counts as `estimated`
- the bridge now exposes a session-first SSE streaming contract with snapshot
  events and terminal completion events
- the reusable crate now exposes an async Apple FM streaming client and stream
  item types for that session-first streaming lane
- same-session stream cancellation now restores the session so a follow-up
  request can succeed without manual repair
- transcript snapshots returned by session inspection stay stable during
  in-flight streaming and update only after successful completion
- the reusable crate now exposes a typed `AppleFmTranscript` model for
  Foundation Models transcript snapshots
- the bridge now exposes explicit typed transcript export at
  `/v1/sessions/{id}/transcript`
- session create now accepts typed `transcript` dictionary payloads in addition
  to raw `transcript_json` compatibility input
- the reusable client now supports typed transcript export plus
  `from_transcript`-style session restore helpers
- invalid transcript input now fails explicitly and transcript/history tool
  mentions remain historical unless tools are supplied again
- the reusable crate now exposes structured-generation schema/content/id types
  plus Rust-native typed structured-generation support
- the bridge now exposes `/v1/sessions/{id}/responses/structured` for real
  schema-guided generation via Apple's structured-generation path
- the reusable client now supports typed, explicit-schema, and raw JSON-schema
  structured generation plus typed decode helpers
- structured-generation coverage now includes nested objects, lists,
  validation failures, and an ignored real live-bridge receipt on macOS
- the reusable crate now exposes a Rust `AppleFmTool` trait, typed tool
  definitions, tool-call callback request/response types, and typed
  `AppleFmToolCallError`
- the bridge now constructs real Apple FM tools from Rust-provided definitions
  and calls back into a loopback Rust-side tool runtime for execution
- tool coverage now includes direct invocation, typed registration, complex
  argument payloads, explicit failure mapping, session registration behavior,
  and an ignored real multi-tool live receipt on macOS

What is intentionally **not** closed by `FM-1` through `FM-8`:

- typed error taxonomy
- desktop Mission Control cutover

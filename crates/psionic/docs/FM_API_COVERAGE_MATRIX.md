# Apple FM API Coverage Matrix

Status: `FM-1`, `FM-2`, and `FM-3` landed matrix, updated 2026-03-10 from the retained
Apple FM audit plus a direct scan of `~/code/python-apple-fm-sdk`, after
moving the current bridge contract and reusable client into
`psionic-apple-fm`, after landing typed system-model availability, use-case,
and guardrail coverage, and after landing explicit session handles and
transcript-backed restore via raw transcript JSON.

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
| `Transcript` | Reusable transcript type + bridge coverage | planned | `FM-6` / `#3351` | No transcript bridge support on `main` yet. |
| `GenerationOptions` | Rust generation-options type | planned | `FM-4` / `#3349` | Current bridge accepts only minimal lossy request fields. |
| `SamplingMode` | Rust sampling-mode type | planned | `FM-4` / `#3349` | Not yet implemented. |
| `SamplingModeType` | Rust enum | planned | `FM-4` / `#3349` | Not yet implemented. |
| `GenerationSchema` | Rust schema type | planned | `FM-7` / `#3352` | Not yet implemented. |
| `GeneratedContent` | Rust structured-content type | planned | `FM-7` / `#3352` | Not yet implemented. |
| `GenerationID` | Rust generation-id type | planned | `FM-7` / `#3352` | Not yet implemented. |
| `Generable` | Rust-native structured-generation mapping | planned | `FM-7` / `#3352` | Likely Rust derive/builder instead of Python decorator syntax. |
| `GenerationGuide` / `guide` | Rust constraint surface | planned | `FM-7` / `#3352` | Needs a Rust-native equivalent, not prompt hacks. |
| `generable` | Rust-native derive or builder path | planned | `FM-7` / `#3352` | Syntax can differ from Python as long as semantics match. |
| `Tool` | Rust tool trait / callback contract | planned | `FM-8` / `#3353` | Not yet implemented. |
| Foundation Models error family | Rust typed error hierarchy | planned | `FM-9` / `#3354` | Current retained lane collapses most failures into strings. |

## Bridge Contract Ownership

| Contract family | Rust / Psionic owner | Status | Roadmap issue | Notes |
| --- | --- | --- | --- | --- |
| `/health` response shape | `psionic-apple-fm::contract::AppleFmHealthResponse` | landed | `FM-1` / `#3346` | Reusable typed contract moved out of desktop app code. |
| `/v1/models` response shape | `psionic-apple-fm::contract::AppleFmModelsResponse` | landed | `FM-1` / `#3346` | Includes typed model-list envelope + entry types. |
| `/v1/chat/completions` request shape | `psionic-apple-fm::contract::AppleFmChatCompletionRequest` | landed | `FM-1` / `#3346` | Current minimal bridge contract only. |
| `/v1/chat/completions` response shape | `psionic-apple-fm::contract::AppleFmChatCompletionResponse` | landed | `FM-1` / `#3346` | Includes choice/message/usage shapes. |
| Reusable current bridge client | `psionic-apple-fm::AppleFmBridgeClient` | landed | `FM-1` / `#3346` | Desktop now consumes the shared client instead of owning the transport types. |
| Typed system-model availability/configuration contract | `psionic-apple-fm::AppleFmSystemLanguageModelAvailability` | landed | `FM-2` / `#3347` | Health payload now reconstructs typed availability + configuration truth. |
| Typed model listing config/availability fields | `psionic-apple-fm::AppleFmModelInfo` | landed | `FM-2` / `#3347` | `/v1/models` now carries typed use-case/guardrail fields and availability detail. |
| Session create/get/delete/reset/respond contract | `psionic-apple-fm::{AppleFmSession*, AppleFmBridgeClient}` | landed | `FM-3` / `#3348` | Bridge now owns explicit session IDs instead of one hidden shared `LanguageModelSession`. |
| Session-aware bridge protocol | Reusable bridge/session contract | landed | `FM-3` / `#3348` | The bridge now uses explicit session handles; the old one-shot path remains only as a compatibility wrapper. |
| Streaming bridge protocol | Reusable stream contract | planned | `FM-5` / `#3350` | Not yet present in retained bridge. |
| Transcript bridge protocol | Reusable transcript contract | planned | `FM-6` / `#3351` | Not yet present in retained bridge. |
| Structured-generation bridge protocol | Reusable structured-generation contract | planned | `FM-7` / `#3352` | Not yet present in retained bridge. |
| Tool-calling bridge protocol | Reusable tool callback contract | planned | `FM-8` / `#3353` | Not yet present in retained bridge. |

## Behavioral Contract

| Behavior family | Status | Roadmap issue | Notes |
| --- | --- | --- | --- |
| Current minimal bridge contract captured in reusable Rust types and client | landed | `FM-1` / `#3346` | The desktop no longer owns the transport contract types for the current retained endpoints. |
| Typed model availability/use-case/guardrail truth | landed | `FM-2` / `#3347` | Desktop/provider runtime now carries typed system-model status instead of relying only on free-form strings. |
| Session-scoped request serialization semantics | landed | `FM-3` / `#3348` | Each session now has an explicit in-flight contract and rejects overlapping same-session requests with `concurrent_requests`; separate sessions remain independent. |
| Reset-after-failure semantics | landed | `FM-3` / `#3348` | Sessions expose a reusable reset hook without clearing transcript history; typed cancellation transport is still future work. |
| Streaming snapshot semantics | planned | `FM-5` / `#3350` | Python SDK yields full response-so-far snapshots, not deltas. |
| Transcript update timing | planned | `FM-5` / `#3350`, `FM-6` / `#3351` | Transcript updates after successful completion. |
| Raw transcript-backed restore semantics | landed | `FM-3` / `#3348` | Sessions can now be recreated from bridge transcript JSON; typed transcript import/export remains `FM-6`. |
| Typed, structured-generation behavior | planned | `FM-7` / `#3352` | Must not be reduced to “ask for JSON in the prompt”. |
| Real tool-calling flow | planned | `FM-8` / `#3353` | Must be session-aware, not prompt flattening. |
| Typed error mapping | planned | `FM-9` / `#3354` | Must replace generic string failures. |
| Desktop/macOS Mission Control Apple FM truth | planned | `FM-10` / `#3355` | Mission Control is still GPT-OSS-first on `main`. |

## FM-1 Through FM-3 Landed Scope

The following is explicitly landed by `FM-1` through `FM-3` and should remain the
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

What is intentionally **not** closed by `FM-1` through `FM-3`:

- streaming
- transcripts
- structured generation
- tools
- typed error taxonomy
- desktop Mission Control cutover

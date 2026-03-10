# Apple FM API Coverage Matrix

Status: `FM-1` landed matrix, updated 2026-03-10 from the retained Apple FM
audit plus a direct scan of `~/code/python-apple-fm-sdk`, after moving the
current bridge contract and reusable client into `psionic-apple-fm`.

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
| `SystemLanguageModel` | Reusable Rust Apple FM model wrapper | planned | `FM-2` / `#3347` | Current retained bridge only exposes coarse health + model-list results. |
| `SystemLanguageModelUseCase` | Rust enum | planned | `FM-2` / `#3347` | Not yet represented in reusable Rust code. |
| `SystemLanguageModelGuardrails` | Rust enum | planned | `FM-2` / `#3347` | Not yet represented in reusable Rust code. |
| `SystemLanguageModelUnavailableReason` | Rust enum | planned | `FM-2` / `#3347` | Current app path only has strings. |
| `LanguageModelSession` | Reusable Rust session handle and APIs | planned | `FM-3` / `#3348` | Current retained Swift bridge uses one shared hidden session. |
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
| Session-aware bridge protocol | Reusable bridge/session contract | planned | `FM-3` / `#3348` | Current bridge is still one-shot and hidden-session-oriented. |
| Streaming bridge protocol | Reusable stream contract | planned | `FM-5` / `#3350` | Not yet present in retained bridge. |
| Transcript bridge protocol | Reusable transcript contract | planned | `FM-6` / `#3351` | Not yet present in retained bridge. |
| Structured-generation bridge protocol | Reusable structured-generation contract | planned | `FM-7` / `#3352` | Not yet present in retained bridge. |
| Tool-calling bridge protocol | Reusable tool callback contract | planned | `FM-8` / `#3353` | Not yet present in retained bridge. |

## Behavioral Contract

| Behavior family | Status | Roadmap issue | Notes |
| --- | --- | --- | --- |
| Current minimal bridge contract captured in reusable Rust types and client | landed | `FM-1` / `#3346` | The desktop no longer owns the transport contract types for the current retained endpoints. |
| Session serialization semantics | planned | `FM-3` / `#3348` | Must match the Python SDK contract. |
| Reset-after-cancel/failure semantics | planned | `FM-3` / `#3348` | Must not erase transcript history. |
| Streaming snapshot semantics | planned | `FM-5` / `#3350` | Python SDK yields full response-so-far snapshots, not deltas. |
| Transcript update timing | planned | `FM-5` / `#3350`, `FM-6` / `#3351` | Transcript updates after successful completion. |
| Restore-from-transcript semantics | planned | `FM-6` / `#3351` | Historical tool mentions must not auto-enable new tool calls. |
| Typed, structured-generation behavior | planned | `FM-7` / `#3352` | Must not be reduced to “ask for JSON in the prompt”. |
| Real tool-calling flow | planned | `FM-8` / `#3353` | Must be session-aware, not prompt flattening. |
| Typed error mapping | planned | `FM-9` / `#3354` | Must replace generic string failures. |
| Desktop/macOS Mission Control Apple FM truth | planned | `FM-10` / `#3355` | Mission Control is still GPT-OSS-first on `main`. |

## FM-1 Landed Scope

The following is explicitly landed by `FM-1` and should remain the starting
point for later issues:

- `crates/psionic/psionic-apple-fm` exists as the reusable crate for the Apple
  FM bridge contract and client
- the current bridge request/response types are reusable Psionic-owned types
- the current bridge has a reusable blocking client in Psionic
- the desktop Apple FM worker uses those shared types instead of owning its own
  transport contract

What is intentionally **not** closed by `FM-1`:

- model availability/use-case/guardrail enums
- session handles
- streaming
- transcripts
- structured generation
- tools
- typed error taxonomy
- desktop Mission Control cutover

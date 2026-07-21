# @openagentsinc/ai-model

The L0 model-call bridge for the OpenAgents AI SDK. The package had the name
`@openagentsinc/khala-ai-sdk-core` before AISDK-03 (#9149). No release of the
old name went to npm, so no npm alias package exists.

This package keeps AI SDK Core as provider-call transport only. It calls a
`streamText`-compatible function, maps stream parts into
`openagents.khala_runtime_event.v1`, and bridges `@openagentsinc/khala-tools`
tools into AI SDK `tool()` definitions while executing through the OpenAgents
tool dispatcher.

The package does not fork AI SDK Core and does not make AI SDK stream parts the
product transcript schema.

## Effect AI substrate path (STREAM-01)

The module `src/effect-ai.ts` adds an `effect/unstable/ai` path. The path is
additive. The current public API does not change.

- `khalaEffectAiLanguageModelLayer` supplies the Effect AI `LanguageModel`
  service. The same injectable `streamText` transport does the provider call.
- `khalaAiSdkTextStreamPartFromEffectAiStreamPart` maps an Effect AI
  `Response.StreamPart` value to the ingestion vocabulary. The function
  `khalaRuntimeEventFromAiSdkTextStreamPart` stays the one projection point
  for `KhalaRuntimeEvent`.
- `runKhalaEffectAiCoreRuntime` runs one `LanguageModel.streamText` turn. It
  collects the turn into `KhalaRuntimeEvent` values.

A model-call failure on this path is a typed `AiError` value. The map from
`AiError` reasons to harness failure classes is in
`@openagentsinc/harness-conformance` (`ai-error-failure-class.ts`).

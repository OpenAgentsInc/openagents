# Model Provider Abstraction Audit

Date: 2026-06-11

This is system #15 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should select models, route providers, validate
capabilities, expose model options, handle fallbacks, and normalize streaming
responses into the runtime event shape.

## Target

Build a provider-agnostic model gateway. The agent runtime should not depend on
one provider SDK or one provider's message format as the durable domain model.

The gateway should support:

- Multiple providers.
- Model aliases.
- Provider-specific model ids.
- User, environment, settings, and session overrides.
- Entitlement-aware defaults.
- Capability lookup.
- Context-window and output-token limits.
- Validation of custom model ids.
- Fallback suggestions when a provider lags.
- Test providers and scripted streams.

## User-Visible Capability

The user should be able to:

- Pick a model by a stable alias.
- See the resolved model and provider.
- Understand when a chosen model is unavailable.
- See context-window and output-limit differences.
- Use custom provider deployments when configured.
- Switch models mid-session when allowed.
- Avoid confusing provider-specific failures with agent failures.

## Core Design

Define a `ModelGateway` service that is the only runtime boundary for model
calls.

Suggested service boundary:

```ts
interface ModelGateway {
  resolve(request: ModelResolveRequest): Effect.Effect<ModelTarget, ModelError>
  validate(target: ModelTarget): Effect.Effect<ModelValidationResult, ModelError>
  stream(request: ModelRequest): Stream.Stream<ModelStreamEvent, ModelError>
  capabilities(target: ModelTarget): Effect.Effect<ModelCapabilities, ModelError>
}
```

The conversation engine sends a normalized model request. The gateway resolves
provider, model id, options, capabilities, and stream parser.

## Resolution Priority

Model resolution should be deterministic:

1. Session override.
2. Startup flag.
3. Environment override.
4. User or project settings.
5. Agent, skill, or command override.
6. Runtime mode override.
7. Entitlement-aware default.
8. Built-in default.

Each resolution should produce both the user-facing selection and the actual
provider-facing model id.

## Provider Model

Represent providers as typed adapters:

- Provider id.
- Authentication mode.
- Base URL or deployment locator.
- Model-id mapping.
- Capability source.
- Streaming protocol parser.
- Error normalizer.
- Retry policy.
- Pricing source.
- Privacy and telemetry policy.

The provider adapter should emit runtime model events, not provider-native
payloads.

## Capability Shape

Capabilities should include:

- Max input tokens.
- Max output tokens.
- Default output tokens.
- Tool-call support.
- Parallel tool-call support.
- Vision/document support.
- Reasoning or thinking support.
- Cache support.
- Server-side tool support.
- Structured-output support.
- Region or deployment constraints.
- Pricing metadata.

Capabilities may come from static config, provider discovery, user config, or
runtime probes. The source and timestamp should be recorded.

## Alias And Custom Model Handling

Aliases should map to model families, not provider-specific ids.

Rules:

- Preserve user input for display.
- Resolve aliases through provider-aware mapping.
- Let custom deployment ids pass through only when explicitly configured.
- Validate unknown models before saving them.
- Cache validation success for the session.
- Provide fallback suggestions when an unavailable model likely means the
  provider has not caught up.
- Fail closed when entitlement or capability state is unknown.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for the model gateway.
- `Layer` for provider implementations.
- `Schema` for model targets, capabilities, usage, and stream events.
- `Stream` for provider output.
- `Schedule` for retry and backoff.
- `Cache` or memoized Effect for capability discovery and validation.
- `Redacted` or equivalent wrappers for credentials.

Effect AI can be the native model-call substrate. AI-SDK-compatible messages
can be an edge projection when useful, but should not become the durable
runtime store.

## Error Normalization

Normalize provider errors into domain errors:

- Model not found.
- Authentication failed.
- Authorization or entitlement denied.
- Rate limited.
- Provider overloaded.
- Context too large.
- Output limit reached.
- Invalid request.
- Network transient.
- Network permanent.
- Stream protocol violation.

Provider-specific bodies and headers should stay in private diagnostics unless
explicitly safe.

## Safety Rules

- Do not persist provider credentials or raw request payloads.
- Do not expose internal model codenames or private deployment ids in public
  projections.
- Do not silently downgrade models without recording the fallback.
- Do not use a model beyond its known context or output limits.
- Do not let an agent or skill override the model outside policy.
- Do not treat provider discovery failure as proof that a model is unavailable.
- Public closeouts should name broad capability class when exact model identity
  is private.

## Tests

Minimum regression coverage:

- Resolve model from every priority source.
- Map one alias across multiple providers.
- Validate a configured custom model.
- Reject an unconfigured unknown model.
- Cache validation success.
- Load capabilities from static config and discovery cache.
- Enforce context-window and output-token limits.
- Normalize model-not-found, auth, rate-limit, and network errors.
- Record fallback selection as an event.
- Run a scripted test provider that emits text, tool call, usage, and failure.

## OpenAgents Translation Notes

When promoted, map model targets to OpenAgents adapter refs, capability refs,
usage ledgers, policy refs, and public-safe model projections. Verify live
issue state before claiming any provider path is shipped.

## Decision

Provider SDKs should be replaceable adapters. The owned runtime should persist
model targets, capabilities, events, errors, and usage in its own schema, then
project provider-specific payloads only at the edge.

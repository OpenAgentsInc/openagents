// Onboarding stream-source shaping (issue #6154). Bridges the provider-adapter
// streaming seam into the onboarding program's `OnboardingStreamSource`.
//
// The onboarding stream client PREFERS the adapter's TRUE incremental `streamSse`
// so the interview reply streams token-by-token — one `event: delta` per upstream
// fragment — instead of one buffered reply materialized server-side. Adapters
// without a `streamSse` (stub/echo, simple test adapters) fall back to the
// buffered chunk `stream`, which is byte-equivalent but materialized.
//
// This mirrors the `/v1` gateway boundary: the dispatched Effect resolves once the
// upstream stream HEAD is accepted (a non-2xx surfaces as an adapter error BEFORE
// any frame is consumed), so the overflow dispatcher can fail a lane over without
// buffering the body. The lazy frames stay the single source of truth — `final()`
// returns '' for the streamSse path so nothing re-buffers the content (the route
// accumulates the deltas it emits and persists that accumulation, receipt-first).

import { Effect } from 'effect'

import type { OnboardingStreamSource } from '../autopilot-onboarding-program'
import {
  type InferenceAdapterRouteMetadata,
  type InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceStreamChunk,
  type InferenceStreamSource,
} from './provider-adapter'

const routeMetadata = (
  metadata: InferenceAdapterRouteMetadata | undefined,
): Readonly<Record<string, unknown>> | undefined =>
  metadata === undefined ? undefined : { ...metadata }

// Surface an adapter's incremental `InferenceStreamSource.frames` as the
// onboarding `deltas` async-iterable. Empty content deltas (e.g. the terminal
// usage frame) are skipped so the route only sees prose. `final()` returns ''
// so the content is never re-buffered — the route accumulates as it pumps.
export const onboardingSourceFromStreamSse = (
  source: InferenceStreamSource,
): OnboardingStreamSource => ({
  deltas: (async function* () {
    for await (const frame of source.frames) {
      if (frame.contentDelta !== '') {
        yield frame.contentDelta
      }
    }
  })(),
  final: () => '',
  metadata: () => {
    const terminal = source.terminal()
    return {
      ...(terminal.adapterRouteMetadata === undefined
        ? {}
        : { adapterRouteMetadata: routeMetadata(terminal.adapterRouteMetadata) }),
      ...(terminal.finishReason === undefined
        ? {}
        : { finishReason: terminal.finishReason }),
      ...(terminal.servedModel === undefined
        ? {}
        : { servedModel: terminal.servedModel }),
      ...(terminal.usage === undefined ? {} : { usage: terminal.usage }),
    }
  },
})

// Fallback: surface a buffered chunk array as onboarding deltas. Used only for
// adapters that do not implement `streamSse`. The content IS already materialized
// here, so `final()` can cheaply rejoin it (the route still prefers its own
// accumulation when non-empty).
export const onboardingSourceFromChunks = (
  chunks: ReadonlyArray<InferenceStreamChunk>,
): OnboardingStreamSource => ({
  deltas: (async function* () {
    for (const chunk of chunks) {
      if (chunk.contentDelta !== '') {
        yield chunk.contentDelta
      }
    }
  })(),
  final: () => chunks.map(chunk => chunk.contentDelta).join(''),
  metadata: () => {
    const terminal = [...chunks].reverse().find(chunk =>
      chunk.finishReason !== undefined ||
      chunk.usage !== undefined ||
      chunk.servedModel !== undefined ||
      chunk.adapterRouteMetadata !== undefined,
    )
    if (terminal === undefined) return undefined
    return {
      ...(terminal.adapterRouteMetadata === undefined
        ? {}
        : { adapterRouteMetadata: routeMetadata(terminal.adapterRouteMetadata) }),
      ...(terminal.finishReason === undefined
        ? {}
        : { finishReason: terminal.finishReason }),
      ...(terminal.servedModel === undefined
        ? {}
        : { servedModel: terminal.servedModel }),
      ...(terminal.usage === undefined ? {} : { usage: terminal.usage }),
    }
  },
})

// The dispatch operation handed to `dispatchWithOverflow`: prefer the adapter's
// incremental `streamSse`, fall back to the buffered `stream`. Returns an
// `OnboardingStreamSource` either way so the onboarding client is uniform.
export const dispatchOnboardingStreamSource = (
  adapter: InferenceProviderAdapter,
  request: InferenceRequest,
): Effect.Effect<OnboardingStreamSource, InferenceAdapterError> =>
  adapter.streamSse !== undefined
    ? adapter.streamSse(request).pipe(Effect.map(onboardingSourceFromStreamSse))
    : adapter.stream(request).pipe(Effect.map(onboardingSourceFromChunks))

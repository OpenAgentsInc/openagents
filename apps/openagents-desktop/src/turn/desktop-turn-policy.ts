import { Effect, Layer, Schema as S } from "effect"

import {
  CONTEXT_ENVELOPE_SCHEMA_LITERAL,
  ROUTE_DECISION_SCHEMA_LITERAL,
  RouteDecision,
  WorkContextEnvelope,
} from "@openagentsinc/agent-runtime-schema"
import {
  ActionBroker,
  ArtifactResolver,
  ContextSource,
  TurnPolicy,
} from "@openagentsinc/agent-turn-runtime"

/**
 * AFS-01 minimal Desktop host policy layers.
 *
 * These are the deterministic, owner-scoped implementations Electron main
 * composes for the first production composition. They are intentionally small:
 * richer context assembly (IDE-08), readiness filtering, and privacy/cost policy
 * land in later packets. They preserve the AFS invariants that matter now — only
 * the host creates the effective manifest, and only the host derives the route
 * decision from the owner-bound candidate set.
 */

const decodeContext = S.decodeUnknownSync(WorkContextEnvelope)
const decodeDecision = S.decodeUnknownSync(RouteDecision)

const nowIso = (): string => new Date().toISOString()

/** A minimal host context source. It binds the thread and an empty manifest. */
export const desktopContextSourceLayer: Layer.Layer<ContextSource> = Layer.succeed(
  ContextSource,
  ContextSource.of({
    manifest: (input) =>
      Effect.sync(() =>
        decodeContext({
          schema: CONTEXT_ENVELOPE_SCHEMA_LITERAL,
          manifestRef: `context.${input.threadRef}`,
          threadRef: input.threadRef,
          generation: { state: "unknown", reason: "not_observed" },
          createdAt: nowIso(),
          items: [],
          totalByteLength: 0,
          byteLimit: 0,
          truncated: false,
          redacted: false,
        }),
      ),
  }),
)

/**
 * The first-candidate host policy. It selects the first provider ref in the
 * owner-bound ordered set. It fails closed on an empty set. A recommendation
 * cannot add a candidate to the set. Readiness and privacy filtering are added
 * in a later packet.
 */
export const desktopTurnPolicyLayer: Layer.Layer<TurnPolicy> = Layer.succeed(
  TurnPolicy,
  TurnPolicy.of({
    decide: (input) =>
      Effect.sync(() => {
        const ordered = input.candidateSet.ordered
        if (ordered.length === 0) {
          return decodeDecision({
            schema: ROUTE_DECISION_SCHEMA_LITERAL,
            outcome: "closed",
            routeDecisionRef: `route.${input.requestRef}`,
            requestRef: input.requestRef,
            policyArtifactRef: input.candidateSet.policyArtifactRef,
            contextManifestRef: input.context.manifestRef,
            decisionReason: "no_candidate_fail_closed",
            dispositions: [],
            decidedAt: nowIso(),
          })
        }
        const selected = ordered[0]
        return decodeDecision({
          schema: ROUTE_DECISION_SCHEMA_LITERAL,
          outcome: "admitted",
          routeDecisionRef: `route.${input.requestRef}`,
          requestRef: input.requestRef,
          selected,
          effective: selected,
          admittedCandidateSet: ordered,
          policyArtifactRef: input.candidateSet.policyArtifactRef,
          contextManifestRef: input.context.manifestRef,
          disclosure: {
            dataDestination: "remote_provider",
            costClass: "metered_provider_tokens",
            localOnly: false,
            providerRef: selected,
          },
          decisionReason: "admitted_first_candidate",
          dispositions: [],
          decidedAt: nowIso(),
        })
      }),
  }),
)

/** Resolve nothing: no released artifact exists in the AFS-01 local path. */
export const desktopArtifactResolverLayer: Layer.Layer<ArtifactResolver> = Layer.succeed(
  ArtifactResolver,
  ArtifactResolver.of({ resolve: () => Effect.succeed(null) }),
)

/** Advisory-only broker: it performs no host action in AFS-01. */
export const desktopActionBrokerLayer: Layer.Layer<ActionBroker> = Layer.succeed(
  ActionBroker,
  ActionBroker.of({ deliver: () => Effect.void }),
)

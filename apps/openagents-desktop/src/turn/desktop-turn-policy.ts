import { Effect, Layer, Schema as S } from "effect"

import {
  CONTEXT_ENVELOPE_SCHEMA_LITERAL,
  ROUTE_DECISION_SCHEMA_LITERAL,
  RouteDecision,
  WorkContextEnvelope,
} from "@openagentsinc/agent-runtime-schema"
import {
  ArtifactResolver,
  ContextSource,
  TurnPolicy,
} from "@openagentsinc/agent-turn-runtime"

import { buildEditorWorkContext, type EditorContextRegistry } from "./editor-context-binding.ts"

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

const emptyManifest = (threadRef: string) =>
  decodeContext({
    schema: CONTEXT_ENVELOPE_SCHEMA_LITERAL,
    manifestRef: `context.${threadRef}`,
    threadRef,
    generation: { state: "unknown", reason: "not_observed" },
    createdAt: nowIso(),
    items: [],
    totalByteLength: 0,
    byteLimit: 0,
    truncated: false,
    redacted: false,
  })

/**
 * The host context source. It binds the thread and an empty manifest by default,
 * preserving the AFS-03/04 local chat path.
 *
 * AFS-05: when an editor-context registry is supplied and a bound Editor turn has
 * registered its IDE-08 context for the thread, this source builds the effective
 * `WorkContextEnvelope` from that binding — so the Editor agent rail and chat use
 * ONE turn service and one manifest authority. A binding for another project,
 * root, worktree, or generation is refused (host-owned), and the turn falls back
 * to an empty manifest rather than carrying context from another editor.
 */
export const desktopContextSourceLayer = (
  editorContext?: EditorContextRegistry,
): Layer.Layer<ContextSource> =>
  Layer.succeed(
    ContextSource,
    ContextSource.of({
      manifest: (input) =>
        Effect.sync(() => {
          const binding = editorContext?.get(input.threadRef) ?? null
          if (binding !== null) {
            const built = buildEditorWorkContext(binding, editorContext!.expectation())
            if (built.ok) return built.envelope
          }
          return emptyManifest(input.threadRef)
        }),
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

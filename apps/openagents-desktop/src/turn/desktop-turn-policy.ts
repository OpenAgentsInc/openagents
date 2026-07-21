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
 * The first-READY-candidate host policy (#9145). It selects the first provider
 * ref in the owner-bound ordered set whose MAIN-OWNED descriptor readiness is
 * `ready`, recording an honest skipped disposition for every unready lane it
 * passed over. A ref with no descriptor keeps the previous first-candidate
 * behavior (compositions that thread no descriptors are unchanged). It fails
 * closed when no candidate is ready. A recommendation cannot add a candidate to
 * the set. `admitted_first_candidate` stays honest because the choice is the
 * first candidate of the readiness-filtered ordered set.
 */
export const desktopTurnPolicyLayer: Layer.Layer<TurnPolicy> = Layer.succeed(
  TurnPolicy,
  TurnPolicy.of({
    decide: (input) =>
      Effect.sync(() => {
        const ordered = input.candidateSet.ordered
        const descriptors = input.descriptors ?? []
        const descriptorOf = (ref: string) =>
          descriptors.find((descriptor) => descriptor.providerRef === ref)
        const isReady = (ref: string): boolean => {
          const descriptor = descriptorOf(ref)
          return descriptor === undefined || descriptor.readiness.state === "ready"
        }
        // Honest skipped dispositions for every unready owner-bound lane.
        const dispositions = ordered.flatMap((ref) => {
          const descriptor = descriptorOf(ref)
          if (descriptor === undefined || descriptor.readiness.state === "ready") return []
          const reason =
            descriptor.readiness.reason === "account_missing" ||
            descriptor.readiness.reason === "account_unhealthy"
              ? "account_not_ready"
              : "resource_not_ready"
          return [
            {
              providerRef: ref,
              candidate: descriptor.candidate,
              disposition: "skipped",
              reason,
            },
          ]
        })
        const ready = ordered.filter(isReady)
        if (ready.length === 0) {
          return decodeDecision({
            schema: ROUTE_DECISION_SCHEMA_LITERAL,
            outcome: "closed",
            routeDecisionRef: `route.${input.requestRef}`,
            requestRef: input.requestRef,
            policyArtifactRef: input.candidateSet.policyArtifactRef,
            contextManifestRef: input.context.manifestRef,
            decisionReason: "no_candidate_fail_closed",
            dispositions,
            decidedAt: nowIso(),
          })
        }
        const selected = ready[0]
        const selectedDescriptor = descriptorOf(selected)
        // Honest disclosure from the selected lane's descriptor when available;
        // the previous fixed remote disclosure only remains for a descriptorless
        // composition.
        const disclosure =
          selectedDescriptor === undefined
            ? {
                dataDestination: "remote_provider",
                costClass: "metered_provider_tokens",
                localOnly: false,
                providerRef: selected,
              }
            : {
                dataDestination: selectedDescriptor.dataDestination,
                costClass: selectedDescriptor.costClass,
                localOnly: selectedDescriptor.dataDestination === "on_device_local",
                providerRef: selected,
              }
        return decodeDecision({
          schema: ROUTE_DECISION_SCHEMA_LITERAL,
          outcome: "admitted",
          routeDecisionRef: `route.${input.requestRef}`,
          requestRef: input.requestRef,
          selected,
          effective: selected,
          admittedCandidateSet: ready,
          policyArtifactRef: input.candidateSet.policyArtifactRef,
          contextManifestRef: input.context.manifestRef,
          disclosure,
          decisionReason: "admitted_first_candidate",
          dispositions,
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

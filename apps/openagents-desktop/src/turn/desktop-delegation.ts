import {
  decodeAppleFmRouteOutput,
  type AppleFmRouteDecodeResult,
} from "@openagentsinc/apple-fm-runtime"
import type {
  RouteRecommendation,
  TurnProviderCandidate,
  TurnRefusalReason,
} from "@openagentsinc/agent-runtime-schema"

import type { CodexLaneReadiness } from "./desktop-codex-provider.ts"

/**
 * AFS-04 host delegation decision.
 *
 * The Apple FM router can recommend delegating a turn to a stronger subagent.
 * This module is the HOST decision layer: it decodes the model's advisory route
 * output (fail-closed, AFS-02) and combines it with MAIN-OWNED lane readiness to
 * decide, deterministically, whether to answer locally, delegate to Codex, or
 * refuse the delegation honestly.
 *
 * The model proposes; the host acts and reports. The model's output is never the
 * evidence that a subagent ran. A recommendation for an unavailable,
 * unauthenticated, or unadmitted lane produces NO start — it degrades to an
 * honest refusal. Only a `delegate` decision for a ready lane starts one real
 * subagent turn.
 */

/** The delegate-capable candidates the Phase-1 host offers. Codex is the first. */
export const DELEGATE_CANDIDATES: ReadonlyArray<TurnProviderCandidate> = ["codex"]

export type DelegationDecision =
  | { readonly kind: "answer"; readonly text: string }
  | {
      readonly kind: "delegate"
      readonly provider: "codex"
      readonly objective: string
      readonly recommendation: RouteRecommendation
    }
  | { readonly kind: "refuse_delegation"; readonly provider: "codex"; readonly reason: TurnRefusalReason }

export interface DelegationInput {
  /** The Apple FM turn output text (the advisory router result), or null. */
  readonly answerText: string | null
  /** The bounded objective to delegate — the user's original request. */
  readonly objective: string
  /** The delegate-capable candidates the host admits. */
  readonly admittedDelegates?: ReadonlyArray<TurnProviderCandidate>
  /** MAIN-OWNED codex lane readiness (never renderer input). */
  readonly codexReadiness: CodexLaneReadiness
}

/** Map a not-ready codex readiness into the honest delegation refusal reason. */
const refusalReasonForReadiness = (readiness: CodexLaneReadiness): TurnRefusalReason => {
  switch (readiness.unavailableReason) {
    case "no_codex_account":
    case "no_verified_account":
      return "provider_unauthorized"
    case "policy_denied":
      return "provider_unadmitted"
    case "quota_exhausted":
    case "rate_limited":
    case "invalid_config":
    case "not_ready":
    case undefined:
      return "provider_unavailable"
  }
}

/**
 * Decide what the host does with the Apple FM router output. Fail-closed: any
 * output that is not a valid `Recommendation` for an admitted delegate falls
 * back to answering with the advisory text; a malformed structured route is a
 * safe answer of its own decoded text, never a dispatch.
 */
export const decideDelegation = (input: DelegationInput): DelegationDecision => {
  const answerText = input.answerText ?? ""
  const admitted = input.admittedDelegates ?? DELEGATE_CANDIDATES

  const decoded: AppleFmRouteDecodeResult = decodeAppleFmRouteOutput({
    raw: answerText,
    admittedCandidates: admitted,
  })

  if (decoded._tag !== "Recommendation") {
    // Answer or reject → answer locally with the safe advisory text.
    return { kind: "answer", text: answerText.trim() }
  }

  const candidate = decoded.recommendation.candidate
  if (candidate !== "codex") {
    // Phase 1 only wires the codex delegate lane; other lanes answer for now.
    return { kind: "answer", text: answerText.trim() }
  }

  if (!input.codexReadiness.ready) {
    // The recommended lane is not runnable: no start, honest refusal.
    return { kind: "refuse_delegation", provider: "codex", reason: refusalReasonForReadiness(input.codexReadiness) }
  }

  return {
    kind: "delegate",
    provider: "codex",
    objective: input.objective,
    recommendation: decoded.recommendation,
  }
}

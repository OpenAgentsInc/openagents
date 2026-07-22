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
import {
  decodeClaudeLocalStartRequest,
  type ClaudeLocalStartRequest,
} from "../claude-local-contract.ts"

/**
 * AFS-04 host delegation decision.
 *
 * The Apple FM router can recommend delegating a turn to a stronger subagent.
 * This module is the HOST decision layer: it decodes the model's advisory route
 * output (fail-closed, AFS-02) and combines it with MAIN-OWNED lane readiness to
 * decide, deterministically, whether to answer locally, delegate to a connected
 * coding agent, or refuse the delegation honestly.
 *
 * AFS-04 wired Codex first. Issue #9091 generalizes the SAME path to Claude Code
 * (`claude` → the claude-local lane) and Grok (`grok_acp` → the admitted ACP
 * lane), so all three behave identically: fail-closed decode, one start per
 * recommendation, advisory-only, owner-scoped, and honest.
 *
 * The model proposes; the host acts and reports. The model's output is never the
 * evidence that a subagent ran. A recommendation for an unavailable,
 * unauthenticated, or unadmitted lane produces NO start — it degrades to an
 * honest refusal. Only a `delegate` decision for a MAIN-OWNED ready lane starts
 * one real subagent turn.
 */

/** The provider candidates the host can dispatch as a real delegate subagent. */
export type DelegateProvider = "codex" | "claude" | "grok_acp"

/** The delegate-capable candidates the host offers (Codex, Claude Code, Grok). */
export const DELEGATE_CANDIDATES: ReadonlyArray<TurnProviderCandidate> = ["codex", "claude", "grok_acp"]

const DELEGATE_PROVIDER_SET: ReadonlySet<TurnProviderCandidate> = new Set(DELEGATE_CANDIDATES)

/** True when a decoded candidate is one the host can actually dispatch. */
export const isDelegateProvider = (candidate: TurnProviderCandidate): candidate is DelegateProvider =>
  DELEGATE_PROVIDER_SET.has(candidate)

export type OrdinaryDelegationExecution = Readonly<{
  request: ClaudeLocalStartRequest
  mode: Readonly<{ background: true; fullAuto: false }>
}>

/**
 * Build one ordinary delegated start without granting Full Auto authority.
 * Background means that main owns the lifecycle and question boundary. It does
 * not authorize autonomous repository work.
 */
export const makeOrdinaryDelegationExecution = (input: Readonly<{
  requestRef: string
  threadRef: string
  message: string
}>): OrdinaryDelegationExecution | null => {
  const request = decodeClaudeLocalStartRequest({
    turnRef: input.requestRef.slice(0, 120),
    threadRef: input.threadRef,
    message: input.message,
    fullAuto: false,
  })
  return request === null
    ? null
    : { request, mode: { background: true, fullAuto: false } }
}

export type DelegationDecision =
  | { readonly kind: "answer"; readonly text: string }
  | {
      readonly kind: "delegate"
      readonly provider: DelegateProvider
      readonly objective: string
      readonly recommendation: RouteRecommendation
    }
  | { readonly kind: "refuse_delegation"; readonly provider: DelegateProvider; readonly reason: TurnRefusalReason }

export interface DelegationInput {
  /** The Apple FM turn output text (the advisory router result), or null. */
  readonly answerText: string | null
  /** The bounded objective to delegate — the user's original request. */
  readonly objective: string
  /** The delegate-capable candidates the host admits. */
  readonly admittedDelegates?: ReadonlyArray<TurnProviderCandidate>
  /**
   * MAIN-OWNED per-lane readiness, keyed by delegate candidate (never renderer
   * input). A candidate with no entry — a lane the host has not wired — is
   * treated as unavailable and refused, never faked.
   */
  readonly readiness: Readonly<Partial<Record<DelegateProvider, CodexLaneReadiness>>>
}

/** Map a not-ready (or absent) lane readiness into the honest delegation refusal reason. */
const refusalReasonForReadiness = (readiness: CodexLaneReadiness | undefined): TurnRefusalReason => {
  switch (readiness?.unavailableReason) {
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
 * safe answer of its own decoded text, never a dispatch. A recommendation for a
 * delegate candidate whose MAIN-OWNED lane is not ready produces NO start and an
 * honest refusal.
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
  if (!isDelegateProvider(candidate)) {
    // Not a host-dispatchable delegate lane; answer with the advisory text.
    return { kind: "answer", text: answerText.trim() }
  }

  const readiness = input.readiness[candidate]
  if (readiness === undefined || !readiness.ready) {
    // The recommended lane is not runnable (or not wired): no start, honest refusal.
    return { kind: "refuse_delegation", provider: candidate, reason: refusalReasonForReadiness(readiness) }
  }

  return {
    kind: "delegate",
    provider: candidate,
    objective: input.objective,
    recommendation: decoded.recommendation,
  }
}

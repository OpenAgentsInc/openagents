// Cross-client decision consistency core (M5 CL-29 / issue #4935).
//
// This module is intentionally pure. It projects one exactly-once decision
// winner across a set of client-local views without doing bridge I/O.

import type { DecisionVerb, ResolveOutcome } from "@openagentsinc/autopilot-control-protocol"

export type DecisionClientRef = string

export type DecisionClientViewState = "pending" | "resolved" | "resolved_elsewhere" | "cancelled"

export type DecisionClientView = {
  readonly clientRef: DecisionClientRef
  readonly requestId: string
  readonly state: DecisionClientViewState
  readonly resolvedVerb?: DecisionVerb
  readonly resolvedByClientRef?: DecisionClientRef
}

export type IncomingDecisionResolution = {
  readonly requestId: string
  readonly verb: DecisionVerb
  readonly resolvingClientRef: DecisionClientRef
}

export type DecisionWinner = {
  readonly requestId: string
  readonly verb: DecisionVerb
  readonly resolvingClientRef: DecisionClientRef
}

export type ResolvedElsewhereProjection = {
  readonly clientRef: DecisionClientRef
  readonly requestId: string
  readonly state: "resolved_elsewhere"
  readonly resolvedVerb: DecisionVerb
  readonly resolvedByClientRef: DecisionClientRef
}

export type CancelledDecisionProjection = {
  readonly clientRef: DecisionClientRef
  readonly requestId: string
  readonly state: "cancelled"
}

export type DecisionConsistencyResult = {
  readonly outcome: ResolveOutcome
  readonly winner: DecisionWinner | null
  readonly projections: readonly ResolvedElsewhereProjection[]
}

export type DecisionCancellationResult = {
  readonly outcome: "accepted" | "unknown_request"
  readonly projections: readonly CancelledDecisionProjection[]
}

export function applyConsistentDecisionResolution(
  requestId: string,
  clientViews: readonly DecisionClientView[],
  incoming: IncomingDecisionResolution,
): DecisionConsistencyResult {
  const viewsForRequest = clientViews.filter((view) => view.requestId === requestId)
  if (incoming.requestId !== requestId || viewsForRequest.length === 0) {
    return { outcome: "unknown_request", winner: null, projections: [] }
  }

  if (viewsForRequest.some((view) => view.state === "cancelled")) {
    return { outcome: "cancelled", winner: null, projections: [] }
  }

  const existingWinner = findExistingWinner(viewsForRequest)
  if (existingWinner) {
    return {
      outcome: existingWinner.verb === incoming.verb ? "duplicate" : "already_resolved",
      winner: existingWinner,
      projections: resolvedElsewhereForNonWinners(viewsForRequest, existingWinner),
    }
  }

  const winner: DecisionWinner = {
    requestId,
    verb: incoming.verb,
    resolvingClientRef: incoming.resolvingClientRef,
  }

  return {
    outcome: "accepted",
    winner,
    projections: resolvedElsewhereForNonWinners(viewsForRequest, winner),
  }
}

export function applyConsistentDecisionCancellation(
  requestId: string,
  clientViews: readonly DecisionClientView[],
): DecisionCancellationResult {
  const viewsForRequest = clientViews.filter((view) => view.requestId === requestId)
  if (viewsForRequest.length === 0) {
    return { outcome: "unknown_request", projections: [] }
  }

  return {
    outcome: "accepted",
    projections: viewsForRequest.map((view) => ({
      clientRef: view.clientRef,
      requestId,
      state: "cancelled",
    })),
  }
}

function findExistingWinner(clientViews: readonly DecisionClientView[]): DecisionWinner | null {
  for (const view of clientViews) {
    if (view.state === "resolved" && view.resolvedVerb) {
      return {
        requestId: view.requestId,
        verb: view.resolvedVerb,
        resolvingClientRef: view.resolvedByClientRef ?? view.clientRef,
      }
    }
  }

  for (const view of clientViews) {
    if (view.state === "resolved_elsewhere" && view.resolvedVerb && view.resolvedByClientRef) {
      return {
        requestId: view.requestId,
        verb: view.resolvedVerb,
        resolvingClientRef: view.resolvedByClientRef,
      }
    }
  }

  return null
}

function resolvedElsewhereForNonWinners(
  clientViews: readonly DecisionClientView[],
  winner: DecisionWinner,
): ResolvedElsewhereProjection[] {
  return clientViews
    .filter((view) => view.clientRef !== winner.resolvingClientRef)
    .map((view) => ({
      clientRef: view.clientRef,
      requestId: winner.requestId,
      state: "resolved_elsewhere",
      resolvedVerb: winner.verb,
      resolvedByClientRef: winner.resolvingClientRef,
    }))
}

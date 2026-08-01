import type { ForensicRef } from "./primitives.ts";
import type { CleanupState, CoverageStatus, ForensicRunEvent, ForensicRunState } from "./run.ts";

export const FORENSIC_RUN_TRANSITIONS: Readonly<
  Record<ForensicRunState, ReadonlyArray<ForensicRunState>>
> = {
  draft: ["preflight"],
  preflight: ["ready_inputs", "incomplete", "denied"],
  ready_inputs: ["admission_requested"],
  incomplete: ["admission_requested"],
  denied: [],
  admission_requested: ["provisioning", "denied"],
  provisioning: ["worker_ready", "failed"],
  worker_ready: ["running", "cancel_requested", "failed"],
  running: ["settling", "cancel_requested", "failed"],
  settling: ["cleanup_requested", "failed"],
  cancel_requested: ["cleanup_requested", "failed"],
  cleanup_requested: ["cleaned", "recovery_required"],
  cleaned: ["completed", "completed_incomplete", "cancelled", "failed"],
  completed: ["review"],
  completed_incomplete: ["review"],
  cancelled: [],
  failed: ["cleanup_requested"],
  recovery_required: [],
  review: ["candidate", "retained", "dismissed"],
  candidate: ["release_gate"],
  retained: [],
  dismissed: [],
  release_gate: ["admitted", "rejected"],
  admitted: [],
  rejected: [],
};

export interface ForensicTransitionContext {
  readonly coverageStatus: CoverageStatus;
  readonly cleanupState: CleanupState;
  readonly cleanupReceiptRef?: ForensicRef;
}

export type ForensicTransitionDecision =
  | Readonly<{ _tag: "Allowed"; from: ForensicRunState; to: ForensicRunState }>
  | Readonly<{
      _tag: "Refused";
      from: ForensicRunState;
      to: ForensicRunState;
      blockerRef: ForensicRef;
    }>;

export const evaluateForensicRunTransition = (
  from: ForensicRunState,
  to: ForensicRunState,
  context: ForensicTransitionContext,
): ForensicTransitionDecision => {
  if (!FORENSIC_RUN_TRANSITIONS[from].includes(to)) {
    return {
      _tag: "Refused",
      from,
      to,
      blockerRef: "blocker.forensic.run.invalid_transition",
    };
  }

  if (
    to === "cleaned" &&
    (context.cleanupState !== "observed_zero_residue" || context.cleanupReceiptRef === undefined)
  ) {
    return {
      _tag: "Refused",
      from,
      to,
      blockerRef: "blocker.forensic.run.cleanup_not_observed",
    };
  }

  if (
    to === "completed" &&
    (context.coverageStatus !== "complete" ||
      context.cleanupState !== "observed_zero_residue" ||
      context.cleanupReceiptRef === undefined)
  ) {
    return {
      _tag: "Refused",
      from,
      to,
      blockerRef: "blocker.forensic.run.complete_evidence_missing",
    };
  }

  if (
    to === "completed_incomplete" &&
    (context.coverageStatus !== "incomplete" ||
      context.cleanupState !== "observed_zero_residue" ||
      context.cleanupReceiptRef === undefined)
  ) {
    return {
      _tag: "Refused",
      from,
      to,
      blockerRef: "blocker.forensic.run.incomplete_completion_mismatch",
    };
  }

  if (to === "recovery_required" && context.cleanupState !== "failed") {
    return {
      _tag: "Refused",
      from,
      to,
      blockerRef: "blocker.forensic.run.recovery_without_cleanup_failure",
    };
  }

  return { _tag: "Allowed", from, to };
};

export type ForensicEventSequenceDecision =
  | Readonly<{ _tag: "Valid"; runRef: ForensicRef; lastSequence: number }>
  | Readonly<{ _tag: "Invalid"; blockerRef: ForensicRef; atSequence?: number }>;

export const evaluateForensicEventSequence = (
  events: ReadonlyArray<ForensicRunEvent>,
): ForensicEventSequenceDecision => {
  if (events.length === 0) {
    return { _tag: "Invalid", blockerRef: "blocker.forensic.events.empty" };
  }

  const runRef = events[0]?.runRef;
  if (runRef === undefined) {
    return { _tag: "Invalid", blockerRef: "blocker.forensic.events.empty" };
  }

  for (const [index, event] of events.entries()) {
    const expectedSequence = index + 1;
    if (event.runRef !== runRef) {
      return {
        _tag: "Invalid",
        blockerRef: "blocker.forensic.events.mixed_run",
        atSequence: event.sequence,
      };
    }
    if (event.sequence !== expectedSequence) {
      return {
        _tag: "Invalid",
        blockerRef: "blocker.forensic.events.non_dense_sequence",
        atSequence: event.sequence,
      };
    }
  }

  return { _tag: "Valid", runRef, lastSequence: events.length };
};

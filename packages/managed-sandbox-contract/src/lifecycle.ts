import { Schema as S } from "effect";

import {
  NonNegativeInt,
  SandboxFilesystemState,
  SandboxGuestState,
  SandboxIngressState,
  SandboxLifecycle,
  SandboxRuntimeState,
} from "./schemas.ts";

export const SandboxModelStateSchema = S.Struct({
  lifecycle: SandboxLifecycle,
  resourceGeneration: NonNegativeInt,
  lastEventSequence: NonNegativeInt,
  acceptingWork: S.Boolean,
  guestState: SandboxGuestState,
  filesystemState: SandboxFilesystemState,
  ingressState: SandboxIngressState,
  runtimeState: SandboxRuntimeState,
  cleanupComplete: S.Boolean,
});
export type SandboxModelState = typeof SandboxModelStateSchema.Type;

export type SandboxModelEvent = Readonly<{
  kind:
    | "ProvisionRequested"
    | "GuestReady"
    | "RuntimeStarted"
    | "RuntimeTextDelta"
    | "RuntimeToolStarted"
    | "RuntimeToolCompleted"
    | "RuntimeUsageRecorded"
    | "RuntimeInterruptRequested"
    | "RuntimeSettled"
    | "RuntimeFailed"
    | "RuntimeInterrupted"
    | "StopRequested"
    | "FilesystemCheckpointed"
    | "FilesystemCheckpointFailed"
    | "GuestStopped"
    | "ResumeRequested"
    | "DeleteRequested"
    | "CleanupObserved"
    | "OperationFailed"
    | "RecoveryMarked";
  resourceGeneration: number;
  sequence: number;
}>;

export class SandboxTransitionRefused extends Error {
  readonly _tag = "SandboxTransitionRefused";

  constructor(
    readonly code:
      | "generation_mismatch"
      | "event_gap"
      | "invalid_transition"
      | "invariant_violation",
    message: string,
  ) {
    super(message);
  }
}

export const initialSandboxModelState = (resourceGeneration = 0): SandboxModelState => ({
  lifecycle: "provisioning",
  resourceGeneration,
  lastEventSequence: 0,
  acceptingWork: false,
  guestState: "starting",
  filesystemState: "unallocated",
  ingressState: "closed",
  runtimeState: "none",
  cleanupComplete: false,
});

export const sandboxInvariantViolations = (state: SandboxModelState): ReadonlyArray<string> => {
  const violations: Array<string> = [];

  if (state.lifecycle === "deleted") {
    if (!state.cleanupComplete) violations.push("deleted_without_cleanup");
    if (state.guestState !== "absent") violations.push("deleted_with_guest");
    if (state.acceptingWork) violations.push("deleted_accepting_work");
    if (state.filesystemState !== "deleted") {
      violations.push("deleted_with_filesystem");
    }
    if (state.ingressState !== "revoked") {
      violations.push("deleted_with_ingress");
    }
  }

  if (["ready", "idle", "running"].includes(state.lifecycle)) {
    if (state.guestState !== "present") violations.push("live_without_guest");
  }

  if (state.lifecycle === "ready" && !state.acceptingWork) {
    violations.push("ready_not_accepting_work");
  }

  if (state.lifecycle === "running") {
    if (!state.acceptingWork) violations.push("running_not_accepting_work");
    if (
      !(["running", "interrupting"] as const).includes(
        state.runtimeState as "running" | "interrupting",
      )
    ) {
      violations.push("running_without_runtime");
    }
  }

  if (state.lifecycle === "idle") {
    if (!(["none", "settled"] as const).includes(state.runtimeState as "none" | "settled")) {
      violations.push("idle_with_active_runtime");
    }
  }

  if (state.cleanupComplete && state.lifecycle !== "deleted") {
    violations.push("cleanup_before_deleted");
  }

  if (
    ["stopping", "stopped", "deleting", "deleted", "failed", "recovery_required"].includes(
      state.lifecycle,
    ) &&
    state.acceptingWork
  ) {
    violations.push("terminal_or_quiescing_accepting_work");
  }

  return violations;
};

const refuse = (code: SandboxTransitionRefused["code"], message: string): never => {
  throw new SandboxTransitionRefused(code, message);
};

export const applySandboxModelEvent = (
  state: SandboxModelState,
  event: SandboxModelEvent,
): SandboxModelState => {
  if (event.resourceGeneration !== state.resourceGeneration) {
    return refuse(
      "generation_mismatch",
      `expected generation ${state.resourceGeneration}, received ${event.resourceGeneration}`,
    );
  }
  if (event.sequence !== state.lastEventSequence + 1) {
    return refuse(
      "event_gap",
      `expected sequence ${state.lastEventSequence + 1}, received ${event.sequence}`,
    );
  }

  const nextBase = { ...state, lastEventSequence: event.sequence };
  let next: SandboxModelState;

  switch (event.kind) {
    case "ProvisionRequested":
      if (state.lifecycle !== "provisioning" || state.lastEventSequence !== 0) {
        return refuse(
          "invalid_transition",
          "ProvisionRequested requires a new provisioning resource",
        );
      }
      next = nextBase;
      break;
    case "GuestReady":
      if (!["provisioning", "resuming"].includes(state.lifecycle)) {
        return refuse("invalid_transition", "GuestReady requires provisioning or resuming");
      }
      next = {
        ...nextBase,
        lifecycle: "ready",
        acceptingWork: true,
        guestState: "present",
        filesystemState: "attached",
        ingressState: "broker_only",
        runtimeState: "none",
      };
      break;
    case "RuntimeStarted":
      if (!["ready", "idle"].includes(state.lifecycle)) {
        return refuse("invalid_transition", "RuntimeStarted requires ready or idle");
      }
      next = { ...nextBase, lifecycle: "running", runtimeState: "running" };
      break;
    case "RuntimeTextDelta":
    case "RuntimeToolStarted":
    case "RuntimeToolCompleted":
    case "RuntimeUsageRecorded":
      if (state.lifecycle !== "running") {
        return refuse("invalid_transition", `${event.kind} requires running`);
      }
      next = nextBase;
      break;
    case "RuntimeInterruptRequested":
      if (state.lifecycle !== "running" || state.runtimeState !== "running") {
        return refuse("invalid_transition", "RuntimeInterruptRequested requires running");
      }
      next = { ...nextBase, runtimeState: "interrupting" };
      break;
    case "RuntimeSettled":
      if (state.lifecycle !== "running") {
        return refuse("invalid_transition", "RuntimeSettled requires running");
      }
      next = { ...nextBase, lifecycle: "idle", runtimeState: "settled" };
      break;
    case "RuntimeFailed":
      if (state.lifecycle !== "running") {
        return refuse("invalid_transition", "RuntimeFailed requires running");
      }
      next = {
        ...nextBase,
        lifecycle: "failed",
        acceptingWork: false,
        runtimeState: "failed",
      };
      break;
    case "RuntimeInterrupted":
      if (state.lifecycle !== "running") {
        return refuse("invalid_transition", "RuntimeInterrupted requires running");
      }
      next = { ...nextBase, lifecycle: "idle", runtimeState: "settled" };
      break;
    case "StopRequested":
      if (!["ready", "idle"].includes(state.lifecycle)) {
        return refuse("invalid_transition", "StopRequested requires ready or idle");
      }
      next = {
        ...nextBase,
        lifecycle: "stopping",
        acceptingWork: false,
        guestState: "stopping",
        filesystemState: "checkpointing",
        ingressState: "closed",
      };
      break;
    case "FilesystemCheckpointed":
      if (state.lifecycle !== "stopping") {
        return refuse("invalid_transition", "checkpoint requires stopping");
      }
      next = { ...nextBase, filesystemState: "durable" };
      break;
    case "FilesystemCheckpointFailed":
      if (state.lifecycle !== "stopping") {
        return refuse("invalid_transition", "checkpoint failure requires stopping");
      }
      next = {
        ...nextBase,
        lifecycle: "recovery_required",
        guestState: "unknown",
        filesystemState: "unknown",
        runtimeState: "unknown",
      };
      break;
    case "GuestStopped":
      if (state.lifecycle !== "stopping" || state.filesystemState !== "durable") {
        return refuse("invalid_transition", "GuestStopped requires a durable checkpoint");
      }
      next = {
        ...nextBase,
        lifecycle: "stopped",
        guestState: "absent",
        runtimeState: "none",
      };
      break;
    case "ResumeRequested":
      if (state.lifecycle !== "stopped") {
        return refuse("invalid_transition", "ResumeRequested requires stopped");
      }
      next = {
        ...nextBase,
        lifecycle: "resuming",
        guestState: "starting",
        filesystemState: "durable",
        runtimeState: "none",
      };
      break;
    case "DeleteRequested":
      if (!["stopped", "failed", "recovery_required"].includes(state.lifecycle)) {
        return refuse(
          "invalid_transition",
          "DeleteRequested requires stopped, failed, or recovery_required",
        );
      }
      next = {
        ...nextBase,
        lifecycle: "deleting",
        acceptingWork: false,
        ingressState: "revoked",
      };
      break;
    case "CleanupObserved":
      if (state.lifecycle !== "deleting") {
        return refuse("invalid_transition", "CleanupObserved requires deleting");
      }
      next = {
        ...nextBase,
        lifecycle: "deleted",
        guestState: "absent",
        filesystemState: "deleted",
        ingressState: "revoked",
        runtimeState: "none",
        cleanupComplete: true,
      };
      break;
    case "OperationFailed":
      if (state.lifecycle === "deleted") {
        return refuse("invalid_transition", "deleted sandboxes cannot fail again");
      }
      next = {
        ...nextBase,
        lifecycle: "failed",
        acceptingWork: false,
        runtimeState: "failed",
      };
      break;
    case "RecoveryMarked":
      if (!["failed", "stopping", "deleting"].includes(state.lifecycle)) {
        return refuse("invalid_transition", "RecoveryMarked requires an uncertain operation");
      }
      next = {
        ...nextBase,
        lifecycle: "recovery_required",
        acceptingWork: false,
        guestState: "unknown",
        filesystemState: "unknown",
        ingressState: "unknown",
        runtimeState: "unknown",
      };
      break;
  }

  const violations = sandboxInvariantViolations(next);
  if (violations.length > 0) {
    return refuse("invariant_violation", violations.join(","));
  }
  return next;
};

/**
 * Fence a resumed guest into a fresh resource generation before it can accept
 * work. The global native event sequence remains continuous across generations.
 */
export const advanceSandboxModelGeneration = (
  state: SandboxModelState,
  nextGeneration: number,
): SandboxModelState => {
  if (state.lifecycle !== "resuming" || state.acceptingWork || state.guestState !== "starting") {
    return refuse(
      "invalid_transition",
      "generation advance requires a non-accepting resuming resource",
    );
  }
  if (nextGeneration !== state.resourceGeneration + 1) {
    return refuse(
      "generation_mismatch",
      `expected generation ${state.resourceGeneration + 1}, received ${nextGeneration}`,
    );
  }
  return { ...state, resourceGeneration: nextGeneration };
};

/**
 * Exhaustively explores the bounded transition graph to the requested depth.
 * This is intentionally deterministic and dependency-free so the same model
 * can be reused by CI and future TLA+/Alloy correspondence checks.
 */
export const enumerateSandboxModel = (depth: number): ReadonlyArray<SandboxModelState> => {
  const kinds: ReadonlyArray<SandboxModelEvent["kind"]> = [
    "ProvisionRequested",
    "GuestReady",
    "RuntimeStarted",
    "RuntimeTextDelta",
    "RuntimeToolStarted",
    "RuntimeToolCompleted",
    "RuntimeUsageRecorded",
    "RuntimeInterruptRequested",
    "RuntimeSettled",
    "RuntimeFailed",
    "RuntimeInterrupted",
    "StopRequested",
    "FilesystemCheckpointed",
    "FilesystemCheckpointFailed",
    "GuestStopped",
    "ResumeRequested",
    "DeleteRequested",
    "CleanupObserved",
    "OperationFailed",
    "RecoveryMarked",
  ];
  let frontier: ReadonlyArray<SandboxModelState> = [initialSandboxModelState()];
  const seen = new Map<string, SandboxModelState>();

  for (let step = 0; step <= depth; step += 1) {
    const nextFrontier: Array<SandboxModelState> = [];
    for (const state of frontier) {
      const key = JSON.stringify(state);
      if (!seen.has(key)) seen.set(key, state);

      for (const kind of kinds) {
        try {
          nextFrontier.push(
            applySandboxModelEvent(state, {
              kind,
              resourceGeneration: state.resourceGeneration,
              sequence: state.lastEventSequence + 1,
            }),
          );
        } catch (error) {
          if (!(error instanceof SandboxTransitionRefused)) throw error;
        }
      }
    }
    frontier = nextFrontier;
  }

  return [...seen.values()];
};

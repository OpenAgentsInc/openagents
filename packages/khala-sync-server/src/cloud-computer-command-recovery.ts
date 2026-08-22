import type { CloudComputerCommandStatus } from "./cloud-computer-command-store.js";

const REF = /^[a-z][a-z0-9._/-]{2,511}$/u;
const DIGEST = /^sha256:[0-9a-f]{64}$/u;

export type CloudComputerCommandRecoveryCause =
  | "controller_restart"
  | "transport_loss"
  | "runtime_crash"
  | "host_loss"
  | "checkpoint_failure"
  | "cleanup_failure";

type CloudComputerCommandRecoveryEvidenceBase = Readonly<{
  evidenceRef: string;
  evidenceDigest: string;
  computerRef: string;
  workspaceRef: string;
  runtimeRef: string;
  runtimeGeneration: number;
  providerLeaseRef: string;
  observedAt: string;
  durable: true;
}>;
type CloudComputerCommandRecoveryEvidenceInput =
  | (CloudComputerCommandRecoveryEvidenceBase & Readonly<{ kind: "runtime_lost" }>)
  | (CloudComputerCommandRecoveryEvidenceBase & Readonly<{ kind: "host_lost" }>)
  | (CloudComputerCommandRecoveryEvidenceBase & Readonly<{ kind: "checkpoint_failed" }>)
  | (CloudComputerCommandRecoveryEvidenceBase & Readonly<{ kind: "cleanup_failed" }>);
declare const recoveryEvidenceBrand: unique symbol;
export type CloudComputerCommandRecoveryEvidence = CloudComputerCommandRecoveryEvidenceInput &
  Readonly<{ [recoveryEvidenceBrand]: true }>;
const durableEvidence = new WeakSet<object>();

export function assertCloudComputerCommandRecoveryEvidence(
  value: unknown,
): asserts value is CloudComputerCommandRecoveryEvidence {
  if (typeof value !== "object" || value === null || !durableEvidence.has(value)) {
    throw new Error("recovery evidence is not durably verified");
  }
}

export const cloudComputerCommandRecoveryEvidenceAuthority = (adapter: {
  verify: (input: CloudComputerCommandRecoveryEvidenceInput) => Promise<boolean>;
}) => ({
  issue: async (
    input: CloudComputerCommandRecoveryEvidenceInput,
  ): Promise<CloudComputerCommandRecoveryEvidence> => {
    if (
      !REF.test(input.evidenceRef) ||
      !DIGEST.test(input.evidenceDigest) ||
      !REF.test(input.computerRef) ||
      !REF.test(input.workspaceRef) ||
      !REF.test(input.runtimeRef) ||
      !Number.isSafeInteger(input.runtimeGeneration) ||
      input.runtimeGeneration < 1 ||
      !REF.test(input.providerLeaseRef) ||
      !Number.isFinite(Date.parse(input.observedAt)) ||
      !(await adapter.verify(input))
    ) {
      throw new Error("recovery evidence is not durably verified");
    }
    const evidence = Object.freeze({ ...input }) as CloudComputerCommandRecoveryEvidence;
    durableEvidence.add(evidence);
    return evidence;
  },
});

export type CloudComputerCommandRecoverySnapshot = Readonly<{
  status: CloudComputerCommandStatus;
  exposure: "none" | "prepared" | "exposed" | "reservation_recorded" | "acknowledged";
  dispatchRef: string | null;
  providerExecutionRef: string | null;
}>;

export type CloudComputerCommandRecoveryAction =
  | Readonly<{ kind: "redispatch"; dispatchRef: string | null }>
  | Readonly<{ kind: "observe_or_reattach"; providerExecutionRef: string | null }>
  | Readonly<{
      kind: "settle_lost";
      evidence: CloudComputerCommandRecoveryEvidence &
        Readonly<{ kind: "runtime_lost" | "host_lost" }>;
    }>
  | Readonly<{
      kind: "record_checkpoint_failed";
      evidence: CloudComputerCommandRecoveryEvidence & Readonly<{ kind: "checkpoint_failed" }>;
    }>
  | Readonly<{
      kind: "record_cleanup_failed";
      evidence: CloudComputerCommandRecoveryEvidence & Readonly<{ kind: "cleanup_failed" }>;
    }>
  | Readonly<{
      kind: "await_durable_evidence";
      evidenceKind: CloudComputerCommandRecoveryEvidence["kind"];
    }>
  | Readonly<{ kind: "no_action" }>;

const active = new Set<CloudComputerCommandStatus>(["may_have_started", "dispatched", "running"]);
const terminal = new Set<CloudComputerCommandStatus>([
  "completed",
  "failed",
  "cancelled",
  "timed_out",
  "lost",
]);

/** Selects a recovery action without authorizing replay across an exposure boundary. */
export const recoverCloudComputerCommand = (input: {
  cause: CloudComputerCommandRecoveryCause;
  snapshot: CloudComputerCommandRecoverySnapshot;
  evidence?: CloudComputerCommandRecoveryEvidence;
}): CloudComputerCommandRecoveryAction => {
  const { cause, snapshot, evidence } = input;
  const verifiedEvidence =
    evidence !== undefined && durableEvidence.has(evidence) ? evidence : null;
  if (cause === "checkpoint_failure") {
    return verifiedEvidence?.kind === "checkpoint_failed"
      ? { kind: "record_checkpoint_failed", evidence: verifiedEvidence }
      : { kind: "await_durable_evidence", evidenceKind: "checkpoint_failed" };
  }
  if (cause === "cleanup_failure") {
    return verifiedEvidence?.kind === "cleanup_failed"
      ? { kind: "record_cleanup_failed", evidence: verifiedEvidence }
      : { kind: "await_durable_evidence", evidenceKind: "cleanup_failed" };
  }
  if (terminal.has(snapshot.status)) return { kind: "no_action" };
  if (cause === "runtime_crash" || cause === "host_loss") {
    const required = cause === "runtime_crash" ? "runtime_lost" : "host_lost";
    if (verifiedEvidence?.kind !== required) {
      return { kind: "await_durable_evidence", evidenceKind: required };
    }
    return active.has(snapshot.status)
      ? { kind: "settle_lost", evidence: verifiedEvidence }
      : { kind: "no_action" };
  }
  if (active.has(snapshot.status)) {
    return {
      kind: "observe_or_reattach",
      providerExecutionRef: snapshot.providerExecutionRef,
    };
  }
  if (
    (snapshot.status === "admitted" || snapshot.status === "not_dispatched") &&
    (snapshot.exposure === "none" || snapshot.exposure === "prepared")
  ) {
    return { kind: "redispatch", dispatchRef: snapshot.dispatchRef };
  }
  return {
    kind: "observe_or_reattach",
    providerExecutionRef: snapshot.providerExecutionRef,
  };
};

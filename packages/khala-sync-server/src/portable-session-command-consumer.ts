import { createHash } from "node:crypto";

import {
  PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  type PortableCommandExecutionClaim,
  type PortableCommandExecutionClaimRequest,
} from "@openagentsinc/portable-session-contract";

import {
  PostgresPortableSessionCommandQueue,
  PortableSessionCommandQueueError,
} from "./portable-session-command-queue.js";
import {
  PostgresPortableSessionMoveRuntime,
  type PortableSessionMoveRuntimeInput,
} from "./portable-session-move-runtime.js";
import type { PortableSessionMoveResult } from "./portable-session-move.js";

export type PortableSessionCommandResolver = Readonly<{
  resolve: (claim: PortableCommandExecutionClaim) => Promise<PortableSessionMoveRuntimeInput>;
}>;

export type PortableSessionCommandConsumerResult = Readonly<{
  status: "completed" | "failed" | "rejected" | "pending_reconcile";
  claim: PortableCommandExecutionClaim;
  move?: PortableSessionMoveResult;
}>;

export type PortableSessionCommandConsumerConfig = Readonly<{
  queue: Pick<PostgresPortableSessionCommandQueue, "claim" | "markPendingReconcile" | "terminal">;
  resolver: PortableSessionCommandResolver;
  runtime: Pick<PostgresPortableSessionMoveRuntime, "move">;
  now?: () => string;
}>;

export class PortableSessionCommandConsumerError extends Error {
  readonly _tag = "PortableSessionCommandConsumerError";
  override readonly name = "PortableSessionCommandConsumerError";

  constructor(
    readonly code: "resolver_unavailable" | "resolver_mismatch" | "runtime_uncertain",
    message: string,
  ) {
    super(message);
  }
}

const safeRef = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;

const digestRef = (prefix: string, value: unknown): string => {
  const digest = createHash("sha256").update(JSON.stringify(value)).digest("hex");
  return `${prefix}.${digest}`;
};

const validEvidence = (refs: ReadonlyArray<string>): boolean =>
  refs.length <= 256 && refs.every((ref) => safeRef.test(ref));

const matchesClaim = (
  claim: PortableCommandExecutionClaim,
  input: PortableSessionMoveRuntimeInput,
): boolean => {
  const { command, source, destination } = input.move;
  return (
    input.moveRef === claim.claimRef &&
    command.commandRef === claim.commandRef &&
    command.ownerRef === claim.ownerRef &&
    command.sessionRef === claim.sessionRef &&
    command.kind === claim.commandKind &&
    command.expectedAttachmentRef === claim.sourceAttachmentRef &&
    command.expectedGeneration === claim.sourceGeneration &&
    command.destinationTargetRef === claim.destinationTargetRef &&
    source.targetRef === claim.executorEnvironmentRef &&
    destination.targetRef === claim.destinationTargetRef &&
    source.targetRef !== destination.targetRef
  );
};

const resultMatchesClaim = (
  claim: PortableCommandExecutionClaim,
  result: PortableSessionMoveResult,
): boolean => {
  const completedDestinationIsCoherent =
    (result.status !== "completed" && result.status !== "replayed") ||
    (result.destinationAttachmentRef !== undefined &&
      result.destinationGeneration === claim.sourceGeneration + 1);
  return (
    result.commandRef === claim.commandRef &&
    result.sessionRef === claim.sessionRef &&
    result.sourceAttachmentRef === claim.sourceAttachmentRef &&
    result.sourceGeneration === claim.sourceGeneration &&
    completedDestinationIsCoherent &&
    validEvidence(result.evidenceRefs)
  );
};

/**
 * Runs one exact accepted portable command through the canonical Postgres
 * move runtime. The durable queue serializes entry. Unknown post-entry state
 * stays pending reconciliation and is never converted to a false failure.
 */
export class PortableSessionCommandConsumer {
  private readonly now: () => string;

  constructor(private readonly config: PortableSessionCommandConsumerConfig) {
    this.now = config.now ?? (() => new Date().toISOString());
  }

  async execute(
    request: PortableCommandExecutionClaimRequest,
  ): Promise<PortableSessionCommandConsumerResult> {
    const claimed = await this.config.queue.claim(request);
    const claim = claimed.claim;
    if (claim.state === "terminal") {
      const status =
        claim.terminalStatus === "completed"
          ? "completed"
          : claim.terminalStatus === "rejected"
            ? "rejected"
            : "failed";
      return { status, claim };
    }
    if (claim.state === "pending_reconcile") {
      return { status: "pending_reconcile", claim };
    }
    let input: PortableSessionMoveRuntimeInput;
    try {
      input = await this.config.resolver.resolve(claim);
    } catch {
      throw new PortableSessionCommandConsumerError(
        "resolver_unavailable",
        "portable command resolver is unavailable",
      );
    }

    if (!matchesClaim(claim, input)) {
      const terminal = await this.config.queue.terminal({
        schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
        claimRef: claim.claimRef,
        executorEnvironmentRef: claim.executorEnvironmentRef,
        workerInstanceRef: claim.workerInstanceRef,
        claimGeneration: claim.claimGeneration,
        expectedLeaseRevision: claim.leaseRevision,
        terminalStatus: "rejected",
        outcomeRef: digestRef("outcome.portable.resolver-mismatch", claim.claimRef),
        evidenceRefs: [digestRef("evidence.portable.resolver-mismatch", claim.claimRef)],
        completedAt: this.now(),
      });
      return { status: "rejected", claim: terminal.claim };
    }

    let move: PortableSessionMoveResult;
    try {
      move = await this.config.runtime.move(input);
    } catch {
      const pending = await this.markPending(claim, "runtime-uncertain", [
        digestRef("evidence.portable.runtime-uncertain", claim.claimRef),
      ]);
      return pending;
    }

    if (!resultMatchesClaim(claim, move)) {
      return this.markPending(
        claim,
        "result-mismatch",
        [digestRef("evidence.portable.result-mismatch", claim.claimRef)],
        move,
      );
    }

    if (
      move.status === "authority_pending_reconcile" ||
      move.status === "activation_pending_reconcile"
    ) {
      return this.markPending(claim, move.status, move.evidenceRefs, move);
    }

    const terminalStatus = move.status === "failed" ? "failed" : "completed";
    const terminal = await this.config.queue.terminal({
      schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
      claimRef: claim.claimRef,
      executorEnvironmentRef: claim.executorEnvironmentRef,
      workerInstanceRef: claim.workerInstanceRef,
      claimGeneration: claim.claimGeneration,
      expectedLeaseRevision: claim.leaseRevision,
      terminalStatus,
      outcomeRef: digestRef(`outcome.portable.${terminalStatus}`, move),
      evidenceRefs: move.evidenceRefs,
      completedAt: this.now(),
    });
    return { status: terminalStatus, claim: terminal.claim, move };
  }

  private async markPending(
    claim: PortableCommandExecutionClaim,
    reason: string,
    evidenceRefs: ReadonlyArray<string>,
    move?: PortableSessionMoveResult,
  ): Promise<PortableSessionCommandConsumerResult> {
    const pending = await this.config.queue.markPendingReconcile({
      schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
      claimRef: claim.claimRef,
      executorEnvironmentRef: claim.executorEnvironmentRef,
      workerInstanceRef: claim.workerInstanceRef,
      claimGeneration: claim.claimGeneration,
      expectedLeaseRevision: claim.leaseRevision,
      pendingReconcileRef: digestRef(`reconcile.portable.${reason}`, claim.claimRef),
      evidenceRefs,
      observedAt: this.now(),
    });
    return {
      status: "pending_reconcile",
      claim: pending.claim,
      ...(move === undefined ? {} : { move }),
    };
  }
}

export const isPortableSessionCommandClaimConflict = (cause: unknown): boolean =>
  cause instanceof PortableSessionCommandQueueError && cause.code === "claim_conflict";

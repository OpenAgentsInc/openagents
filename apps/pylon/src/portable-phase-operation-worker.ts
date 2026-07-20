import { createHash } from "node:crypto";
import type {
  PortablePhaseOperationRecord,
  PortablePhaseOperationRequest,
  PortablePhaseOperationResultRequest,
} from "@openagentsinc/portable-session-contract";

import type { PylonPortablePhaseOperationClient } from "./portable-phase-operation-client.js";
import type { PylonOwnerLocalExecutionTarget } from "./portable-session-target.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const SHA256 = /^sha256:[a-f0-9]{64}$/u;

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`;

export type PylonPortablePhaseExecutionResult = Readonly<{
  checkpointRef: string | null;
  checkpointObjectRef: string | null;
  checkpointDigest: string | null;
  evidenceRefs: ReadonlyArray<string>;
}>;

export class PylonPortablePhaseExecutionError extends Error {
  override readonly name = "PylonPortablePhaseExecutionError";

  constructor(readonly errorRef: string) {
    super("Pylon portable phase execution failed");
    if (!SAFE_REF.test(errorRef)) throw new Error("portable phase error ref is invalid");
  }
}

export type PylonPortablePhaseExecutor = Readonly<{
  execute: (
    request: PortablePhaseOperationRequest,
    signal: AbortSignal,
  ) => Promise<PylonPortablePhaseExecutionResult>;
}>;

type TargetCall =
  | Readonly<{
      kind: "quiesce";
      input: Parameters<PylonOwnerLocalExecutionTarget["quiesceGraph"]>[0];
    }>
  | Readonly<{
      kind: "checkpoint-create";
      input: Parameters<PylonOwnerLocalExecutionTarget["createCheckpoint"]>[0];
      checkpointObjectRef: string;
    }>
  | Readonly<{
      kind: "source-cleanup";
      input: Parameters<PylonOwnerLocalExecutionTarget["cleanupSource"]>[0];
    }>
  | Readonly<{
      kind: "checkpoint-stage";
      input: Parameters<PylonOwnerLocalExecutionTarget["stageCheckpoint"]>[0];
    }>
  | Readonly<{
      kind: "destination-activate";
      input: Parameters<PylonOwnerLocalExecutionTarget["activate"]>[0];
    }>
  | Readonly<{
      kind: "staged-abort";
      input: Parameters<PylonOwnerLocalExecutionTarget["abortStaged"]>[0];
    }>;

export type PylonPortablePhaseTargetResolver = Readonly<{
  resolve: (request: PortablePhaseOperationRequest) => Promise<
    | Readonly<{
        target: PylonOwnerLocalExecutionTarget;
        call: TargetCall;
      }>
    | undefined
  >;
}>;

const assertCallBinding = (request: PortablePhaseOperationRequest, call: TargetCall): void => {
  const input = call.input;
  const sessionRef = "sessionRef" in input ? input.sessionRef : input.bundle.checkpoint.sessionRef;
  if (
    call.kind !== request.kind ||
    input.operationRef !== request.operationRef ||
    sessionRef !== request.sessionRef
  ) {
    throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.context-mismatch");
  }
  if (
    request.kind === "quiesce" ||
    request.kind === "checkpoint-create" ||
    request.kind === "source-cleanup"
  ) {
    if (
      !("attachmentRef" in input) ||
      input.attachmentRef !== request.attachmentRef ||
      !("generation" in input) ||
      input.generation !== request.attachmentGeneration
    ) {
      throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.context-mismatch");
    }
  } else if (
    !("destinationAttachmentRef" in input) ||
    input.destinationAttachmentRef !== request.attachmentRef ||
    !("destinationGeneration" in input) ||
    input.destinationGeneration !== request.attachmentGeneration
  ) {
    throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.context-mismatch");
  }
  if (
    call.kind === "checkpoint-create" &&
    (call.input.checkpointRef !== request.checkpointRef || !SAFE_REF.test(call.checkpointObjectRef))
  ) {
    throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.context-mismatch");
  }
  if (
    call.kind === "checkpoint-stage" &&
    (call.input.bundle.checkpoint.checkpointRef !== request.checkpointRef ||
      call.input.bundle.checkpoint.digest !== request.checkpointDigest)
  ) {
    throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.context-mismatch");
  }
  if (call.kind === "destination-activate" && call.input.checkpointRef !== request.checkpointRef) {
    throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.context-mismatch");
  }
};

const result = (evidenceRefs: ReadonlyArray<string>): PylonPortablePhaseExecutionResult => ({
  checkpointRef: null,
  checkpointObjectRef: null,
  checkpointDigest: null,
  evidenceRefs,
});

/**
 * Bind refs-only phase requests to exact private target inputs. The resolver is
 * the only component that can read local graph, cursor, artifact, lease, or
 * execution-binding state. An absent binding is an explicit unsupported phase.
 */
export const makePylonPortablePhaseExecutor = (
  resolver: PylonPortablePhaseTargetResolver,
): PylonPortablePhaseExecutor => ({
  execute: async (request, signal) => {
    if (signal.aborted) throw signal.reason;
    const resolved = await resolver.resolve(request);
    if (resolved === undefined) {
      throw new PylonPortablePhaseExecutionError(
        `error.pylon.portable-phase.unsupported-${request.kind}`,
      );
    }
    if (resolved.target.targetRef !== request.targetRef) {
      throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.target-mismatch");
    }
    assertCallBinding(request, resolved.call);
    if (signal.aborted) throw signal.reason;
    switch (resolved.call.kind) {
      case "quiesce":
        return result((await resolved.target.quiesceGraph(resolved.call.input)).evidenceRefs);
      case "checkpoint-create": {
        const bundle = await resolved.target.createCheckpoint(resolved.call.input);
        return {
          checkpointRef: bundle.checkpoint.checkpointRef,
          checkpointObjectRef: resolved.call.checkpointObjectRef,
          checkpointDigest: bundle.checkpoint.digest,
          evidenceRefs: bundle.checkpoint.receiptRefs,
        };
      }
      case "source-cleanup":
        return result((await resolved.target.cleanupSource(resolved.call.input)).evidenceRefs);
      case "checkpoint-stage":
        return result((await resolved.target.stageCheckpoint(resolved.call.input)).evidenceRefs);
      case "destination-activate":
        return result((await resolved.target.activate(resolved.call.input)).evidenceRefs);
      case "staged-abort":
        return result((await resolved.target.abortStaged(resolved.call.input)).evidenceRefs);
    }
  },
});

type ActiveClaim = {
  record: PortablePhaseOperationRecord;
  claimRef: string;
  claimGeneration: number;
  leaseRevision: number;
  leaseExpiresAt: string;
  completion?: PortablePhaseOperationResultRequest;
  uncertain: boolean;
};

export type PylonPortablePhaseWorkerOptions = Readonly<{
  client: PylonPortablePhaseOperationClient;
  executor: PylonPortablePhaseExecutor;
  pylonRef: string;
  targetRef: string;
  workerInstanceRef: string;
  now?: () => Date;
  pollLimit?: number;
  leaseDurationMs?: number;
  renewalIntervalMs?: number;
  waitForRenewal?: (milliseconds: number, signal: AbortSignal) => Promise<"renew">;
}>;

const abortableDelay = (milliseconds: number, signal: AbortSignal): Promise<"renew"> =>
  new Promise((resolve, reject) => {
    if (signal.aborted) return reject(signal.reason);
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve("renew");
    }, milliseconds);
    const onAbort = () => {
      clearTimeout(timer);
      reject(signal.reason);
    };
    signal.addEventListener("abort", onAbort, { once: true });
  });

const validateResult = (
  request: PortablePhaseOperationRequest,
  execution: PylonPortablePhaseExecutionResult,
): void => {
  if (
    execution.evidenceRefs.length > 256 ||
    new Set(execution.evidenceRefs).size !== execution.evidenceRefs.length ||
    execution.evidenceRefs.some((ref) => !SAFE_REF.test(ref))
  ) {
    throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.unsafe-result");
  }
  const hasCheckpoint =
    execution.checkpointRef !== null &&
    execution.checkpointObjectRef !== null &&
    execution.checkpointDigest !== null;
  if ((request.kind === "checkpoint-create") !== hasCheckpoint) {
    throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.invalid-result");
  }
  if (
    execution.checkpointRef !== null &&
    execution.checkpointObjectRef !== null &&
    execution.checkpointDigest !== null &&
    (!SAFE_REF.test(execution.checkpointRef) ||
      !SAFE_REF.test(execution.checkpointObjectRef) ||
      !SHA256.test(execution.checkpointDigest))
  )
    throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.invalid-result");
};

const exactRecordBinding = (
  record: PortablePhaseOperationRecord,
  request: PortablePhaseOperationRequest,
): boolean =>
  record.request.operationRef === request.operationRef &&
  record.request.commandRef === request.commandRef &&
  record.request.commandExecutionClaimRef === request.commandExecutionClaimRef &&
  record.request.ownerRef === request.ownerRef &&
  record.request.sessionRef === request.sessionRef &&
  record.request.attachmentRef === request.attachmentRef &&
  record.request.attachmentGeneration === request.attachmentGeneration &&
  record.request.pylonRef === request.pylonRef &&
  record.request.targetRef === request.targetRef &&
  record.request.kind === request.kind &&
  record.request.checkpointRef === request.checkpointRef &&
  record.request.checkpointObjectRef === request.checkpointObjectRef &&
  record.request.checkpointDigest === request.checkpointDigest &&
  record.request.expiresAt === request.expiresAt &&
  record.request.evidenceRefs.length === request.evidenceRefs.length &&
  record.request.evidenceRefs.every((ref, index) => ref === request.evidenceRefs[index]);

export class PylonPortablePhaseWorker {
  private readonly active = new Map<string, ActiveClaim>();
  private readonly now: () => Date;
  private readonly pollLimit: number;
  private readonly leaseDurationMs: number;
  private readonly renewalIntervalMs: number;

  constructor(private readonly options: PylonPortablePhaseWorkerOptions) {
    if (
      ![options.pylonRef, options.targetRef, options.workerInstanceRef].every((ref) =>
        SAFE_REF.test(ref),
      )
    ) {
      throw new Error("portable phase worker refs are invalid");
    }
    this.pollLimit = options.pollLimit ?? 8;
    this.leaseDurationMs = options.leaseDurationMs ?? 30_000;
    this.renewalIntervalMs = options.renewalIntervalMs ?? 10_000;
    if (
      !Number.isSafeInteger(this.pollLimit) ||
      this.pollLimit < 1 ||
      this.pollLimit > 32 ||
      !Number.isSafeInteger(this.leaseDurationMs) ||
      this.leaseDurationMs < 2_000 ||
      !Number.isSafeInteger(this.renewalIntervalMs) ||
      this.renewalIntervalMs < 250 ||
      this.renewalIntervalMs >= this.leaseDurationMs
    ) {
      throw new Error("portable phase worker timing is invalid");
    }
    this.now = options.now ?? (() => new Date());
  }

  uncertainOperationRefs(): ReadonlyArray<string> {
    return [...this.active]
      .filter(([, claim]) => claim.uncertain)
      .map(([operationRef]) => operationRef)
      .sort();
  }

  private leaseExpiry(record: PortablePhaseOperationRecord, after: Date): string {
    const requestExpiry = new Date(record.request.expiresAt).valueOf();
    const next = Math.min(requestExpiry, after.valueOf() + this.leaseDurationMs);
    if (next <= after.valueOf())
      throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.expired");
    return new Date(next).toISOString();
  }

  private async postCompletion(claim: ActiveClaim, signal: AbortSignal): Promise<void> {
    if (claim.completion === undefined) return;
    const response = await this.options.client.complete(claim.completion, signal);
    if (
      !exactRecordBinding(response.operation, claim.record.request) ||
      !["completed", "failed", "replayed"].includes(response.status)
    ) {
      throw new Error("portable phase completion acknowledgement is invalid");
    }
    this.active.delete(claim.record.request.operationRef);
  }

  private async executeClaim(claim: ActiveClaim, signal: AbortSignal): Promise<void> {
    const request = claim.record.request;
    const execution = this.options.executor
      .execute(request, signal)
      .then((value) => ({ _tag: "success" as const, value }))
      .catch((error) => ({ _tag: "failure" as const, error }));
    let settled: Awaited<typeof execution>;
    while (true) {
      let next: Awaited<typeof execution> | "renew";
      try {
        next = await Promise.race([
          execution,
          (this.options.waitForRenewal ?? abortableDelay)(this.renewalIntervalMs, signal),
        ]);
      } catch (error) {
        if (signal.aborted) {
          claim.uncertain = true;
          return;
        }
        throw error;
      }
      if (next !== "renew") {
        settled = next;
        break;
      }
      const renewedExpiry = this.leaseExpiry(claim.record, new Date(claim.leaseExpiresAt));
      if (renewedExpiry === claim.leaseExpiresAt) continue;
      const renewed = await this.options.client.renew(
        {
          schema: "openagents.portable_phase_operation.v1",
          claimRef: claim.claimRef,
          sessionRef: request.sessionRef,
          attachmentRef: request.attachmentRef,
          attachmentGeneration: request.attachmentGeneration,
          pylonRef: request.pylonRef,
          targetRef: request.targetRef,
          workerInstanceRef: this.options.workerInstanceRef,
          claimGeneration: claim.claimGeneration,
          expectedLeaseRevision: claim.leaseRevision,
          leaseExpiresAt: renewedExpiry,
        },
        signal,
      );
      if (
        !exactRecordBinding(renewed.operation, request) ||
        renewed.operation.claimRef !== claim.claimRef ||
        renewed.operation.claimGeneration !== claim.claimGeneration ||
        renewed.operation.leaseRevision === null ||
        renewed.operation.leaseExpiresAt === null
      )
        throw new Error("portable phase renewal acknowledgement is invalid");
      claim.leaseRevision = renewed.operation.leaseRevision;
      claim.leaseExpiresAt = renewed.operation.leaseExpiresAt;
    }
    if (signal.aborted) {
      claim.uncertain = true;
      return;
    }
    let status: "completed" | "failed";
    let output: PylonPortablePhaseExecutionResult;
    let errorRef: string | null;
    if (settled._tag === "success") {
      try {
        validateResult(request, settled.value);
        status = "completed";
        output = settled.value;
        errorRef = null;
      } catch (error) {
        status = "failed";
        output = result([]);
        errorRef =
          error instanceof PylonPortablePhaseExecutionError
            ? error.errorRef
            : "error.pylon.portable-phase.invalid-result";
      }
    } else {
      if (signal.aborted) {
        claim.uncertain = true;
        return;
      }
      status = "failed";
      output = result([]);
      errorRef =
        settled.error instanceof PylonPortablePhaseExecutionError
          ? settled.error.errorRef
          : "error.pylon.portable-phase.operation-failed";
    }
    const completedAt = this.now().toISOString();
    claim.completion = {
      schema: "openagents.portable_phase_operation.v1",
      claimRef: claim.claimRef,
      sessionRef: request.sessionRef,
      attachmentRef: request.attachmentRef,
      attachmentGeneration: request.attachmentGeneration,
      pylonRef: request.pylonRef,
      targetRef: request.targetRef,
      workerInstanceRef: this.options.workerInstanceRef,
      claimGeneration: claim.claimGeneration,
      expectedLeaseRevision: claim.leaseRevision,
      resultRef: stableRef("result.pylon.portable-phase", `${request.operationRef}:${status}`),
      resultStatus: status,
      checkpointRef: output.checkpointRef,
      checkpointObjectRef: output.checkpointObjectRef,
      checkpointDigest: output.checkpointDigest,
      evidenceRefs: output.evidenceRefs,
      errorRef,
      completedAt,
    };
    await this.postCompletion(claim, signal);
  }

  async runPass(signal: AbortSignal = new AbortController().signal): Promise<number> {
    for (const claim of this.active.values()) {
      if (signal.aborted) return 0;
      if (claim.uncertain) continue;
      if (claim.completion !== undefined) await this.postCompletion(claim, signal);
    }
    const pending = await this.options.client.pending(this.pollLimit, signal);
    let handled = 0;
    for (const record of pending) {
      if (signal.aborted) break;
      const request = record.request;
      if (
        request.pylonRef !== this.options.pylonRef ||
        request.targetRef !== this.options.targetRef ||
        this.active.has(request.operationRef)
      )
        continue;
      const leaseExpiresAt = this.leaseExpiry(record, this.now());
      const claimed = await this.options.client.claim(
        {
          schema: "openagents.portable_phase_operation.v1",
          operationRef: request.operationRef,
          claimRef: stableRef(
            "claim.pylon.portable-phase",
            `${request.operationRef}:${this.options.workerInstanceRef}`,
          ),
          sessionRef: request.sessionRef,
          attachmentRef: request.attachmentRef,
          attachmentGeneration: request.attachmentGeneration,
          pylonRef: request.pylonRef,
          targetRef: request.targetRef,
          workerInstanceRef: this.options.workerInstanceRef,
          leaseExpiresAt,
        },
        signal,
      );
      if (
        !exactRecordBinding(claimed.operation, request) ||
        claimed.operation.state !== "claimed" ||
        claimed.operation.claimRef === null ||
        claimed.operation.claimGeneration === null ||
        claimed.operation.leaseRevision === null ||
        claimed.operation.leaseExpiresAt === null
      ) {
        throw new Error("portable phase claim acknowledgement is invalid");
      }
      const claimRef = claimed.operation.claimRef;
      const claimGeneration = claimed.operation.claimGeneration;
      const leaseRevision = claimed.operation.leaseRevision;
      const acknowledgedLeaseExpiry = claimed.operation.leaseExpiresAt;
      if (
        claimRef === null ||
        claimGeneration === null ||
        leaseRevision === null ||
        acknowledgedLeaseExpiry === null
      )
        throw new Error("portable phase claim acknowledgement is incomplete");
      const active: ActiveClaim = {
        record: claimed.operation,
        claimRef,
        claimGeneration,
        leaseRevision,
        leaseExpiresAt: acknowledgedLeaseExpiry,
        uncertain: false,
      };
      this.active.set(request.operationRef, active);
      await this.executeClaim(active, signal);
      handled += 1;
    }
    return handled;
  }
}

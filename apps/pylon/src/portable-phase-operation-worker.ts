import { createHash } from "node:crypto";
import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  validateIdePortableDestinationActivationReceipt,
  type IdePortableDestinationActivationReceipt,
  type PortableCommandExecutionClaim,
  type PortablePhaseOperationClaimRequest,
  type PortablePhaseOperationRecord,
  type PortablePhaseOperationRequest,
  type PortablePhaseOperationResultRequest,
} from "@openagentsinc/portable-session-contract";
import { Effect } from "effect";
import type { PylonPortableCheckpointArtifactClient } from "./portable-checkpoint-artifact-client.js";
import type { PylonPortablePhaseOperationClient } from "./portable-phase-operation-client.js";
import type {
  PylonPortablePhaseClaimJournal,
  PylonPortablePhaseClaimJournalEntry,
} from "./portable-phase-operation-claim-journal.js";
import type { PylonOwnerLocalExecutionTarget } from "./portable-session-target.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const SHA256 = /^sha256:[a-f0-9]{64}$/u;

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`;

export type PylonPortablePhaseExecutionResult = Readonly<{
  checkpointRef: string | null;
  checkpointObjectRef: string | null;
  checkpointDigest: string | null;
  checkpointManifestDigest: string | null;
  destinationActivationReceipt: IdePortableDestinationActivationReceipt | null;
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
  recoverySemantics: (
    request: PortablePhaseOperationRequest,
  ) => Promise<"not_proven" | "operation_ref_idempotent">;
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
      artifactTransport?: Readonly<{
        commandClaim: PortableCommandExecutionClaim
        byteLimit: number
      }>;
    }>
  | Readonly<{
      kind: "source-cleanup";
      input: Parameters<PylonOwnerLocalExecutionTarget["cleanupSource"]>[0];
    }>
  | Readonly<{
      kind: "checkpoint-stage";
      input: Parameters<PylonOwnerLocalExecutionTarget["stageCheckpoint"]>[0];
      artifactTransport?: Readonly<{
        commandClaim: PortableCommandExecutionClaim
        manifestDigest: string
      }>;
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
        operationRefSemantics: "not_proven" | "operation_ref_idempotent";
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
  checkpointManifestDigest: null,
  destinationActivationReceipt: null,
  evidenceRefs,
});

/**
 * Bind refs-only phase requests to exact private target inputs. The resolver is
 * the only component that can read local graph, cursor, artifact, lease, or
 * execution-binding state. An absent binding is an explicit unsupported phase.
 */
export const makePylonPortablePhaseExecutor = (
  resolver: PylonPortablePhaseTargetResolver,
  artifactTransport?: PylonPortableCheckpointArtifactClient,
): PylonPortablePhaseExecutor => {
  const resolveExact = async (request: PortablePhaseOperationRequest) => {
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
    return resolved;
  };
  return {
    recoverySemantics: async (request) => {
      try {
        return (await resolveExact(request)).operationRefSemantics;
      } catch {
        return "not_proven";
      }
    },
    execute: async (request, signal) => {
      if (signal.aborted) throw signal.reason;
      const resolved = await resolveExact(request);
      if (signal.aborted) throw signal.reason;
      switch (resolved.call.kind) {
        case "quiesce":
          return result((await resolved.target.quiesceGraph(resolved.call.input)).evidenceRefs);
        case "checkpoint-create": {
          const bundle = await resolved.target.createCheckpoint(resolved.call.input);
          let transportEvidenceRefs: ReadonlyArray<string> = [];
          let checkpointManifestDigest: string | null = null;
          if (artifactTransport !== undefined) {
            const context = resolved.call.artifactTransport;
            const artifacts = resolved.target.checkpointArtifacts;
            if (context === undefined || artifacts === undefined) {
              throw new PylonPortablePhaseExecutionError(
                "error.pylon.portable-phase.unsupported-checkpoint-create",
              );
            }
            const exported = await artifacts.exportCustodyObject({
              checkpointRef: bundle.checkpoint.checkpointRef,
              sourcePylonRef: request.pylonRef,
              commandClaim: context.commandClaim,
              byteLimit: context.byteLimit,
            });
            try {
              if (
                exported.manifest.objectRef !== resolved.call.checkpointObjectRef ||
                exported.manifest.checkpointDigest !== bundle.checkpoint.digest
              ) {
                throw new PylonPortablePhaseExecutionError(
                  "error.pylon.portable-phase.context-mismatch",
                );
              }
              const published = await Effect.runPromise(
                artifactTransport.publish({
                  operationRef: request.operationRef,
                  manifest: exported.manifest,
                  bytes: exported.bytes,
                  signal,
                }),
              );
              transportEvidenceRefs = [
                `manifest.portable-checkpoint.${published.manifestDigest.slice("sha256:".length)}`,
              ];
              checkpointManifestDigest = published.manifestDigest;
            } finally {
              exported.bytes.fill(0);
            }
          }
          return {
            checkpointRef: bundle.checkpoint.checkpointRef,
            checkpointObjectRef: resolved.call.checkpointObjectRef,
            checkpointDigest: bundle.checkpoint.digest,
            checkpointManifestDigest,
            destinationActivationReceipt: null,
            evidenceRefs: [...bundle.checkpoint.receiptRefs, ...transportEvidenceRefs],
          };
        }
        case "source-cleanup":
          return result((await resolved.target.cleanupSource(resolved.call.input)).evidenceRefs);
        case "checkpoint-stage": {
          if (artifactTransport !== undefined) {
            const context = resolved.call.artifactTransport;
            const artifacts = resolved.target.checkpointArtifacts;
            if (
              context === undefined ||
              artifacts === undefined ||
              request.checkpointObjectRef === null ||
              request.checkpointDigest === null
            ) {
              throw new PylonPortablePhaseExecutionError(
                "error.pylon.portable-phase.unsupported-checkpoint-stage",
              );
            }
            const redeemed = await Effect.runPromise(
              artifactTransport.redeem({
                operationRef: request.operationRef,
                manifestDigest: context.manifestDigest,
                checkpointObjectRef: request.checkpointObjectRef,
                checkpointDigest: request.checkpointDigest,
                commandClaimRef: context.commandClaim.claimRef,
                signal,
              }),
            );
            try {
              if (
                canonicalJson(redeemed.manifest.commandClaim) !==
                canonicalJson(context.commandClaim)
              ) {
                throw new PylonPortablePhaseExecutionError(
                  "error.pylon.portable-phase.context-mismatch",
                );
              }
              await artifacts.importCustodyObject({
                manifest: redeemed.manifest,
                bytes: redeemed.bytes,
              });
            } finally {
              redeemed.bytes.fill(0);
            }
          }
          return result((await resolved.target.stageCheckpoint(resolved.call.input)).evidenceRefs);
        }
        case "destination-activate": {
          const receipt = await resolved.target.activate(resolved.call.input);
          return {
            checkpointRef: null,
            checkpointObjectRef: null,
            checkpointDigest: null,
            checkpointManifestDigest: null,
            destinationActivationReceipt: receipt,
            evidenceRefs: receipt.evidenceRefs,
          };
        }
        case "staged-abort":
          return result((await resolved.target.abortStaged(resolved.call.input)).evidenceRefs);
      }
    },
  };
};

type ActiveClaim = {
  record: PortablePhaseOperationRecord;
  claimRequest: PortablePhaseOperationClaimRequest;
  claimRef: string;
  claimGeneration: number;
  leaseRevision: number;
  leaseExpiresAt: string;
  completion?: PortablePhaseOperationResultRequest;
  uncertain: boolean;
};

export class PylonPortablePhaseRecoveryError extends Error {
  override readonly name = "PylonPortablePhaseRecoveryError";

  constructor(
    readonly reason:
      | "claim_expired"
      | "claim_taken_over"
      | "non_idempotent_uncertain"
      | "revision_drift"
      | "server_bytes_drift",
  ) {
    super(`Pylon portable phase recovery failed closed: ${reason}`);
  }
}

export type PylonPortablePhaseWorkerOptions = Readonly<{
  client: PylonPortablePhaseOperationClient;
  executor: PylonPortablePhaseExecutor;
  journal: PylonPortablePhaseClaimJournal;
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
  now: Date,
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
    execution.checkpointManifestDigest !== null &&
    (request.kind !== "checkpoint-create" || !SHA256.test(execution.checkpointManifestDigest))
  ) {
    throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.invalid-result");
  }
  if (request.kind === "destination-activate") {
    if (request.checkpointRef === null || execution.destinationActivationReceipt === null) {
      throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.invalid-result");
    }
    const receipt = execution.destinationActivationReceipt;
    validateIdePortableDestinationActivationReceipt(receipt, {
      operationRef: request.operationRef,
      sessionRef: request.sessionRef,
      checkpointRef: request.checkpointRef,
      destinationTargetRef: request.targetRef,
      destinationAttachmentRef: request.attachmentRef,
      destinationGeneration: request.attachmentGeneration,
      authenticationPolicyRef: receipt.authentication.policyRef,
      now,
    });
  } else if (execution.destinationActivationReceipt !== null) {
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
  private recovered = false;

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
    const refs = [...this.active]
      .filter(([, claim]) => claim.uncertain)
      .map(([operationRef]) => operationRef);
    // This is a new array. Sorting it cannot mutate worker state.
    // eslint-disable-next-line unicorn/no-array-sort
    return refs.sort();
  }

  private leaseExpiry(record: PortablePhaseOperationRecord, after: Date): string {
    const requestExpiry = new Date(record.request.expiresAt).valueOf();
    const next = Math.min(requestExpiry, after.valueOf() + this.leaseDurationMs);
    if (next <= after.valueOf())
      throw new PylonPortablePhaseExecutionError("error.pylon.portable-phase.expired");
    return new Date(next).toISOString();
  }

  private claimFromRecord(
    record: PortablePhaseOperationRecord,
    claimRequest: PortablePhaseOperationClaimRequest,
  ): ActiveClaim {
    if (
      record.state !== "claimed" ||
      record.claimRef === null ||
      record.claimRef !== claimRequest.claimRef ||
      record.workerInstanceRef !== this.options.workerInstanceRef ||
      claimRequest.workerInstanceRef !== this.options.workerInstanceRef ||
      record.request.operationRef !== claimRequest.operationRef ||
      record.request.pylonRef !== claimRequest.pylonRef ||
      record.request.targetRef !== claimRequest.targetRef ||
      record.claimGeneration === null ||
      record.leaseRevision === null ||
      record.leaseExpiresAt === null
    ) {
      throw new PylonPortablePhaseRecoveryError("claim_taken_over");
    }
    return {
      record,
      claimRequest,
      claimRef: record.claimRef,
      claimGeneration: record.claimGeneration,
      leaseRevision: record.leaseRevision,
      leaseExpiresAt: record.leaseExpiresAt,
      uncertain: false,
    };
  }

  private journalEntry(
    claim: ActiveClaim,
    state: PylonPortablePhaseClaimJournalEntry["state"],
  ): PylonPortablePhaseClaimJournalEntry {
    return {
      record: claim.record,
      claimRequest: claim.claimRequest,
      claimGeneration: claim.claimGeneration,
      leaseRevision: claim.leaseRevision,
      leaseExpiresAt: claim.leaseExpiresAt,
      state,
      completion: claim.completion ?? null,
    };
  }

  private async persistClaim(
    claim: ActiveClaim,
    state: PylonPortablePhaseClaimJournalEntry["state"],
  ): Promise<void> {
    await this.options.journal.put(this.journalEntry(claim, state));
  }

  private assertServerClaim(
    server: PortablePhaseOperationRecord,
    entry: PylonPortablePhaseClaimJournalEntry,
  ): void {
    if (!exactRecordBinding(server, entry.record.request)) {
      throw new PylonPortablePhaseRecoveryError("server_bytes_drift");
    }
    if (
      server.state !== "claimed" ||
      server.claimRef !== entry.claimRequest.claimRef ||
      server.workerInstanceRef !== this.options.workerInstanceRef ||
      server.claimGeneration !== entry.claimGeneration
    ) {
      throw new PylonPortablePhaseRecoveryError("claim_taken_over");
    }
    if (
      server.leaseRevision !== entry.leaseRevision ||
      server.leaseExpiresAt !== entry.leaseExpiresAt
    ) {
      throw new PylonPortablePhaseRecoveryError("revision_drift");
    }
    if (
      server.leaseExpiresAt === null ||
      new Date(server.leaseExpiresAt) <= this.now() ||
      new Date(server.request.expiresAt) <= this.now()
    ) {
      throw new PylonPortablePhaseRecoveryError("claim_expired");
    }
  }

  private async renewClaim(
    claim: ActiveClaim,
    state: "claimed" | "executing",
    signal: AbortSignal,
  ): Promise<void> {
    if (new Date(claim.leaseExpiresAt) <= this.now()) {
      throw new PylonPortablePhaseRecoveryError("claim_expired");
    }
    const renewedExpiry = this.leaseExpiry(claim.record, new Date(claim.leaseExpiresAt));
    if (new Date(renewedExpiry) <= new Date(claim.leaseExpiresAt)) return;
    const request = claim.record.request;
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
      renewed.operation.workerInstanceRef !== this.options.workerInstanceRef ||
      renewed.operation.claimGeneration !== claim.claimGeneration ||
      renewed.operation.leaseRevision !== claim.leaseRevision + 1 ||
      renewed.operation.leaseExpiresAt !== renewedExpiry
    ) {
      throw new PylonPortablePhaseRecoveryError("revision_drift");
    }
    claim.record = renewed.operation;
    claim.leaseRevision = renewed.operation.leaseRevision;
    claim.leaseExpiresAt = renewed.operation.leaseExpiresAt;
    await this.persistClaim(claim, state);
  }

  private async postCompletion(claim: ActiveClaim, signal: AbortSignal): Promise<void> {
    if (claim.completion === undefined) return;
    const response = await this.options.client.complete(claim.completion, signal);
    const completion = claim.completion;
    if (
      !exactRecordBinding(response.operation, claim.record.request) ||
      !["completed", "failed", "replayed"].includes(response.status) ||
      response.operation.state !== completion.resultStatus ||
      response.operation.leaseRevision !== completion.expectedLeaseRevision + 1 ||
      response.operation.resultRef !== completion.resultRef ||
      response.operation.resultStatus !== completion.resultStatus ||
      response.operation.resultCheckpointRef !== completion.checkpointRef ||
      response.operation.resultCheckpointObjectRef !== completion.checkpointObjectRef ||
      response.operation.resultCheckpointDigest !== completion.checkpointDigest ||
      response.operation.resultCheckpointManifestDigest !== completion.checkpointManifestDigest ||
      canonicalJson(response.operation.resultDestinationActivationReceipt) !==
        canonicalJson(completion.destinationActivationReceipt) ||
      response.operation.errorRef !== completion.errorRef ||
      response.operation.completedAt !== completion.completedAt ||
      response.operation.resultEvidenceRefs.length !== completion.evidenceRefs.length ||
      !response.operation.resultEvidenceRefs.every(
        (ref, index) => ref === completion.evidenceRefs[index],
      )
    ) {
      throw new Error("portable phase completion acknowledgement is invalid");
    }
    await this.options.journal.remove(claim.record.request.operationRef);
    this.active.delete(claim.record.request.operationRef);
  }

  private async executeClaim(claim: ActiveClaim, signal: AbortSignal): Promise<void> {
    const request = claim.record.request;
    await this.persistClaim(claim, "executing");
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
          await this.persistClaim(claim, "uncertain");
          return;
        }
        throw error;
      }
      if (next !== "renew") {
        settled = next;
        break;
      }
      await this.renewClaim(claim, "executing", signal);
    }
    if (signal.aborted) {
      claim.uncertain = true;
      await this.persistClaim(claim, "uncertain");
      return;
    }
    let status: "completed" | "failed";
    let output: PylonPortablePhaseExecutionResult;
    let errorRef: string | null;
    if (settled._tag === "success") {
      try {
        validateResult(request, settled.value, this.now());
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
        await this.persistClaim(claim, "uncertain");
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
      checkpointManifestDigest: output.checkpointManifestDigest,
      destinationActivationReceipt: output.destinationActivationReceipt,
      evidenceRefs: output.evidenceRefs,
      errorRef,
      completedAt,
    };
    await this.persistClaim(claim, "completion_pending");
    await this.postCompletion(claim, signal);
  }

  private async recoverJournal(signal: AbortSignal): Promise<void> {
    if (this.recovered) return;
    const entries = await this.options.journal.entries();
    for (const entry of entries) {
      if (signal.aborted) return;
      const operationRef = entry.record.request.operationRef;
      if (this.active.has(operationRef)) continue;
      let server = await this.options.client.read(operationRef, signal);
      if (!exactRecordBinding(server, entry.record.request)) {
        throw new PylonPortablePhaseRecoveryError("server_bytes_drift");
      }
      if (entry.state === "claiming") {
        if (server.state === "pending") {
          if (new Date(entry.claimRequest.leaseExpiresAt) <= this.now()) {
            await this.options.journal.remove(operationRef);
            continue;
          }
          const claimed = await this.options.client.claim(entry.claimRequest, signal);
          server = claimed.operation;
        }
        if (
          server.state !== "claimed" ||
          server.claimRef !== entry.claimRequest.claimRef ||
          server.workerInstanceRef !== this.options.workerInstanceRef
        ) {
          throw new PylonPortablePhaseRecoveryError("claim_taken_over");
        }
        if (server.leaseExpiresAt === null || new Date(server.leaseExpiresAt) <= this.now()) {
          throw new PylonPortablePhaseRecoveryError("claim_expired");
        }
        const claim = this.claimFromRecord(server, entry.claimRequest);
        this.active.set(operationRef, claim);
        await this.persistClaim(claim, "claimed");
        await this.renewClaim(claim, "claimed", signal);
        await this.executeClaim(claim, signal);
        continue;
      }

      if (entry.state === "completion_pending") {
        if (
          server.claimRef !== entry.claimRequest.claimRef ||
          server.workerInstanceRef !== this.options.workerInstanceRef ||
          server.claimGeneration !== entry.claimGeneration
        ) {
          throw new PylonPortablePhaseRecoveryError("claim_taken_over");
        }
        const claim = this.claimFromRecord(entry.record, entry.claimRequest);
        claim.completion = entry.completion ?? undefined;
        this.active.set(operationRef, claim);
        await this.postCompletion(claim, signal);
        continue;
      }

      this.assertServerClaim(server, entry);
      const claim = this.claimFromRecord(server, entry.claimRequest);
      this.active.set(operationRef, claim);
      if (entry.state === "executing" || entry.state === "uncertain") {
        if (
          (await this.options.executor.recoverySemantics(server.request)) !==
          "operation_ref_idempotent"
        ) {
          claim.uncertain = true;
          await this.persistClaim(claim, "uncertain");
          throw new PylonPortablePhaseRecoveryError("non_idempotent_uncertain");
        }
      }
      await this.renewClaim(claim, "claimed", signal);
      await this.executeClaim(claim, signal);
    }
    this.recovered = true;
  }

  async runPass(signal: AbortSignal = new AbortController().signal): Promise<number> {
    await this.recoverJournal(signal);
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
      const claimRequest = {
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
      } as const;
      await this.options.journal.put({
        record,
        claimRequest,
        claimGeneration: null,
        leaseRevision: null,
        leaseExpiresAt: null,
        state: "claiming",
        completion: null,
      });
      // A transport failure after this point has an unknown server outcome.
      // Reconcile the durable intent before the next poll in this process.
      this.recovered = false;
      const claimed = await this.options.client.claim(claimRequest, signal);
      if (
        !exactRecordBinding(claimed.operation, request) ||
        claimed.operation.state !== "claimed" ||
        claimed.operation.claimRef === null ||
        claimed.operation.claimRef !== claimRequest.claimRef ||
        claimed.operation.workerInstanceRef !== this.options.workerInstanceRef ||
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
        claimRequest,
        claimRef,
        claimGeneration,
        leaseRevision,
        leaseExpiresAt: acknowledgedLeaseExpiry,
        uncertain: false,
      };
      this.active.set(request.operationRef, active);
      await this.persistClaim(active, "claimed");
      await this.executeClaim(active, signal);
      handled += 1;
    }
    return handled;
  }
}

import { createHash } from "node:crypto";

import type {
  PortableOwnerLocalCapabilityOperationClaimRequest,
  PortableOwnerLocalCapabilityOperationRecord,
  PortableOwnerLocalCapabilityOperationRequest,
  PortableOwnerLocalCapabilityOperationResultRequest,
} from "@openagentsinc/portable-session-contract";

import type { PylonPortableOwnerLocalCapabilityOperationClient } from "./portable-owner-local-capability-operation-client.js";
import type {
  PylonPortableOwnerLocalCapabilityOperationJournal,
  PylonPortableOwnerLocalCapabilityOperationJournalEntry,
} from "./portable-owner-local-capability-operation-journal.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`;

export type PylonPortableOwnerLocalCapabilityExecutionOutcome =
  | Readonly<{
      status: "completed";
      resultInstallationRef: string | null;
      receiptRef: string;
      evidenceRefs: ReadonlyArray<string>;
      errorRef: null;
      executableProfileRef?: string;
    }>
  | Readonly<{
      status: "failed";
      resultInstallationRef: null;
      receiptRef: null;
      evidenceRefs: ReadonlyArray<string>;
      errorRef: string;
      executableProfileRef?: never;
    }>;

/** Private buffers are cleanup handles only. The worker never persists or returns them. */
export type PylonPortableOwnerLocalCapabilityExecution = Readonly<{
  outcome: PylonPortableOwnerLocalCapabilityExecutionOutcome;
  privateBuffers?: ReadonlyArray<Uint8Array>;
}>;

export type PylonPortableOwnerLocalCapabilityExecutionClaim = Readonly<{
  claimRef: string;
  workerInstanceRef: string;
  claimGeneration: number;
  expectedLeaseRevision: number;
  expectedLeaseExpiresAt: string;
}>;

export type PylonPortableOwnerLocalCapabilityExecutor = Readonly<{
  execute: (
    request: PortableOwnerLocalCapabilityOperationRequest,
    claim: PylonPortableOwnerLocalCapabilityExecutionClaim,
    signal: AbortSignal,
  ) => Promise<PylonPortableOwnerLocalCapabilityExecution>;
  recoverySemantics: (
    request: PortableOwnerLocalCapabilityOperationRequest,
  ) => Promise<"not_proven" | "operation_ref_idempotent">;
}>;

export class PylonPortableOwnerLocalCapabilityRecoveryError extends Error {
  override readonly name = "PylonPortableOwnerLocalCapabilityRecoveryError";

  constructor(
    readonly reason:
      | "claim_expired"
      | "claim_taken_over"
      | "non_idempotent_uncertain"
      | "revision_drift"
      | "server_bytes_drift",
  ) {
    super(`Pylon portable owner-local capability recovery failed closed: ${reason}`);
  }
}

type ActiveClaim = {
  record: PortableOwnerLocalCapabilityOperationRecord;
  claimRequest: PortableOwnerLocalCapabilityOperationClaimRequest;
  claimGeneration: number;
  leaseRevision: number;
  leaseExpiresAt: string;
  completion?: PortableOwnerLocalCapabilityOperationResultRequest;
  uncertain: boolean;
};

export type PylonPortableOwnerLocalCapabilityWorkerOptions = Readonly<{
  client: PylonPortableOwnerLocalCapabilityOperationClient;
  executor: PylonPortableOwnerLocalCapabilityExecutor;
  journal: PylonPortableOwnerLocalCapabilityOperationJournal;
  pylonRef: string;
  targetRef: string;
  workerInstanceRef: string;
  now?: () => Date;
  pollLimit?: number;
  leaseDurationMs?: number;
  renewalIntervalMs?: number;
  waitForRenewal?: (milliseconds: number, signal: AbortSignal) => Promise<"renew">;
  faultInjector?: (step: "claim_durable", operationRef: string) => Promise<void> | void;
}>;

const delay = (milliseconds: number, signal: AbortSignal): Promise<"renew"> =>
  new Promise((resolve, reject) => {
    if (signal.aborted) return reject(signal.reason);
    const timer = setTimeout(() => resolve("renew"), milliseconds);
    signal.addEventListener(
      "abort",
      () => {
        clearTimeout(timer);
        reject(signal.reason);
      },
      { once: true },
    );
  });

const exactBinding = (
  record: PortableOwnerLocalCapabilityOperationRecord,
  expected: PortableOwnerLocalCapabilityOperationRequest,
): boolean => JSON.stringify(record.request) === JSON.stringify(expected);

const exactCompletion = (
  record: PortableOwnerLocalCapabilityOperationRecord,
  completion: PortableOwnerLocalCapabilityOperationResultRequest,
): boolean =>
  record.state === completion.resultStatus &&
  record.resultRef === completion.resultRef &&
  record.resultStatus === completion.resultStatus &&
  record.resultInstallationRef === completion.resultInstallationRef &&
  record.request.executableProfileRef === completion.executableProfileRef &&
  record.receiptRef === completion.receiptRef &&
  record.errorRef === completion.errorRef &&
  record.completedAt === completion.completedAt &&
  record.resultEvidenceRefs.length === completion.evidenceRefs.length &&
  record.resultEvidenceRefs.every((ref, index) => ref === completion.evidenceRefs[index]);

const validateOutcome = (
  request: PortableOwnerLocalCapabilityOperationRequest,
  outcome: PylonPortableOwnerLocalCapabilityExecutionOutcome,
): void => {
  const exactInstall =
    request.action === "install" &&
    outcome.status === "completed" &&
    outcome.resultInstallationRef !== null &&
    SAFE_REF.test(outcome.resultInstallationRef) &&
    outcome.receiptRef !== null &&
    SAFE_REF.test(outcome.receiptRef) &&
    outcome.evidenceRefs.length === 1;
  const exactWipe =
    request.action === "wipe" &&
    outcome.status === "completed" &&
    outcome.resultInstallationRef === null &&
    outcome.receiptRef !== null &&
    SAFE_REF.test(outcome.receiptRef) &&
    outcome.evidenceRefs.length === 0;
  if (
    outcome.evidenceRefs.length > 256 ||
    new Set(outcome.evidenceRefs).size !== outcome.evidenceRefs.length ||
    outcome.evidenceRefs.some((ref) => !SAFE_REF.test(ref)) ||
    (outcome.status === "completed" &&
      outcome.executableProfileRef !== request.executableProfileRef) ||
    (outcome.status === "completed" && !exactInstall && !exactWipe) ||
    (outcome.status === "failed" &&
      (outcome.resultInstallationRef !== null || !SAFE_REF.test(outcome.errorRef)))
  )
    throw new Error("portable capability executor returned an unsafe refs-only outcome");
};

export class PylonPortableOwnerLocalCapabilityWorker {
  private readonly active = new Map<string, ActiveClaim>();
  private readonly now: () => Date;
  private readonly pollLimit: number;
  private readonly leaseDurationMs: number;
  private readonly renewalIntervalMs: number;
  private recovered = false;

  constructor(private readonly options: PylonPortableOwnerLocalCapabilityWorkerOptions) {
    if (
      ![options.pylonRef, options.targetRef, options.workerInstanceRef].every((ref) =>
        SAFE_REF.test(ref),
      )
    )
      throw new Error("portable capability worker refs are invalid");
    this.now = options.now ?? (() => new Date());
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
    )
      throw new Error("portable capability worker timing is invalid");
  }

  private leaseExpiry(record: PortableOwnerLocalCapabilityOperationRecord, after: Date): string {
    const next = Math.min(
      new Date(record.request.expiresAt).valueOf(),
      after.valueOf() + this.leaseDurationMs,
    );
    if (next <= after.valueOf())
      throw new PylonPortableOwnerLocalCapabilityRecoveryError("claim_expired");
    return new Date(next).toISOString();
  }

  private entry(
    claim: ActiveClaim,
    state: PylonPortableOwnerLocalCapabilityOperationJournalEntry["state"],
  ): PylonPortableOwnerLocalCapabilityOperationJournalEntry {
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

  private async persist(
    claim: ActiveClaim,
    state: PylonPortableOwnerLocalCapabilityOperationJournalEntry["state"],
  ): Promise<void> {
    await this.options.journal.put(this.entry(claim, state));
  }

  private fromRecord(
    record: PortableOwnerLocalCapabilityOperationRecord,
    claimRequest: PortableOwnerLocalCapabilityOperationClaimRequest,
  ): ActiveClaim {
    if (
      record.state !== "claimed" ||
      record.claimRef !== claimRequest.claimRef ||
      record.workerInstanceRef !== this.options.workerInstanceRef ||
      record.claimGeneration === null ||
      record.leaseRevision === null ||
      record.leaseExpiresAt === null
    )
      throw new PylonPortableOwnerLocalCapabilityRecoveryError("claim_taken_over");
    return {
      record,
      claimRequest,
      claimGeneration: record.claimGeneration,
      leaseRevision: record.leaseRevision,
      leaseExpiresAt: record.leaseExpiresAt,
      uncertain: false,
    };
  }

  private async renew(claim: ActiveClaim, signal: AbortSignal): Promise<void> {
    const expiry = this.leaseExpiry(claim.record, new Date(claim.leaseExpiresAt));
    if (expiry === claim.leaseExpiresAt) return;
    const request = claim.record.request;
    const response = await this.options.client.renew(
      {
        schema: "openagents.portable_owner_local_capability_operation.v1",
        claimRef: claim.claimRequest.claimRef,
        pylonRef: request.pylonRef,
        targetRef: request.targetRef,
        sessionRef: request.sessionRef,
        attachmentRef: request.attachmentRef,
        attachmentGeneration: request.attachmentGeneration,
        workerInstanceRef: this.options.workerInstanceRef,
        claimGeneration: claim.claimGeneration,
        expectedLeaseRevision: claim.leaseRevision,
        leaseExpiresAt: expiry,
      },
      signal,
    );
    if (
      !exactBinding(response.operation, request) ||
      response.operation.leaseRevision !== claim.leaseRevision + 1 ||
      response.operation.leaseExpiresAt !== expiry
    )
      throw new PylonPortableOwnerLocalCapabilityRecoveryError("revision_drift");
    claim.record = response.operation;
    claim.leaseRevision += 1;
    claim.leaseExpiresAt = expiry;
    await this.persist(claim, "executing");
  }

  private async complete(claim: ActiveClaim, signal: AbortSignal): Promise<void> {
    if (claim.completion === undefined) return;
    const response = await this.options.client.complete(claim.completion, signal);
    if (
      !exactBinding(response.operation, claim.record.request) ||
      !exactCompletion(response.operation, claim.completion)
    )
      throw new Error("portable capability completion acknowledgement is invalid");
    await this.options.journal.remove(claim.record.request.operationRef);
    this.active.delete(claim.record.request.operationRef);
  }

  private async execute(claim: ActiveClaim, signal: AbortSignal): Promise<void> {
    await this.persist(claim, "executing");
    const running = this.options.executor
      .execute(
        claim.record.request,
        {
          claimRef: claim.claimRequest.claimRef,
          workerInstanceRef: this.options.workerInstanceRef,
          claimGeneration: claim.claimGeneration,
          expectedLeaseRevision: claim.leaseRevision,
          expectedLeaseExpiresAt: claim.leaseExpiresAt,
        },
        signal,
      )
      .then(
        (value) => ({ tag: "success" as const, value }),
        (error: unknown) => ({ tag: "failure" as const, error }),
      );
    let settled: Awaited<typeof running>;
    while (true) {
      const next = await Promise.race([
        running,
        (this.options.waitForRenewal ?? delay)(this.renewalIntervalMs, signal),
      ]);
      if (next !== "renew") {
        settled = next;
        break;
      }
      await this.renew(claim, signal);
    }
    if (signal.aborted) {
      claim.uncertain = true;
      await this.persist(claim, "uncertain");
      return;
    }
    let outcome: PylonPortableOwnerLocalCapabilityExecutionOutcome;
    let privateBuffers: ReadonlyArray<Uint8Array> = [];
    try {
      if (settled.tag === "failure") throw settled.error;
      privateBuffers = settled.value.privateBuffers ?? [];
      validateOutcome(claim.record.request, settled.value.outcome);
      outcome = settled.value.outcome;
    } catch {
      outcome = {
        status: "failed",
        resultInstallationRef: null,
        receiptRef: null,
        evidenceRefs: [],
        errorRef: "error.pylon.portable-capability.operation-failed",
      };
    } finally {
      for (const bytes of privateBuffers) bytes.fill(0);
    }
    const request = claim.record.request;
    claim.completion = {
      schema: "openagents.portable_owner_local_capability_operation.v1",
      claimRef: claim.claimRequest.claimRef,
      pylonRef: request.pylonRef,
      targetRef: request.targetRef,
      sessionRef: request.sessionRef,
      attachmentRef: request.attachmentRef,
      attachmentGeneration: request.attachmentGeneration,
      workerInstanceRef: this.options.workerInstanceRef,
      claimGeneration: claim.claimGeneration,
      expectedLeaseRevision: claim.leaseRevision,
      resultRef: stableRef(
        "result.pylon.portable-capability",
        `${request.operationRef}:${outcome.status}`,
      ),
      resultStatus: outcome.status,
      resultInstallationRef: outcome.resultInstallationRef,
      ...(request.executableProfileRef === undefined
        ? {}
        : { executableProfileRef: request.executableProfileRef }),
      receiptRef: outcome.receiptRef,
      evidenceRefs: outcome.evidenceRefs,
      errorRef: outcome.errorRef,
      completedAt: this.now().toISOString(),
    };
    await this.persist(claim, "completion_pending");
    await this.complete(claim, signal);
  }

  private async recover(signal: AbortSignal): Promise<void> {
    if (this.recovered) return;
    for (const entry of await this.options.journal.entries()) {
      let server = await this.options.client.read(entry.record.request.operationRef, signal);
      if (!exactBinding(server, entry.record.request))
        throw new PylonPortableOwnerLocalCapabilityRecoveryError("server_bytes_drift");
      if (entry.state === "claiming") {
        if (server.state === "pending")
          server = (await this.options.client.claim(entry.claimRequest, signal)).operation;
        const claim = this.fromRecord(server, entry.claimRequest);
        this.active.set(server.request.operationRef, claim);
        await this.persist(claim, "claimed");
        await this.execute(claim, signal);
        continue;
      }
      if (entry.state === "completion_pending") {
        if (server.state === "completed" || server.state === "failed") {
          if (entry.completion === null || !exactCompletion(server, entry.completion))
            throw new PylonPortableOwnerLocalCapabilityRecoveryError("server_bytes_drift");
          await this.options.journal.remove(server.request.operationRef);
          continue;
        }
        const claim = this.fromRecord(entry.record, entry.claimRequest);
        claim.completion = entry.completion ?? undefined;
        this.active.set(server.request.operationRef, claim);
        await this.complete(claim, signal);
        continue;
      }
      if (
        server.state !== "claimed" ||
        server.claimRef !== entry.claimRequest.claimRef ||
        server.leaseRevision !== entry.leaseRevision
      )
        throw new PylonPortableOwnerLocalCapabilityRecoveryError("claim_taken_over");
      const claim = this.fromRecord(server, entry.claimRequest);
      this.active.set(server.request.operationRef, claim);
      if (
        (entry.state === "executing" || entry.state === "uncertain") &&
        (await this.options.executor.recoverySemantics(server.request)) !==
          "operation_ref_idempotent"
      ) {
        claim.uncertain = true;
        await this.persist(claim, "uncertain");
        throw new PylonPortableOwnerLocalCapabilityRecoveryError("non_idempotent_uncertain");
      }
      await this.renew(claim, signal);
      await this.execute(claim, signal);
    }
    this.recovered = true;
  }

  async runPass(signal: AbortSignal = new AbortController().signal): Promise<number> {
    await this.recover(signal);
    let handled = 0;
    const observedOperationRefs = new Set<string>();
    for (const record of await this.options.client.pending(this.pollLimit, signal)) {
      const request = record.request;
      if (
        signal.aborted ||
        request.pylonRef !== this.options.pylonRef ||
        request.targetRef !== this.options.targetRef ||
        this.active.has(request.operationRef) ||
        observedOperationRefs.has(request.operationRef)
      )
        continue;
      observedOperationRefs.add(request.operationRef);
      const claimRequest = {
        schema: "openagents.portable_owner_local_capability_operation.v1",
        operationRef: request.operationRef,
        claimRef: stableRef(
          "claim.pylon.portable-capability",
          `${request.operationRef}:${this.options.workerInstanceRef}`,
        ),
        pylonRef: request.pylonRef,
        targetRef: request.targetRef,
        sessionRef: request.sessionRef,
        attachmentRef: request.attachmentRef,
        attachmentGeneration: request.attachmentGeneration,
        workerInstanceRef: this.options.workerInstanceRef,
        leaseExpiresAt: this.leaseExpiry(record, this.now()),
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
      this.recovered = false;
      const response = await this.options.client.claim(claimRequest, signal);
      const claim = this.fromRecord(response.operation, claimRequest);
      this.active.set(request.operationRef, claim);
      await this.persist(claim, "claimed");
      await this.options.faultInjector?.("claim_durable", request.operationRef);
      await this.execute(claim, signal);
      handled += 1;
    }
    return handled;
  }
}

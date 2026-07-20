import {
  PortableAgentGraphSchema,
  PortableCheckpointSchema,
  PortableCommandExecutionClaimSchema,
  PortablePhaseOperationRecordSchema,
  PortablePhaseOperationRequestSchema,
  PortableRef,
  PortableSessionExecutionBindingSchema,
  PortableTargetDescriptorSchema,
  type PortableCommandExecutionClaim,
  type PortablePhaseOperationKind,
  type PortablePhaseOperationRecord,
  type PortablePhaseOperationRequest,
  type PortableTargetDescriptor,
  validateIdePortableDestinationActivationReceipt,
} from "@openagentsinc/portable-session-contract";
import { canonicalJson } from "@openagentsinc/khala-sync";
import { Duration, Effect, Schedule, Schema } from "effect";

import {
  PostgresPortablePhaseOperationStore,
  PortablePhaseOperationStoreError,
} from "./portable-phase-operation-store.js";
import type {
  PortableCheckpointBundle,
  PortableSessionExecutionTarget,
  PortableSourceCleanupReceipt,
  PortableTargetActivationReceipt,
  PortableTargetStageReceipt,
} from "./portable-session-move.js";
import type { SyncSql } from "./sql.js";

const FORBIDDEN_PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|authorization|refreshToken|mnemonic|password|credential|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:/iu;

const PortableCheckpointBundleSchema = Schema.Struct({
  checkpoint: PortableCheckpointSchema,
  executionBinding: PortableSessionExecutionBindingSchema,
  graph: PortableAgentGraphSchema,
  threadCursors: Schema.Array(
    Schema.Struct({
      threadRef: PortableRef,
      transcriptRef: PortableRef,
      activityCursor: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
      eventCursor: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
    }),
  ),
});

const decodeClaim = Schema.decodeUnknownSync(PortableCommandExecutionClaimSchema);
const decodeTarget = Schema.decodeUnknownSync(PortableTargetDescriptorSchema);
const decodeRequest = Schema.decodeUnknownSync(PortablePhaseOperationRequestSchema);
const decodeRecord = Schema.decodeUnknownSync(PortablePhaseOperationRecordSchema);
const decodeBundle = Schema.decodeUnknownSync(PortableCheckpointBundleSchema);
const decodeGraph = Schema.decodeUnknownSync(PortableAgentGraphSchema);
const decodeRefs = Schema.decodeUnknownSync(Schema.Array(PortableRef));

type OperationRow = Readonly<{
  request_json: unknown;
  request_fingerprint: string;
  state: string;
  claim_ref: string | null;
  claim_fingerprint: string | null;
  worker_instance_ref: string | null;
  claim_generation: string | number | null;
  lease_revision: string | number | null;
  claimed_at: Date | string | null;
  lease_expires_at: Date | string | null;
  result_ref: string | null;
  result_fingerprint: string | null;
  result_status: string | null;
  result_checkpoint_ref: string | null;
  result_checkpoint_object_ref: string | null;
  result_checkpoint_digest: string | null;
  result_checkpoint_manifest_digest: string | null;
  result_destination_activation_receipt_json: unknown;
  result_evidence_refs_json: unknown;
  error_ref: string | null;
  completed_at: Date | string | null;
  updated_at: Date | string;
}>;

export class PortablePhaseTargetError extends Error {
  readonly _tag = "PortablePhaseTargetError";
  override readonly name = "PortablePhaseTargetError";

  constructor(
    readonly code:
      | "invalid"
      | "conflict"
      | "failed"
      | "expired"
      | "timeout"
      | "canceled"
      | "unsafe_result",
    message: string,
    readonly errorRef?: string,
  ) {
    super(message);
  }
}

export type PortablePhaseTargetCheckpointArtifact = Readonly<{
  checkpointRef: string;
  checkpointObjectRef: string;
  checkpointDigest: string;
  checkpointManifestDigest: string | null;
}>;

export type PostgresPortablePhaseTargetConfig = Readonly<{
  sql: SyncSql;
  commandExecutionClaim: PortableCommandExecutionClaim;
  target: PortableTargetDescriptor;
  operationExpiresAt: string;
  resolvePylonRef: (
    input: Readonly<{
      ownerRef: string;
      sessionRef: string;
      targetRef: string;
      commandExecutionClaimRef: string;
    }>,
  ) => Promise<string>;
  resolveCheckpointBundle: (
    artifact: PortablePhaseTargetCheckpointArtifact,
  ) => Promise<PortableCheckpointBundle>;
  pollInterval?: Duration.Input;
  timeout?: Duration.Input;
  signal?: AbortSignal;
  now?: () => string;
  /** Wake transport seam. The durable store remains the only phase authority. */
  onEnqueued?: (
    result: Readonly<{
      status: "enqueued" | "replayed";
      operation: PortablePhaseOperationRecord;
      /** Exact private-context transport binding for checkpoint-stage admission. */
      artifactTransport: Readonly<{
        commandClaim: PortableCommandExecutionClaim;
        manifestDigest: string;
      }> | null;
    }>,
  ) => Promise<void>;
}>;

const parseJson = (value: unknown): unknown =>
  typeof value === "string" ? JSON.parse(value) : value;

const iso = (value: Date | string | null): string | null =>
  value === null ? null : new Date(value).toISOString();

const positive = (value: string | number | null): number | null => {
  if (value === null) return null;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new PortablePhaseTargetError("invalid", "portable phase numeric state is invalid");
  }
  return parsed;
};

const publicSafe = <A>(value: A): A => {
  if (FORBIDDEN_PRIVATE_MATERIAL.test(JSON.stringify(value))) {
    throw new PortablePhaseTargetError(
      "unsafe_result",
      "portable phase target value contains forbidden private material",
    );
  }
  return value;
};

const recordFromRow = (row: OperationRow): PortablePhaseOperationRecord =>
  decodeRecord({
    request: parseJson(row.request_json),
    requestFingerprint: row.request_fingerprint,
    state: row.state,
    claimRef: row.claim_ref,
    claimFingerprint: row.claim_fingerprint,
    workerInstanceRef: row.worker_instance_ref,
    claimGeneration: positive(row.claim_generation),
    leaseRevision: positive(row.lease_revision),
    claimedAt: iso(row.claimed_at),
    leaseExpiresAt: iso(row.lease_expires_at),
    resultRef: row.result_ref,
    resultFingerprint: row.result_fingerprint,
    resultStatus: row.result_status,
    resultCheckpointRef: row.result_checkpoint_ref,
    resultCheckpointObjectRef: row.result_checkpoint_object_ref,
    resultCheckpointDigest: row.result_checkpoint_digest,
    resultCheckpointManifestDigest: row.result_checkpoint_manifest_digest,
    resultDestinationActivationReceipt: parseJson(row.result_destination_activation_receipt_json),
    resultEvidenceRefs: parseJson(row.result_evidence_refs_json),
    errorRef: row.error_ref,
    completedAt: iso(row.completed_at),
    updatedAt: iso(row.updated_at),
  });

type PendingPhase = Readonly<{ _tag: "PendingPhase" }>;

/**
 * Refs-only server target for a Pylon that executes one claimed portable move.
 * The adapter never claims a phase. It only enqueues exact bytes and observes
 * the durable terminal record written by the bound Pylon worker.
 */
export class PostgresPortablePhaseTarget implements PortableSessionExecutionTarget {
  readonly targetRef: string;
  readonly targetClass: PortableTargetDescriptor["targetClass"];

  private readonly claim: PortableCommandExecutionClaim;
  private readonly target: PortableTargetDescriptor;
  private readonly store: PostgresPortablePhaseOperationStore;
  private resolvedPylonRef: Promise<string> | undefined;

  constructor(private readonly config: PostgresPortablePhaseTargetConfig) {
    try {
      this.claim = decodeClaim(config.commandExecutionClaim);
      this.target = decodeTarget(config.target);
    } catch {
      throw new PortablePhaseTargetError("invalid", "portable phase target scope is invalid");
    }
    this.targetRef = this.target.targetRef;
    this.targetClass = this.target.targetClass;
    this.store = new PostgresPortablePhaseOperationStore(config.sql, config.now);
    const current = new Date(config.now?.() ?? new Date().toISOString());
    if (
      this.target.ownerRef !== this.claim.ownerRef ||
      this.target.health !== "ready" ||
      !["claimed", "pending_reconcile"].includes(this.claim.state) ||
      new Date(this.claim.leaseExpiresAt) <= current ||
      !Number.isFinite(new Date(config.operationExpiresAt).valueOf()) ||
      new Date(config.operationExpiresAt) <= current ||
      new Date(config.operationExpiresAt) > new Date(this.claim.leaseExpiresAt)
    ) {
      throw new PortablePhaseTargetError(
        "invalid",
        "portable phase target requires ready owner authority and an active execution claim",
      );
    }
  }

  async quiesceGraph(
    input: Parameters<PortableSessionExecutionTarget["quiesceGraph"]>[0],
  ): Promise<
    Readonly<{ quiescedAgentRefs: ReadonlyArray<string>; evidenceRefs: ReadonlyArray<string> }>
  > {
    const graph = decodeGraph(input.graph);
    const result = await this.run("quiesce", {
      operationRef: input.operationRef,
      sessionRef: input.sessionRef,
      attachmentRef: input.attachmentRef,
      attachmentGeneration: input.generation,
    });
    return publicSafe({
      quiescedAgentRefs: graph.nodes.map((node) => node.agentRef),
      evidenceRefs: result.resultEvidenceRefs,
    });
  }

  async createCheckpoint(
    input: Parameters<PortableSessionExecutionTarget["createCheckpoint"]>[0],
  ): Promise<PortableCheckpointBundle> {
    const result = await this.run("checkpoint-create", {
      operationRef: input.operationRef,
      sessionRef: input.sessionRef,
      attachmentRef: input.attachmentRef,
      attachmentGeneration: input.generation,
      checkpointRef: input.checkpointRef,
    });
    const artifact = this.artifact(result);
    const bundle = await this.resolveArtifactBundle(artifact);
    if (
      bundle.checkpoint.eventLogCursor !== input.eventLogCursor ||
      canonicalJson(bundle.executionBinding) !== canonicalJson(input.executionBinding) ||
      canonicalJson(bundle.graph) !== canonicalJson(input.graph) ||
      canonicalJson(bundle.threadCursors) !== canonicalJson(input.threadCursors)
    ) {
      throw new PortablePhaseTargetError(
        "conflict",
        "resolved checkpoint bundle differs from canonical source state",
      );
    }
    return bundle;
  }

  async cleanupSource(
    input: Parameters<PortableSessionExecutionTarget["cleanupSource"]>[0],
  ): Promise<PortableSourceCleanupReceipt> {
    const agentRefs = decodeRefs(input.agentRefs);
    const result = await this.run("source-cleanup", {
      operationRef: input.operationRef,
      sessionRef: input.sessionRef,
      attachmentRef: input.attachmentRef,
      attachmentGeneration: input.generation,
    });
    return publicSafe({
      cleanedAgentRefs: agentRefs,
      processes: "released",
      scratch: "released",
      ports: "released",
      evidenceRefs: result.resultEvidenceRefs,
    });
  }

  async stageCheckpoint(
    input: Parameters<PortableSessionExecutionTarget["stageCheckpoint"]>[0],
  ): Promise<PortableTargetStageReceipt> {
    const bundle = decodeBundle(publicSafe(input.bundle));
    const artifact = await this.existingCheckpointArtifact(
      bundle.checkpoint.checkpointRef,
      bundle.checkpoint.digest,
    );
    const durableBundle = await this.resolveArtifactBundle(artifact);
    if (canonicalJson(durableBundle) !== canonicalJson(bundle)) {
      throw new PortablePhaseTargetError(
        "conflict",
        "staged checkpoint bundle differs from the durable artifact",
      );
    }
    const result = await this.run("checkpoint-stage", {
      operationRef: input.operationRef,
      sessionRef: bundle.checkpoint.sessionRef,
      attachmentRef: input.destinationAttachmentRef,
      attachmentGeneration: input.destinationGeneration,
      ...artifact,
    });
    return publicSafe({
      checkpointDigest: bundle.checkpoint.digest,
      repositoryPostImageDigest: bundle.checkpoint.repositoryPostImageDigest,
      diffDigest: bundle.checkpoint.diffDigest,
      graphDigest: bundle.checkpoint.graphDigest,
      threadCursors: bundle.threadCursors,
      acceptingWork: false,
      evidenceRefs: result.resultEvidenceRefs,
    });
  }

  async activate(
    input: Parameters<PortableSessionExecutionTarget["activate"]>[0],
  ): Promise<PortableTargetActivationReceipt> {
    const artifact = await this.existingCheckpointArtifact(input.checkpointRef);
    const bundle = await this.resolveArtifactBundle(artifact);
    if (canonicalJson(bundle.executionBinding) !== canonicalJson(input.executionBinding)) {
      throw new PortablePhaseTargetError(
        "conflict",
        "portable activation execution binding differs from the durable checkpoint",
      );
    }
    const result = await this.run("destination-activate", {
      operationRef: input.operationRef,
      sessionRef: input.sessionRef,
      attachmentRef: input.destinationAttachmentRef,
      attachmentGeneration: input.destinationGeneration,
      ...artifact,
    });
    const authenticationPolicyRef = `policy.portable.destination.${this.targetClass}.v1`;
    const receipt = result.resultDestinationActivationReceipt;
    if (receipt === null || result.completedAt === null) {
      throw new PortablePhaseTargetError(
        "invalid",
        "portable destination activation receipt is missing",
      );
    }
    return validateIdePortableDestinationActivationReceipt(publicSafe(receipt), {
      operationRef: input.operationRef,
      sessionRef: input.sessionRef,
      checkpointRef: input.checkpointRef,
      destinationTargetRef: this.targetRef,
      destinationAttachmentRef: input.destinationAttachmentRef,
      destinationGeneration: input.destinationGeneration,
      authenticationPolicyRef,
      now: new Date(result.completedAt),
    });
  }

  async abortStaged(
    input: Parameters<PortableSessionExecutionTarget["abortStaged"]>[0],
  ): Promise<Readonly<{ evidenceRefs: ReadonlyArray<string> }>> {
    const result = await this.run("staged-abort", {
      operationRef: input.operationRef,
      sessionRef: input.sessionRef,
      attachmentRef: input.destinationAttachmentRef,
      attachmentGeneration: input.destinationGeneration,
    });
    return publicSafe({ evidenceRefs: result.resultEvidenceRefs });
  }

  private async run(
    kind: PortablePhaseOperationKind,
    input: Readonly<{
      operationRef: string;
      sessionRef: string;
      attachmentRef: string;
      attachmentGeneration: number;
      checkpointRef?: string;
      checkpointObjectRef?: string;
      checkpointDigest?: string;
      checkpointManifestDigest?: string | null;
    }>,
  ): Promise<PortablePhaseOperationRecord> {
    this.assertCommandScope(
      input.sessionRef,
      input.attachmentRef,
      input.attachmentGeneration,
      kind,
    );
    await this.assertPhaseOrder(kind, input.attachmentRef, input.attachmentGeneration);
    const pylonRef = await this.pylonRef();
    const request = decodeRequest({
      schema: "openagents.portable_phase_operation.v1",
      operationRef: input.operationRef,
      commandRef: this.claim.commandRef,
      commandExecutionClaimRef: this.claim.claimRef,
      ownerRef: this.claim.ownerRef,
      sessionRef: input.sessionRef,
      attachmentRef: input.attachmentRef,
      attachmentGeneration: input.attachmentGeneration,
      targetRef: this.targetRef,
      pylonRef,
      kind,
      checkpointRef: input.checkpointRef ?? null,
      checkpointObjectRef: input.checkpointObjectRef ?? null,
      checkpointDigest: input.checkpointDigest ?? null,
      evidenceRefs: [],
      expiresAt: this.config.operationExpiresAt,
    });
    try {
      const enqueued = await this.store.enqueue(request);
      await this.config.onEnqueued?.({
        ...enqueued,
        artifactTransport:
          kind === "checkpoint-stage" && input.checkpointManifestDigest !== null &&
            input.checkpointManifestDigest !== undefined
            ? {
                commandClaim: this.claim,
                manifestDigest: input.checkpointManifestDigest,
              }
            : null,
      });
      return await this.awaitTerminal(request);
    } catch (error) {
      if (error instanceof PortablePhaseTargetError) throw error;
      if (error instanceof PortablePhaseOperationStoreError) {
        throw new PortablePhaseTargetError(
          error.code === "expired"
            ? "expired"
            : error.code === "unsafe_material"
              ? "unsafe_result"
              : error.code === "conflict" || error.code === "stale_generation"
                ? "conflict"
                : "invalid",
          error.message,
        );
      }
      throw error;
    }
  }

  private async awaitTerminal(
    request: PortablePhaseOperationRequest,
  ): Promise<PortablePhaseOperationRecord> {
    const read = Effect.tryPromise({
      try: () => this.readOperation(request.operationRef),
      catch: (cause) =>
        new PortablePhaseTargetError(
          "invalid",
          cause instanceof Error ? cause.message : "portable phase state read failed",
        ),
    }).pipe(
      Effect.flatMap((record) => {
        if (record === undefined || record.state === "pending" || record.state === "claimed") {
          return Effect.fail<PendingPhase>({ _tag: "PendingPhase" });
        }
        return Effect.succeed(record);
      }),
      Effect.retry({
        schedule: Schedule.spaced(this.config.pollInterval ?? "25 millis"),
        while: (error) => error._tag === "PendingPhase",
      }),
      Effect.timeoutOrElse({
        duration: this.config.timeout ?? "30 seconds",
        orElse: () =>
          Effect.fail(
            new PortablePhaseTargetError(
              "timeout",
              `portable phase ${request.kind} did not reach a durable terminal state`,
            ),
          ),
      }),
    );
    let record: PortablePhaseOperationRecord;
    try {
      record = await Effect.runPromise(read, { signal: this.config.signal });
    } catch (error) {
      if (this.config.signal?.aborted) {
        throw new PortablePhaseTargetError("canceled", "portable phase wait was canceled");
      }
      throw error;
    }
    if (
      record.requestFingerprint !== (await this.store.enqueue(request)).operation.requestFingerprint
    ) {
      throw new PortablePhaseTargetError("conflict", "portable phase terminal bytes changed");
    }
    if (record.state === "failed") {
      throw new PortablePhaseTargetError(
        "failed",
        `portable phase ${request.kind} failed`,
        record.errorRef ?? undefined,
      );
    }
    if (record.state === "expired") {
      throw new PortablePhaseTargetError("expired", `portable phase ${request.kind} expired`);
    }
    if (record.state !== "completed") {
      throw new PortablePhaseTargetError("invalid", "portable phase terminal state is invalid");
    }
    return record;
  }

  private async readOperation(
    operationRef: string,
  ): Promise<PortablePhaseOperationRecord | undefined> {
    const rows: OperationRow[] = await this.config.sql`
      SELECT request_json, request_fingerprint, state, claim_ref, claim_fingerprint,
             worker_instance_ref, claim_generation, lease_revision, claimed_at,
             lease_expires_at, result_ref, result_fingerprint, result_status,
             result_checkpoint_ref, result_checkpoint_object_ref,
             result_checkpoint_digest, result_checkpoint_manifest_digest,
             result_destination_activation_receipt_json,
             result_evidence_refs_json, error_ref,
             completed_at, updated_at
      FROM khala_sync_portable_phase_operations
      WHERE operation_ref = ${operationRef}
    `;
    return rows[0] === undefined ? undefined : recordFromRow(rows[0]);
  }

  private async existingCheckpointArtifact(
    checkpointRef: string,
    checkpointDigest?: string,
  ): Promise<PortablePhaseTargetCheckpointArtifact> {
    const rows: OperationRow[] = await this.config.sql`
      SELECT request_json, request_fingerprint, state, claim_ref, claim_fingerprint,
             worker_instance_ref, claim_generation, lease_revision, claimed_at,
             lease_expires_at, result_ref, result_fingerprint, result_status,
             result_checkpoint_ref, result_checkpoint_object_ref,
             result_checkpoint_digest, result_checkpoint_manifest_digest,
             result_destination_activation_receipt_json,
             result_evidence_refs_json, error_ref,
             completed_at, updated_at
      FROM khala_sync_portable_phase_operations
      WHERE command_execution_claim_ref = ${this.claim.claimRef}
        AND kind = 'checkpoint-create'
    `;
    const record = rows[0] === undefined ? undefined : recordFromRow(rows[0]);
    if (record?.state !== "completed") {
      throw new PortablePhaseTargetError(
        "conflict",
        "portable checkpoint creation is not complete",
      );
    }
    const artifact = this.artifact(record);
    if (
      artifact.checkpointRef !== checkpointRef ||
      (checkpointDigest !== undefined && artifact.checkpointDigest !== checkpointDigest)
    ) {
      throw new PortablePhaseTargetError("conflict", "portable checkpoint artifact differs");
    }
    return artifact;
  }

  private async assertPhaseOrder(
    kind: PortablePhaseOperationKind,
    attachmentRef: string,
    attachmentGeneration: number,
  ): Promise<void> {
    const predecessor: Partial<Record<PortablePhaseOperationKind, PortablePhaseOperationKind>> = {
      "checkpoint-create": "quiesce",
      "checkpoint-stage": "checkpoint-create",
      "source-cleanup": "checkpoint-stage",
      "destination-activate": "source-cleanup",
      "staged-abort": "checkpoint-stage",
    };
    const required = predecessor[kind];
    if (required === undefined) return;
    const rows: Array<{ state: string; request_json: unknown }> = await this.config.sql`
      SELECT state, request_json
      FROM khala_sync_portable_phase_operations
      WHERE command_execution_claim_ref = ${this.claim.claimRef} AND kind = ${required}
    `;
    if (rows[0]?.state !== "completed") {
      throw new PortablePhaseTargetError(
        "conflict",
        `portable phase ${kind} requires completed ${required}`,
      );
    }
    if (kind === "destination-activate" || kind === "staged-abort") {
      const stagedRows: Array<{ state: string; request_json: unknown }> = await this.config.sql`
        SELECT state, request_json
        FROM khala_sync_portable_phase_operations
        WHERE command_execution_claim_ref = ${this.claim.claimRef}
          AND kind = 'checkpoint-stage'
      `;
      const staged = stagedRows[0];
      if (staged?.state !== "completed") {
        throw new PortablePhaseTargetError("conflict", "portable destination stage is incomplete");
      }
      const stagedRequest = decodeRequest(parseJson(staged.request_json));
      if (
        stagedRequest.attachmentRef !== attachmentRef ||
        stagedRequest.attachmentGeneration !== attachmentGeneration ||
        stagedRequest.targetRef !== this.targetRef
      ) {
        throw new PortablePhaseTargetError(
          "conflict",
          "portable destination phase differs from the exact staged attachment",
        );
      }
    }
  }

  private async resolveArtifactBundle(
    artifact: PortablePhaseTargetCheckpointArtifact,
  ): Promise<PortableCheckpointBundle> {
    const resolved = publicSafe(await this.config.resolveCheckpointBundle(artifact));
    const bundle = decodeBundle(resolved);
    if (
      bundle.checkpoint.checkpointRef !== artifact.checkpointRef ||
      bundle.checkpoint.digest !== artifact.checkpointDigest ||
      bundle.checkpoint.sessionRef !== this.claim.sessionRef ||
      bundle.checkpoint.sourceAttachmentRef !== this.claim.sourceAttachmentRef ||
      bundle.checkpoint.sourceGeneration !== this.claim.sourceGeneration ||
      bundle.executionBinding.ownerRef !== this.claim.ownerRef ||
      bundle.executionBinding.sessionRef !== this.claim.sessionRef
    ) {
      throw new PortablePhaseTargetError(
        "conflict",
        "resolved checkpoint bundle differs from the durable artifact binding",
      );
    }
    return bundle;
  }

  private pylonRef(): Promise<string> {
    this.resolvedPylonRef ??= this.config
      .resolvePylonRef({
        ownerRef: this.claim.ownerRef,
        sessionRef: this.claim.sessionRef,
        targetRef: this.targetRef,
        commandExecutionClaimRef: this.claim.claimRef,
      })
      .then((value) =>
        Schema.decodeUnknownSync(PortablePhaseOperationRequestSchema.fields.pylonRef)(value),
      )
      .catch(() => {
        throw new PortablePhaseTargetError("invalid", "portable phase Pylon binding is invalid");
      });
    return this.resolvedPylonRef;
  }

  private artifact(record: PortablePhaseOperationRecord): PortablePhaseTargetCheckpointArtifact {
    if (
      record.resultCheckpointRef === null ||
      record.resultCheckpointObjectRef === null ||
      record.resultCheckpointDigest === null
    ) {
      throw new PortablePhaseTargetError("invalid", "portable checkpoint result is incomplete");
    }
    return publicSafe({
      checkpointRef: record.resultCheckpointRef,
      checkpointObjectRef: record.resultCheckpointObjectRef,
      checkpointDigest: record.resultCheckpointDigest,
      checkpointManifestDigest: record.resultCheckpointManifestDigest,
    });
  }

  private assertCommandScope(
    sessionRef: string,
    attachmentRef: string,
    generation: number,
    kind: PortablePhaseOperationKind,
  ): void {
    const source = kind === "quiesce" || kind === "checkpoint-create" || kind === "source-cleanup";
    if (
      sessionRef !== this.claim.sessionRef ||
      (source
        ? attachmentRef !== this.claim.sourceAttachmentRef ||
          generation !== this.claim.sourceGeneration ||
          this.targetRef !== this.claim.executorEnvironmentRef
        : generation !== this.claim.sourceGeneration + 1 ||
          this.targetRef !== this.claim.destinationTargetRef)
    ) {
      throw new PortablePhaseTargetError(
        "conflict",
        "portable phase target differs from the command execution claim",
      );
    }
  }
}

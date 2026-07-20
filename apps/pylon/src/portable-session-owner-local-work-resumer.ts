import type { LegacySqliteDatabase as Database } from "@openagentsinc/sqlite-runtime";
import { createHash } from "node:crypto";
import { chmod, mkdir, readFile, rename, rm, rmdir, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";

import { canonicalJson } from "@openagentsinc/khala-sync";
import { Effect, Schema } from "effect";

import { PylonPortableSessionOperationLedger } from "./portable-session-operation-ledger.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const CHECKPOINT_SCHEMA = "openagents.pylon.portable_owner_local_work_checkpoint.v1" as const;
const RECEIPT_SCHEMA = "openagents.pylon.portable_owner_local_work_resume.v1" as const;
const PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:task|prompt|token|authorization|password|secret|credential|path|hostname|processId|pid|socket|port)"\s*:/iu;

const Ref = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(SAFE_REF),
);

export const PylonPortableOwnerLocalWorkCheckpointSchema = Schema.Struct({
  schema: Schema.Literal(CHECKPOINT_SCHEMA),
  workRef: Ref,
  handlerRef: Ref,
  sessionRef: Ref,
  sourceAttachmentRef: Ref,
  sourceGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  agentRef: Ref,
});

export interface PylonPortableOwnerLocalWorkCheckpoint extends Schema.Schema.Type<
  typeof PylonPortableOwnerLocalWorkCheckpointSchema
> {}

export const PylonPortableOwnerLocalWorkResumeReceiptSchema = Schema.Struct({
  schema: Schema.Literal(RECEIPT_SCHEMA),
  receiptRef: Ref,
  operationRef: Ref,
  workRef: Ref,
  handlerRef: Ref,
  agentRef: Ref,
  sessionRef: Ref,
  sourceAttachmentRef: Ref,
  sourceGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  destinationAttachmentRef: Ref,
  destinationGeneration: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  acceptedWorkRefs: Schema.Array(Schema.Struct({ agentRef: Ref, workRef: Ref })).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(1),
  ),
  resultRef: Ref,
  evidenceRefs: Schema.Array(Ref).check(Schema.isMinLength(1), Schema.isMaxLength(32)),
  processState: Schema.Literal("excluded"),
  replay: Schema.Literals(["executed", "recovered", "replayed"]),
});

export interface PylonPortableOwnerLocalWorkResumeReceipt extends Schema.Schema.Type<
  typeof PylonPortableOwnerLocalWorkResumeReceiptSchema
> {}

const decodeWorkCheckpoint = Schema.decodeUnknownSync(PylonPortableOwnerLocalWorkCheckpointSchema);
const decodeWorkReceipt = Schema.decodeUnknownSync(PylonPortableOwnerLocalWorkResumeReceiptSchema);

export type PylonPortableOwnerLocalWorkResult = Readonly<{
  resultRef: string;
  evidenceRefs: ReadonlyArray<string>;
}>;

export type PylonPortableOwnerLocalWorkHandlerInput = Readonly<{
  workRef: string;
  agentRef: string;
  workspaceRoot: string;
  sourceGeneration: number;
  destinationGeneration: number;
  idempotencyRef: string;
}>;

/**
 * A handler is admitted only when it can reconcile the durable effect for one
 * stable idempotency ref. Reconcile must return the exact prior result when the
 * effect exists, and null only when execute is safe to call with the same ref.
 */
export interface PylonPortableOwnerLocalWorkHandler {
  readonly recoveryContract: "durable_idempotency_reconcile_v1";
  readonly reconcile: (
    input: PylonPortableOwnerLocalWorkHandlerInput,
  ) => Promise<PylonPortableOwnerLocalWorkResult | null>;
  readonly execute: (
    input: PylonPortableOwnerLocalWorkHandlerInput,
  ) => Promise<PylonPortableOwnerLocalWorkResult>;
}

export interface PylonPortableOwnerLocalWorkRecoveryAuthority {
  readonly authorityRef: string;
  readonly verifyInterrupted: (
    input: Readonly<{
      interruptionEvidenceRef: string;
      workRef: string;
      operationRef: string;
      sessionRef: string;
      destinationAttachmentRef: string;
      destinationGeneration: number;
      activeRecoveryEvidenceRef: string | null;
    }>,
  ) => Promise<boolean>;
}

type WorkRow = Readonly<{
  work_ref: string;
  fingerprint: string;
  session_ref: string;
  source_attachment_ref: string;
  source_generation: number;
  agent_ref: string;
  handler_ref: string;
  state: "accepted" | "running" | "settled";
  operation_ref: string | null;
  destination_attachment_ref: string | null;
  destination_generation: number | null;
  recovery_evidence_ref: string | null;
  receipt_json: string | null;
}>;

export class PylonPortableOwnerLocalWorkResumeError extends Error {
  readonly _tag = "PylonPortableOwnerLocalWorkResumeError";
  override readonly name = "PylonPortableOwnerLocalWorkResumeError";

  constructor(
    readonly reason:
      | "conflicting_replay"
      | "executor_failed"
      | "invalid_checkpoint"
      | "stale_generation"
      | "unverified_recovery"
      | "unsafe_handler"
      | "unknown_handler",
    message: string,
  ) {
    super(message);
  }
}

const checkpointLeaf = (workRef: string): string =>
  `${createHash("sha256").update(workRef).digest("hex")}.json`;

const checkpointPath = (workspaceRoot: string, workRef: string): string =>
  join(workspaceRoot, ".openagents", "portable-work", checkpointLeaf(workRef));

const fingerprint = (checkpoint: PylonPortableOwnerLocalWorkCheckpoint): string =>
  `sha256:${createHash("sha256").update(canonicalJson(checkpoint)).digest("hex")}`;

const stableRef = (prefix: string, seed: string): string =>
  `${prefix}.${createHash("sha256").update(seed).digest("hex").slice(0, 32)}`;

const assertRef = (value: string, field: string): void => {
  if (!SAFE_REF.test(value)) {
    throw new PylonPortableOwnerLocalWorkResumeError(
      "invalid_checkpoint",
      `${field} is not a public-safe ref`,
    );
  }
};

const assertWorkspace = (workspaceRoot: string): void => {
  if (!isAbsolute(workspaceRoot)) {
    throw new PylonPortableOwnerLocalWorkResumeError(
      "invalid_checkpoint",
      "owner-local resume requires an absolute workspace root",
    );
  }
  const controlRoot = resolve(workspaceRoot, ".openagents", "portable-work");
  if (relative(resolve(workspaceRoot), controlRoot).startsWith("..")) {
    throw new PylonPortableOwnerLocalWorkResumeError(
      "invalid_checkpoint",
      "owner-local resume control root escapes the workspace",
    );
  }
};

const decodeCheckpoint = (value: unknown): PylonPortableOwnerLocalWorkCheckpoint => {
  try {
    const checkpoint = decodeWorkCheckpoint(value);
    if (PRIVATE_MATERIAL.test(canonicalJson(checkpoint))) throw new Error("private material");
    return checkpoint;
  } catch {
    throw new PylonPortableOwnerLocalWorkResumeError(
      "invalid_checkpoint",
      "owner-local accepted-work checkpoint is invalid",
    );
  }
};

const decodeReceipt = (value: unknown): PylonPortableOwnerLocalWorkResumeReceipt => {
  try {
    const receipt = decodeWorkReceipt(value);
    if (
      PRIVATE_MATERIAL.test(canonicalJson(receipt)) ||
      new Set(receipt.evidenceRefs).size !== receipt.evidenceRefs.length
    ) {
      throw new Error("unsafe receipt");
    }
    return receipt;
  } catch {
    throw new PylonPortableOwnerLocalWorkResumeError(
      "conflicting_replay",
      "settled owner-local work receipt is invalid",
    );
  }
};

const assertAdmittedHandler = (
  handler: PylonPortableOwnerLocalWorkHandler | undefined,
): PylonPortableOwnerLocalWorkHandler => {
  if (handler === undefined) {
    throw new PylonPortableOwnerLocalWorkResumeError(
      "unknown_handler",
      "owner-local accepted-work handler is not installed at the destination",
    );
  }
  if (
    handler.recoveryContract !== "durable_idempotency_reconcile_v1" ||
    typeof handler.reconcile !== "function" ||
    typeof handler.execute !== "function"
  ) {
    throw new PylonPortableOwnerLocalWorkResumeError(
      "unsafe_handler",
      "owner-local accepted-work handler has no admitted durable recovery contract",
    );
  }
  return handler;
};

const validateResult = (
  completed: PylonPortableOwnerLocalWorkResult,
): PylonPortableOwnerLocalWorkResult => {
  assertRef(completed.resultRef, "resultRef");
  if (
    completed.evidenceRefs.length === 0 ||
    completed.evidenceRefs.length > 32 ||
    new Set(completed.evidenceRefs).size !== completed.evidenceRefs.length
  ) {
    throw new Error("executor evidence is invalid");
  }
  for (const evidenceRef of completed.evidenceRefs) assertRef(evidenceRef, "evidenceRef");
  return completed;
};

export const createPylonPortableOwnerLocalWorkResumer = (
  input: Readonly<{
    database: Database;
    ledger: PylonPortableSessionOperationLedger;
    handlers: ReadonlyMap<string, PylonPortableOwnerLocalWorkHandler>;
    recoveryAuthority?: PylonPortableOwnerLocalWorkRecoveryAuthority;
  }>,
) => {
  if (input.recoveryAuthority !== undefined) {
    assertRef(input.recoveryAuthority.authorityRef, "recoveryAuthority.authorityRef");
  }
  input.database.exec(`
    CREATE TABLE IF NOT EXISTS pylon_portable_owner_local_work (
      work_ref TEXT PRIMARY KEY,
      fingerprint TEXT NOT NULL,
      session_ref TEXT NOT NULL,
      source_attachment_ref TEXT NOT NULL,
      source_generation INTEGER NOT NULL,
      agent_ref TEXT NOT NULL,
      handler_ref TEXT NOT NULL,
      state TEXT NOT NULL CHECK (state IN ('accepted', 'running', 'settled')),
      operation_ref TEXT,
      destination_attachment_ref TEXT,
      destination_generation INTEGER,
      recovery_evidence_ref TEXT,
      receipt_json TEXT
    )
  `);

  const columns = input.database
    .query("PRAGMA table_info(pylon_portable_owner_local_work)")
    .all() as ReadonlyArray<Readonly<{ name: string }>> | undefined;
  if (!(columns ?? []).some((column) => column.name === "recovery_evidence_ref")) {
    input.database.exec(
      "ALTER TABLE pylon_portable_owner_local_work ADD COLUMN recovery_evidence_ref TEXT",
    );
  }

  const readRow = (workRef: string): WorkRow | null =>
    input.database
      .query(`
    SELECT work_ref, fingerprint, session_ref, source_attachment_ref,
           source_generation, agent_ref, handler_ref, state, operation_ref,
           destination_attachment_ref, destination_generation, recovery_evidence_ref,
           receipt_json
    FROM pylon_portable_owner_local_work WHERE work_ref = ?
  `)
      .get(workRef) as WorkRow | null;

  const assertFence = (
    scope: Readonly<{
      sessionRef: string;
      attachmentRef: string;
      generation: number;
    }>,
  ): void => {
    const fence = Effect.runSync(input.ledger.readSession(scope.sessionRef));
    if (
      fence.attachmentRef !== scope.attachmentRef ||
      fence.generation !== scope.generation ||
      fence.acceptingWork !== true
    ) {
      throw new PylonPortableOwnerLocalWorkResumeError(
        "stale_generation",
        "owner-local work does not match the exclusive accepting generation",
      );
    }
  };

  const removeCheckpoint = async (workspaceRoot: string, workRef: string): Promise<void> => {
    const path = checkpointPath(workspaceRoot, workRef);
    await rm(path, { force: true });
    await rmdir(dirname(path)).catch(() => undefined);
    await rmdir(dirname(dirname(path))).catch(() => undefined);
  };

  return {
    accept: async (
      accepted: Readonly<{
        workRef: string;
        handlerRef: string;
        sessionRef: string;
        sourceAttachmentRef: string;
        sourceGeneration: number;
        agentRef: string;
        workspaceRoot: string;
      }>,
    ): Promise<PylonPortableOwnerLocalWorkCheckpoint> => {
      assertWorkspace(accepted.workspaceRoot);
      for (const [field, value] of Object.entries({
        workRef: accepted.workRef,
        handlerRef: accepted.handlerRef,
        sessionRef: accepted.sessionRef,
        sourceAttachmentRef: accepted.sourceAttachmentRef,
        agentRef: accepted.agentRef,
      }))
        assertRef(value, field);
      if (!Number.isSafeInteger(accepted.sourceGeneration) || accepted.sourceGeneration <= 0) {
        throw new PylonPortableOwnerLocalWorkResumeError(
          "invalid_checkpoint",
          "owner-local accepted-work generation is invalid",
        );
      }
      assertAdmittedHandler(input.handlers.get(accepted.handlerRef));
      assertFence({
        sessionRef: accepted.sessionRef,
        attachmentRef: accepted.sourceAttachmentRef,
        generation: accepted.sourceGeneration,
      });
      const checkpoint = decodeCheckpoint({
        schema: CHECKPOINT_SCHEMA,
        workRef: accepted.workRef,
        handlerRef: accepted.handlerRef,
        sessionRef: accepted.sessionRef,
        sourceAttachmentRef: accepted.sourceAttachmentRef,
        sourceGeneration: accepted.sourceGeneration,
        agentRef: accepted.agentRef,
      });
      const exactFingerprint = fingerprint(checkpoint);
      input.database
        .transaction(() => {
          const prior = readRow(accepted.workRef);
          if (prior !== null) {
            if (prior.fingerprint !== exactFingerprint || prior.state !== "accepted") {
              throw new PylonPortableOwnerLocalWorkResumeError(
                "conflicting_replay",
                "owner-local accepted work conflicts with its prior admission",
              );
            }
            return;
          }
          input.database
            .query(`
          INSERT INTO pylon_portable_owner_local_work
            (work_ref, fingerprint, session_ref, source_attachment_ref,
             source_generation, agent_ref, handler_ref, state)
          VALUES (?, ?, ?, ?, ?, ?, ?, 'accepted')
        `)
            .run(
              accepted.workRef,
              exactFingerprint,
              accepted.sessionRef,
              accepted.sourceAttachmentRef,
              accepted.sourceGeneration,
              accepted.agentRef,
              accepted.handlerRef,
            );
        })
        .immediate();
      const path = checkpointPath(accepted.workspaceRoot, accepted.workRef);
      await mkdir(dirname(path), { recursive: true, mode: 0o700 });
      const temporary = `${path}.tmp`;
      await writeFile(temporary, `${canonicalJson(checkpoint)}\n`, { mode: 0o600 });
      await rename(temporary, path);
      await chmod(path, 0o600);
      return checkpoint;
    },

    resume: async (
      resume: Readonly<{
        operationRef: string;
        workRef: string;
        agentRef: string;
        sessionRef: string;
        destinationAttachmentRef: string;
        destinationGeneration: number;
        workspaceRoot: string;
        interruptionEvidenceRef?: string;
      }>,
    ): Promise<PylonPortableOwnerLocalWorkResumeReceipt> => {
      assertWorkspace(resume.workspaceRoot);
      for (const [field, value] of Object.entries({
        operationRef: resume.operationRef,
        workRef: resume.workRef,
        agentRef: resume.agentRef,
        sessionRef: resume.sessionRef,
        destinationAttachmentRef: resume.destinationAttachmentRef,
      }))
        assertRef(value, field);
      if (resume.interruptionEvidenceRef !== undefined) {
        assertRef(resume.interruptionEvidenceRef, "interruptionEvidenceRef");
      }
      if (
        !Number.isSafeInteger(resume.destinationGeneration) ||
        resume.destinationGeneration <= 1
      ) {
        throw new PylonPortableOwnerLocalWorkResumeError(
          "invalid_checkpoint",
          "owner-local destination generation is invalid",
        );
      }
      assertFence({
        sessionRef: resume.sessionRef,
        attachmentRef: resume.destinationAttachmentRef,
        generation: resume.destinationGeneration,
      });
      const settledReplay = readRow(resume.workRef);
      if (settledReplay?.state === "settled") {
        if (
          settledReplay.operation_ref !== resume.operationRef ||
          settledReplay.session_ref !== resume.sessionRef ||
          settledReplay.agent_ref !== resume.agentRef ||
          settledReplay.destination_attachment_ref !== resume.destinationAttachmentRef ||
          Number(settledReplay.destination_generation) !== resume.destinationGeneration ||
          settledReplay.receipt_json === null
        ) {
          throw new PylonPortableOwnerLocalWorkResumeError(
            "conflicting_replay",
            "owner-local settled work conflicts with the resume request",
          );
        }
        await removeCheckpoint(resume.workspaceRoot, resume.workRef);
        return {
          ...decodeReceipt(JSON.parse(settledReplay.receipt_json)),
          replay: "replayed",
        };
      }
      let checkpoint: PylonPortableOwnerLocalWorkCheckpoint;
      try {
        checkpoint = decodeCheckpoint(
          JSON.parse(await readFile(checkpointPath(resume.workspaceRoot, resume.workRef), "utf8")),
        );
      } catch (error) {
        if (error instanceof PylonPortableOwnerLocalWorkResumeError) throw error;
        throw new PylonPortableOwnerLocalWorkResumeError(
          "invalid_checkpoint",
          "owner-local accepted-work checkpoint is unavailable",
        );
      }
      if (
        checkpoint.workRef !== resume.workRef ||
        checkpoint.agentRef !== resume.agentRef ||
        checkpoint.sessionRef !== resume.sessionRef ||
        checkpoint.sourceGeneration + 1 !== resume.destinationGeneration
      ) {
        throw new PylonPortableOwnerLocalWorkResumeError(
          "stale_generation",
          "owner-local accepted-work checkpoint does not bind the destination generation",
        );
      }
      const handler = assertAdmittedHandler(input.handlers.get(checkpoint.handlerRef));
      let verifiedRecoveryPrior: string | null | undefined;
      if (resume.interruptionEvidenceRef !== undefined) {
        const interrupted = readRow(resume.workRef);
        if (
          interrupted === null ||
          interrupted.state !== "running" ||
          interrupted.operation_ref !== resume.operationRef ||
          interrupted.session_ref !== resume.sessionRef ||
          interrupted.agent_ref !== resume.agentRef ||
          interrupted.destination_attachment_ref !== resume.destinationAttachmentRef ||
          Number(interrupted.destination_generation) !== resume.destinationGeneration ||
          interrupted.recovery_evidence_ref === resume.interruptionEvidenceRef ||
          input.recoveryAuthority === undefined
        ) {
          throw new PylonPortableOwnerLocalWorkResumeError(
            "unverified_recovery",
            "owner-local work recovery has no verified interrupted executor",
          );
        }
        const verified = await input.recoveryAuthority.verifyInterrupted({
          interruptionEvidenceRef: resume.interruptionEvidenceRef,
          workRef: resume.workRef,
          operationRef: resume.operationRef,
          sessionRef: resume.sessionRef,
          destinationAttachmentRef: resume.destinationAttachmentRef,
          destinationGeneration: resume.destinationGeneration,
          activeRecoveryEvidenceRef: interrupted.recovery_evidence_ref,
        });
        if (!verified) {
          throw new PylonPortableOwnerLocalWorkResumeError(
            "unverified_recovery",
            "owner-local work recovery has no verified interrupted executor",
          );
        }
        verifiedRecoveryPrior = interrupted.recovery_evidence_ref;
      }
      const claim = input.database
        .transaction(() => {
          const row = readRow(resume.workRef);
          if (row === null || row.fingerprint !== fingerprint(checkpoint)) {
            throw new PylonPortableOwnerLocalWorkResumeError(
              "invalid_checkpoint",
              "owner-local accepted work has no exact durable admission",
            );
          }
          if (row.state === "settled") {
            if (
              row.operation_ref !== resume.operationRef ||
              row.destination_attachment_ref !== resume.destinationAttachmentRef ||
              Number(row.destination_generation) !== resume.destinationGeneration ||
              row.receipt_json === null
            ) {
              throw new PylonPortableOwnerLocalWorkResumeError(
                "conflicting_replay",
                "owner-local settled work conflicts with the resume request",
              );
            }
            return {
              kind: "settled" as const,
              receipt: decodeReceipt(JSON.parse(row.receipt_json)),
            };
          }
          if (row.state === "running") {
            if (
              resume.interruptionEvidenceRef === undefined ||
              row.operation_ref !== resume.operationRef ||
              row.destination_attachment_ref !== resume.destinationAttachmentRef ||
              Number(row.destination_generation) !== resume.destinationGeneration ||
              row.recovery_evidence_ref === resume.interruptionEvidenceRef
            ) {
              throw new PylonPortableOwnerLocalWorkResumeError(
                "conflicting_replay",
                "owner-local accepted work already has an active executor",
              );
            }
            const recoveryCas =
              verifiedRecoveryPrior === null
                ? "recovery_evidence_ref IS NULL"
                : "recovery_evidence_ref = ?";
            const recoveryArguments = [
              resume.interruptionEvidenceRef,
              resume.workRef,
              resume.operationRef,
              resume.destinationAttachmentRef,
              resume.destinationGeneration,
              ...(verifiedRecoveryPrior === null ? [] : [verifiedRecoveryPrior]),
            ];
            const recovered = input.database
              .query(`
            UPDATE pylon_portable_owner_local_work
            SET recovery_evidence_ref = ?
            WHERE work_ref = ? AND state = 'running' AND operation_ref = ?
              AND destination_attachment_ref = ? AND destination_generation = ?
              AND ${recoveryCas}
          `)
              .run(...recoveryArguments);
            if (recovered.changes !== 1) {
              throw new PylonPortableOwnerLocalWorkResumeError(
                "conflicting_replay",
                "owner-local work recovery lost its interruption evidence fence",
              );
            }
            return { kind: "recover" as const };
          }
          if (resume.interruptionEvidenceRef !== undefined) {
            throw new PylonPortableOwnerLocalWorkResumeError(
              "conflicting_replay",
              "owner-local recovery evidence does not bind an interrupted executor",
            );
          }
          const claimed = input.database
            .query(`
          UPDATE pylon_portable_owner_local_work
          SET state = 'running', operation_ref = ?, destination_attachment_ref = ?,
              destination_generation = ?
          WHERE work_ref = ? AND state = 'accepted'
        `)
            .run(
              resume.operationRef,
              resume.destinationAttachmentRef,
              resume.destinationGeneration,
              resume.workRef,
            );
          if (claimed.changes !== 1) {
            throw new PylonPortableOwnerLocalWorkResumeError(
              "conflicting_replay",
              "owner-local work executor lost its exclusive claim",
            );
          }
          return { kind: "execute" as const };
        })
        .immediate();
      if (claim.kind === "settled") {
        await removeCheckpoint(resume.workspaceRoot, resume.workRef);
        return { ...claim.receipt, replay: "replayed" };
      }

      const handlerInput: PylonPortableOwnerLocalWorkHandlerInput = {
        workRef: checkpoint.workRef,
        agentRef: checkpoint.agentRef,
        workspaceRoot: resume.workspaceRoot,
        sourceGeneration: checkpoint.sourceGeneration,
        destinationGeneration: resume.destinationGeneration,
        idempotencyRef: stableRef(
          "idempotency.pylon.portable.owner-local-work",
          `${fingerprint(checkpoint)}:${resume.operationRef}`,
        ),
      };
      let completed: PylonPortableOwnerLocalWorkResult;
      try {
        const reconciled = claim.kind === "recover" ? await handler.reconcile(handlerInput) : null;
        completed = validateResult(reconciled ?? (await handler.execute(handlerInput)));
      } catch (error) {
        throw new PylonPortableOwnerLocalWorkResumeError(
          "executor_failed",
          error instanceof Error
            ? `owner-local executor failed: ${error.message}`
            : "owner-local executor failed",
        );
      }
      const receipt = decodeReceipt({
        schema: RECEIPT_SCHEMA,
        receiptRef: stableRef("receipt.pylon.portable.owner-local-work", resume.operationRef),
        operationRef: resume.operationRef,
        workRef: checkpoint.workRef,
        handlerRef: checkpoint.handlerRef,
        agentRef: checkpoint.agentRef,
        sessionRef: checkpoint.sessionRef,
        sourceAttachmentRef: checkpoint.sourceAttachmentRef,
        sourceGeneration: checkpoint.sourceGeneration,
        destinationAttachmentRef: resume.destinationAttachmentRef,
        destinationGeneration: resume.destinationGeneration,
        acceptedWorkRefs: [{ agentRef: checkpoint.agentRef, workRef: checkpoint.workRef }],
        resultRef: completed.resultRef,
        evidenceRefs: completed.evidenceRefs,
        processState: "excluded",
        replay: claim.kind === "recover" ? "recovered" : "executed",
      });
      input.database
        .transaction(() => {
          const settled = input.database
            .query(`
          UPDATE pylon_portable_owner_local_work
          SET state = 'settled', receipt_json = ?
          WHERE work_ref = ? AND state = 'running' AND operation_ref = ?
            AND destination_attachment_ref = ? AND destination_generation = ?
        `)
            .run(
              canonicalJson(receipt),
              resume.workRef,
              resume.operationRef,
              resume.destinationAttachmentRef,
              resume.destinationGeneration,
            );
          if (settled.changes !== 1) {
            throw new PylonPortableOwnerLocalWorkResumeError(
              "conflicting_replay",
              "owner-local work lost its generation fence before settlement",
            );
          }
        })
        .immediate();
      await removeCheckpoint(resume.workspaceRoot, resume.workRef);
      return receipt;
    },

    readState: (workRef: string): "accepted" | "running" | "settled" | null => {
      assertRef(workRef, "workRef");
      return readRow(workRef)?.state ?? null;
    },
  };
};

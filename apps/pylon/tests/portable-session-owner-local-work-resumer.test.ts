import { openLegacySqliteDatabase, type LegacySqliteDatabase } from "@openagentsinc/sqlite-runtime";
import { access, cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, test } from "vite-plus/test";

import { PylonPortableSessionOperationLedger } from "../src/portable-session-operation-ledger.js";
import {
  createPylonPortableOwnerLocalWorkResumer,
  type PylonPortableOwnerLocalWorkHandler,
  PylonPortableOwnerLocalWorkResumeError,
} from "../src/portable-session-owner-local-work-resumer.js";

const roots: string[] = [];

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

const sessionRef = "session.ide13.owner-local.resume";
const sourceAttachmentRef = "attachment.ide13.owner-local.resume.1";
const destinationAttachmentRef = "attachment.ide13.owner-local.resume.2";
const agentRef = "agent.ide13.owner-local.resume.root";
const workRef = "work.ide13.owner-local.resume.safe-edit";
const handlerRef = "handler.ide13.owner-local.resume.safe-edit.v1";
const operationRef = "operation.ide13.owner-local.resume.work.2";

const insertFence = (
  database: LegacySqliteDatabase,
  attachmentRef: string,
  generation: number,
): void => {
  database
    .query(`
      INSERT INTO pylon_portable_session_fences
        (session_ref, attachment_ref, generation, accepting_work, revision)
      VALUES (?, ?, ?, 1, 0)
    `)
    .run(sessionRef, attachmentRef, generation);
};

const advanceFence = (
  database: LegacySqliteDatabase,
  sourceAttachment: string,
  destinationAttachment: string,
  sourceGeneration: number,
): void => {
  const advanced = database
    .query(`
      UPDATE pylon_portable_session_fences
      SET attachment_ref = ?, generation = ?, accepting_work = 1, revision = revision + 1
      WHERE session_ref = ? AND attachment_ref = ? AND generation = ?
    `)
    .run(
      destinationAttachment,
      sourceGeneration + 1,
      sessionRef,
      sourceAttachment,
      sourceGeneration,
    );
  expect(advanced.changes).toBe(1);
};

describe("Pylon owner-local portable accepted-work resumer", () => {
  test("checkpoints one admitted ref, runs it only at the destination, and settles without control residue", async () => {
    const root = await mkdtemp(join(tmpdir(), "openagents-portable-work-resume-"));
    roots.push(root);
    const source = join(root, "source");
    const destination = join(root, "destination");
    await mkdir(source, { recursive: true });
    await writeFile(join(source, "tracked.txt"), "source generation\n", "utf8");

    const database = openLegacySqliteDatabase(join(root, "portable.sqlite"));
    const ledger = new PylonPortableSessionOperationLedger(database);
    insertFence(database, sourceAttachmentRef, 1);
    let executionCount = 0;
    const resumer = createPylonPortableOwnerLocalWorkResumer({
      database,
      ledger,
      handlers: new Map([
        [
          handlerRef,
          {
            recoveryContract: "durable_idempotency_reconcile_v1",
            reconcile: async () => null,
            execute: async (input) => {
              executionCount += 1;
              expect(input.sourceGeneration).toBe(1);
              expect(input.destinationGeneration).toBe(2);
              const path = join(input.workspaceRoot, "tracked.txt");
              await writeFile(
                path,
                `${await readFile(path, "utf8")}resumed generation 2\n`,
                "utf8",
              );
              return {
                resultRef: "result.ide13.owner-local.resume.safe-edit",
                evidenceRefs: ["evidence.ide13.owner-local.resume.safe-edit.settled"],
              };
            },
          },
        ],
      ]),
    });

    await resumer.accept({
      workRef,
      handlerRef,
      sessionRef,
      sourceAttachmentRef,
      sourceGeneration: 1,
      agentRef,
      workspaceRoot: source,
    });
    expect(executionCount).toBe(0);
    expect(resumer.readState(workRef)).toBe("accepted");

    await cp(source, destination, { recursive: true });
    advanceFence(database, sourceAttachmentRef, destinationAttachmentRef, 1);

    const request = {
      operationRef,
      workRef,
      agentRef,
      sessionRef,
      destinationAttachmentRef,
      destinationGeneration: 2,
      workspaceRoot: destination,
    };
    const completed = await resumer.resume(request);
    expect(completed).toMatchObject({
      replay: "executed",
      processState: "excluded",
      acceptedWorkRefs: [{ agentRef, workRef }],
      resultRef: "result.ide13.owner-local.resume.safe-edit",
    });
    expect(executionCount).toBe(1);
    expect(resumer.readState(workRef)).toBe("settled");
    expect(await readFile(join(destination, "tracked.txt"), "utf8")).toBe(
      "source generation\nresumed generation 2\n",
    );
    await expect(access(join(destination, ".openagents"))).rejects.toMatchObject({
      code: "ENOENT",
    });

    const replay = await resumer.resume(request);
    expect(replay.replay).toBe("replayed");
    expect(replay.receiptRef).toBe(completed.receiptRef);
    expect(executionCount).toBe(1);
  });

  test("reconciles a durable effect after executor death without executing the handler twice", async () => {
    const root = await mkdtemp(join(tmpdir(), "openagents-portable-work-crash-recovery-"));
    roots.push(root);
    const workspace = join(root, "workspace");
    const databasePath = join(root, "portable.sqlite");
    const durableEffectPath = join(workspace, "durable-effect.json");
    await mkdir(workspace, { recursive: true });
    let database = openLegacySqliteDatabase(databasePath);
    let ledger = new PylonPortableSessionOperationLedger(database);
    insertFence(database, sourceAttachmentRef, 1);
    let executionCount = 0;
    let reconciliationCount = 0;
    const result = {
      resultRef: "result.ide13.owner-local.resume.crash-recovered",
      evidenceRefs: ["evidence.ide13.owner-local.resume.crash-recovered"],
    };
    const handler: PylonPortableOwnerLocalWorkHandler = {
      recoveryContract: "durable_idempotency_reconcile_v1",
      reconcile: async (input) => {
        reconciliationCount += 1;
        const durable = await readFile(durableEffectPath, "utf8").then(JSON.parse, () => null);
        return durable?.idempotencyRef === input.idempotencyRef ? result : null;
      },
      execute: async (input) => {
        executionCount += 1;
        await writeFile(
          durableEffectPath,
          `${JSON.stringify({ idempotencyRef: input.idempotencyRef })}\n`,
          "utf8",
        );
        throw new Error("injected executor death after durable effect");
      },
    };
    let resumer = createPylonPortableOwnerLocalWorkResumer({
      database,
      ledger,
      handlers: new Map([[handlerRef, handler]]),
    });
    await resumer.accept({
      workRef,
      handlerRef,
      sessionRef,
      sourceAttachmentRef,
      sourceGeneration: 1,
      agentRef,
      workspaceRoot: workspace,
    });
    advanceFence(database, sourceAttachmentRef, destinationAttachmentRef, 1);
    const request = {
      operationRef,
      workRef,
      agentRef,
      sessionRef,
      destinationAttachmentRef,
      destinationGeneration: 2,
      workspaceRoot: workspace,
    };
    await expect(resumer.resume(request)).rejects.toMatchObject({ reason: "executor_failed" });
    expect(resumer.readState(workRef)).toBe("running");

    database.close();
    database = openLegacySqliteDatabase(databasePath);
    ledger = new PylonPortableSessionOperationLedger(database);
    resumer = createPylonPortableOwnerLocalWorkResumer({
      database,
      ledger,
      handlers: new Map([[handlerRef, handler]]),
    });
    await expect(resumer.resume(request)).rejects.toMatchObject({ reason: "conflicting_replay" });
    await expect(
      resumer.resume({
        ...request,
        interruptionEvidenceRef: "evidence.ide13.owner-local.executor.interrupted.1",
      }),
    ).rejects.toMatchObject({ reason: "unverified_recovery" });
    resumer = createPylonPortableOwnerLocalWorkResumer({
      database,
      ledger,
      handlers: new Map([[handlerRef, handler]]),
      recoveryAuthority: {
        authorityRef: "authority.ide13.owner-local.executor-process-death.v1",
        verifyInterrupted: async (input) =>
          input.interruptionEvidenceRef === "evidence.ide13.owner-local.executor.interrupted.1" &&
          input.workRef === workRef &&
          input.operationRef === operationRef &&
          input.sessionRef === sessionRef &&
          input.destinationAttachmentRef === destinationAttachmentRef &&
          input.destinationGeneration === 2 &&
          input.activeRecoveryEvidenceRef === null,
      },
    });
    const recovered = await resumer.resume({
      ...request,
      interruptionEvidenceRef: "evidence.ide13.owner-local.executor.interrupted.1",
    });
    expect(recovered.replay).toBe("recovered");
    expect(recovered.resultRef).toBe(result.resultRef);
    expect(executionCount).toBe(1);
    expect(reconciliationCount).toBe(1);
    expect(resumer.readState(workRef)).toBe("settled");
    const replayed = await resumer.resume(request);
    expect(replayed.replay).toBe("replayed");
    expect(executionCount).toBe(1);
    expect(reconciliationCount).toBe(1);
    database.close();
  });

  test("refuses a handler without the durable recovery contract", async () => {
    const root = await mkdtemp(join(tmpdir(), "openagents-portable-work-unsafe-handler-"));
    roots.push(root);
    const workspace = join(root, "workspace");
    await mkdir(workspace, { recursive: true });
    const database = openLegacySqliteDatabase(join(root, "portable.sqlite"));
    const ledger = new PylonPortableSessionOperationLedger(database);
    insertFence(database, sourceAttachmentRef, 1);
    const unsafeHandler = (async () => ({
      resultRef: "result.ide13.owner-local.resume.unsafe",
      evidenceRefs: ["evidence.ide13.owner-local.resume.unsafe"],
    })) as unknown as PylonPortableOwnerLocalWorkHandler;
    const resumer = createPylonPortableOwnerLocalWorkResumer({
      database,
      ledger,
      handlers: new Map([[handlerRef, unsafeHandler]]),
    });
    await expect(
      resumer.accept({
        workRef,
        handlerRef,
        sessionRef,
        sourceAttachmentRef,
        sourceGeneration: 1,
        agentRef,
        workspaceRoot: workspace,
      }),
    ).rejects.toMatchObject({ reason: "unsafe_handler" });
    database.close();
  });

  test("admits only one concurrent recovery claim against the observed interruption epoch", async () => {
    const root = await mkdtemp(join(tmpdir(), "openagents-portable-work-recovery-cas-"));
    roots.push(root);
    const workspace = join(root, "workspace");
    await mkdir(workspace, { recursive: true });
    const database = openLegacySqliteDatabase(join(root, "portable.sqlite"));
    const ledger = new PylonPortableSessionOperationLedger(database);
    insertFence(database, sourceAttachmentRef, 1);
    const failedHandler: PylonPortableOwnerLocalWorkHandler = {
      recoveryContract: "durable_idempotency_reconcile_v1",
      reconcile: async () => null,
      execute: async () => {
        throw new Error("injected interrupted executor");
      },
    };
    let resumer = createPylonPortableOwnerLocalWorkResumer({
      database,
      ledger,
      handlers: new Map([[handlerRef, failedHandler]]),
    });
    await resumer.accept({
      workRef,
      handlerRef,
      sessionRef,
      sourceAttachmentRef,
      sourceGeneration: 1,
      agentRef,
      workspaceRoot: workspace,
    });
    advanceFence(database, sourceAttachmentRef, destinationAttachmentRef, 1);
    const request = {
      operationRef,
      workRef,
      agentRef,
      sessionRef,
      destinationAttachmentRef,
      destinationGeneration: 2,
      workspaceRoot: workspace,
    };
    await expect(resumer.resume(request)).rejects.toMatchObject({ reason: "executor_failed" });
    let verificationCount = 0;
    let releaseVerification: (() => void) | undefined;
    const verificationBarrier = new Promise<void>((resolve) => {
      releaseVerification = resolve;
    });
    let reconciliationCount = 0;
    const recoveryHandler: PylonPortableOwnerLocalWorkHandler = {
      recoveryContract: "durable_idempotency_reconcile_v1",
      reconcile: async () => {
        reconciliationCount += 1;
        return {
          resultRef: "result.ide13.owner-local.resume.concurrent-recovery",
          evidenceRefs: ["evidence.ide13.owner-local.resume.concurrent-recovery"],
        };
      },
      execute: async () => {
        throw new Error("concurrent recovery must reconcile the prior effect");
      },
    };
    resumer = createPylonPortableOwnerLocalWorkResumer({
      database,
      ledger,
      handlers: new Map([[handlerRef, recoveryHandler]]),
      recoveryAuthority: {
        authorityRef: "authority.ide13.owner-local.executor-process-death.concurrent",
        verifyInterrupted: async ({ activeRecoveryEvidenceRef }) => {
          expect(activeRecoveryEvidenceRef).toBeNull();
          verificationCount += 1;
          if (verificationCount === 2) releaseVerification?.();
          await verificationBarrier;
          return true;
        },
      },
    });
    const attempts = await Promise.allSettled([
      resumer.resume({
        ...request,
        interruptionEvidenceRef: "evidence.ide13.owner-local.executor.interrupted.concurrent.a",
      }),
      resumer.resume({
        ...request,
        interruptionEvidenceRef: "evidence.ide13.owner-local.executor.interrupted.concurrent.b",
      }),
    ]);
    expect(attempts.filter((attempt) => attempt.status === "fulfilled")).toHaveLength(1);
    expect(attempts.filter((attempt) => attempt.status === "rejected")).toHaveLength(1);
    expect(reconciliationCount).toBe(1);
    expect(resumer.readState(workRef)).toBe("settled");
    database.close();
  });

  test("refuses stale generations, conflicting replay, and an unknown destination handler", async () => {
    const root = await mkdtemp(join(tmpdir(), "openagents-portable-work-fence-"));
    roots.push(root);
    const workspace = join(root, "workspace");
    await mkdir(workspace, { recursive: true });
    const database = openLegacySqliteDatabase(join(root, "portable.sqlite"));
    const ledger = new PylonPortableSessionOperationLedger(database);
    insertFence(database, sourceAttachmentRef, 1);
    const handlers = new Map([
      [
        handlerRef,
        {
          recoveryContract: "durable_idempotency_reconcile_v1",
          reconcile: async () => null,
          execute: async () => ({
            resultRef: "result.ide13.owner-local.resume.fixture",
            evidenceRefs: ["evidence.ide13.owner-local.resume.fixture"],
          }),
        },
      ],
    ]);
    const resumer = createPylonPortableOwnerLocalWorkResumer({ database, ledger, handlers });
    await resumer.accept({
      workRef,
      handlerRef,
      sessionRef,
      sourceAttachmentRef,
      sourceGeneration: 1,
      agentRef,
      workspaceRoot: workspace,
    });

    await expect(
      resumer.accept({
        workRef,
        handlerRef,
        sessionRef,
        sourceAttachmentRef: "attachment.ide13.owner-local.resume.conflict",
        sourceGeneration: 1,
        agentRef,
        workspaceRoot: workspace,
      }),
    ).rejects.toBeInstanceOf(PylonPortableOwnerLocalWorkResumeError);

    await expect(
      resumer.resume({
        operationRef,
        workRef,
        agentRef,
        sessionRef,
        destinationAttachmentRef,
        destinationGeneration: 2,
        workspaceRoot: workspace,
      }),
    ).rejects.toMatchObject({ reason: "stale_generation" });

    const restartedWithoutHandler = createPylonPortableOwnerLocalWorkResumer({
      database,
      ledger,
      handlers: new Map(),
    });
    advanceFence(database, sourceAttachmentRef, destinationAttachmentRef, 1);
    await expect(
      restartedWithoutHandler.resume({
        operationRef,
        workRef,
        agentRef,
        sessionRef,
        destinationAttachmentRef,
        destinationGeneration: 2,
        workspaceRoot: workspace,
      }),
    ).rejects.toMatchObject({ reason: "unknown_handler" });
  });
});

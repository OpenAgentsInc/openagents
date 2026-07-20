import { openLegacySqliteDatabase, type LegacySqliteDatabase } from "@openagentsinc/sqlite-runtime";
import { access, cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, test } from "vite-plus/test";

import { PylonPortableSessionOperationLedger } from "../src/portable-session-operation-ledger.js";
import {
  createPylonPortableOwnerLocalWorkResumer,
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
          async (input) => {
            executionCount += 1;
            expect(input.sourceGeneration).toBe(1);
            expect(input.destinationGeneration).toBe(2);
            const path = join(input.workspaceRoot, "tracked.txt");
            await writeFile(path, `${await readFile(path, "utf8")}resumed generation 2\n`, "utf8");
            return {
              resultRef: "result.ide13.owner-local.resume.safe-edit",
              evidenceRefs: ["evidence.ide13.owner-local.resume.safe-edit.settled"],
            };
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
        async () => ({
          resultRef: "result.ide13.owner-local.resume.fixture",
          evidenceRefs: ["evidence.ide13.owner-local.resume.fixture"],
        }),
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

import { openLegacySqliteDatabase } from "@openagentsinc/sqlite-runtime";
import { spawn } from "node:child_process";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { access, cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { PylonPortableSessionOperationLedger } from "../src/portable-session-operation-ledger.js";
import {
  createPylonPortableOwnerLocalWorkResumer,
  type PylonPortableOwnerLocalWorkHandler,
} from "../src/portable-session-owner-local-work-resumer.js";

const EVIDENCE_PATH =
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-executor-crash-recovery.json";
const GIT_SHA = /^[0-9a-f]{40}$/u;
const execFileAsync = promisify(execFile);
const sessionRef = "session.ide13.owner-local.crash-recovery";
const sourceAttachmentRef = "attachment.ide13.owner-local.crash-recovery.1";
const destinationAttachmentRef = "attachment.ide13.owner-local.crash-recovery.2";
const agentRef = "agent.ide13.owner-local.crash-recovery.root";
const workRef = "work.ide13.owner-local.crash-recovery.safe-edit";
const handlerRef = "handler.ide13.owner-local.crash-recovery.safe-edit.v1";
const operationRef = "operation.ide13.owner-local.crash-recovery.work.2";
const interruptionEvidenceRef = "evidence.ide13.owner-local.executor-process-exit.86";
const authorityRef = "authority.pylon.owner-local.executor-process-death.v1";
const result = {
  resultRef: "result.ide13.owner-local.crash-recovery.safe-edit",
  evidenceRefs: ["evidence.ide13.owner-local.crash-recovery.safe-edit.applied"],
};

const git = async (cwd: string, ...args: ReadonlyArray<string>): Promise<string> => {
  const { stdout } = await execFileAsync("git", args, { cwd, encoding: "utf8" });
  return stdout.trim();
};

const insertFence = (
  database: ReturnType<typeof openLegacySqliteDatabase>,
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

const advanceFence = (database: ReturnType<typeof openLegacySqliteDatabase>): void => {
  const advanced = database
    .query(`
      UPDATE pylon_portable_session_fences
      SET attachment_ref = ?, generation = 2, accepting_work = 1, revision = revision + 1
      WHERE session_ref = ? AND attachment_ref = ? AND generation = 1
    `)
    .run(destinationAttachmentRef, sessionRef, sourceAttachmentRef);
  if (advanced.changes !== 1) throw new Error("owner-local crash proof lost its generation fence");
};

const requestFor = (workspaceRoot: string) => ({
  operationRef,
  workRef,
  agentRef,
  sessionRef,
  destinationAttachmentRef,
  destinationGeneration: 2,
  workspaceRoot,
});

const durableEffectPath = (workspaceRoot: string): string =>
  join(workspaceRoot, "durable-effect.json");

const makeCrashHandler = (): PylonPortableOwnerLocalWorkHandler => ({
  recoveryContract: "durable_idempotency_reconcile_v1",
  reconcile: async () => null,
  execute: async (input) => {
    await writeFile(
      durableEffectPath(input.workspaceRoot),
      `${JSON.stringify({ idempotencyRef: input.idempotencyRef, applicationCount: 1 })}\n`,
      "utf8",
    );
    process.exit(86);
  },
});

const runCrashWorker = async (databasePath: string, workspaceRoot: string): Promise<never> => {
  const database = openLegacySqliteDatabase(databasePath);
  const ledger = new PylonPortableSessionOperationLedger(database);
  const resumer = createPylonPortableOwnerLocalWorkResumer({
    database,
    ledger,
    handlers: new Map([[handlerRef, makeCrashHandler()]]),
  });
  await resumer.resume(requestFor(workspaceRoot));
  throw new Error("crash worker returned after its injected process exit");
};

const runChild = async (
  databasePath: string,
  workspaceRoot: string,
): Promise<Readonly<{ pid: number; exitCode: number }>> => {
  const child = spawn(
    process.execPath,
    [
      "--import",
      "tsx",
      fileURLToPath(import.meta.url),
      "--crash-worker",
      databasePath,
      workspaceRoot,
    ],
    { cwd: resolve(join(import.meta.dirname, "../../..")), stdio: ["ignore", "pipe", "pipe"] },
  );
  const pid = child.pid;
  if (pid === undefined) throw new Error("owner-local crash worker did not start");
  const stderr: Array<Buffer> = [];
  child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
  const exitCode = await new Promise<number>((resolveExit, rejectExit) => {
    child.once("error", rejectExit);
    child.once("exit", (code) => resolveExit(code ?? -1));
  });
  if (exitCode !== 86) {
    throw new Error(
      `owner-local crash worker exited ${exitCode}: ${Buffer.concat(stderr).toString("utf8").trim()}`,
    );
  }
  return { pid, exitCode };
};

const processIsAlive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

export const runIde13OwnerLocalExecutorCrashRecovery = async (
  input: Readonly<{
    candidateCommitSha?: string;
    outputPath?: string;
    repositoryRoot?: string;
  }> = {},
) => {
  const repositoryRoot = resolve(input.repositoryRoot ?? join(import.meta.dirname, "../../.."));
  const headCommitSha = await git(repositoryRoot, "rev-parse", "HEAD");
  const candidateCommitSha = input.candidateCommitSha ?? headCommitSha;
  if (!GIT_SHA.test(candidateCommitSha)) throw new Error("crash-recovery candidate is invalid");
  await git(repositoryRoot, "merge-base", "--is-ancestor", candidateCommitSha, headCommitSha);
  const laterPaths = (
    await git(repositoryRoot, "diff", "--name-only", candidateCommitSha, headCommitSha)
  )
    .split("\n")
    .filter(Boolean);
  if (laterPaths.some((path) => path !== EVIDENCE_PATH)) {
    throw new Error("crash-recovery candidate omits an implementation change");
  }
  const baseCommitSha = await git(repositoryRoot, "merge-base", candidateCommitSha, "origin/main");
  const root = await mkdtemp(join(tmpdir(), "openagents-ide13-executor-crash-recovery-"));
  const source = join(root, "source");
  const destination = join(root, "destination");
  const databasePath = join(root, "portable.sqlite");
  let database = openLegacySqliteDatabase(databasePath);
  try {
    await mkdir(source, { recursive: true });
    await writeFile(join(source, "tracked.txt"), "source accepted work\n", "utf8");
    let ledger = new PylonPortableSessionOperationLedger(database);
    insertFence(database, sourceAttachmentRef, 1);
    const admissionHandler: PylonPortableOwnerLocalWorkHandler = {
      recoveryContract: "durable_idempotency_reconcile_v1",
      reconcile: async () => null,
      execute: async () => result,
    };
    const admission = createPylonPortableOwnerLocalWorkResumer({
      database,
      ledger,
      handlers: new Map([[handlerRef, admissionHandler]]),
    });
    await admission.accept({
      workRef,
      handlerRef,
      sessionRef,
      sourceAttachmentRef,
      sourceGeneration: 1,
      agentRef,
      workspaceRoot: source,
    });
    await cp(source, destination, { recursive: true });
    advanceFence(database);
    database.close();

    const crashed = await runChild(databasePath, destination);
    const processConfirmedDead = !processIsAlive(crashed.pid);
    if (!processConfirmedDead) throw new Error("owner-local crash worker remains alive");
    const durable = JSON.parse(await readFile(durableEffectPath(destination), "utf8")) as {
      idempotencyRef?: unknown;
      applicationCount?: unknown;
    };
    if (typeof durable.idempotencyRef !== "string" || durable.applicationCount !== 1) {
      throw new Error("owner-local crash worker did not leave one durable effect");
    }

    database = openLegacySqliteDatabase(databasePath);
    ledger = new PylonPortableSessionOperationLedger(database);
    let reconciliationCount = 0;
    let recoveryExecuteCount = 0;
    let exactScopeVerified = false;
    const recoveryHandler: PylonPortableOwnerLocalWorkHandler = {
      recoveryContract: "durable_idempotency_reconcile_v1",
      reconcile: async (handlerInput) => {
        reconciliationCount += 1;
        const observed = JSON.parse(await readFile(durableEffectPath(destination), "utf8")) as {
          idempotencyRef?: unknown;
          applicationCount?: unknown;
        };
        return observed.idempotencyRef === handlerInput.idempotencyRef &&
          observed.applicationCount === 1
          ? result
          : null;
      },
      execute: async () => {
        recoveryExecuteCount += 1;
        throw new Error("recovery must not execute an already-applied handler");
      },
    };
    const resumer = createPylonPortableOwnerLocalWorkResumer({
      database,
      ledger,
      handlers: new Map([[handlerRef, recoveryHandler]]),
      recoveryAuthority: {
        authorityRef,
        verifyInterrupted: async (scope) => {
          exactScopeVerified =
            scope.interruptionEvidenceRef === interruptionEvidenceRef &&
            scope.workRef === workRef &&
            scope.operationRef === operationRef &&
            scope.sessionRef === sessionRef &&
            scope.destinationAttachmentRef === destinationAttachmentRef &&
            scope.destinationGeneration === 2 &&
            scope.activeRecoveryEvidenceRef === null &&
            !processIsAlive(crashed.pid);
          return exactScopeVerified;
        },
      },
    });
    if (resumer.readState(workRef) !== "running") {
      throw new Error("crashed owner-local executor did not retain a running recovery row");
    }
    const recovered = await resumer.resume({
      ...requestFor(destination),
      interruptionEvidenceRef,
    });
    const replayed = await resumer.resume(requestFor(destination));
    const controlResidue = await access(join(destination, ".openagents")).then(
      () => 1,
      () => 0,
    );
    if (
      recovered.replay !== "recovered" ||
      replayed.replay !== "replayed" ||
      recovered.receiptRef !== replayed.receiptRef ||
      resumer.readState(workRef) !== "settled" ||
      reconciliationCount !== 1 ||
      recoveryExecuteCount !== 0 ||
      controlResidue !== 0
    ) {
      throw new Error("owner-local executor crash recovery did not settle without duplicate work");
    }
    const receipt = {
      schemaVersion: "openagents.desktop.ide-portable-owner-local-executor-crash-recovery.v1",
      evidenceClass: "real_local",
      generatedAt: new Date().toISOString(),
      candidateCommitSha,
      baseCommitSha,
      placement: {
        targetClass: "owner_local",
        sourceGeneration: 1,
        destinationGeneration: 2,
      },
      crash: {
        childProcessStarted: true,
        exitCode: crashed.exitCode,
        processConfirmedDead,
        durableEffectApplicationCount: durable.applicationCount,
        preRecoveryState: "running",
      },
      recovery: {
        authorityRef,
        interruptionEvidenceRef,
        exactScopeVerified,
        recoveryContract: recoveryHandler.recoveryContract,
        reconciliationCount,
        recoveryExecuteCount,
        firstResult: recovered.replay,
        replayResult: replayed.replay,
        settlementState: resumer.readState(workRef),
        resultRef: recovered.resultRef,
        receiptRef: recovered.receiptRef,
        evidenceRefs: recovered.evidenceRefs,
      },
      teardown: { controlResidue },
      authority: {
        productionDispatchEnabled: false,
        networkCalls: 0,
        providerCalls: 0,
        secretMaterialInReceipt: false,
      },
      limitations: [
        "Recovery is admitted only for registered handlers that reconcile a stable durable idempotency ref.",
        "The resumer does not provide general exactly-once semantics for arbitrary external side effects.",
        "The owner-local supervisor must supply exact process-death authority before recovery can replace a running executor.",
      ],
    };
    if (input.outputPath !== undefined) {
      await mkdir(dirname(input.outputPath), { recursive: true });
      await writeFile(input.outputPath, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
    }
    return receipt;
  } finally {
    database.close();
    await rm(root, { recursive: true, force: true });
  }
};

if (process.argv[2] === "--crash-worker") {
  const databasePath = process.argv[3];
  const workspaceRoot = process.argv[4];
  if (databasePath === undefined || workspaceRoot === undefined) {
    throw new Error("crash worker input is missing");
  }
  await runCrashWorker(databasePath, workspaceRoot);
} else if (import.meta.main) {
  const repositoryRoot = resolve(join(import.meta.dirname, "../../.."));
  const receipt = await runIde13OwnerLocalExecutorCrashRecovery({
    repositoryRoot,
    outputPath: resolve(repositoryRoot, EVIDENCE_PATH),
    candidateCommitSha: process.env.OPENAGENTS_IDE13_CANDIDATE_COMMIT_SHA,
  });
  process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
  process.exit(0);
}

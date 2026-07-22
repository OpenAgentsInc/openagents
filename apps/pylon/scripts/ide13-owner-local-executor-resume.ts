import { Runtime } from "@openagentsinc/runtime-platform";
import { openLegacySqliteDatabase } from "@openagentsinc/sqlite-runtime";
import { createHash, randomBytes } from "node:crypto";
import { access, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

import type {
  PortableAgentGraph,
  PortableSessionExecutionBinding,
} from "@openagentsinc/portable-session-contract";
import { Effect, Schema } from "effect";

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap.js";
import { PYLON_DEV_CHECK_SCHEMA, type PylonDevCheckProjection } from "../src/dev-loop.js";
import {
  createControlSessionActions,
  type ControlSessionActions,
  type ControlSessionExecutor,
} from "../src/node/control-sessions.js";
import {
  makeEvidenceBoundPortableDestinationAuthenticator,
  makePylonPortableDestinationHelperSupervisor,
} from "../src/portable-destination-helper-supervisor.js";
import { makePylonPortableDestinationProductionHelpers } from "../src/portable-destination-production-helper-adapters.js";
import { PylonPortableCheckpointArtifactStore } from "../src/portable-session-checkpoint-artifact.js";
import { createPylonPortableLocalRehydrator } from "../src/portable-session-local-rehydrator.js";
import {
  createPylonOwnerLocalDestinationLifecycle,
  type PylonPortableAuthorityAttachment,
} from "../src/portable-session-destination.js";
import { PylonPortableSessionOperationLedger } from "../src/portable-session-operation-ledger.js";
import { createPylonPortableOwnerLocalWorkResumer } from "../src/portable-session-owner-local-work-resumer.js";
import { createPylonOwnerLocalExecutionTarget } from "../src/portable-session-target.js";

const Ref = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);
const Sha256 = Schema.String.check(Schema.isPattern(/^[a-f0-9]{64}$/u));
const GitSha = Schema.String.check(Schema.isPattern(/^[a-f0-9]{40}$/u));

export const Ide13OwnerLocalExecutorResumeReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-portable-owner-local-executor-resume.v1"),
  evidenceClass: Schema.Literal("real_local"),
  generatedAt: Schema.String,
  candidateCommitSha: GitSha,
  baseCommitSha: GitSha,
  placement: Schema.Struct({
    targetClass: Schema.Literal("owner_local"),
    adapterRef: Schema.Literal("adapter.pylon.owner-local.control-session.v1"),
    sourceTargetRef: Ref,
    destinationTargetRef: Ref,
    sourceGeneration: Schema.Literal(1),
    destinationGeneration: Schema.Literal(2),
    finalFenceGeneration: Schema.Literal(3),
  }),
  checkpoint: Schema.Struct({
    artifactRef: Ref,
    sha256: Sha256,
    bytes: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
    encryptedCustody: Schema.Literal(true),
    custodyDeleted: Schema.Literal(true),
  }),
  execution: Schema.Struct({
    workRef: Ref,
    handlerRef: Ref,
    acceptedWorkRefCount: Schema.Literal(1),
    sourceProcessLifecycle: Schema.Literal("settled"),
    destinationExecutor: Schema.Literal("registered_bounded_handler"),
    processStateTransferred: Schema.Literal(false),
    firstRun: Schema.Literal("executed"),
    replay: Schema.Literal("replayed"),
    duplicateExecutionCount: Schema.Literal(0),
    staleGenerationRefused: Schema.Literal(true),
    settlementState: Schema.Literal("settled"),
    resultRef: Ref,
    receiptRef: Ref,
    evidenceRefs: Schema.Array(Ref).check(Schema.isMinLength(1), Schema.isMaxLength(32)),
  }),
  teardown: Schema.Struct({
    nonTerminalControlSessions: Schema.Literal(0),
    activeHelperReservations: Schema.Literal(0),
    activeCustodyObjects: Schema.Literal(0),
    workControlResidue: Schema.Literal(0),
    rehydratedWorkspaceResidue: Schema.Literal(0),
  }),
  authority: Schema.Struct({
    productionDispatchEnabled: Schema.Literal(false),
    networkCalls: Schema.Literal(0),
    providerCalls: Schema.Literal(0),
    secretMaterialInReceipt: Schema.Literal(false),
  }),
  result: Schema.String,
});

export interface Ide13OwnerLocalExecutorResumeReceipt extends Schema.Schema.Type<
  typeof Ide13OwnerLocalExecutorResumeReceiptSchema
> {}

const decodeReceipt = Schema.decodeUnknownSync(Ide13OwnerLocalExecutorResumeReceiptSchema);
const GIT_SHA = /^[a-f0-9]{40}$/u;
const EVIDENCE_PATH =
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-executor-resume.json";

const sha256 = (value: string | Uint8Array): string =>
  createHash("sha256").update(value).digest("hex");

const git = async (cwd: string, ...args: string[]): Promise<string> => {
  const child = Runtime.spawn(["git", ...args], { cwd, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (exitCode !== 0) throw new Error(`git ${args[0] ?? "command"} failed: ${stderr.trim()}`);
  return stdout.trim();
};

const passedDevCheck = (commit: string): PylonDevCheckProjection => ({
  schema: PYLON_DEV_CHECK_SCHEMA,
  observedAt: new Date().toISOString(),
  action: "check",
  state: "passed",
  changeSummary: {
    repo: { state: "clean", rootRef: "repository.ide13.executor-resume", branch: "main", commit },
    dirty: { state: "clean", changedCount: 0, stagedCount: 0, unstagedCount: 0, untrackedCount: 0 },
    changedFileRefs: [],
    areaRefs: [],
    blockerRefs: [],
  },
  checkPlan: { state: "ready", commandRefs: [], blockerRefs: [] },
  commandResults: [],
  latestRecordRef: null,
  branchUntouched: true,
  commitUntouched: true,
  pushPerformed: false,
  blockerRefs: [],
});

const waitForTerminal = async (
  actions: ControlSessionActions,
  sessionRef: string,
): Promise<void> => {
  const poll = async (attempt: number): Promise<void> => {
    const row = (await actions.list()).find((candidate) => candidate.sessionRef === sessionRef);
    if (row?.state === "cancelled" || row?.state === "completed" || row?.state === "failed") return;
    if (attempt >= 199) throw new Error("owner-local source process did not settle");
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 10));
    return poll(attempt + 1);
  };
  return poll(0);
};

const cursorsFor = (generation: number) => [
  {
    threadRef: "thread.ide13.owner-local.executor-resume.root",
    transcriptRef: "transcript.ide13.owner-local.executor-resume.root",
    activityCursor: generation,
    eventCursor: generation,
  },
];

export const runIde13OwnerLocalExecutorResume = async (
  input: Readonly<{
    candidateCommitSha?: string;
    outputPath?: string;
    repositoryRoot?: string;
  }> = {},
): Promise<Ide13OwnerLocalExecutorResumeReceipt> => {
  const repositoryRoot = resolve(input.repositoryRoot ?? join(import.meta.dirname, "../../.."));
  const headCommitSha = await git(repositoryRoot, "rev-parse", "HEAD");
  const candidateCommitSha = input.candidateCommitSha ?? headCommitSha;
  if (!GIT_SHA.test(candidateCommitSha))
    throw new Error("executor-resume candidate commit is invalid");
  await git(repositoryRoot, "merge-base", "--is-ancestor", candidateCommitSha, headCommitSha);
  const laterPaths = (
    await git(repositoryRoot, "diff", "--name-only", candidateCommitSha, headCommitSha)
  )
    .split("\n")
    .filter(Boolean);
  if (laterPaths.some((path) => path !== EVIDENCE_PATH)) {
    throw new Error("executor-resume candidate omits an implementation change");
  }
  const baseCommitSha = await git(repositoryRoot, "merge-base", candidateCommitSha, "origin/main");
  const root = await mkdtemp(join(tmpdir(), "openagents-ide13-executor-resume-"));
  const database = openLegacySqliteDatabase(join(root, "portable.sqlite"));
  const custodyKey = randomBytes(32);
  const sourceTargetRef = "target.ide13.owner-local.executor-resume.source";
  const destinationTargetRef = "target.ide13.owner-local.executor-resume.destination";
  const sessionRef = "session.ide13.owner-local.executor-resume";
  const sourceAttachmentRef = "attachment.ide13.owner-local.executor-resume.1";
  const destinationAttachmentRef = "attachment.ide13.owner-local.executor-resume.2";
  const finalAttachmentRef = "attachment.ide13.owner-local.executor-resume.3";
  const agentRef = "agent.ide13.owner-local.executor-resume.root";
  const workRef = "work.ide13.owner-local.executor-resume.safe-edit";
  const handlerRef = "handler.ide13.owner-local.executor-resume.safe-edit.v1";
  const capabilityLeaseRefs = ["lease.ide13.owner-local.executor-resume.tooling"];
  const stageOperationRef = "operation.ide13.owner-local.executor-resume.destination.stage";
  const resumeOperationRef = "operation.ide13.owner-local.executor-resume.work.2";
  let handlerExecutionCount = 0;
  let sourceRevision = "";
  let sourceWorkspace = "";
  try {
    const ledger = new PylonPortableSessionOperationLedger(database);
    const helperAdapters = makePylonPortableDestinationProductionHelpers();
    const supervisor = makePylonPortableDestinationHelperSupervisor({
      authenticator: makeEvidenceBoundPortableDestinationAuthenticator(),
      adapters: helperAdapters.adapters,
      unsupportedOmissionRefs: helperAdapters.unsupportedOmissionRefs,
    });
    let sourceReady: (() => void) | undefined;
    const ready = new Promise<void>((resolveReady) => {
      sourceReady = resolveReady;
    });
    const executor: ControlSessionExecutor = async (execution) => {
      sourceWorkspace = execution.cwd;
      await writeFile(join(execution.cwd, "tracked.txt"), "source accepted work\n", "utf8");
      sourceReady?.();
      await new Promise<never>((_resolve, reject) => {
        const stop = () => reject(new Error("owner-local executor source quiesced"));
        if (execution.abortSignal.aborted) return stop();
        execution.abortSignal.addEventListener("abort", stop, { once: true });
      });
      return {
        commandCount: 0,
        devCheck: passedDevCheck(sourceRevision),
        editedFileCount: 1,
        eventCount: 0,
        externalSessionRef: null,
        responseDigestRef: null,
        totalTokens: 0,
      };
    };
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
      PYLON_HOME: join(root, "pylon-home"),
    });
    const actions = createControlSessionActions({
      env: {},
      executor,
      portableDestinationHelperSupervisor: supervisor,
      portableLedger: ledger,
      summary,
      workspaceCheckoutRunner: async (workingDirectory) => {
        await mkdir(workingDirectory, { recursive: true });
        await git(workingDirectory, "init", "-b", "main");
        await git(workingDirectory, "config", "user.email", "executor-resume@openagents.invalid");
        await git(workingDirectory, "config", "user.name", "OpenAgents Executor Resume");
        await writeFile(join(workingDirectory, "tracked.txt"), "base\n", "utf8");
        await git(workingDirectory, "add", "tracked.txt");
        await git(workingDirectory, "commit", "-m", "executor resume base");
        sourceRevision = await git(workingDirectory, "rev-parse", "HEAD");
      },
    });
    const started = await actions.spawn({
      type: "session.spawn",
      adapter: "codex",
      repoRef: {
        branch: "main",
        commitSha: candidateCommitSha,
        fullName: "OpenAgentsInc/openagents",
        provider: "github",
        visibility: "public",
      },
      objective: "Run the bounded owner-local accepted-work resume proof.",
      verify: ["true"],
    });
    await ready;

    const graphFor = (generation: number): PortableAgentGraph => ({
      rootAgentRef: agentRef,
      nodes: [
        {
          agentRef,
          threadRef: "thread.ide13.owner-local.executor-resume.root",
          transcriptRef: "transcript.ide13.owner-local.executor-resume.root",
          activityCursor: generation,
          lifecycle: "quiesced",
          attachmentGeneration: generation,
        },
      ],
    });
    const binding = {
      sessionRef,
      attachmentRef: sourceAttachmentRef,
      generation: 1,
      agents: [{ agentRef, controlSessionRef: started.sessionRef }],
    };
    const executionBinding: PortableSessionExecutionBinding = {
      schema: "openagents.portable_session_execution_binding.v1",
      sessionRef,
      ownerRef: "owner.ide13.owner-local.executor-resume",
      runRef: "run.ide13.owner-local.executor-resume",
      repositoryRef: "repository.OpenAgentsInc.openagents.executor-resume",
      pinnedBaseRef: `commit.${sourceRevision}`,
    };
    let authority: PylonPortableAuthorityAttachment = {
      sessionRef,
      targetRef: sourceTargetRef,
      attachmentRef: sourceAttachmentRef,
      generation: 1,
      state: "active",
      authorityEvidenceRef: "evidence.ide13.owner-local.executor-resume.authority.1",
    };
    const authorityPort = { readCurrentAttachment: async () => authority };
    const producer = new PylonPortableCheckpointArtifactStore();
    const custodyDirectory = join(root, "custody");
    const custodyConfig = {
      custodyDirectory,
      policy: "owner_managed" as const,
      keyRef: "key.ide13.owner-local.executor-resume",
      keyProvider: { loadKey: async () => Uint8Array.from(custodyKey) },
      retentionSeconds: 3_600,
    };
    const custody = new PylonPortableCheckpointArtifactStore(custodyConfig);
    const rehydratedRoot = join(root, "rehydrated");
    const rehydrator = createPylonPortableLocalRehydrator({
      targetRef: destinationTargetRef,
      custodyRoot: rehydratedRoot,
      artifacts: new PylonPortableCheckpointArtifactStore(custodyConfig),
      lifecycle: actions.portable,
    });
    const destination = createPylonOwnerLocalDestinationLifecycle({
      targetRef: destinationTargetRef,
      ledger,
      authority: authorityPort,
      rehydrator,
    });
    const source = await createPylonOwnerLocalExecutionTarget({
      targetRef: sourceTargetRef,
      ledger,
      lifecycle: actions.portable,
      binding,
      destination,
      checkpointArtifacts: producer,
    });
    const handlers = new Map([
      [
        handlerRef,
        {
          recoveryContract: "durable_idempotency_reconcile_v1" as const,
          reconcile: async (handlerInput: Readonly<{ workspaceRoot: string }>) =>
            (await readFile(join(handlerInput.workspaceRoot, "tracked.txt"), "utf8")).endsWith(
              "destination settled work\n",
            )
              ? {
                  resultRef: "result.ide13.owner-local.executor-resume.safe-edit",
                  evidenceRefs: ["evidence.ide13.owner-local.executor-resume.safe-edit.settled"],
                }
              : null,
          execute: async (
            handlerInput: Readonly<{
              workspaceRoot: string;
              sourceGeneration: number;
              destinationGeneration: number;
            }>,
          ) => {
            handlerExecutionCount += 1;
            if (handlerInput.sourceGeneration !== 1 || handlerInput.destinationGeneration !== 2) {
              throw new Error("bounded handler received a stale generation");
            }
            const trackedPath = join(handlerInput.workspaceRoot, "tracked.txt");
            await writeFile(
              trackedPath,
              `${await readFile(trackedPath, "utf8")}destination settled work\n`,
              "utf8",
            );
            return {
              resultRef: "result.ide13.owner-local.executor-resume.safe-edit",
              evidenceRefs: ["evidence.ide13.owner-local.executor-resume.safe-edit.settled"],
            };
          },
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
      workspaceRoot: sourceWorkspace,
    });
    await source.quiesceGraph({
      operationRef: "operation.ide13.owner-local.executor-resume.quiesce.1",
      sessionRef,
      attachmentRef: sourceAttachmentRef,
      generation: 1,
      graph: graphFor(1),
      threadCursors: cursorsFor(1),
    });
    await waitForTerminal(actions, started.sessionRef);
    const checkpoint = await source.createCheckpoint({
      operationRef: "operation.ide13.owner-local.executor-resume.move.checkpoint",
      checkpointRef: "checkpoint.ide13.owner-local.executor-resume.1",
      sessionRef,
      attachmentRef: sourceAttachmentRef,
      generation: 1,
      eventLogCursor: 1,
      executionBinding,
      graph: graphFor(1),
      threadCursors: cursorsFor(1),
    });
    const artifact = await producer.resolve({
      ownerRef: executionBinding.ownerRef,
      targetRef: destinationTargetRef,
      sessionRef,
      attachmentRef: destinationAttachmentRef,
      generation: 2,
      checkpointRef: checkpoint.checkpoint.checkpointRef,
      bundle: checkpoint,
    });
    const artifactReceipt = {
      artifactRef: artifact.artifactRef,
      sha256: artifact.digest.slice("sha256:".length),
      bytes: artifact.bytes.byteLength,
    };
    try {
      await custody.registerArtifact({ bundle: checkpoint, artifact });
    } finally {
      artifact.bytes.fill(0);
    }
    await source.cleanupSource({
      operationRef: "operation.ide13.owner-local.executor-resume.move.source.cleanup",
      sessionRef,
      attachmentRef: sourceAttachmentRef,
      generation: 1,
      agentRefs: [agentRef],
    });
    await destination.stageCheckpoint({
      operationRef: stageOperationRef,
      bundle: checkpoint,
      destinationAttachmentRef,
      destinationGeneration: 2,
      capabilityLeaseRefs,
    });
    authority = {
      sessionRef,
      targetRef: destinationTargetRef,
      attachmentRef: destinationAttachmentRef,
      generation: 2,
      state: "active",
      checkpointRef: checkpoint.checkpoint.checkpointRef,
      authorityEvidenceRef: "evidence.ide13.owner-local.executor-resume.authority.2",
    };
    await destination.activate({
      operationRef: "operation.ide13.owner-local.executor-resume.destination.activate",
      checkpointRef: checkpoint.checkpoint.checkpointRef,
      sessionRef,
      executionBinding,
      destinationAttachmentRef,
      destinationGeneration: 2,
      capabilityLeaseRefs,
    });
    const destinationWorkspace = join(rehydratedRoot, sha256(stageOperationRef));
    const resumeInput = {
      operationRef: resumeOperationRef,
      workRef,
      agentRef,
      sessionRef,
      destinationAttachmentRef,
      destinationGeneration: 2,
      workspaceRoot: destinationWorkspace,
    };
    const executed = await resumer.resume(resumeInput);
    const replayed = await resumer.resume(resumeInput);
    if (
      handlerExecutionCount !== 1 ||
      executed.replay !== "executed" ||
      replayed.replay !== "replayed" ||
      executed.receiptRef !== replayed.receiptRef ||
      (await readFile(join(destinationWorkspace, "tracked.txt"), "utf8")) !==
        "source accepted work\ndestination settled work\n"
    ) {
      throw new Error("owner-local destination did not settle exactly one accepted work ref");
    }
    const deletion = await custody.deleteArtifact({
      operationRef: "operation.ide13.owner-local.executor-resume.custody.delete",
      ownerRef: executionBinding.ownerRef,
      sessionRef,
      checkpointRef: checkpoint.checkpoint.checkpointRef,
      bundle: checkpoint,
    });
    if (deletion.state !== "deleted" || deletion.verifiedAbsent !== true) {
      throw new Error("owner-local executor-resume custody was not deleted");
    }

    const destinationSource = await createPylonOwnerLocalExecutionTarget({
      targetRef: destinationTargetRef,
      ledger,
      lifecycle: actions.portable,
      binding: { ...binding, attachmentRef: destinationAttachmentRef, generation: 2 },
    });
    await destinationSource.quiesceGraph({
      operationRef: "operation.ide13.owner-local.executor-resume.quiesce.2",
      sessionRef,
      attachmentRef: destinationAttachmentRef,
      generation: 2,
      graph: graphFor(2),
      threadCursors: cursorsFor(2),
    });
    const finalCheckpoint = await destinationSource.createCheckpoint({
      operationRef: "operation.ide13.owner-local.executor-resume.teardown.checkpoint",
      checkpointRef: "checkpoint.ide13.owner-local.executor-resume.2",
      sessionRef,
      attachmentRef: destinationAttachmentRef,
      generation: 2,
      eventLogCursor: 2,
      executionBinding,
      graph: graphFor(2),
      threadCursors: cursorsFor(2),
    });
    await destinationSource.cleanupSource({
      operationRef: "operation.ide13.owner-local.executor-resume.teardown.source.cleanup",
      sessionRef,
      attachmentRef: destinationAttachmentRef,
      generation: 2,
      agentRefs: [agentRef],
    });
    await Effect.runPromise(
      ledger.activateGeneration({
        operationRef: "operation.ide13.owner-local.executor-resume.fence.3",
        sessionRef,
        sourceAttachmentRef: destinationAttachmentRef,
        sourceGeneration: 2,
        destinationAttachmentRef: finalAttachmentRef,
        destinationGeneration: 3,
        authorityEvidenceRef: "evidence.ide13.owner-local.executor-resume.authority.3",
      }),
    );
    let staleGenerationRefused = false;
    try {
      await resumer.resume(resumeInput);
    } catch {
      staleGenerationRefused = true;
    }
    if (!staleGenerationRefused)
      throw new Error("settled work replay crossed a newer generation fence");
    await actions.portable.shutdownHelpers?.();

    const controlRows = await actions.list();
    const nonTerminalControlSessions = controlRows.filter(
      (row) => row.state !== "cancelled" && row.state !== "completed" && row.state !== "failed",
    ).length;
    const activeHelperReservations = supervisor.disposalFailures().length;
    const activeCustodyObjects = (await readdir(custodyDirectory)).filter((name) =>
      name.endsWith(".checkpoint.aesgcm"),
    ).length;
    const workControlResidue = await access(join(destinationWorkspace, ".openagents")).then(
      () => 1,
      () => 0,
    );
    const rehydratedWorkspaceResidue = await access(destinationWorkspace).then(
      () => 1,
      () => 0,
    );
    const receipt = decodeReceipt(
      {
        schemaVersion: "openagents.desktop.ide-portable-owner-local-executor-resume.v1",
        evidenceClass: "real_local",
        generatedAt: new Date().toISOString(),
        candidateCommitSha,
        baseCommitSha,
        placement: {
          targetClass: "owner_local",
          adapterRef: "adapter.pylon.owner-local.control-session.v1",
          sourceTargetRef,
          destinationTargetRef,
          sourceGeneration: 1,
          destinationGeneration: 2,
          finalFenceGeneration: 3,
        },
        checkpoint: {
          ...artifactReceipt,
          encryptedCustody: true,
          custodyDeleted: true,
        },
        execution: {
          workRef,
          handlerRef,
          acceptedWorkRefCount: 1,
          sourceProcessLifecycle: "settled",
          destinationExecutor: "registered_bounded_handler",
          processStateTransferred: false,
          firstRun: "executed",
          replay: "replayed",
          duplicateExecutionCount: 0,
          staleGenerationRefused,
          settlementState: resumer.readState(workRef),
          resultRef: executed.resultRef,
          receiptRef: executed.receiptRef,
          evidenceRefs: executed.evidenceRefs,
        },
        teardown: {
          nonTerminalControlSessions,
          activeHelperReservations,
          activeCustodyObjects,
          workControlResidue,
          rehydratedWorkspaceResidue,
        },
        authority: {
          productionDispatchEnabled: false,
          networkCalls: 0,
          providerCalls: 0,
          secretMaterialInReceipt: false,
        },
        result:
          "One refs-only accepted work checkpoint crossed a real owner-local generation 1 to 2 artifact move. A registered bounded handler ran once in the rehydrated destination workspace, settled durably, replayed without duplicate execution, and refused the stale generation after generation 3 became active. No live process state moved. Production command dispatch stayed disabled.",
      },
      { onExcessProperty: "error" },
    );
    if (
      nonTerminalControlSessions !== 0 ||
      activeHelperReservations !== 0 ||
      activeCustodyObjects !== 0 ||
      workControlResidue !== 0 ||
      rehydratedWorkspaceResidue !== 0 ||
      finalCheckpoint.checkpoint.sourceGeneration !== 2
    ) {
      throw new Error("owner-local executor-resume teardown left residue");
    }
    if (input.outputPath !== undefined) {
      await mkdir(dirname(input.outputPath), { recursive: true });
      await writeFile(input.outputPath, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
    }
    return receipt;
  } finally {
    custodyKey.fill(0);
    database.close();
    await rm(root, { recursive: true, force: true });
  }
};

if (import.meta.main) {
  const repositoryRoot = resolve(join(import.meta.dirname, "../../.."));
  const outputPath = resolve(repositoryRoot, EVIDENCE_PATH);
  const receipt = await runIde13OwnerLocalExecutorResume({
    repositoryRoot,
    outputPath,
    candidateCommitSha: process.env.OPENAGENTS_IDE13_CANDIDATE_COMMIT_SHA,
  });
  process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
  process.exit(0);
}

import { Runtime } from "@openagentsinc/runtime-platform";
import { openLegacySqliteDatabase } from "@openagentsinc/sqlite-runtime";
import { createHash, randomBytes } from "node:crypto";
import { mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

import {
  IdePortableDestinationHelperReadinessSchema,
  type PortableAgentGraph,
  type PortableSessionExecutionBinding,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";

import {
  IdePortablePlacementCohortSchema,
  IDE_PORTABLE_NON_PHASE_ACCEPTANCE_METRICS,
  IDE_PORTABLE_PHASES,
} from "../../openagents-desktop/src/ide/portable-evidence-contract.ts";
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
import {
  makePylonPortableDestinationProductionHelpers,
  PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
} from "../src/portable-destination-production-helper-adapters.js";
import { repositoryOwnedPylonPortableExecutableProfileCatalog } from "../src/portable-executable-profile-catalog.js";
import { createPylonPortableLocalRehydrator } from "../src/portable-session-local-rehydrator.js";
import {
  createPylonOwnerLocalDestinationLifecycle,
  type PylonPortableAuthorityAttachment,
} from "../src/portable-session-destination.js";
import {
  PylonPortableCheckpointArtifactStore,
  type PylonPortableCheckpointDeletionReceipt,
} from "../src/portable-session-checkpoint-artifact.js";
import { PylonPortableSessionOperationLedger } from "../src/portable-session-operation-ledger.js";
import { createPylonOwnerLocalExecutionTarget } from "../src/portable-session-target.js";

const Ref = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);

export const Ide13OwnerLocalRealCohortReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-portable-owner-local-cohort.v1"),
  evidenceContractVersion: Schema.Literal("openagents.desktop.ide-portable-evidence.v3"),
  generatedAt: Schema.String,
  cohort: IdePortablePlacementCohortSchema,
  helpers: Schema.Array(IdePortableDestinationHelperReadinessSchema).check(
    Schema.isMinLength(5),
    Schema.isMaxLength(5),
  ),
  authority: Schema.Struct({
    catalogRef: Ref,
    admittedExecutableProfileRefs: Schema.Array(Ref).check(Schema.isMaxLength(0)),
    capabilityLeaseRefs: Schema.Array(Ref).check(Schema.isMinLength(1), Schema.isMaxLength(16)),
    unsupportedProfileOmissionRef: Ref,
  }),
  execution: Schema.Struct({
    acceptedWorkRefCount: Schema.Literal(0),
    controlSessionProcessLifecycle: Schema.Literal("settled"),
    executorResumed: Schema.Literal(false),
    omissionRef: Ref,
  }),
  proofs: Schema.Struct({
    abortReceiptRef: Ref,
    replayReceiptRef: Ref,
    staleGenerationReceiptRef: Ref,
    sourceCustodyDeletionReceiptRef: Ref,
    failbackCustodyDeletionReceiptRef: Ref,
    teardownReceiptRef: Ref,
  }),
});

export interface Ide13OwnerLocalRealCohortReceipt extends Schema.Schema.Type<
  typeof Ide13OwnerLocalRealCohortReceiptSchema
> {}

const decodeReceipt = Schema.decodeUnknownSync(Ide13OwnerLocalRealCohortReceiptSchema);

type Metric = Ide13OwnerLocalRealCohortReceipt["cohort"]["metrics"][number];
type Phase = (typeof IDE_PORTABLE_PHASES)[number];

const sha256 = (value: string | Uint8Array): string =>
  createHash("sha256").update(value).digest("hex");

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${sha256(value).slice(0, 32)}`;

const git = async (cwd: string, ...args: string[]): Promise<string> => {
  const process = Runtime.spawn(["git", ...args], { cwd, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(process.stdout).text(),
    new Response(process.stderr).text(),
    process.exited,
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
    repo: { state: "clean", rootRef: "repository.ide13.cohort", branch: "main", commit },
    dirty: {
      state: "clean",
      changedCount: 0,
      stagedCount: 0,
      unstagedCount: 0,
      untrackedCount: 0,
    },
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
  for (let attempt = 0; attempt < 100; attempt += 1) {
    const row = (await actions.list()).find((candidate) => candidate.sessionRef === sessionRef);
    if (row?.state === "cancelled" || row?.state === "completed" || row?.state === "failed") return;
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 10));
  }
  throw new Error("owner-local control session did not reach a terminal state");
};

const phaseMetric = (phase: Phase, milliseconds: number, receiptRef: string): Metric => ({
  metricRef: `metric.ide13.owner-local.phase.${phase}`,
  metric: "phase_latency",
  phase,
  unit: "milliseconds",
  repetitions: 1,
  p50: milliseconds,
  p95: milliseconds,
  p99: milliseconds,
  thresholdP95: 30_000,
  thresholdP99: 30_000,
  passed: milliseconds <= 30_000,
  receiptRef,
});

const pointMetric = (
  metric: (typeof IDE_PORTABLE_NON_PHASE_ACCEPTANCE_METRICS)[number],
  value: number,
  unit: Metric["unit"],
  threshold: number,
  receiptRef: string,
): Metric => ({
  metricRef: `metric.ide13.owner-local.${metric}`,
  metric,
  phase: null,
  unit,
  repetitions: 1,
  p50: value,
  p95: value,
  p99: value,
  thresholdP95: threshold,
  thresholdP99: threshold,
  passed: value <= threshold,
  receiptRef,
});

const measure = async <A>(
  operation: () => Promise<A>,
): Promise<Readonly<{ value: A; milliseconds: number }>> => {
  const startedAt = performance.now();
  const value = await operation();
  return { value, milliseconds: performance.now() - startedAt };
};

const exactHelperMatrix = (helpers: Ide13OwnerLocalRealCohortReceipt["helpers"]): void => {
  const byKind = new Map(helpers.map((helper) => [helper.kind, helper]));
  for (const kind of ["pty", "watcher"] as const) {
    const helper = byKind.get(kind);
    if (
      helper?.readiness !== "ready" ||
      helper.instanceRef === null ||
      helper.versionRef === null
    ) {
      throw new Error(`owner-local ${kind} helper is not ready`);
    }
  }
  for (const kind of ["lsp", "dap", "native"] as const) {
    const helper = byKind.get(kind);
    if (
      helper?.readiness !== "unsupported" ||
      helper.instanceRef !== null ||
      helper.versionRef !== null ||
      helper.omissionRef !== PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING
    ) {
      throw new Error(`owner-local ${kind} helper authority is not fail-closed`);
    }
  }
};

const assertDeletion = (receipt: PylonPortableCheckpointDeletionReceipt): string => {
  if (
    receipt.state !== "deleted" ||
    receipt.verifiedAbsent !== true ||
    receipt.publicSafe !== true
  ) {
    throw new Error("checkpoint custody deletion was not verified");
  }
  return receipt.receiptRef;
};

export const runIde13OwnerLocalRealCohort = async (
  input: Readonly<{
    outputPath?: string;
    repositoryRoot?: string;
  }> = {},
): Promise<Ide13OwnerLocalRealCohortReceipt> => {
  const repositoryRoot = resolve(input.repositoryRoot ?? join(import.meta.dirname, "../../.."));
  const candidateCommitSha = await git(repositoryRoot, "rev-parse", "HEAD");
  const baseCommitSha = await git(repositoryRoot, "merge-base", "HEAD", "origin/main");
  const root = await mkdtemp(join(tmpdir(), "openagents-ide13-owner-local-cohort-"));
  const database = openLegacySqliteDatabase(join(root, "portable.sqlite"));
  const custodyKeyA = randomBytes(32);
  const custodyKeyB = randomBytes(32);
  const capabilityLeaseRefs = ["lease.ide13.owner-local.cohort.tooling"];
  const targetARef = "target.ide13.owner-local.cohort.a";
  const targetBRef = "target.ide13.owner-local.cohort.b";
  const sessionRef = "session.ide13.owner-local.cohort";
  const attachmentA1 = "attachment.ide13.owner-local.cohort.a.1";
  const attachmentB2 = "attachment.ide13.owner-local.cohort.b.2";
  const attachmentA3 = "attachment.ide13.owner-local.cohort.a.3";
  const agentRef = "agent.ide13.owner-local.cohort.root";
  const phaseMilliseconds = new Map<Phase, number>();
  let maxRssBytes = process.memoryUsage().rss;
  const cpuStarted = process.cpuUsage();
  const wallStarted = performance.now();
  try {
    const ledger = new PylonPortableSessionOperationLedger(database);
    const helpers = makePylonPortableDestinationProductionHelpers();
    const supervisor = makePylonPortableDestinationHelperSupervisor({
      authenticator: makeEvidenceBoundPortableDestinationAuthenticator(),
      adapters: helpers.adapters,
      unsupportedOmissionRefs: helpers.unsupportedOmissionRefs,
    });
    let executorReady: (() => void) | undefined;
    const ready = new Promise<void>((resolveReady) => {
      executorReady = resolveReady;
    });
    let sourceRevision = "";
    const executor: ControlSessionExecutor = async (execution) => {
      await writeFile(
        join(execution.cwd, "cohort-agent-state.txt"),
        "owner-local cohort state\n",
        "utf8",
      );
      executorReady?.();
      await new Promise<never>((_resolve, reject) => {
        const rejectAbort = () => reject(new Error("owner-local cohort quiesced"));
        if (execution.abortSignal.aborted) return rejectAbort();
        execution.abortSignal.addEventListener("abort", rejectAbort, { once: true });
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
    const home = join(root, "pylon-home");
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home });
    const actions = createControlSessionActions({
      env: {},
      executor,
      portableDestinationHelperSupervisor: supervisor,
      portableLedger: ledger,
      summary,
      workspaceCheckoutRunner: async (workingDirectory) => {
        await mkdir(workingDirectory, { recursive: true });
        await git(workingDirectory, "init", "-b", "main");
        await git(workingDirectory, "config", "user.email", "cohort@openagents.invalid");
        await git(workingDirectory, "config", "user.name", "OpenAgents Cohort");
        await writeFile(join(workingDirectory, "tracked.txt"), "base\n", "utf8");
        await git(workingDirectory, "add", "tracked.txt");
        await git(workingDirectory, "commit", "-m", "owner-local cohort base");
        sourceRevision = await git(workingDirectory, "rev-parse", "HEAD");
        await writeFile(join(workingDirectory, "tracked.txt"), "moved\n", "utf8");
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
      objective: "Run the source-controlled owner-local portability cohort workload.",
      verify: ["true"],
    });
    await ready;

    const binding = {
      sessionRef,
      attachmentRef: attachmentA1,
      generation: 1,
      agents: [{ agentRef, controlSessionRef: started.sessionRef }],
    };
    const graphFor = (generation: number): PortableAgentGraph => ({
      rootAgentRef: agentRef,
      nodes: [
        {
          agentRef,
          threadRef: "thread.ide13.owner-local.cohort.root",
          transcriptRef: "transcript.ide13.owner-local.cohort.root",
          activityCursor: generation,
          lifecycle: "quiesced",
          attachmentGeneration: generation,
        },
      ],
    });
    const cursorsFor = (generation: number) => [
      {
        threadRef: "thread.ide13.owner-local.cohort.root",
        transcriptRef: "transcript.ide13.owner-local.cohort.root",
        activityCursor: generation,
        eventCursor: generation,
      },
    ];
    const executionBinding: PortableSessionExecutionBinding = {
      schema: "openagents.portable_session_execution_binding.v1",
      sessionRef,
      ownerRef: "owner.ide13.owner-local.cohort",
      runRef: "run.ide13.owner-local.cohort",
      repositoryRef: "repository.OpenAgentsInc.openagents.cohort",
      pinnedBaseRef: `commit.${sourceRevision}`,
    };
    let authority: PylonPortableAuthorityAttachment = {
      sessionRef,
      targetRef: targetARef,
      attachmentRef: attachmentA1,
      generation: 1,
      state: "active",
      authorityEvidenceRef: "evidence.ide13.owner-local.authority.a.1",
    };
    const authorityPort = { readCurrentAttachment: async () => authority };
    const setAuthority = (next: PylonPortableAuthorityAttachment): void => {
      if (
        next.sessionRef !== authority.sessionRef ||
        next.generation !== authority.generation + 1
      ) {
        throw new Error("owner-local cohort authority transition is not generation-fenced");
      }
      authority = next;
    };
    const producerA = new PylonPortableCheckpointArtifactStore();
    const custodyAConfig = {
      custodyDirectory: join(root, "custody-a"),
      policy: "owner_managed" as const,
      keyRef: "key.ide13.owner-local.cohort.a",
      keyProvider: { loadKey: async () => Uint8Array.from(custodyKeyA) },
      retentionSeconds: 3_600,
    };
    const custodyA = new PylonPortableCheckpointArtifactStore(custodyAConfig);
    const rehydratorB = createPylonPortableLocalRehydrator({
      targetRef: targetBRef,
      custodyRoot: join(root, "rehydrated-b"),
      artifacts: new PylonPortableCheckpointArtifactStore(custodyAConfig),
      lifecycle: actions.portable,
    });
    const destinationB = createPylonOwnerLocalDestinationLifecycle({
      targetRef: targetBRef,
      ledger,
      authority: authorityPort,
      rehydrator: rehydratorB,
    });
    const sourceA = await createPylonOwnerLocalExecutionTarget({
      targetRef: targetARef,
      ledger,
      lifecycle: actions.portable,
      binding,
      destination: destinationB,
      checkpointArtifacts: producerA,
    });

    const quiesceA = await measure(() =>
      sourceA.quiesceGraph({
        operationRef: "operation.ide13.owner-local.move.quiesce",
        sessionRef,
        attachmentRef: attachmentA1,
        generation: 1,
        graph: graphFor(1),
        threadCursors: cursorsFor(1),
      }),
    );
    phaseMilliseconds.set("quiesce", quiesceA.milliseconds);
    await waitForTerminal(actions, started.sessionRef);
    const checkpointA = await measure(() =>
      sourceA.createCheckpoint({
        operationRef: "operation.ide13.owner-local.move.checkpoint",
        checkpointRef: "checkpoint.ide13.owner-local.move.1",
        sessionRef,
        attachmentRef: attachmentA1,
        generation: 1,
        eventLogCursor: 1,
        executionBinding,
        graph: graphFor(1),
        threadCursors: cursorsFor(1),
      }),
    );
    phaseMilliseconds.set("checkpoint", checkpointA.milliseconds);
    const upload = await measure(async () => {
      const artifact = await producerA.resolve({
        ownerRef: executionBinding.ownerRef,
        targetRef: targetBRef,
        sessionRef,
        attachmentRef: attachmentB2,
        generation: 2,
        checkpointRef: checkpointA.value.checkpoint.checkpointRef,
        bundle: checkpointA.value,
      });
      try {
        await custodyA.registerArtifact({ bundle: checkpointA.value, artifact });
        return {
          artifactRef: artifact.artifactRef,
          digest: artifact.digest,
          bytes: artifact.bytes.byteLength,
        };
      } finally {
        artifact.bytes.fill(0);
      }
    });
    phaseMilliseconds.set("upload", upload.milliseconds);
    await sourceA.cleanupSource({
      operationRef: "operation.ide13.owner-local.move.source.cleanup",
      sessionRef,
      attachmentRef: attachmentA1,
      generation: 1,
      agentRefs: [agentRef],
    });
    const stageBInput = {
      operationRef: "operation.ide13.owner-local.move.destination.stage",
      bundle: checkpointA.value,
      destinationAttachmentRef: attachmentB2,
      destinationGeneration: 2,
      capabilityLeaseRefs,
    };
    const stageB = await measure(() => destinationB.stageCheckpoint(stageBInput));
    phaseMilliseconds.set("redeem", stageB.milliseconds);
    const replayedStageB = await destinationB.stageCheckpoint(stageBInput);
    if (
      replayedStageB.destinationRunnerSessionReservationRef !==
      stageB.value.destinationRunnerSessionReservationRef
    ) {
      throw new Error("owner-local destination stage replay changed its runner reservation");
    }
    let staleGenerationRejected = false;
    try {
      await destinationB.stageCheckpoint({ ...stageBInput, destinationGeneration: 3 });
    } catch {
      staleGenerationRejected = true;
    }
    if (!staleGenerationRejected)
      throw new Error("owner-local stale destination generation was accepted");
    setAuthority({
      sessionRef,
      targetRef: targetBRef,
      attachmentRef: attachmentB2,
      generation: 2,
      state: "active",
      checkpointRef: checkpointA.value.checkpoint.checkpointRef,
      authorityEvidenceRef: "evidence.ide13.owner-local.authority.b.2",
    });
    const activateBInput = {
      operationRef: "operation.ide13.owner-local.move.destination.activate",
      checkpointRef: checkpointA.value.checkpoint.checkpointRef,
      sessionRef,
      executionBinding,
      destinationAttachmentRef: attachmentB2,
      destinationGeneration: 2,
      capabilityLeaseRefs,
    };
    const activatedB = await measure(() => destinationB.activate(activateBInput));
    phaseMilliseconds.set("attach", activatedB.milliseconds);
    const helperReadinessStarted = performance.now();
    exactHelperMatrix(activatedB.value.helpers);
    phaseMilliseconds.set("helper_readiness", performance.now() - helperReadinessStarted);
    const replayedActivationB = await destinationB.activate(activateBInput);
    if (replayedActivationB.receiptRef !== activatedB.value.receiptRef) {
      throw new Error("owner-local activation replay changed its receipt");
    }
    const deletedA = await custodyA.deleteArtifact({
      operationRef: "operation.ide13.owner-local.move.custody.delete",
      ownerRef: executionBinding.ownerRef,
      sessionRef,
      checkpointRef: checkpointA.value.checkpoint.checkpointRef,
      bundle: checkpointA.value,
    });

    const failbackStarted = performance.now();
    const producerB = new PylonPortableCheckpointArtifactStore();
    const custodyBConfig = {
      custodyDirectory: join(root, "custody-b"),
      policy: "owner_managed" as const,
      keyRef: "key.ide13.owner-local.cohort.b",
      keyProvider: { loadKey: async () => Uint8Array.from(custodyKeyB) },
      retentionSeconds: 3_600,
    };
    const custodyB = new PylonPortableCheckpointArtifactStore(custodyBConfig);
    const rehydratorA = createPylonPortableLocalRehydrator({
      targetRef: targetARef,
      custodyRoot: join(root, "rehydrated-a"),
      artifacts: new PylonPortableCheckpointArtifactStore(custodyBConfig),
      lifecycle: actions.portable,
    });
    const destinationA = createPylonOwnerLocalDestinationLifecycle({
      targetRef: targetARef,
      ledger,
      authority: authorityPort,
      rehydrator: rehydratorA,
    });
    const sourceB = await createPylonOwnerLocalExecutionTarget({
      targetRef: targetBRef,
      ledger,
      lifecycle: actions.portable,
      binding: { ...binding, attachmentRef: attachmentB2, generation: 2 },
      destination: destinationA,
      checkpointArtifacts: producerB,
    });
    await sourceB.quiesceGraph({
      operationRef: "operation.ide13.owner-local.failback.quiesce",
      sessionRef,
      attachmentRef: attachmentB2,
      generation: 2,
      graph: graphFor(2),
      threadCursors: cursorsFor(2),
    });
    const checkpointB = await sourceB.createCheckpoint({
      operationRef: "operation.ide13.owner-local.failback.checkpoint",
      checkpointRef: "checkpoint.ide13.owner-local.failback.2",
      sessionRef,
      attachmentRef: attachmentB2,
      generation: 2,
      eventLogCursor: 2,
      executionBinding,
      graph: graphFor(2),
      threadCursors: cursorsFor(2),
    });
    const artifactB = await producerB.resolve({
      ownerRef: executionBinding.ownerRef,
      targetRef: targetARef,
      sessionRef,
      attachmentRef: attachmentA3,
      generation: 3,
      checkpointRef: checkpointB.checkpoint.checkpointRef,
      bundle: checkpointB,
    });
    try {
      await custodyB.registerArtifact({ bundle: checkpointB, artifact: artifactB });
    } finally {
      artifactB.bytes.fill(0);
    }
    await sourceB.cleanupSource({
      operationRef: "operation.ide13.owner-local.failback.source.cleanup",
      sessionRef,
      attachmentRef: attachmentB2,
      generation: 2,
      agentRefs: [agentRef],
    });
    await destinationA.stageCheckpoint({
      operationRef: "operation.ide13.owner-local.failback.destination.stage",
      bundle: checkpointB,
      destinationAttachmentRef: attachmentA3,
      destinationGeneration: 3,
      capabilityLeaseRefs,
    });
    setAuthority({
      sessionRef,
      targetRef: targetARef,
      attachmentRef: attachmentA3,
      generation: 3,
      state: "active",
      checkpointRef: checkpointB.checkpoint.checkpointRef,
      authorityEvidenceRef: "evidence.ide13.owner-local.authority.a.3",
    });
    const activatedA = await destinationA.activate({
      operationRef: "operation.ide13.owner-local.failback.destination.activate",
      checkpointRef: checkpointB.checkpoint.checkpointRef,
      sessionRef,
      executionBinding,
      destinationAttachmentRef: attachmentA3,
      destinationGeneration: 3,
      capabilityLeaseRefs,
    });
    exactHelperMatrix(activatedA.helpers);
    phaseMilliseconds.set("failback", performance.now() - failbackStarted);
    const deletedB = await custodyB.deleteArtifact({
      operationRef: "operation.ide13.owner-local.failback.custody.delete",
      ownerRef: executionBinding.ownerRef,
      sessionRef,
      checkpointRef: checkpointB.checkpoint.checkpointRef,
      bundle: checkpointB,
    });

    const abortController = new AbortController();
    const abortReservationRef = "runner-session-reservation.ide13.owner-local.abort";
    const abortActivation = await supervisor.activate({
      destinationRunnerSessionReservationRef: abortReservationRef,
      sessionRef: "session.ide13.owner-local.abort",
      destinationAttachmentRef: "attachment.ide13.owner-local.abort.1",
      destinationGeneration: 1,
      workspaceRef: "workspace.ide13.owner-local.abort",
      workingDirectory: repositoryRoot,
      authorityEvidenceRef: "evidence.ide13.owner-local.abort.authority",
      authenticationPolicyRef: "policy.ide13.owner-local.abort.authentication",
      capabilityLeaseRefs,
      signal: abortController.signal,
    });
    exactHelperMatrix(abortActivation.helpers);
    const abortReplay = await supervisor.activate({
      destinationRunnerSessionReservationRef: abortReservationRef,
      sessionRef: "session.ide13.owner-local.abort",
      destinationAttachmentRef: "attachment.ide13.owner-local.abort.1",
      destinationGeneration: 1,
      workspaceRef: "workspace.ide13.owner-local.abort",
      workingDirectory: repositoryRoot,
      authorityEvidenceRef: "evidence.ide13.owner-local.abort.authority",
      authenticationPolicyRef: "policy.ide13.owner-local.abort.authentication",
      capabilityLeaseRefs,
      signal: abortController.signal,
    });
    if (abortReplay.evidenceRefs.join("\n") !== abortActivation.evidenceRefs.join("\n")) {
      throw new Error("owner-local helper replay changed its evidence");
    }
    abortController.abort(new Error("owner-local cohort abort proof"));
    await supervisor.disposeReservation(abortReservationRef);

    const teardownStarted = performance.now();
    const sourceA3 = await createPylonOwnerLocalExecutionTarget({
      targetRef: targetARef,
      ledger,
      lifecycle: actions.portable,
      binding: { ...binding, attachmentRef: attachmentA3, generation: 3 },
    });
    await sourceA3.quiesceGraph({
      operationRef: "operation.ide13.owner-local.teardown.quiesce",
      sessionRef,
      attachmentRef: attachmentA3,
      generation: 3,
      graph: graphFor(3),
      threadCursors: cursorsFor(3),
    });
    const finalCheckpoint = await sourceA3.createCheckpoint({
      operationRef: "operation.ide13.owner-local.teardown.checkpoint",
      checkpointRef: "checkpoint.ide13.owner-local.teardown.3",
      sessionRef,
      attachmentRef: attachmentA3,
      generation: 3,
      eventLogCursor: 3,
      executionBinding,
      graph: graphFor(3),
      threadCursors: cursorsFor(3),
    });
    const teardown = await sourceA3.cleanupSource({
      operationRef: "operation.ide13.owner-local.teardown.source.cleanup",
      sessionRef,
      attachmentRef: attachmentA3,
      generation: 3,
      agentRefs: [agentRef],
    });
    await actions.portable.shutdownHelpers?.();
    phaseMilliseconds.set("teardown", performance.now() - teardownStarted);
    if (supervisor.disposalFailures().length !== 0) {
      throw new Error("owner-local helper teardown recorded a disposal failure");
    }
    if (
      repositoryOwnedPylonPortableExecutableProfileCatalog.resolve(
        "profile.ide13.owner-local.unadmitted",
      ) !== null
    ) {
      throw new Error("owner-local executable profile catalog unexpectedly admitted a profile");
    }
    const nonTerminalSessions = (await actions.list()).filter(
      (row) => row.state !== "cancelled" && row.state !== "completed" && row.state !== "failed",
    ).length;
    const activeCustodyObjects = (
      await Promise.all(
        [custodyAConfig.custodyDirectory, custodyBConfig.custodyDirectory].map(
          async (directory) =>
            (await readdir(directory)).filter((name) => name.endsWith(".checkpoint.aesgcm")).length,
        ),
      )
    ).reduce((sum, count) => sum + count, 0);
    maxRssBytes = Math.max(maxRssBytes, process.memoryUsage().rss);
    const cpu = process.cpuUsage(cpuStarted);
    const wallMicroseconds = Math.max((performance.now() - wallStarted) * 1_000, 1);
    const cpuPercent = ((cpu.user + cpu.system) / wallMicroseconds) * 100;
    const resourceResidue =
      nonTerminalSessions + activeCustodyObjects + supervisor.disposalFailures().length;
    const metricReceipt = stableRef(
      "receipt.ide13.owner-local.metrics",
      `${candidateCommitSha}:${upload.value.digest}:${finalCheckpoint.checkpoint.digest}`,
    );
    const phaseReceipts = {
      quiesce: quiesceA.value.evidenceRefs[0] ?? stableRef("receipt.ide13.quiesce", sessionRef),
      checkpoint:
        checkpointA.value.checkpoint.receiptRefs[0] ?? checkpointA.value.checkpoint.checkpointRef,
      upload: stableRef("receipt.ide13.owner-local.upload", upload.value.digest),
      redeem: stageB.value.evidenceRefs[0] ?? stageB.value.destinationRunnerSessionReservationRef,
      attach: activatedB.value.receiptRef,
      helper_readiness: stableRef("receipt.ide13.owner-local.helpers", activatedB.value.receiptRef),
      failback: activatedA.receiptRef,
      teardown: teardown.evidenceRefs[0] ?? finalCheckpoint.checkpoint.checkpointRef,
    } satisfies Record<Phase, string>;
    const phaseOperationRefs = {
      quiesce: "operation.ide13.owner-local.move.quiesce",
      checkpoint: "operation.ide13.owner-local.move.checkpoint",
      upload: "operation.ide13.owner-local.move.custody.upload",
      redeem: "operation.ide13.owner-local.move.destination.stage",
      attach: "operation.ide13.owner-local.move.destination.activate",
      helper_readiness: "operation.ide13.owner-local.move.helper-readiness",
      failback: "operation.ide13.owner-local.failback.destination.activate",
      teardown: "operation.ide13.owner-local.teardown.source.cleanup",
    } satisfies Record<Phase, string>;
    const phaseGenerations = {
      quiesce: 1,
      checkpoint: 1,
      upload: 1,
      redeem: 2,
      attach: 2,
      helper_readiness: 2,
      failback: 3,
      teardown: 3,
    } satisfies Record<Phase, number>;
    const metrics: Metric[] = [
      ...IDE_PORTABLE_PHASES.map((phase) =>
        phaseMetric(phase, phaseMilliseconds.get(phase) ?? 0, phaseReceipts[phase]),
      ),
      pointMetric("checkpoint_size", upload.value.bytes, "bytes", 64 * 1024 * 1024, metricReceipt),
      pointMetric("cpu", cpuPercent, "percent", 100, metricReceipt),
      pointMetric("memory", maxRssBytes, "bytes", 2 * 1024 * 1024 * 1024, metricReceipt),
      pointMetric("network", 0, "bytes_per_second", 0, metricReceipt),
      pointMetric("queue", nonTerminalSessions, "count", 0, metricReceipt),
      pointMetric("lease", capabilityLeaseRefs.length, "count", 1, metricReceipt),
      pointMetric("resource_cleanup", resourceResidue, "count", 0, metricReceipt),
      pointMetric(
        "teardown",
        phaseMilliseconds.get("teardown") ?? 0,
        "milliseconds",
        30_000,
        metricReceipt,
      ),
    ];
    const receipt = decodeReceipt(
      {
        schemaVersion: "openagents.desktop.ide-portable-owner-local-cohort.v1",
        evidenceContractVersion: "openagents.desktop.ide-portable-evidence.v3",
        generatedAt: new Date().toISOString(),
        cohort: {
          cohortRef: "cohort.ide13.owner-local.real.1",
          targetClass: "owner_local",
          evidenceClass: "real_local",
          journeyScope: "full_move",
          journeys: {
            mainJourneyReceiptRef: activatedB.value.receiptRef,
            failbackJourneyReceiptRef: activatedA.receiptRef,
            faultMatrixReceiptRef: null,
          },
          operatingSystem:
            process.platform === "darwin"
              ? "darwin"
              : process.platform === "win32"
                ? "windows"
                : process.platform === "linux"
                  ? "linux"
                  : "unknown",
          architecture:
            process.arch === "arm64" ? "arm64" : process.arch === "x64" ? "x64" : "unknown",
          adapter: {
            kind: "production",
            ref: "adapter.pylon.owner-local.control-session.v1",
            name: "Pylon owner-local control-session target",
            version: "1",
          },
          targetRef: targetARef,
          artifact: {
            ref: upload.value.artifactRef,
            sha256: upload.value.digest.slice("sha256:".length),
            bytes: upload.value.bytes,
          },
          candidateCommitSha,
          baseCommitSha,
          capabilityState: "degraded",
          custody: "owner_device",
          networkDestinations: [],
          dataDestinations: ["owner_device"],
          retentionSeconds: 3_600,
          costFact:
            "No provider or network call ran. The cohort used local CPU, memory, SQLite, and filesystem resources.",
          phaseReceipts: IDE_PORTABLE_PHASES.map((phase) => ({
            phase,
            evidenceClass: "real_local",
            receiptRef: phaseReceipts[phase],
            operationRef: phaseOperationRefs[phase],
            attachmentGeneration: phaseGenerations[phase],
            result: "passed",
          })),
          metrics,
          result:
            "The source-controlled local cohort completed generation 1 to 2 move, generation 2 to 3 failback, replay, stale-generation refusal, abort teardown, encrypted artifact deletion, and final helper teardown. The retained control-session process stayed settled, no Codex executor resumed, and no work ref was accepted. LSP, DAP, and native stayed unsupported because no signed executable profile is admitted.",
        },
        helpers: activatedA.helpers,
        authority: {
          catalogRef: "catalog.pylon.portable-executable-profile.empty.v1",
          admittedExecutableProfileRefs: [],
          capabilityLeaseRefs,
          unsupportedProfileOmissionRef:
            PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
        },
        execution: {
          acceptedWorkRefCount: 0,
          controlSessionProcessLifecycle: "settled",
          executorResumed: false,
          omissionRef: "omission.ide13.owner-local.codex-executor-resumption-not-implemented",
        },
        proofs: {
          abortReceiptRef: stableRef("receipt.ide13.owner-local.abort", abortReservationRef),
          replayReceiptRef: stableRef(
            "receipt.ide13.owner-local.replay",
            `${replayedStageB.destinationRunnerSessionReservationRef}:${replayedActivationB.receiptRef}`,
          ),
          staleGenerationReceiptRef: stableRef(
            "receipt.ide13.owner-local.stale-generation-refused",
            stageBInput.operationRef,
          ),
          sourceCustodyDeletionReceiptRef: assertDeletion(deletedA),
          failbackCustodyDeletionReceiptRef: assertDeletion(deletedB),
          teardownReceiptRef: teardown.evidenceRefs[0] ?? finalCheckpoint.checkpoint.checkpointRef,
        },
      },
      { onExcessProperty: "error" },
    );
    if (metrics.some((metric) => !metric.passed)) {
      throw new Error("owner-local cohort metric threshold failed");
    }
    if (input.outputPath !== undefined) {
      await mkdir(dirname(input.outputPath), { recursive: true });
      await writeFile(input.outputPath, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
    }
    return receipt;
  } finally {
    custodyKeyA.fill(0);
    custodyKeyB.fill(0);
    database.close();
    await rm(root, { recursive: true, force: true });
  }
};

if (import.meta.main) {
  const repositoryRoot = resolve(join(import.meta.dirname, "../../.."));
  const outputPath = resolve(
    repositoryRoot,
    "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-real-cohort.json",
  );
  const receipt = await runIde13OwnerLocalRealCohort({ outputPath, repositoryRoot });
  process.stdout.write(
    `${JSON.stringify({
      outputPath,
      cohortRef: receipt.cohort.cohortRef,
      evidenceClass: receipt.cohort.evidenceClass,
      result: "passed",
    })}\n`,
  );
}

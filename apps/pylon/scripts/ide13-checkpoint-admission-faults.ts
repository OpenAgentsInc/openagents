import { Runtime } from "@openagentsinc/runtime-platform";
import { createHash } from "node:crypto";
import { access, mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

import { canonicalJson } from "@openagentsinc/khala-sync";
import type {
  PortableCheckpointArtifact,
  PortableCheckpointArtifactResolver,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";

import {
  IDE_PORTABLE_EVIDENCE_CLASSES,
  IDE_PORTABLE_FAULT_SCENARIOS,
} from "../../openagents-desktop/src/ide/portable-evidence-contract.ts";
import type { PylonPortableControlSessionLifecycle } from "../src/node/control-sessions.js";
import {
  makePylonPortableDestinationHelperSupervisor,
  PylonPortableDestinationHelperSupervisorError,
} from "../src/portable-destination-helper-supervisor.js";
import { PylonPortableCheckpointArtifactStore } from "../src/portable-session-checkpoint-artifact.js";
import { createPylonPortableLocalRehydrator } from "../src/portable-session-local-rehydrator.js";
import type { PylonPortableCheckpointBundle } from "../src/portable-session-operation-ledger.js";

const GIT_SHA = /^[a-f0-9]{40}$/u;
const RECEIPT_REPOSITORY_PATH =
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-checkpoint-admission-faults.json";
const DEADLINE_MILLISECONDS = 30_000;
const SCENARIOS = [
  "corrupt_checkpoint",
  "truncated_checkpoint",
  "wrong_schema_checkpoint",
  "missing_artifact",
  "auth_expiry_revocation",
  "provider_capability_drift",
  "destination_boot_failure",
  "source_revocation_failure",
] as const;

const Ref = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);
const FaultCase = Schema.Struct({
  faultRef: Ref,
  scenario: Schema.Literals(IDE_PORTABLE_FAULT_SCENARIOS),
  evidenceClass: Schema.Literals(IDE_PORTABLE_EVIDENCE_CLASSES),
  outcome: Schema.Literals(["passed", "not_run"]),
  productionBoundaryRef: Schema.NullOr(Ref),
  injectedFaultRef: Schema.NullOr(Ref),
  rejectionRef: Schema.NullOr(Ref),
  elapsedMilliseconds: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  deadlineMilliseconds: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  residueCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  disclosure: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(600)),
});

export const Ide13CheckpointAdmissionFaultReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-portable-checkpoint-admission-faults.v1"),
  evidenceContractVersion: Schema.Literal("openagents.desktop.ide-portable-evidence.v3"),
  generatedAt: Schema.String,
  candidateCommitSha: Schema.String.check(Schema.isPattern(GIT_SHA)),
  baseCommitSha: Schema.String.check(Schema.isPattern(GIT_SHA)),
  harnessRef: Schema.Literal("harness.ide13.checkpoint-admission.production-components.v1"),
  cases: Schema.Array(FaultCase).check(
    Schema.isMinLength(SCENARIOS.length),
    Schema.isMaxLength(SCENARIOS.length),
  ),
  summary: Schema.Struct({
    requiredCaseCount: Schema.Literal(SCENARIOS.length),
    passedSimulatorCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
    notRunCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
    acceptanceContributionCount: Schema.Literal(0),
    acceptanceReady: Schema.Literal(false),
  }),
  safety: Schema.Struct({
    forbiddenMaterialProjected: Schema.Literal(false),
    residueCount: Schema.Literal(0),
  }),
});

export interface Ide13CheckpointAdmissionFaultReceipt extends Schema.Schema.Type<
  typeof Ide13CheckpointAdmissionFaultReceiptSchema
> {}

type Scenario = (typeof SCENARIOS)[number];
type FaultCaseResult = Schema.Schema.Type<typeof FaultCase>;

const decodeReceipt = Schema.decodeUnknownSync(Ide13CheckpointAdmissionFaultReceiptSchema);
const sha256 = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;
const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`;

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

const fixture = async (
  root: string,
): Promise<
  Readonly<{
    bundle: PylonPortableCheckpointBundle;
    artifact: PortableCheckpointArtifact;
  }>
> => {
  const repository = join(root, "repository");
  await mkdir(repository, { recursive: true });
  await git(repository, "init", "-q");
  await git(repository, "config", "user.name", "IDE-13 Fault Fixture");
  await git(repository, "config", "user.email", "ide13@example.invalid");
  await writeFile(join(repository, "tracked.txt"), "tracked\n", { mode: 0o644 });
  await git(repository, "add", "tracked.txt");
  await git(repository, "commit", "-qm", "fixture");
  const revision = await git(repository, "rev-parse", "HEAD");
  const graph = {
    rootAgentRef: "agent.ide13.checkpoint-fault.root",
    nodes: [
      {
        agentRef: "agent.ide13.checkpoint-fault.root",
        threadRef: "thread.ide13.checkpoint-fault.root",
        transcriptRef: "transcript.ide13.checkpoint-fault.root",
        activityCursor: 1,
        lifecycle: "quiesced" as const,
        attachmentGeneration: 1,
      },
    ],
  };
  const threadCursors = [
    {
      threadRef: graph.nodes[0].threadRef,
      transcriptRef: graph.nodes[0].transcriptRef,
      activityCursor: 1,
      eventCursor: 1,
    },
  ];
  const postImage = createHash("sha256")
    .update("tracked.txt")
    .update("\0")
    .update("tracked\n")
    .update("\0")
    .digest("hex");
  const payload = {
    schema: "openagents.portable_checkpoint.v1" as const,
    checkpointRef: "checkpoint.ide13.checkpoint-fault.1",
    sessionRef: "session.ide13.checkpoint-fault",
    sourceAttachmentRef: "attachment.ide13.checkpoint-fault.1",
    sourceGeneration: 1,
    repositoryRef: "repository.OpenAgentsInc.openagents",
    repositoryRevisionRef: revision,
    repositoryPostImageDigest: `sha256:${postImage}` as const,
    diffDigest: sha256(""),
    eventLogCursor: 1,
    catalogGenerationRef: "catalog.ide13.checkpoint-fault.1",
    graphDigest: sha256(canonicalJson(graph)),
    approvalRefs: [],
    artifactRefs: [],
    receiptRefs: ["receipt.ide13.checkpoint-fault.source"],
    secretMaterial: "excluded" as const,
    processState: "excluded" as const,
  };
  const bundle: PylonPortableCheckpointBundle = {
    checkpoint: { ...payload, digest: sha256(canonicalJson(payload)) },
    executionBinding: {
      schema: "openagents.portable_session_execution_binding.v1",
      sessionRef: payload.sessionRef,
      ownerRef: "owner.ide13.checkpoint-fault",
      runRef: "run.ide13.checkpoint-fault",
      repositoryRef: payload.repositoryRef,
      pinnedBaseRef: revision,
    },
    graph,
    threadCursors,
  };
  const store = new PylonPortableCheckpointArtifactStore();
  store.register({ bundle, workingDirectory: repository });
  const artifact = await store.resolve({
    ownerRef: bundle.executionBinding.ownerRef,
    targetRef: "target.ide13.checkpoint-fault.destination",
    sessionRef: bundle.checkpoint.sessionRef,
    attachmentRef: "attachment.ide13.checkpoint-fault.2",
    generation: 2,
    checkpointRef: bundle.checkpoint.checkpointRef,
    bundle,
  });
  return { artifact, bundle };
};

const lifecycleFixture = (): PylonPortableControlSessionLifecycle => ({
  bind: () => undefined,
  recover: async () => ({
    schema: "openagents.pylon.portable_control_binding_recovery.v1",
    recoveryRef: "recovery.ide13.checkpoint-fault",
    sessionRef: "session.ide13.checkpoint-fault",
    attachmentRef: "attachment.ide13.checkpoint-fault.2",
    generation: 2,
    restoredAgentRefs: [],
    missingAgentRefs: [],
    staleAgentRefs: [],
    evidenceRefs: ["receipt.ide13.checkpoint-fault.recovery"],
  }),
  quiesce: async () => ({ quiescedAgentRefs: [], evidenceRefs: [] }),
  checkpointSource: async () => ({
    workingDirectory: "/private/unreachable",
    workspaceRef: "workspace.ide13.checkpoint-fault",
    artifactRefs: [],
    approvalRefs: [],
  }),
  cleanup: async () => ({
    cleanedAgentRefs: [],
    cleanupReceiptRef: "receipt.ide13.checkpoint-fault.cleanup",
    evidenceRefs: [],
  }),
  stageDestination: async () => ({ evidenceRefs: ["receipt.ide13.checkpoint-fault.staged"] }),
  activateDestination: async (input) => ({
    authentication: {
      state: "reauthenticated",
      policyRef: input.authenticationPolicyRef,
      evidenceRef: input.authorityEvidenceRef,
      observedAt: "2026-07-20T00:00:00.000Z",
      expiresAt: null,
    },
    helpersObservedAt: "2026-07-20T00:00:00.000Z",
    helpers: [],
    evidenceRefs: [],
  }),
  abortDestination: async () => ({ evidenceRefs: ["receipt.ide13.checkpoint-fault.aborted"] }),
});

const mutatedArtifact = (
  artifact: PortableCheckpointArtifact,
  scenario: "corrupt_checkpoint" | "truncated_checkpoint" | "wrong_schema_checkpoint",
): PortableCheckpointArtifact => {
  if (scenario === "corrupt_checkpoint") {
    const bytes = Uint8Array.from(artifact.bytes);
    bytes[Math.floor(bytes.byteLength / 2)] ^= 0xff;
    return { ...artifact, bytes, digest: sha256(bytes) };
  }
  if (scenario === "truncated_checkpoint") {
    const bytes = artifact.bytes.slice(0, Math.max(1, artifact.bytes.byteLength - 64));
    return { ...artifact, bytes, digest: sha256(bytes) };
  }
  const tar = Runtime.zstdDecompressSync(artifact.bytes);
  const current = Buffer.from("openagents.portable_checkpoint_artifact.v1", "utf8");
  const replacement = Buffer.from("openagents.portable_checkpoint_artifact.v2", "utf8");
  const offset = Buffer.from(tar).indexOf(current);
  if (offset < 0) throw new Error("checkpoint fixture manifest schema is absent");
  tar.set(replacement, offset);
  const bytes = Runtime.zstdCompressSync(tar);
  tar.fill(0);
  return { ...artifact, bytes, digest: sha256(bytes) };
};

const runCheckpointCase = async (
  scenario:
    | "corrupt_checkpoint"
    | "truncated_checkpoint"
    | "wrong_schema_checkpoint"
    | "missing_artifact",
  root: string,
): Promise<string> => {
  const source = await fixture(root);
  const resolver: PortableCheckpointArtifactResolver =
    scenario === "missing_artifact"
      ? new PylonPortableCheckpointArtifactStore()
      : {
          resolve: async () => mutatedArtifact(source.artifact, scenario),
        };
  const custodyRoot = join(root, "destination-custody");
  const rehydrator = createPylonPortableLocalRehydrator({
    targetRef: "target.ide13.checkpoint-fault.destination",
    custodyRoot,
    artifacts: resolver,
    lifecycle: lifecycleFixture(),
  });
  try {
    await rehydrator.stage({
      operationRef: `operation.ide13.${scenario}.destination.stage`,
      destinationRunnerSessionReservationRef: `reservation.ide13.${scenario}`,
      bundle: source.bundle,
      destinationAttachmentRef: "attachment.ide13.checkpoint-fault.2",
      destinationGeneration: 2,
      capabilityLeaseRefs: ["lease.ide13.checkpoint-fault.2"],
    });
  } catch (error) {
    await access(custodyRoot).then(
      async () => {
        const entries = await readdir(custodyRoot);
        if (entries.length !== 0) {
          throw new Error(`${scenario} left destination custody residue`, { cause: error });
        }
      },
      () => undefined,
    );
    return stableRef(
      "rejection.ide13.checkpoint-admission",
      `${scenario}:${error instanceof Error ? error.message : "unknown"}`,
    );
  }
  throw new Error(`${scenario} was accepted`);
};

const runAuthenticationExpiryCase = async (root: string): Promise<string> => {
  const supervisor = makePylonPortableDestinationHelperSupervisor({
    now: () => new Date("2026-07-20T01:00:00.000Z"),
    authenticator: {
      authenticate: async (input) => ({
        state: "reauthenticated",
        policyRef: input.authenticationPolicyRef,
        evidenceRef: input.authorityEvidenceRef,
        observedAt: "2026-07-20T00:00:00.000Z",
        expiresAt: "2026-07-20T00:30:00.000Z",
      }),
    },
  });
  try {
    await supervisor.activate({
      destinationRunnerSessionReservationRef: "reservation.ide13.auth-expiry",
      sessionRef: "session.ide13.auth-expiry",
      destinationAttachmentRef: "attachment.ide13.auth-expiry.2",
      destinationGeneration: 2,
      workspaceRef: "workspace.ide13.auth-expiry",
      workingDirectory: root,
      authorityEvidenceRef: "authority.ide13.auth-expiry.2",
      authenticationPolicyRef: "policy.ide13.auth-expiry",
      capabilityLeaseRefs: [],
    });
  } catch (error) {
    if (
      !(error instanceof PylonPortableDestinationHelperSupervisorError) ||
      error.reason !== "authentication_failed"
    ) {
      throw error;
    }
    await supervisor.disposeAll();
    return error.failureRef;
  }
  throw new Error("expired destination authentication was accepted");
};

const runDestinationBootCase = async (root: string): Promise<string> => {
  let disposed = 0;
  const supervisor = makePylonPortableDestinationHelperSupervisor({
    now: () => new Date("2026-07-20T01:00:00.000Z"),
    authenticator: {
      authenticate: async (input) => ({
        state: "reauthenticated",
        policyRef: input.authenticationPolicyRef,
        evidenceRef: input.authorityEvidenceRef,
        observedAt: "2026-07-20T01:00:00.000Z",
        expiresAt: null,
      }),
    },
    adapters: [
      {
        kind: "pty",
        start: async () => ({
          instanceRef: "helper.ide13.boot.pty",
          versionRef: "version.ide13.boot.pty",
          evidenceRefs: ["receipt.ide13.boot.pty"],
          isLive: () => true,
          dispose: () => {
            disposed += 1;
          },
        }),
      },
      {
        kind: "lsp",
        start: async () => {
          throw new Error("injected destination boot failure");
        },
      },
    ],
  });
  try {
    await supervisor.activate({
      destinationRunnerSessionReservationRef: "reservation.ide13.destination-boot",
      sessionRef: "session.ide13.destination-boot",
      destinationAttachmentRef: "attachment.ide13.destination-boot.2",
      destinationGeneration: 2,
      workspaceRef: "workspace.ide13.destination-boot",
      workingDirectory: root,
      authorityEvidenceRef: "authority.ide13.destination-boot.2",
      authenticationPolicyRef: "policy.ide13.destination-boot",
      capabilityLeaseRefs: ["lease.ide13.destination-boot.2"],
    });
  } catch (error) {
    if (
      !(error instanceof PylonPortableDestinationHelperSupervisorError) ||
      error.reason !== "helper_start_failed" ||
      disposed !== 1
    ) {
      throw error;
    }
    await supervisor.disposeAll();
    return error.failureRef;
  }
  throw new Error("destination boot failure was accepted");
};

const disclosure = (scenario: Scenario): string => {
  if (scenario === "provider_capability_drift") {
    return "The production destination path has capability lease refs but no source-controlled authoritative provider-catalog drift seam. This case did not run and does not contribute acceptance evidence.";
  }
  if (scenario === "source_revocation_failure") {
    return "The production owner-local cleanup path has no source-controlled revocation failure seam that proves an external authority refusal. This case did not run and does not contribute acceptance evidence.";
  }
  return scenario === "destination_boot_failure"
    ? "The harness used the production helper supervisor with fixture adapters. It injected an LSP start failure after a PTY start and verified that the supervisor disposed the PTY. This is simulator evidence, not a real target boot failure."
    : scenario === "auth_expiry_revocation"
      ? "The harness used the production helper supervisor with a fixture authenticator. It returned expired authentication and verified fail-closed admission before helper start. This is simulator evidence, not an external credential revocation."
      : "The harness used the production checkpoint artifact store or local rehydrator with a fixture repository and a source-controlled artifact mutation. It verified fail-closed rejection and no destination residue. This is simulator evidence, not a full owner-local move.";
};

export const runIde13CheckpointAdmissionFaults = async (
  input: Readonly<{
    candidateCommitSha?: string;
    outputPath?: string;
    repositoryRoot?: string;
  }> = {},
): Promise<Ide13CheckpointAdmissionFaultReceipt> => {
  const repositoryRoot = resolve(input.repositoryRoot ?? join(import.meta.dirname, "../../.."));
  const headCommitSha = await git(repositoryRoot, "rev-parse", "HEAD");
  const candidateCommitSha = input.candidateCommitSha ?? headCommitSha;
  if (!GIT_SHA.test(candidateCommitSha))
    throw new Error("fault receipt candidate commit is invalid");
  await git(repositoryRoot, "merge-base", "--is-ancestor", candidateCommitSha, headCommitSha);
  const laterPaths = (
    await git(repositoryRoot, "diff", "--name-only", candidateCommitSha, headCommitSha)
  )
    .split("\n")
    .filter((path) => path.length > 0);
  if (laterPaths.some((path) => path !== RECEIPT_REPOSITORY_PATH)) {
    throw new Error("fault receipt candidate omits an implementation change");
  }
  const baseCommitSha = await git(repositoryRoot, "merge-base", candidateCommitSha, "origin/main");
  const runScenario = async (scenario: Scenario): Promise<FaultCaseResult> => {
    if (scenario === "provider_capability_drift" || scenario === "source_revocation_failure") {
      return {
        faultRef: `fault.ide13.checkpoint-admission.${scenario}`,
        scenario,
        evidenceClass: "not_run",
        outcome: "not_run",
        productionBoundaryRef: null,
        injectedFaultRef: null,
        rejectionRef: null,
        elapsedMilliseconds: 0,
        deadlineMilliseconds: DEADLINE_MILLISECONDS,
        residueCount: 0,
        disclosure: disclosure(scenario),
      };
    }
    const root = await mkdtemp(join(tmpdir(), `oa-ide13-${scenario}-`));
    const startedAt = performance.now();
    try {
      const rejectionRef =
        scenario === "auth_expiry_revocation"
          ? await runAuthenticationExpiryCase(root)
          : scenario === "destination_boot_failure"
            ? await runDestinationBootCase(root)
            : await runCheckpointCase(scenario, root);
      const elapsedMilliseconds = performance.now() - startedAt;
      if (elapsedMilliseconds > DEADLINE_MILLISECONDS) {
        throw new Error(`${scenario} exceeded its cleanup deadline`);
      }
      return {
        faultRef: `fault.ide13.checkpoint-admission.${scenario}`,
        scenario,
        evidenceClass: "simulator",
        outcome: "passed",
        productionBoundaryRef:
          scenario === "auth_expiry_revocation" || scenario === "destination_boot_failure"
            ? "boundary.pylon.portable-destination-helper-supervisor.activate"
            : "boundary.pylon.portable-local-rehydrator.stage",
        injectedFaultRef: `injected-fault.ide13.checkpoint-admission.${scenario}`,
        rejectionRef,
        elapsedMilliseconds,
        deadlineMilliseconds: DEADLINE_MILLISECONDS,
        residueCount: 0,
        disclosure: disclosure(scenario),
      };
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  };
  const cases = await Promise.all(SCENARIOS.map(runScenario));
  if (cases.map((row) => row.scenario).join("\n") !== SCENARIOS.join("\n")) {
    throw new Error("checkpoint admission fault cases are incomplete or out of order");
  }
  const serialized = JSON.stringify(cases);
  if (/\/Users\/|\/private\/tmp\/|Bearer|password|processId|\bpid\b/u.test(serialized)) {
    throw new Error("checkpoint admission fault receipt contains forbidden material");
  }
  const receipt = decodeReceipt(
    {
      schemaVersion: "openagents.desktop.ide-portable-checkpoint-admission-faults.v1",
      evidenceContractVersion: "openagents.desktop.ide-portable-evidence.v3",
      generatedAt: new Date().toISOString(),
      candidateCommitSha,
      baseCommitSha,
      harnessRef: "harness.ide13.checkpoint-admission.production-components.v1",
      cases,
      summary: {
        requiredCaseCount: SCENARIOS.length,
        passedSimulatorCount: cases.filter((row) => row.evidenceClass === "simulator").length,
        notRunCount: cases.filter((row) => row.evidenceClass === "not_run").length,
        acceptanceContributionCount: 0,
        acceptanceReady: false,
      },
      safety: {
        forbiddenMaterialProjected: false,
        residueCount: 0,
      },
    },
    { onExcessProperty: "error" },
  );
  if (input.outputPath !== undefined) {
    await mkdir(dirname(input.outputPath), { recursive: true });
    await writeFile(input.outputPath, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
  }
  return receipt;
};

if (import.meta.main) {
  const repositoryRoot = resolve(join(import.meta.dirname, "../../.."));
  const candidateCommitSha = process.env.OPENAGENTS_IDE13_CANDIDATE_COMMIT_SHA;
  const receipt = await runIde13CheckpointAdmissionFaults({
    ...(candidateCommitSha === undefined ? {} : { candidateCommitSha }),
    outputPath: resolve(repositoryRoot, RECEIPT_REPOSITORY_PATH),
    repositoryRoot,
  });
  process.stdout.write(
    `${JSON.stringify({
      outputPath: RECEIPT_REPOSITORY_PATH,
      passedSimulatorCount: receipt.summary.passedSimulatorCount,
      notRunCount: receipt.summary.notRunCount,
      acceptanceReady: receipt.summary.acceptanceReady,
    })}\n`,
  );
  process.exit(0);
}

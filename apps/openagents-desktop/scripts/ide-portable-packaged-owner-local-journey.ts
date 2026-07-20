import { execFileSync, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { chromium, type Browser, type Page } from "playwright";
import { Schema } from "effect";

import {
  IdePortableClientCommandResultSchema,
  IdePortableClientSnapshotSchema,
} from "../src/ide/portable-client-contract.ts";
import { packagedArtifactTreeDigest, resolvePackagedApp } from "./ide-packaged-artifact.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const evidenceRoot = path.join(appRoot, "benchmarks", "ide");
const receiptPath = path.join(evidenceRoot, "2026-07-20-ide-13-packaged-owner-local-journey.json");
const screenshotPath = path.join(
  evidenceRoot,
  "2026-07-20-ide-13-packaged-owner-local-journey.png",
);
const tracePath = path.join(
  evidenceRoot,
  "2026-07-20-ide-13-packaged-owner-local-journey-trace.json",
);
const screenshotRef =
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-packaged-owner-local-journey.png";
const traceRef =
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-packaged-owner-local-journey-trace.json";
const allowedEvidencePaths = new Set([
  screenshotRef,
  traceRef,
  path.relative(repositoryRoot, receiptPath),
]);

const Sha40 = Schema.String.check(Schema.isPattern(/^[a-f0-9]{40}$/u));
const Sha256 = Schema.String.check(Schema.isPattern(/^[a-f0-9]{64}$/u));
const PublicRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._/-]*$/u),
);
const ArtifactRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._/ -]*$/u),
);
const BoundedText = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1_000));
const ReceiptRef = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);

const OwnerLocalCohortSourceSchema = Schema.Struct({
  cohort: Schema.Struct({
    cohortRef: ReceiptRef,
    targetClass: Schema.Literal("owner_local"),
    evidenceClass: Schema.Literal("real_local"),
    journeyScope: Schema.Literal("full_move"),
    candidateCommitSha: Sha40,
    targetRef: ReceiptRef,
    adapter: Schema.Struct({
      kind: Schema.Literal("production"),
      ref: ReceiptRef,
    }),
    phaseReceipts: Schema.Array(
      Schema.Struct({
        phase: Schema.Literals([
          "quiesce",
          "checkpoint",
          "upload",
          "redeem",
          "attach",
          "helper_readiness",
          "failback",
          "teardown",
        ]),
        evidenceClass: Schema.Literal("real_local"),
        receiptRef: ReceiptRef,
        attachmentGeneration: Schema.Number.check(
          Schema.isInt(),
          Schema.isGreaterThanOrEqualTo(1),
          Schema.isLessThanOrEqualTo(3),
        ),
        result: Schema.Literal("passed"),
      }),
    ).check(Schema.isMinLength(8), Schema.isMaxLength(8)),
  }),
  helpers: Schema.Array(
    Schema.Struct({
      kind: Schema.Literals(["pty", "lsp", "dap", "watcher", "native"]),
      readiness: Schema.Literals(["ready", "unsupported"]),
    }),
  ).check(Schema.isMinLength(5), Schema.isMaxLength(5)),
  execution: Schema.Struct({
    acceptedWorkRefCount: Schema.Literal(0),
    controlSessionProcessLifecycle: Schema.Literal("settled"),
    executorResumed: Schema.Literal(false),
    omissionRef: ReceiptRef,
  }),
  proofs: Schema.Struct({
    replayReceiptRef: ReceiptRef,
    staleGenerationReceiptRef: ReceiptRef,
    sourceCustodyDeletionReceiptRef: ReceiptRef,
    failbackCustodyDeletionReceiptRef: ReceiptRef,
    teardownReceiptRef: ReceiptRef,
  }),
});

const OwnerLocalJourneyProjectionSchema = Schema.Struct({
  cohortRef: ReceiptRef,
  targetClass: Schema.Literal("owner_local"),
  evidenceClass: Schema.Literal("real_local"),
  journeyScope: Schema.Literal("full_move"),
  candidateCommitSha: Sha40,
  targetRef: ReceiptRef,
  adapterKind: Schema.Literal("production"),
  adapterRef: ReceiptRef,
  move: Schema.Struct({
    sourceGeneration: Schema.Literal(1),
    destinationGeneration: Schema.Literal(2),
    attachReceiptRef: ReceiptRef,
    helperReadinessReceiptRef: ReceiptRef,
  }),
  failback: Schema.Struct({
    sourceGeneration: Schema.Literal(2),
    destinationGeneration: Schema.Literal(3),
    receiptRef: ReceiptRef,
  }),
  teardownReceiptRef: ReceiptRef,
  readyHelpers: Schema.Array(Schema.Literals(["pty", "lsp", "watcher"])).check(
    Schema.isMinLength(3),
    Schema.isMaxLength(3),
  ),
  unsupportedHelpers: Schema.Array(Schema.Literals(["dap", "native"])).check(
    Schema.isMinLength(2),
    Schema.isMaxLength(2),
  ),
  replayReceiptRef: ReceiptRef,
  staleGenerationReceiptRef: ReceiptRef,
  sourceCustodyDeletionReceiptRef: ReceiptRef,
  failbackCustodyDeletionReceiptRef: ReceiptRef,
  acceptedWorkRefCount: Schema.Literal(0),
  executorResumed: Schema.Literal(false),
  executorResumptionOmissionRef: ReceiptRef,
});

export const Ide13PackagedOwnerLocalJourneyReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal(
    "openagents.desktop.ide-portable-packaged-owner-local-composite.v1",
  ),
  issue: Schema.Literal("IDE-13"),
  evidenceClass: Schema.Literal("real_local"),
  proofClass: Schema.Literal("packaged_shell_concurrent_owner_local_target"),
  candidateCommitSha: Sha40,
  capturedAt: Schema.String,
  physicalHost: Schema.Struct({
    operatingSystem: Schema.Literal("darwin"),
    architecture: Schema.Literal("arm64"),
    targetTopology: Schema.Literal("one_physical_host_two_logical_owner_local_targets"),
  }),
  packagedArtifact: Schema.Struct({
    ref: ArtifactRef,
    treeSha256: Sha256,
    files: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
    bytes: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
    portabilityBoundaryPresent: Schema.Literal(true),
  }),
  packagedShell: Schema.Struct({
    isolatedAppProof: Schema.Literal(true),
    signedOutLocalOnly: Schema.Literal(true),
    phaseBefore: Schema.Literal("unavailable"),
    phaseAfter: Schema.Literal("unavailable"),
    invalidCommandRefused: Schema.Literal(true),
    liveBeforeOwnerLocalJourney: Schema.Literal(true),
    liveAfterOwnerLocalJourney: Schema.Literal(true),
    authenticatedSyncClaimed: Schema.Literal(false),
    initiatedMoveClaimed: Schema.Literal(false),
  }),
  ownerLocalJourney: OwnerLocalJourneyProjectionSchema,
  concurrency: Schema.Struct({
    packagedApplicationAliveAtOwnerLocalStart: Schema.Literal(true),
    packagedApplicationAliveAtOwnerLocalCompletion: Schema.Literal(true),
    rendererExternalDestinationCount: Schema.Literal(0),
  }),
  teardown: Schema.Struct({
    applicationPidCaptured: Schema.Literal(true),
    survivingProcessCount: Schema.Literal(0),
    temporaryRootsRemoved: Schema.Literal(true),
    termination: Schema.Literals(["sigterm", "sigkill_fallback"]),
  }),
  security: Schema.Struct({
    privateMaterialIncluded: Schema.Literal(false),
    credentialShapeIncluded: Schema.Literal(false),
    absoluteOwnerPathIncluded: Schema.Literal(false),
  }),
  screenshotRef: PublicRef,
  traceRef: PublicRef,
  passed: Schema.Literal(true),
  limitations: Schema.Array(BoundedText).check(Schema.isMinLength(4), Schema.isMaxLength(8)),
});

export const Ide13PackagedOwnerLocalJourneyTraceSchema = Schema.Struct({
  schemaVersion: Schema.Literal(
    "openagents.desktop.ide-portable-packaged-owner-local-composite-trace.v1",
  ),
  issue: Schema.Literal("IDE-13"),
  candidateCommitSha: Sha40,
  artifactTreeSha256: Sha256,
  events: Schema.Array(
    Schema.Struct({
      kind: Schema.String.check(Schema.isMaxLength(80)),
      message: Schema.String.check(Schema.isMaxLength(500)),
    }),
  ).check(Schema.isMaxLength(100)),
  privateMaterialIncluded: Schema.Literal(false),
  authenticatedSyncClaimed: Schema.Literal(false),
  packagedShellInitiatedMoveClaimed: Schema.Literal(false),
});

const git = (...args: ReadonlyArray<string>): string =>
  execFileSync("git", [...args], { cwd: repositoryRoot, encoding: "utf8" }).trim();

const waitFor = async (
  predicate: () => boolean,
  failure: string,
  timeoutMs = 30_000,
): Promise<void> => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(failure);
};

const waitForRenderer = async (browser: Browser): Promise<Page> => {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const page = browser
      .contexts()
      .flatMap((context) => context.pages())
      .find((candidate) => candidate.url().startsWith("openagents-app://renderer/"));
    if (page !== undefined) return page;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error("IDE-13 packaged owner-local renderer did not appear");
};

const processTable = (): ReadonlyArray<Readonly<{ pid: number; parentPid: number }>> =>
  execFileSync("ps", ["-axo", "pid=,ppid="], { encoding: "utf8" })
    .split("\n")
    .map((line) => line.trim().split(/\s+/u))
    .filter((parts) => parts.length === 2)
    .map((parts) => ({
      pid: Number.parseInt(parts[0] ?? "", 10),
      parentPid: Number.parseInt(parts[1] ?? "", 10),
    }))
    .filter((value) => Number.isSafeInteger(value.pid) && Number.isSafeInteger(value.parentPid));

const descendantPids = (rootPid: number): ReadonlyArray<number> => {
  const rows = processTable();
  const descendants = new Set<number>();
  let frontier = [rootPid];
  while (frontier.length > 0) {
    const parents = new Set(frontier);
    frontier = rows
      .filter((row) => parents.has(row.parentPid) && !descendants.has(row.pid))
      .map((row) => row.pid);
    for (const pid of frontier) descendants.add(pid);
  }
  return [...descendants];
};

const isRunning = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

const destinationFor = (value: string): string | null => {
  try {
    const url = new URL(value);
    if (!["http:", "https:", "ws:", "wss:"].includes(url.protocol)) return null;
    if (["127.0.0.1", "localhost", "::1"].includes(url.hostname)) return null;
    return `${url.protocol}//${url.host}`;
  } catch {
    return null;
  }
};

const assertCandidate = (candidateCommitSha: string, headCommitSha: string): void => {
  if (!/^[a-f0-9]{40}$/u.test(candidateCommitSha)) {
    throw new Error("IDE-13 packaged owner-local candidate commit is invalid");
  }
  git("merge-base", "--is-ancestor", candidateCommitSha, headCommitSha);
  const laterPaths = git("diff", "--name-only", candidateCommitSha, headCommitSha)
    .split("\n")
    .filter((value) => value.length > 0);
  if (laterPaths.some((value) => !allowedEvidencePaths.has(value))) {
    throw new Error("IDE-13 packaged owner-local candidate omits an implementation change");
  }
};

const runOwnerLocalCohort = (
  candidateCommitSha: string,
  outputPath: string,
): typeof OwnerLocalJourneyProjectionSchema.Type => {
  const candidateWorktreeParent = mkdtempSync(
    path.join(tmpdir(), "openagents-ide13-packaged-candidate-"),
  );
  const candidateWorktree = path.join(candidateWorktreeParent, "repository");
  const moduleUrl = pathToFileURL(
    path.join(repositoryRoot, "apps/pylon/scripts/ide13-owner-local-real-cohort.ts"),
  ).href;
  const program = [
    `import { runIde13OwnerLocalRealCohort } from ${JSON.stringify(moduleUrl)};`,
    "await runIde13OwnerLocalRealCohort({",
    "candidateCommitSha: process.env.OPENAGENTS_IDE13_PACKAGED_CANDIDATE,",
    "outputPath: process.env.OPENAGENTS_IDE13_PACKAGED_COHORT_OUTPUT,",
    "repositoryRoot: process.env.OPENAGENTS_IDE13_PACKAGED_REPOSITORY_ROOT,",
    "});",
    "process.exit(0);",
  ].join("\n");
  let worktreeAdded = false;
  try {
    execFileSync("git", ["worktree", "add", "--detach", candidateWorktree, candidateCommitSha], {
      cwd: repositoryRoot,
      stdio: ["ignore", "ignore", "pipe"],
    });
    worktreeAdded = true;
    execFileSync(process.execPath, ["--import", "tsx", "--input-type=module", "--eval", program], {
      cwd: repositoryRoot,
      env: {
        ...process.env,
        OPENAGENTS_IDE13_PACKAGED_CANDIDATE: candidateCommitSha,
        OPENAGENTS_IDE13_PACKAGED_COHORT_OUTPUT: outputPath,
        OPENAGENTS_IDE13_PACKAGED_REPOSITORY_ROOT: candidateWorktree,
      },
      stdio: ["ignore", "ignore", "pipe"],
      timeout: 60_000,
    });
  } finally {
    if (worktreeAdded) {
      execFileSync("git", ["worktree", "remove", "--force", candidateWorktree], {
        cwd: repositoryRoot,
        stdio: ["ignore", "ignore", "pipe"],
      });
    }
    rmSync(candidateWorktreeParent, { recursive: true, force: true });
  }
  const source = Schema.decodeUnknownSync(OwnerLocalCohortSourceSchema)(
    JSON.parse(readFileSync(outputPath, "utf8")),
  );
  const phases = new Map(source.cohort.phaseReceipts.map((phase) => [phase.phase, phase]));
  const exact = (
    phase: (typeof source.cohort.phaseReceipts)[number]["phase"],
    generation: number,
  ) => {
    const receipt = phases.get(phase);
    if (receipt === undefined || receipt.attachmentGeneration !== generation) {
      throw new Error(`IDE-13 owner-local ${phase} generation is invalid`);
    }
    return receipt.receiptRef;
  };
  const readyHelpers = source.helpers
    .filter((helper) => helper.readiness === "ready")
    .map((helper) => helper.kind);
  const unsupportedHelpers = source.helpers
    .filter((helper) => helper.readiness === "unsupported")
    .map((helper) => helper.kind);
  return Schema.decodeUnknownSync(OwnerLocalJourneyProjectionSchema)({
    cohortRef: source.cohort.cohortRef,
    targetClass: source.cohort.targetClass,
    evidenceClass: source.cohort.evidenceClass,
    journeyScope: source.cohort.journeyScope,
    candidateCommitSha: source.cohort.candidateCommitSha,
    targetRef: source.cohort.targetRef,
    adapterKind: source.cohort.adapter.kind,
    adapterRef: source.cohort.adapter.ref,
    move: {
      sourceGeneration: 1,
      destinationGeneration: 2,
      attachReceiptRef: exact("attach", 2),
      helperReadinessReceiptRef: exact("helper_readiness", 2),
    },
    failback: {
      sourceGeneration: 2,
      destinationGeneration: 3,
      receiptRef: exact("failback", 3),
    },
    teardownReceiptRef: exact("teardown", 3),
    readyHelpers,
    unsupportedHelpers,
    replayReceiptRef: source.proofs.replayReceiptRef,
    staleGenerationReceiptRef: source.proofs.staleGenerationReceiptRef,
    sourceCustodyDeletionReceiptRef: source.proofs.sourceCustodyDeletionReceiptRef,
    failbackCustodyDeletionReceiptRef: source.proofs.failbackCustodyDeletionReceiptRef,
    acceptedWorkRefCount: source.execution.acceptedWorkRefCount,
    executorResumed: source.execution.executorResumed,
    executorResumptionOmissionRef: source.execution.omissionRef,
  });
};

const main = async (): Promise<void> => {
  if (process.platform !== "darwin" || process.arch !== "arm64") {
    throw new Error("IDE-13 packaged owner-local journey requires macOS arm64");
  }
  const headCommitSha = git("rev-parse", "HEAD");
  const candidateCommitSha = process.env.OPENAGENTS_IDE13_CANDIDATE_COMMIT_SHA ?? headCommitSha;
  assertCandidate(candidateCommitSha, headCommitSha);

  const appPath = resolvePackagedApp();
  const artifact = packagedArtifactTreeDigest(appPath);
  const artifactRef = path.relative(repositoryRoot, appPath).split(path.sep).join("/");
  const asarPath = path.join(appPath, "Contents", "Resources", "app.asar");
  const asarBytes = readFileSync(asarPath);
  const portabilityBoundaryPresent = [
    "desktop:ide-portable-snapshot",
    "desktop:ide-portable-command",
  ].every((marker) => asarBytes.includes(Buffer.from(marker)));
  if (!portabilityBoundaryPresent) {
    throw new Error("IDE-13 packaged artifact omits the portable Desktop boundary");
  }

  const workspace = mkdtempSync(
    path.join(tmpdir(), "openagents-ide13-packaged-owner-local-workspace-"),
  );
  const profile = mkdtempSync(
    path.join(tmpdir(), "openagents-ide13-packaged-owner-local-profile-"),
  );
  const cohortOutputPath = path.join(workspace, "owner-local-cohort.json");
  writeFileSync(
    path.join(workspace, "portable-proof.txt"),
    "packaged owner-local composite proof\n",
    {
      mode: 0o600,
    },
  );
  const sourcePath = path.join(workspace, "portable-proof.txt");
  const events: Array<{ kind: string; message: string }> = [];
  const observedExternalDestinations = new Set<string>();
  const forbiddenCredential =
    /(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_-]{8,}|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*/gu;
  const home = process.env.HOME ?? "__no_home__";
  const sanitize = (value: string): string =>
    value
      .replaceAll(workspace, "«workspace»")
      .replaceAll(profile, "«profile»")
      .replaceAll(home, "«home»")
      .replace(forbiddenCredential, "«redacted»")
      .slice(0, 500);

  const appProcess = spawn(
    "open",
    ["-n", "-W", "-a", appPath, sourcePath, "--args", "--remote-debugging-port=0"],
    {
      cwd: workspace,
      env: {
        ...process.env,
        OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
        OPENAGENTS_DESKTOP_USER_DATA: profile,
        OPENAGENTS_DESKTOP_LAUNCH_CWD: workspace,
        OA_DESKTOP_SKIP_DEV_VOICE_HELPER: "1",
      },
      stdio: "ignore",
    },
  );

  let browser: Browser | null = null;
  let applicationPid: number | null = null;
  let processIds: ReadonlyArray<number> = [];
  let termination: "sigterm" | "sigkill_fallback" = "sigterm";
  let receiptInput: Omit<
    typeof Ide13PackagedOwnerLocalJourneyReceiptSchema.Type,
    "teardown"
  > | null = null;
  let traceInput: typeof Ide13PackagedOwnerLocalJourneyTraceSchema.Type | null = null;
  try {
    const portPath = path.join(profile, "DevToolsActivePort");
    await waitFor(
      () => existsSync(portPath),
      "IDE-13 packaged owner-local DevTools port did not appear",
    );
    const port = readFileSync(portPath, "utf8").split("\n")[0] ?? "";
    const pidText =
      execFileSync("lsof", ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"], {
        encoding: "utf8",
      })
        .trim()
        .split("\n")[0] ?? "";
    applicationPid = Number.parseInt(pidText, 10);
    if (!Number.isSafeInteger(applicationPid)) {
      throw new Error("IDE-13 packaged owner-local application PID is unavailable");
    }
    processIds = [applicationPid, ...descendantPids(applicationPid)];

    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    const page = await waitForRenderer(browser);
    page.on("console", (message) => {
      events.push({ kind: `console:${message.type()}`, message: sanitize(message.text()) });
    });
    page.on("pageerror", (error) => {
      events.push({ kind: "pageerror", message: sanitize(error.message) });
    });
    page.on("request", (request) => {
      const destination = destinationFor(request.url());
      if (destination !== null) observedExternalDestinations.add(destination);
    });
    const surface = page.getByLabel("Portable coding placement");
    await surface.waitFor({ state: "visible", timeout: 30_000 });
    const readBridge = async () =>
      page.evaluate(async () => {
        const api = (
          globalThis as unknown as Readonly<{
            openagentsDesktop?: Readonly<{
              idePortability?: Readonly<{
                snapshot?: () => Promise<unknown>;
                command?: (value: unknown) => Promise<unknown>;
              }>;
            }>;
          }>
        ).openagentsDesktop?.idePortability;
        return {
          snapshot: await api?.snapshot?.(),
          invalidCommand: await api?.command?.({ invalid: true }),
          resourceUrls: performance.getEntriesByType("resource").map((entry) => entry.name),
        };
      });

    const before = await readBridge();
    const snapshotBefore = Schema.decodeUnknownSync(IdePortableClientSnapshotSchema)(
      before.snapshot,
    );
    const invalidCommand = Schema.decodeUnknownSync(IdePortableClientCommandResultSchema)(
      before.invalidCommand,
    );
    if (
      snapshotBefore.status.phase !== "unavailable" ||
      invalidCommand._tag !== "Refused" ||
      invalidCommand.reason !== "invalid_input"
    ) {
      throw new Error("IDE-13 isolated packaged shell did not remain fail-closed");
    }

    const aliveAtStart = isRunning(applicationPid);
    const cohort = runOwnerLocalCohort(candidateCommitSha, cohortOutputPath);
    const aliveAtCompletion = isRunning(applicationPid);
    const after = await readBridge();
    const snapshotAfter = Schema.decodeUnknownSync(IdePortableClientSnapshotSchema)(after.snapshot);
    for (const resourceUrl of [...before.resourceUrls, ...after.resourceUrls]) {
      const destination = destinationFor(resourceUrl);
      if (destination !== null) observedExternalDestinations.add(destination);
    }
    if (!aliveAtStart || !aliveAtCompletion || snapshotAfter.status.phase !== "unavailable") {
      throw new Error(
        "IDE-13 packaged shell did not remain live and fail-closed through the owner-local journey",
      );
    }
    if (observedExternalDestinations.size !== 0) {
      throw new Error(
        "IDE-13 packaged owner-local renderer used an undeclared external destination",
      );
    }
    const errors = events.filter(
      (event) => event.kind === "console:error" || event.kind === "pageerror",
    );
    if (errors.length !== 0) {
      throw new Error(
        `IDE-13 packaged owner-local renderer diagnostics failed: ${JSON.stringify(errors)}`,
      );
    }
    await surface.screenshot({ path: screenshotPath });

    receiptInput = {
      schemaVersion: "openagents.desktop.ide-portable-packaged-owner-local-composite.v1",
      issue: "IDE-13",
      evidenceClass: "real_local",
      proofClass: "packaged_shell_concurrent_owner_local_target",
      candidateCommitSha,
      capturedAt: new Date().toISOString(),
      physicalHost: {
        operatingSystem: "darwin",
        architecture: "arm64",
        targetTopology: "one_physical_host_two_logical_owner_local_targets",
      },
      packagedArtifact: {
        ref: artifactRef,
        treeSha256: artifact.sha256,
        files: artifact.files,
        bytes: artifact.bytes,
        portabilityBoundaryPresent: true,
      },
      packagedShell: {
        isolatedAppProof: true,
        signedOutLocalOnly: true,
        phaseBefore: "unavailable",
        phaseAfter: "unavailable",
        invalidCommandRefused: true,
        liveBeforeOwnerLocalJourney: true,
        liveAfterOwnerLocalJourney: true,
        authenticatedSyncClaimed: false,
        initiatedMoveClaimed: false,
      },
      ownerLocalJourney: cohort,
      concurrency: {
        packagedApplicationAliveAtOwnerLocalStart: true,
        packagedApplicationAliveAtOwnerLocalCompletion: true,
        rendererExternalDestinationCount: 0,
      },
      security: {
        privateMaterialIncluded: false,
        credentialShapeIncluded: false,
        absoluteOwnerPathIncluded: false,
      },
      screenshotRef,
      traceRef,
      passed: true,
      limitations: [
        "The packaged application ran with isolated app proof and had no authenticated Sync authority.",
        "The packaged shell did not initiate or authenticate the owner-local move.",
        "The production Pylon target ran on the same physical Mac with two logical owner-local targets. No cross-device target ran.",
        "The destination activated helpers after the move, but no Codex executor resumed and no work ref was accepted.",
        "DAP and native helpers stayed unsupported because no signed executable profile was admitted.",
        "Renderer network observation started after CDP attachment. This receipt does not claim global main-process packet capture.",
      ],
    };
    traceInput = {
      schemaVersion: "openagents.desktop.ide-portable-packaged-owner-local-composite-trace.v1",
      issue: "IDE-13",
      candidateCommitSha,
      artifactTreeSha256: artifact.sha256,
      events: [
        {
          kind: "packaged_shell_ready",
          message: "The isolated packaged shell exposed the fail-closed portable boundary.",
        },
        {
          kind: "owner_local_move",
          message:
            "The production owner-local target completed generation 1 to 2 on the same physical host.",
        },
        {
          kind: "owner_local_failback",
          message:
            "The production owner-local target completed generation 2 to 3 failback and teardown.",
        },
        {
          kind: "packaged_shell_still_live",
          message:
            "The isolated packaged shell stayed live and unavailable after the owner-local journey.",
        },
      ],
      privateMaterialIncluded: false,
      authenticatedSyncClaimed: false,
      packagedShellInitiatedMoveClaimed: false,
    };
  } finally {
    await browser?.close().catch(() => undefined);
    if (applicationPid !== null && isRunning(applicationPid)) {
      try {
        process.kill(applicationPid, "SIGTERM");
      } catch {
        // The process ended between observation and signal.
      }
    }
    if (applicationPid !== null) {
      await waitFor(
        () => !processIds.some(isRunning),
        "IDE-13 packaged owner-local application did not stop after SIGTERM",
        10_000,
      ).catch(async () => {
        termination = "sigkill_fallback";
        for (const pid of [...processIds].reverse()) {
          if (!isRunning(pid)) continue;
          try {
            process.kill(pid, "SIGKILL");
          } catch {
            // The process ended between observation and signal.
          }
        }
        await waitFor(
          () => !processIds.some(isRunning),
          "IDE-13 packaged owner-local application processes survived SIGKILL",
          5_000,
        );
      });
    }
    appProcess.kill("SIGTERM");
    await Promise.race([
      new Promise<void>((resolve) => appProcess.once("exit", () => resolve())),
      new Promise<void>((resolve) => setTimeout(resolve, 5_000)),
    ]);
    if (appProcess.exitCode === null) appProcess.kill("SIGKILL");
    rmSync(workspace, { recursive: true, force: true, maxRetries: 8, retryDelay: 100 });
    rmSync(profile, { recursive: true, force: true, maxRetries: 8, retryDelay: 100 });
  }

  if (applicationPid === null || receiptInput === null || traceInput === null) {
    throw new Error("IDE-13 packaged owner-local journey did not complete");
  }
  const receipt = Schema.decodeUnknownSync(Ide13PackagedOwnerLocalJourneyReceiptSchema)({
    ...receiptInput,
    teardown: {
      applicationPidCaptured: true,
      survivingProcessCount: processIds.filter(isRunning).length,
      temporaryRootsRemoved: !existsSync(workspace) && !existsSync(profile),
      termination,
    },
  });
  const trace = Schema.decodeUnknownSync(Ide13PackagedOwnerLocalJourneyTraceSchema)(traceInput);
  const serialized = `${JSON.stringify(receipt)}\n${JSON.stringify(trace)}`;
  forbiddenCredential.lastIndex = 0;
  if (
    serialized.includes(workspace) ||
    serialized.includes(profile) ||
    serialized.includes(home) ||
    forbiddenCredential.test(serialized)
  ) {
    throw new Error("IDE-13 packaged owner-local evidence contains private material");
  }
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 });
  writeFileSync(tracePath, `${JSON.stringify(trace, null, 2)}\n`, { mode: 0o600 });
  const digest = createHash("sha256").update(serialized).digest("hex");
  process.stdout.write(
    `[openagents-desktop] IDE-13 packaged owner-local composite evidence ${digest}: ${receiptPath}\n`,
  );
};

if (import.meta.main) await main();

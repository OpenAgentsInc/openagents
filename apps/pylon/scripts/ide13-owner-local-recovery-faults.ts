import { Runtime } from "@openagentsinc/runtime-platform";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

import type {
  PortableOwnerLocalCapabilityOperationRecord,
  PortableOwnerLocalCapabilityOperationResultRequest,
} from "@openagentsinc/portable-session-contract";
import { Schema } from "effect";

import { runIde13OwnerLocalRealCohort } from "./ide13-owner-local-real-cohort.js";
import { makePylonPortableOwnerLocalCapabilityOperationJournal } from "../src/portable-owner-local-capability-operation-journal.js";
import {
  PylonPortableOwnerLocalCapabilityRecoveryError,
  PylonPortableOwnerLocalCapabilityWorker,
} from "../src/portable-owner-local-capability-operation-worker.js";

const GIT_SHA = /^[a-f0-9]{40}$/u;
const RECEIPT_REPOSITORY_PATH =
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-recovery-faults.json";
const DEADLINE_MILLISECONDS = 120_000;
const REF = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);

const RecoveryCaseSchema = Schema.Struct({
  scenario: Schema.Literals([
    "coordinator_crash",
    "checkpoint_store_crash",
    "duplicate_event",
    "lease_expiry_clock_skew",
  ]),
  evidenceClass: Schema.Literal("real_local"),
  outcome: Schema.Literal("passed"),
  productionBoundaryRef: REF,
  injectedFaultRef: REF,
  recoveryPointRef: REF,
  receiptRef: REF,
  elapsedMilliseconds: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  deadlineMilliseconds: Schema.Literal(DEADLINE_MILLISECONDS),
  disclosure: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
});

export const Ide13OwnerLocalRecoveryFaultReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-portable-owner-local-recovery-faults.v1"),
  generatedAt: Schema.String,
  candidateCommitSha: Schema.String.check(Schema.isPattern(GIT_SHA)),
  baseCommitSha: Schema.String.check(Schema.isPattern(GIT_SHA)),
  cohortRef: Schema.Literal("cohort.ide13.owner-local.real.1"),
  cases: Schema.Array(RecoveryCaseSchema).check(Schema.isMinLength(4), Schema.isMaxLength(4)),
  safety: Schema.Struct({
    completionCount: Schema.Literal(2),
    duplicateExecutionObserved: Schema.Literal(false),
    expiredClaimAccepted: Schema.Literal(false),
    checkpointCiphertextResidueCount: Schema.Literal(0),
    journalEntryResidueCount: Schema.Literal(0),
    forbiddenMaterialProjected: Schema.Literal(false),
  }),
  unsupported: Schema.Array(
    Schema.Struct({
      scenario: Schema.Literals(["reordered_event", "cancellation_and_app_restart"]),
      evidenceClass: Schema.Literal("not_run"),
      reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
    }),
  ).check(Schema.isMinLength(2), Schema.isMaxLength(2)),
});

export interface Ide13OwnerLocalRecoveryFaultReceipt extends Schema.Schema.Type<
  typeof Ide13OwnerLocalRecoveryFaultReceiptSchema
> {}

const decodeReceipt = Schema.decodeUnknownSync(Ide13OwnerLocalRecoveryFaultReceiptSchema);

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

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`;

const pylonRef = "pylon.ide13.recovery";
const targetRef = "target.ide13.recovery";
const workerInstanceRef = "worker.ide13.recovery";
const operationRef = "operation.ide13.recovery.install";
const baseNow = new Date("2026-07-20T12:00:00.000Z");

const pendingRecord = (
  expiresAt = "2026-07-20T12:10:00.000Z",
): PortableOwnerLocalCapabilityOperationRecord => ({
  request: {
    schema: "openagents.portable_owner_local_capability_operation.v1",
    operationRef,
    action: "install",
    capability: "provider",
    commandExecutionClaimRef: "claim.ide13.recovery.command",
    ownerRef: "owner.ide13.recovery",
    pylonRef,
    sessionRef: "session.ide13.recovery",
    attachmentRef: "attachment.ide13.recovery",
    attachmentGeneration: 1,
    targetRef,
    sourceLeaseRef: "lease.ide13.recovery.source",
    sourceGrantRef: "grant.ide13.recovery.source",
    destinationLeaseRef: "lease.ide13.recovery.destination",
    destinationGrantRef: "grant.ide13.recovery.destination",
    installationRef: null,
    permissionRefs: ["permission.ide13.recovery"],
    permissionFingerprint: `sha256:${"2".repeat(64)}`,
    expiresAt,
  },
  requestFingerprint: `sha256:${"1".repeat(64)}`,
  state: "pending",
  claimRef: null,
  claimFingerprint: null,
  workerInstanceRef: null,
  claimGeneration: null,
  leaseRevision: null,
  claimedAt: null,
  leaseExpiresAt: null,
  resultRef: null,
  resultFingerprint: null,
  resultStatus: null,
  resultInstallationRef: null,
  receiptRef: null,
  resultEvidenceRefs: [],
  errorRef: null,
  completedAt: null,
  updatedAt: baseNow.toISOString(),
});

type ServerFixture = ReturnType<typeof makeServerFixture>;

const makeServerFixture = (duplicates = 1) => {
  let server = pendingRecord();
  let pending = true;
  let completionCount = 0;
  let executionCount = 0;
  const client = {
    pending: async () => (pending ? Array.from({ length: duplicates }, () => server) : []),
    read: async () => server,
    claim: async (
      request: Parameters<
        ConstructorParameters<typeof PylonPortableOwnerLocalCapabilityWorker>[0]["client"]["claim"]
      >[0],
    ) => {
      server = {
        ...server,
        state: "claimed",
        claimRef: request.claimRef,
        workerInstanceRef,
        claimGeneration: 1,
        leaseRevision: 1,
        claimedAt: baseNow.toISOString(),
        leaseExpiresAt: request.leaseExpiresAt,
      };
      return { operation: server, status: "claimed" as const };
    },
    renew: async (
      request: Parameters<
        ConstructorParameters<typeof PylonPortableOwnerLocalCapabilityWorker>[0]["client"]["renew"]
      >[0],
    ) => {
      server = {
        ...server,
        leaseRevision: request.expectedLeaseRevision + 1,
        leaseExpiresAt: request.leaseExpiresAt,
      };
      return { operation: server, status: "renewed" as const };
    },
    complete: async (request: PortableOwnerLocalCapabilityOperationResultRequest) => {
      completionCount += 1;
      pending = false;
      server = {
        ...server,
        state: request.resultStatus,
        leaseRevision: request.expectedLeaseRevision + 1,
        resultRef: request.resultRef,
        resultStatus: request.resultStatus,
        resultInstallationRef: request.resultInstallationRef,
        receiptRef: request.receiptRef,
        resultEvidenceRefs: [...request.evidenceRefs],
        errorRef: request.errorRef,
        completedAt: request.completedAt,
      };
      return { operation: server, status: request.resultStatus };
    },
  };
  return {
    client,
    executionCount: () => executionCount,
    completionCount: () => completionCount,
    executor: {
      recoverySemantics: async () => "operation_ref_idempotent" as const,
      execute: async () => {
        executionCount += 1;
        return {
          outcome: {
            status: "completed" as const,
            resultInstallationRef: "installation.ide13.recovery",
            executableProfileRef: undefined,
            receiptRef: "receipt.ide13.recovery.install",
            evidenceRefs: ["evidence.ide13.recovery.install"],
            errorRef: null,
          },
        };
      },
    },
  };
};

const workerFor = (
  fixture: ServerFixture,
  journalDirectory: string,
  faultInjector?: (step: "claim_durable", operation: string) => Promise<void> | void,
) =>
  new PylonPortableOwnerLocalCapabilityWorker({
    pylonRef,
    targetRef,
    workerInstanceRef,
    now: () => baseNow,
    client: fixture.client,
    executor: fixture.executor,
    journal: makePylonPortableOwnerLocalCapabilityOperationJournal({
      directory: journalDirectory,
      pylonRef,
      targetRef,
      workerInstanceRef,
    }),
    ...(faultInjector === undefined ? {} : { faultInjector }),
  });

const runWorkerRecoveryCases = async (root: string) => {
  const crashJournal = join(root, "coordinator-journal");
  const crashFixture = makeServerFixture();
  let injected = false;
  await workerFor(crashFixture, crashJournal, (step, operation) => {
    if (step === "claim_durable" && operation === operationRef && !injected) {
      injected = true;
      throw new Error("injected coordinator crash");
    }
  })
    .runPass()
    .then(
      () => {
        throw new Error("coordinator crash injection did not stop the worker");
      },
      (error: unknown) => {
        if (!(error instanceof Error) || error.message !== "injected coordinator crash") {
          throw error;
        }
      },
    );
  if (!injected) throw new Error("coordinator crash boundary was not reached");
  await workerFor(crashFixture, crashJournal).runPass();
  if (crashFixture.executionCount() !== 1 || crashFixture.completionCount() !== 1) {
    throw new Error("coordinator restart did not complete exactly once");
  }

  const duplicateJournal = join(root, "duplicate-journal");
  const duplicateFixture = makeServerFixture(2);
  await workerFor(duplicateFixture, duplicateJournal).runPass();
  if (duplicateFixture.executionCount() !== 1 || duplicateFixture.completionCount() !== 1) {
    throw new Error("duplicate pending event caused duplicate execution");
  }

  const expiredJournal = join(root, "expired-journal");
  const expiredFixture = makeServerFixture();
  const expiredRecord = pendingRecord("2026-07-20T11:59:59.000Z");
  let claimAttempted = false;
  const expiredWorker = new PylonPortableOwnerLocalCapabilityWorker({
    pylonRef,
    targetRef,
    workerInstanceRef,
    now: () => baseNow,
    client: {
      ...expiredFixture.client,
      pending: async () => [expiredRecord],
      claim: async () => {
        claimAttempted = true;
        throw new Error("expired claim reached the server");
      },
    },
    executor: expiredFixture.executor,
    journal: makePylonPortableOwnerLocalCapabilityOperationJournal({
      directory: expiredJournal,
      pylonRef,
      targetRef,
      workerInstanceRef,
    }),
  });
  await expiredWorker.runPass().then(
    () => {
      throw new Error("expired lease was accepted");
    },
    (error: unknown) => {
      if (
        !(error instanceof PylonPortableOwnerLocalCapabilityRecoveryError) ||
        error.reason !== "claim_expired"
      ) {
        throw error;
      }
    },
  );
  if (claimAttempted) throw new Error("expired lease crossed the claim boundary");

  const journalResidue = (
    await Promise.all(
      [crashJournal, duplicateJournal, expiredJournal].map(async (directory) => {
        const journal = makePylonPortableOwnerLocalCapabilityOperationJournal({
          directory,
          pylonRef,
          targetRef,
          workerInstanceRef,
        });
        return (await journal.entries()).length;
      }),
    )
  ).reduce((sum, count) => sum + count, 0);
  return {
    completionCount: crashFixture.completionCount() + duplicateFixture.completionCount(),
    duplicateExecutionObserved: duplicateFixture.executionCount() !== 1,
    expiredClaimAccepted: claimAttempted,
    journalEntryResidueCount: journalResidue,
  } as const;
};

export const runIde13OwnerLocalRecoveryFaults = async (
  input: Readonly<{
    candidateCommitSha?: string;
    outputPath?: string;
    repositoryRoot?: string;
  }> = {},
): Promise<Ide13OwnerLocalRecoveryFaultReceipt> => {
  const repositoryRoot = resolve(input.repositoryRoot ?? join(import.meta.dirname, "../../.."));
  const headCommitSha = await git(repositoryRoot, "rev-parse", "HEAD");
  const candidateCommitSha = input.candidateCommitSha ?? headCommitSha;
  if (!GIT_SHA.test(candidateCommitSha)) throw new Error("recovery candidate commit is invalid");
  await git(repositoryRoot, "merge-base", "--is-ancestor", candidateCommitSha, headCommitSha);
  const laterPaths = (
    await git(repositoryRoot, "diff", "--name-only", candidateCommitSha, headCommitSha)
  )
    .split("\n")
    .filter((path) => path.length > 0);
  if (laterPaths.some((path) => path !== RECEIPT_REPOSITORY_PATH)) {
    throw new Error("recovery candidate omits an implementation change");
  }
  const baseCommitSha = await git(repositoryRoot, "merge-base", candidateCommitSha, "origin/main");
  const root = await mkdtemp(join(tmpdir(), "openagents-ide13-recovery-faults-"));
  try {
    const workerStartedAt = performance.now();
    const workerSafety = await runWorkerRecoveryCases(root);
    const workerElapsed = performance.now() - workerStartedAt;
    const storeStartedAt = performance.now();
    const cohort = await runIde13OwnerLocalRealCohort({
      candidateCommitSha,
      injectedCheckpointStoreCrash: true,
      repositoryRoot,
    });
    const storeElapsed = performance.now() - storeStartedAt;
    const cleanup = cohort.cohort.metrics.find((metric) => metric.metric === "resource_cleanup");
    if (cleanup?.p99 !== 0) throw new Error("checkpoint store recovery left cohort residue");
    const cases = [
      {
        scenario: "coordinator_crash" as const,
        evidenceClass: "real_local" as const,
        outcome: "passed" as const,
        productionBoundaryRef: "boundary.pylon.owner-local.worker.claim-durable",
        injectedFaultRef: "injected-fault.ide13.owner-local.coordinator-crash",
        recoveryPointRef: "recovery.pylon.owner-local.file-journal.claimed",
        receiptRef: stableRef("receipt.ide13.owner-local.coordinator-crash", candidateCommitSha),
        elapsedMilliseconds: workerElapsed,
        deadlineMilliseconds: DEADLINE_MILLISECONDS,
        disclosure:
          "A source-controlled fault hook raised an exception after the production worker made its claimed lease durable. A new worker object recovered the file journal and completed the operation exactly once. This was not an external process death.",
      },
      {
        scenario: "checkpoint_store_crash" as const,
        evidenceClass: "real_local" as const,
        outcome: "passed" as const,
        productionBoundaryRef:
          "boundary.pylon.owner-local.checkpoint-custody.delete-object-removed",
        injectedFaultRef: "injected-fault.ide13.owner-local.checkpoint-store-crash",
        recoveryPointRef: cohort.proofs.sourceCustodyDeletionReceiptRef,
        receiptRef: stableRef(
          "receipt.ide13.owner-local.checkpoint-store-crash",
          `${candidateCommitSha}:${cohort.proofs.sourceCustodyDeletionReceiptRef}`,
        ),
        elapsedMilliseconds: storeElapsed,
        deadlineMilliseconds: DEADLINE_MILLISECONDS,
        disclosure:
          "A source-controlled fault hook raised an exception after the production custody store removed ciphertext and before closeout. A new store object resumed the durable delete intent and verified absence. This was not an external process death.",
      },
      {
        scenario: "duplicate_event" as const,
        evidenceClass: "real_local" as const,
        outcome: "passed" as const,
        productionBoundaryRef: "boundary.pylon.owner-local.worker.pending-batch",
        injectedFaultRef: "injected-fault.ide13.owner-local.duplicate-pending-operation",
        recoveryPointRef: "recovery.pylon.owner-local.operation-ref-deduplication",
        receiptRef: stableRef("receipt.ide13.owner-local.duplicate-event", candidateCommitSha),
        elapsedMilliseconds: workerElapsed,
        deadlineMilliseconds: DEADLINE_MILLISECONDS,
        disclosure:
          "The production worker received the same operation twice in one pending batch. Operation-ref admission executed and completed it once.",
      },
      {
        scenario: "lease_expiry_clock_skew" as const,
        evidenceClass: "real_local" as const,
        outcome: "passed" as const,
        productionBoundaryRef: "boundary.pylon.owner-local.worker.lease-expiry",
        injectedFaultRef: "injected-fault.ide13.owner-local.clock-after-request-expiry",
        recoveryPointRef: "recovery.pylon.owner-local.claim-expired-refusal",
        receiptRef: stableRef("receipt.ide13.owner-local.lease-expiry", candidateCommitSha),
        elapsedMilliseconds: workerElapsed,
        deadlineMilliseconds: DEADLINE_MILLISECONDS,
        disclosure:
          "The production worker clock was later than the request expiry. The worker refused the claim before a server mutation or executor call.",
      },
    ];
    if (cases.some((row) => row.elapsedMilliseconds > row.deadlineMilliseconds)) {
      throw new Error("owner-local recovery fault exceeded its deadline");
    }
    const serialized = JSON.stringify(cases);
    const receipt = decodeReceipt(
      {
        schemaVersion: "openagents.desktop.ide-portable-owner-local-recovery-faults.v1",
        generatedAt: new Date().toISOString(),
        candidateCommitSha,
        baseCommitSha,
        cohortRef: "cohort.ide13.owner-local.real.1",
        cases,
        safety: {
          ...workerSafety,
          checkpointCiphertextResidueCount: 0,
          forbiddenMaterialProjected:
            /\/Users\/|\/private\/tmp\/|Bearer|password|processId|pid/u.test(serialized),
        },
        unsupported: [
          {
            scenario: "reordered_event",
            evidenceClass: "not_run",
            reason:
              "No production event-sequence contract identifies a safe reordered owner-local operation stream. This case did not run.",
          },
          {
            scenario: "cancellation_and_app_restart",
            evidenceClass: "not_run",
            reason:
              "The coordinator restart probe does not prove cancellation plus packaged app restart. This combined case did not run.",
          },
        ],
      },
      { onExcessProperty: "error" },
    );
    if (input.outputPath !== undefined) {
      await mkdir(dirname(input.outputPath), { recursive: true });
      await writeFile(input.outputPath, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
    }
    return receipt;
  } finally {
    await rm(root, { recursive: true, force: true });
  }
};

if (import.meta.main) {
  const repositoryRoot = resolve(join(import.meta.dirname, "../../.."));
  const outputPath = resolve(repositoryRoot, RECEIPT_REPOSITORY_PATH);
  const candidateCommitSha = process.env.OPENAGENTS_IDE13_CANDIDATE_COMMIT_SHA;
  const receipt = await runIde13OwnerLocalRecoveryFaults({
    ...(candidateCommitSha === undefined ? {} : { candidateCommitSha }),
    outputPath,
    repositoryRoot,
  });
  process.stdout.write(
    `${JSON.stringify({ outputPath: RECEIPT_REPOSITORY_PATH, cases: receipt.cases.length })}\n`,
  );
  process.exit(0);
}

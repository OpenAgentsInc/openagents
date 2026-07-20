import { Runtime } from "@openagentsinc/runtime-platform";
import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

import { Schema } from "effect";

import {
  IDE_PORTABLE_FAULT_SCENARIOS,
  IDE_PORTABLE_REQUIRED_FAULT_CASES,
  IdePortableEvidenceClassSchema,
  IdePortablePhaseSchema,
} from "../../openagents-desktop/src/ide/portable-evidence-contract.ts";
import {
  type Ide13OwnerLocalAuthorityFaultProof,
  type Ide13OwnerLocalAuthorityFaultScenario,
  runIde13OwnerLocalRealCohort,
} from "./ide13-owner-local-real-cohort.js";

const GIT_SHA = /^[a-f0-9]{40}$/u;
const REF = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);
const RECEIPT_REPOSITORY_PATH =
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-real-fault-matrix.json";
const DEADLINE_MILLISECONDS = 120_000;

const FaultCaseSchema = Schema.Struct({
  faultRef: REF,
  scenario: Schema.Literals(IDE_PORTABLE_FAULT_SCENARIOS),
  phase: Schema.NullOr(IdePortablePhaseSchema),
  evidenceClass: IdePortableEvidenceClassSchema,
  outcome: Schema.Literals(["passed", "not_run"]),
  productionBoundaryRef: Schema.NullOr(REF),
  injectedFaultRef: Schema.NullOr(REF),
  recoveryPointRef: Schema.NullOr(REF),
  receiptRef: Schema.NullOr(REF),
  elapsedMilliseconds: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  deadlineMilliseconds: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  disclosure: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
});

export const Ide13OwnerLocalRealFaultMatrixReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-portable-owner-local-fault-matrix.v1"),
  evidenceContractVersion: Schema.Literal("openagents.desktop.ide-portable-evidence.v3"),
  generatedAt: Schema.String,
  candidateCommitSha: Schema.String.check(Schema.isPattern(GIT_SHA)),
  baseCommitSha: Schema.String.check(Schema.isPattern(GIT_SHA)),
  cohortRef: Schema.Literal("cohort.ide13.owner-local.real.1"),
  targetClass: Schema.Literal("owner_local"),
  adapterRef: Schema.Literal("adapter.pylon.owner-local.control-session.v1"),
  cases: Schema.Array(FaultCaseSchema).check(
    Schema.isMinLength(IDE_PORTABLE_REQUIRED_FAULT_CASES.length),
    Schema.isMaxLength(IDE_PORTABLE_REQUIRED_FAULT_CASES.length),
  ),
  summary: Schema.Struct({
    requiredCaseCount: Schema.Literal(IDE_PORTABLE_REQUIRED_FAULT_CASES.length),
    passedRealLocalCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
    notRunCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
    acceptanceReady: Schema.Literal(false),
  }),
  safety: Schema.Struct({
    secondWriterObserved: Schema.Literal(false),
    staleMutationAccepted: Schema.Literal(false),
    forbiddenMaterialProjected: Schema.Literal(false),
    orphanPtyCount: Schema.Literal(0),
    orphanLspCount: Schema.Literal(0),
    orphanWatcherCount: Schema.Literal(0),
    custodyObjectResidueCount: Schema.Literal(0),
    capabilityLeaseResidueCount: Schema.Literal(0),
    queueRowResidueCount: Schema.Literal(0),
    sqliteResidueCount: Schema.Literal(0),
    sessionResidueCount: Schema.Literal(0),
    proofReceiptRefs: Schema.Array(REF).check(Schema.isMinLength(1), Schema.isMaxLength(64)),
  }),
  omissions: Schema.Array(
    Schema.Struct({
      omissionRef: REF,
      scenario: FaultCaseSchema.fields.scenario,
      reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
    }),
  ).check(Schema.isMinLength(1), Schema.isMaxLength(64)),
});

export interface Ide13OwnerLocalRealFaultMatrixReceipt extends Schema.Schema.Type<
  typeof Ide13OwnerLocalRealFaultMatrixReceiptSchema
> {}

const decodeReceipt = Schema.decodeUnknownSync(Ide13OwnerLocalRealFaultMatrixReceiptSchema);

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

const identity = (fault: (typeof IDE_PORTABLE_REQUIRED_FAULT_CASES)[number]): string =>
  `${fault.scenario}:${fault.phase ?? "all"}`;

const omittedReason = (scenario: string): string =>
  `The production owner-local composition does not have a source-controlled ${scenario} injection seam. This case did not run and does not contribute acceptance evidence.`;

const AUTHORITY_FAULT_SCENARIOS = new Set<Ide13OwnerLocalAuthorityFaultScenario>([
  "old_generation_command",
  "dual_attachment_claim",
  "source_revocation_failure",
]);

export const runIde13OwnerLocalRealFaultMatrix = async (
  input: Readonly<{
    candidateCommitSha?: string;
    outputPath?: string;
    repositoryRoot?: string;
  }> = {},
): Promise<Ide13OwnerLocalRealFaultMatrixReceipt> => {
  const repositoryRoot = resolve(input.repositoryRoot ?? join(import.meta.dirname, "../../.."));
  const headCommitSha = await git(repositoryRoot, "rev-parse", "HEAD");
  const candidateCommitSha = input.candidateCommitSha ?? headCommitSha;
  if (!GIT_SHA.test(candidateCommitSha))
    throw new Error("fault matrix candidate commit is invalid");
  await git(repositoryRoot, "merge-base", "--is-ancestor", candidateCommitSha, headCommitSha);
  const laterPaths = (
    await git(repositoryRoot, "diff", "--name-only", candidateCommitSha, headCommitSha)
  )
    .split("\n")
    .filter((path) => path.length > 0);
  if (laterPaths.some((path) => path !== RECEIPT_REPOSITORY_PATH)) {
    throw new Error("fault matrix candidate omits an implementation change");
  }
  const baseCommitSha = await git(repositoryRoot, "merge-base", candidateCommitSha, "origin/main");

  const safetyProofRefs = new Set<string>();
  const cases = await Promise.all(
    IDE_PORTABLE_REQUIRED_FAULT_CASES.map(
      async (fault): Promise<Schema.Schema.Type<typeof FaultCaseSchema>> => {
        if (
          AUTHORITY_FAULT_SCENARIOS.has(fault.scenario as Ide13OwnerLocalAuthorityFaultScenario)
        ) {
          const scenario = fault.scenario as Ide13OwnerLocalAuthorityFaultScenario;
          const startedAt = performance.now();
          let faultProof: Ide13OwnerLocalAuthorityFaultProof | null = null;
          const cohort = await runIde13OwnerLocalRealCohort({
            candidateCommitSha,
            injectedAuthorityFaultScenario: scenario,
            onInjectedAuthorityFaultProof: (proof) => {
              faultProof = proof;
            },
            repositoryRoot,
          });
          const elapsedMilliseconds = performance.now() - startedAt;
          if (elapsedMilliseconds > DEADLINE_MILLISECONDS) {
            throw new Error(`owner-local ${identity(fault)} exceeded its cleanup deadline`);
          }
          if (faultProof === null || faultProof.scenario !== scenario) {
            throw new Error(`owner-local ${identity(fault)} lacks an exact authority fault proof`);
          }
          const resourceCleanup = cohort.cohort.metrics.find(
            (row) => row.metric === "resource_cleanup",
          );
          const queue = cohort.cohort.metrics.find((row) => row.metric === "queue");
          if (resourceCleanup?.p99 !== 0 || queue?.p99 !== 0) {
            throw new Error(`owner-local ${identity(fault)} left resource or session residue`);
          }
          safetyProofRefs.add(faultProof.receiptRef);
          safetyProofRefs.add(cohort.proofs.teardownReceiptRef);
          safetyProofRefs.add(cohort.proofs.sourceCustodyDeletionReceiptRef);
          safetyProofRefs.add(cohort.proofs.failbackCustodyDeletionReceiptRef);
          return {
            faultRef: `fault.ide13.owner-local.${scenario}`,
            scenario,
            phase: null,
            evidenceClass: "real_local",
            outcome: "passed",
            productionBoundaryRef: faultProof.productionBoundaryRef,
            injectedFaultRef: faultProof.injectedFaultRef,
            recoveryPointRef: faultProof.recoveryPointRef,
            receiptRef: faultProof.receiptRef,
            elapsedMilliseconds,
            deadlineMilliseconds: DEADLINE_MILLISECONDS,
            disclosure: faultProof.disclosure,
          };
        }
        if (fault.scenario !== "transition_partition" || fault.phase === null) {
          return {
            faultRef: `fault.ide13.owner-local.${fault.scenario}`,
            scenario: fault.scenario,
            phase: null,
            evidenceClass: "not_run",
            outcome: "not_run",
            productionBoundaryRef: null,
            injectedFaultRef: null,
            recoveryPointRef: null,
            receiptRef: null,
            elapsedMilliseconds: 0,
            deadlineMilliseconds: DEADLINE_MILLISECONDS,
            disclosure: omittedReason(fault.scenario),
          };
        }

        const startedAt = performance.now();
        const cohort = await runIde13OwnerLocalRealCohort({
          candidateCommitSha,
          injectedTransitionPartitionPhase: fault.phase,
          repositoryRoot,
        });
        const elapsedMilliseconds = performance.now() - startedAt;
        if (elapsedMilliseconds > DEADLINE_MILLISECONDS) {
          throw new Error(`owner-local ${identity(fault)} exceeded its cleanup deadline`);
        }
        const phaseReceipt = cohort.cohort.phaseReceipts.find((row) => row.phase === fault.phase);
        if (phaseReceipt?.receiptRef === null || phaseReceipt?.receiptRef === undefined) {
          throw new Error(`owner-local ${identity(fault)} lacks an exact phase receipt`);
        }
        const resourceCleanup = cohort.cohort.metrics.find(
          (row) => row.metric === "resource_cleanup",
        );
        const queue = cohort.cohort.metrics.find((row) => row.metric === "queue");
        if (resourceCleanup?.p99 !== 0 || queue?.p99 !== 0) {
          throw new Error(`owner-local ${identity(fault)} left resource or session residue`);
        }
        safetyProofRefs.add(cohort.proofs.staleGenerationReceiptRef);
        safetyProofRefs.add(cohort.proofs.teardownReceiptRef);
        safetyProofRefs.add(cohort.proofs.sourceCustodyDeletionReceiptRef);
        safetyProofRefs.add(cohort.proofs.failbackCustodyDeletionReceiptRef);
        const caseReceiptRef = stableRef(
          "receipt.ide13.owner-local.fault",
          `${candidateCommitSha}:${identity(fault)}:${phaseReceipt.receiptRef}:${cohort.proofs.teardownReceiptRef}`,
        );
        return {
          faultRef: `fault.ide13.owner-local.transition-partition.${fault.phase}`,
          scenario: fault.scenario,
          phase: fault.phase,
          evidenceClass: "real_local",
          outcome: "passed",
          productionBoundaryRef: `boundary.pylon.owner-local.${fault.phase}.dispatch`,
          injectedFaultRef: `injected-fault.ide13.owner-local.transition-partition.${fault.phase}`,
          recoveryPointRef: phaseReceipt.receiptRef,
          receiptRef: caseReceiptRef,
          elapsedMilliseconds,
          deadlineMilliseconds: DEADLINE_MILLISECONDS,
          disclosure:
            "The source-controlled harness injected one transient disconnect at the production owner-local phase dispatch boundary, retried the exact operation, completed the real local composition, and then verified cleanup. This was an injected fault, not an external outage.",
        };
      },
    ),
  );

  const expectedIdentities = IDE_PORTABLE_REQUIRED_FAULT_CASES.map(identity);
  const actualIdentities = cases.map(identity);
  if (
    actualIdentities.length !== expectedIdentities.length ||
    new Set(actualIdentities).size !== expectedIdentities.length ||
    expectedIdentities.some((value) => !actualIdentities.includes(value))
  ) {
    throw new Error("owner-local fault matrix is incomplete or duplicated");
  }
  const passedRealLocalCount = cases.filter(
    (fault) => fault.evidenceClass === "real_local" && fault.outcome === "passed",
  ).length;
  const notRun = cases.filter((fault) => fault.outcome === "not_run");
  const serializedCases = JSON.stringify(cases);
  const receipt = decodeReceipt(
    {
      schemaVersion: "openagents.desktop.ide-portable-owner-local-fault-matrix.v1",
      evidenceContractVersion: "openagents.desktop.ide-portable-evidence.v3",
      generatedAt: new Date().toISOString(),
      candidateCommitSha,
      baseCommitSha,
      cohortRef: "cohort.ide13.owner-local.real.1",
      targetClass: "owner_local",
      adapterRef: "adapter.pylon.owner-local.control-session.v1",
      cases,
      summary: {
        requiredCaseCount: IDE_PORTABLE_REQUIRED_FAULT_CASES.length,
        passedRealLocalCount,
        notRunCount: notRun.length,
        acceptanceReady: false,
      },
      safety: {
        secondWriterObserved: false,
        staleMutationAccepted: false,
        forbiddenMaterialProjected:
          /\/Users\/|\/private\/tmp\/|Bearer|password|processId|pid/u.test(serializedCases),
        orphanPtyCount: 0,
        orphanLspCount: 0,
        orphanWatcherCount: 0,
        custodyObjectResidueCount: 0,
        capabilityLeaseResidueCount: 0,
        queueRowResidueCount: 0,
        sqliteResidueCount: 0,
        sessionResidueCount: 0,
        proofReceiptRefs: [...safetyProofRefs].toSorted(),
      },
      omissions: notRun.map((fault) => ({
        omissionRef: `omission.ide13.owner-local.fault.${fault.scenario}`,
        scenario: fault.scenario,
        reason: omittedReason(fault.scenario),
      })),
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
  const outputPath = resolve(repositoryRoot, RECEIPT_REPOSITORY_PATH);
  const candidateCommitSha = process.env.OPENAGENTS_IDE13_CANDIDATE_COMMIT_SHA;
  const receipt = await runIde13OwnerLocalRealFaultMatrix({
    ...(candidateCommitSha === undefined ? {} : { candidateCommitSha }),
    outputPath,
    repositoryRoot,
  });
  process.stdout.write(
    `${JSON.stringify({
      outputPath: RECEIPT_REPOSITORY_PATH,
      passedRealLocalCount: receipt.summary.passedRealLocalCount,
      notRunCount: receipt.summary.notRunCount,
      acceptanceReady: receipt.summary.acceptanceReady,
    })}\n`,
  );
  process.exit(0);
}

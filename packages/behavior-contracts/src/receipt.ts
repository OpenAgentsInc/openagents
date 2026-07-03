import type {
  BehaviorContract,
  BehaviorContractRegistryDocument,
} from "./contract"
import type { BehaviorContractCoverageReport } from "./coverage"
import type { BehaviorContractRegistryValidation } from "./registry"

export const BehaviorContractReceiptSchemaVersion =
  "openagents.behavior_contract_receipt.v1"

export type BehaviorContractReceiptCheckStatus = "pass" | "fail" | "skipped"

export type BehaviorContractReceiptCheck = Readonly<{
  checkedAt: string
  evidenceRefs: readonly string[]
  id: string
  status: BehaviorContractReceiptCheckStatus
  summary: string
}>

export type BehaviorContractReceiptStatus = "pass" | "fail" | "skipped"

export type BehaviorContractReceipt = Readonly<{
  checkedAt: string
  checks: readonly BehaviorContractReceiptCheck[]
  contractId: string
  evidenceRefs: readonly string[]
  receiptId: string
  registryVersion: string
  schema: typeof BehaviorContractReceiptSchemaVersion
  statement: string
  status: BehaviorContractReceiptStatus
  surface: string
}>

export type BehaviorContractReceiptSweepCheck = Readonly<{
  evidenceRefs: readonly string[]
  id: string
  status: "pass" | "fail"
  summary: string
}>

const contractIssues = (
  validation: BehaviorContractRegistryValidation,
  contractId: string,
) => validation.issues.filter(issue => issue.contractId === contractId)

const contractCoverageFailures = (
  coverage: BehaviorContractCoverageReport,
  contractId: string,
) =>
  coverage.results.filter(result =>
    result.contractId === contractId &&
    result.status !== "covered" &&
    result.status !== "skipped_kind" &&
    result.status !== "skipped_state"
  )

const receiptStatus = (
  contract: BehaviorContract,
  checks: readonly BehaviorContractReceiptCheck[],
): BehaviorContractReceiptStatus => {
  if (contract.state !== "enforced") return "skipped"
  return checks.some(check => check.status === "fail") ? "fail" : "pass"
}

export const buildBehaviorContractReceipts = (
  document: BehaviorContractRegistryDocument,
  input: Readonly<{
    checkedAt: string
    coverage: BehaviorContractCoverageReport
    registryValidation: BehaviorContractRegistryValidation
    runId: string
    sweepChecks: readonly BehaviorContractReceiptSweepCheck[]
  }>,
): readonly BehaviorContractReceipt[] =>
  document.contracts.map(contract => {
    const registryIssues = contractIssues(input.registryValidation, contract.contractId)
    const coverageFailures = contractCoverageFailures(input.coverage, contract.contractId)
    const checks: BehaviorContractReceiptCheck[] = [
      {
        checkedAt: input.checkedAt,
        evidenceRefs: contract.evidenceRefs,
        id: "registry_entry_valid",
        status: registryIssues.length === 0 ? "pass" : "fail",
        summary: registryIssues.length === 0
          ? "Registry entry passed mechanical validation."
          : registryIssues.map(issue => `${issue.kind}: ${issue.detail}`).join("; "),
      },
      {
        checkedAt: input.checkedAt,
        evidenceRefs: contract.oracles.map(oracle => oracle.ref),
        id: "oracle_coverage_linked",
        status: coverageFailures.length === 0 ? "pass" : "fail",
        summary: coverageFailures.length === 0
          ? "Oracle refs are linked to the owning contract or intentionally handled by their runner."
          : coverageFailures.map(result => `${result.oracleId}: ${result.status} (${result.ref})`).join("; "),
      },
      ...input.sweepChecks.map((check): BehaviorContractReceiptCheck => ({
        checkedAt: input.checkedAt,
        evidenceRefs: check.evidenceRefs,
        id: check.id,
        status: contract.state === "enforced" ? check.status : "skipped",
        summary: contract.state === "enforced"
          ? check.summary
          : `Contract state is ${contract.state}; nightly sweep check recorded but not used as a green claim.`,
      })),
    ]
    const evidenceRefs = [
      ...contract.evidenceRefs,
      ...checks.flatMap(check => check.evidenceRefs),
    ]

    return {
      checkedAt: input.checkedAt,
      checks,
      contractId: contract.contractId,
      evidenceRefs: [...new Set(evidenceRefs)].sort(),
      receiptId: `${input.runId}:${contract.contractId}`,
      registryVersion: document.version,
      schema: BehaviorContractReceiptSchemaVersion,
      statement: contract.statement,
      status: receiptStatus(contract, checks),
      surface: contract.surface,
    }
  })

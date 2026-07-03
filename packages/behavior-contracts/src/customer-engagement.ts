import { Schema as S } from "effect"
import {
  BehaviorContractRegistryDocument,
  type BehaviorContractRegistryDocument as BehaviorContractRegistryDocumentType,
} from "./contract"
import {
  validateBehaviorContractRegistry,
  type BehaviorContractRegistryIssue,
} from "./registry"

export const CustomerBehaviorContractEngagementSchemaVersion =
  "openagents.customer_behavior_contract_engagement.v1"

export const BehaviorContractCatalogCategory = S.Literals([
  "indicator-truthfulness",
  "stated-flow-availability",
  "latency-budget",
  "error-state-honesty",
  "dead-control-detection",
  "cross-surface-consistency",
  "copy-claim-safety",
  "stated-expectation-pinning",
  "accessibility-floor",
  "money-path-integrity",
])
export type BehaviorContractCatalogCategory =
  | "indicator-truthfulness"
  | "stated-flow-availability"
  | "latency-budget"
  | "error-state-honesty"
  | "dead-control-detection"
  | "cross-surface-consistency"
  | "copy-claim-safety"
  | "stated-expectation-pinning"
  | "accessibility-floor"
  | "money-path-integrity"

export const CustomerBehaviorContractVisibility = S.Literals([
  "client-private",
  "public-demo",
  "public-opt-in",
])
export type CustomerBehaviorContractVisibility =
  | "client-private"
  | "public-demo"
  | "public-opt-in"

export const CustomerBehaviorContractCadenceTier = S.Literals([
  "on-deploy",
  "nightly",
  "weekly",
  "manual",
])
export type CustomerBehaviorContractCadenceTier =
  | "on-deploy"
  | "nightly"
  | "weekly"
  | "manual"

export const CustomerBehaviorContractAlertChannel = S.Literals([
  "client-webhook",
  "client-email",
  "private-forum-thread",
  "manual",
])
export type CustomerBehaviorContractAlertChannel =
  | "client-webhook"
  | "client-email"
  | "private-forum-thread"
  | "manual"

export const BehaviorContractOracleReceiptStatus = S.Literals([
  "pass",
  "fail",
  "pending",
  "blocked",
])
export type BehaviorContractOracleReceiptStatus =
  | "pass"
  | "fail"
  | "pending"
  | "blocked"

export const CustomerBehaviorContractTarget = S.Struct({
  baseUrl: S.String,
  clientRef: S.String,
  environment: S.String,
  evidenceUrl: S.String,
  optInEvidenceRef: S.optional(S.String),
  surface: S.String,
  visibility: CustomerBehaviorContractVisibility,
})
export type CustomerBehaviorContractTarget = {
  readonly baseUrl: string
  readonly clientRef: string
  readonly environment: string
  readonly evidenceUrl: string
  readonly optInEvidenceRef?: string
  readonly surface: string
  readonly visibility: CustomerBehaviorContractVisibility
}

export const CustomerBehaviorContractCadence = S.Struct({
  alertChannel: CustomerBehaviorContractAlertChannel,
  alertDestinationRef: S.String,
  tiers: S.Array(CustomerBehaviorContractCadenceTier),
})
export type CustomerBehaviorContractCadence = {
  readonly alertChannel: CustomerBehaviorContractAlertChannel
  readonly alertDestinationRef: string
  readonly tiers: ReadonlyArray<CustomerBehaviorContractCadenceTier>
}

export const BehaviorContractOracleReceipt = S.Struct({
  checkedAt: S.String,
  contractId: S.String,
  evidenceRefs: S.Array(S.String),
  receiptRef: S.String,
  status: BehaviorContractOracleReceiptStatus,
  summary: S.String,
})
export type BehaviorContractOracleReceipt = {
  readonly checkedAt: string
  readonly contractId: string
  readonly evidenceRefs: ReadonlyArray<string>
  readonly receiptRef: string
  readonly status: BehaviorContractOracleReceiptStatus
  readonly summary: string
}

export const BehaviorContractReceiptPack = S.Struct({
  latestSweepRef: S.String,
  receipts: S.Array(BehaviorContractOracleReceipt),
})
export type BehaviorContractReceiptPack = {
  readonly latestSweepRef: string
  readonly receipts: ReadonlyArray<BehaviorContractOracleReceipt>
}

export const CustomerBehaviorContractEngagementDocument = S.Struct({
  cadence: CustomerBehaviorContractCadence,
  engagementId: S.String,
  registry: BehaviorContractRegistryDocument,
  receiptPack: BehaviorContractReceiptPack,
  schemaVersion: S.Literal(CustomerBehaviorContractEngagementSchemaVersion),
  selectedCatalogCategories: S.Array(BehaviorContractCatalogCategory),
  target: CustomerBehaviorContractTarget,
  version: S.String,
})
export type CustomerBehaviorContractEngagementDocument = {
  readonly cadence: CustomerBehaviorContractCadence
  readonly engagementId: string
  readonly registry: BehaviorContractRegistryDocumentType
  readonly receiptPack: BehaviorContractReceiptPack
  readonly schemaVersion: typeof CustomerBehaviorContractEngagementSchemaVersion
  readonly selectedCatalogCategories: ReadonlyArray<BehaviorContractCatalogCategory>
  readonly target: CustomerBehaviorContractTarget
  readonly version: string
}

export const decodeCustomerBehaviorContractEngagementDocument = (
  input: unknown,
): CustomerBehaviorContractEngagementDocument =>
  S.decodeUnknownSync(CustomerBehaviorContractEngagementDocument)(
    input,
  ) as CustomerBehaviorContractEngagementDocument

export type CustomerBehaviorContractEngagementIssue = {
  readonly contractId: string | null
  readonly detail: string
  readonly kind:
    | "duplicate_receipt_contract"
    | "empty_cadence"
    | "empty_catalog_categories"
    | "empty_evidence_url"
    | "enforced_without_pass_receipt"
    | "invalid_engagement_id"
    | "invalid_version"
    | "pending_without_blocker"
    | "public_opt_in_without_evidence"
    | "receipt_for_unknown_contract"
    | "registry_validation_failed"
}

export type CustomerBehaviorContractEngagementValidation = {
  readonly issues: ReadonlyArray<CustomerBehaviorContractEngagementIssue>
  readonly ok: boolean
  readonly registryIssues: ReadonlyArray<BehaviorContractRegistryIssue>
}

export const validateCustomerBehaviorContractEngagement = (
  document: CustomerBehaviorContractEngagementDocument,
): CustomerBehaviorContractEngagementValidation => {
  const issues: CustomerBehaviorContractEngagementIssue[] = []
  const registryValidation = validateBehaviorContractRegistry(document.registry)

  if (!/^qa_swarm\.[a-z0-9_]+(\.[a-z0-9_]+)*\.v[0-9]+$/u.test(document.engagementId)) {
    issues.push({
      contractId: null,
      detail: "engagementId must match qa_swarm.<slug>.v<N>",
      kind: "invalid_engagement_id",
    })
  }
  if (!/^\d{4}-\d{2}-\d{2}\.\d+$/u.test(document.version)) {
    issues.push({
      contractId: null,
      detail: `engagement version "${document.version}" is not YYYY-MM-DD.N`,
      kind: "invalid_version",
    })
  }
  if (document.selectedCatalogCategories.length === 0) {
    issues.push({
      contractId: null,
      detail: "engagement must select at least one invariant catalog category",
      kind: "empty_catalog_categories",
    })
  }
  if (document.cadence.tiers.length === 0) {
    issues.push({
      contractId: null,
      detail: "engagement must name the cadence tiers that run the oracle pack",
      kind: "empty_cadence",
    })
  }
  if (document.target.evidenceUrl.trim() === "") {
    issues.push({
      contractId: null,
      detail: "engagement must carry a QA Swarm evidence URL",
      kind: "empty_evidence_url",
    })
  }
  if (
    document.target.visibility === "public-opt-in" &&
    document.target.optInEvidenceRef === undefined
  ) {
    issues.push({
      contractId: null,
      detail: "public customer evidence requires an explicit opt-in evidence ref",
      kind: "public_opt_in_without_evidence",
    })
  }

  if (!registryValidation.ok) {
    for (const registryIssue of registryValidation.issues) {
      issues.push({
        contractId: registryIssue.contractId,
        detail: registryIssue.detail,
        kind: "registry_validation_failed",
      })
    }
  }

  const contractIds = new Set(
    document.registry.contracts.map(contract => contract.contractId),
  )
  const receiptContractIds = new Set<string>()
  for (const receipt of document.receiptPack.receipts) {
    if (!contractIds.has(receipt.contractId)) {
      issues.push({
        contractId: receipt.contractId,
        detail: "receipt contractId is not present in the registry",
        kind: "receipt_for_unknown_contract",
      })
    }
    if (receiptContractIds.has(receipt.contractId)) {
      issues.push({
        contractId: receipt.contractId,
        detail: "receipt pack carries more than one latest receipt for the same contract",
        kind: "duplicate_receipt_contract",
      })
    }
    receiptContractIds.add(receipt.contractId)
  }

  for (const contract of document.registry.contracts) {
    if (contract.state === "pending" && contract.blockerRefs.length === 0) {
      issues.push({
        contractId: contract.contractId,
        detail: "pending customer contracts must name the blocker that keeps them non-green",
        kind: "pending_without_blocker",
      })
    }
    if (contract.state === "enforced") {
      const receipt = document.receiptPack.receipts.find(
        candidate => candidate.contractId === contract.contractId,
      )
      if (receipt?.status !== "pass") {
        issues.push({
          contractId: contract.contractId,
          detail: "enforced customer contracts need a latest passing receipt",
          kind: "enforced_without_pass_receipt",
        })
      }
    }
  }

  return {
    issues,
    ok: issues.length === 0,
    registryIssues: registryValidation.issues,
  }
}

export const renderCustomerBehaviorContractEngagementMarkdown = (
  document: CustomerBehaviorContractEngagementDocument,
): string => {
  const lines: string[] = []
  lines.push(`# ${document.engagementId}`)
  lines.push("")
  lines.push(`Version: \`${document.version}\``)
  lines.push(`Target: \`${document.target.clientRef}\` / \`${document.target.surface}\``)
  lines.push(`Visibility: \`${document.target.visibility}\``)
  lines.push(`Cadence: ${document.cadence.tiers.map(tier => `\`${tier}\``).join(", ")}`)
  lines.push(`Alert channel: \`${document.cadence.alertChannel}\``)
  lines.push(`Evidence URL: \`${document.target.evidenceUrl}\``)
  lines.push("")
  lines.push("## Contracts")
  lines.push("")
  for (const contract of document.registry.contracts) {
    const receipt = document.receiptPack.receipts.find(
      candidate => candidate.contractId === contract.contractId,
    )
    lines.push(`- \`${contract.contractId}\` — ${contract.state}`)
    lines.push(`  Statement: ${contract.statement}`)
    lines.push(`  Latest receipt: ${receipt?.status ?? "none"} (${receipt?.receiptRef ?? "none"})`)
  }
  return lines.join("\n")
}

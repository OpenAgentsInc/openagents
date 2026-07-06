import {
  behaviorContractIdPattern,
  isSeamBehaviorContractId,
  type BehaviorContractRegistryDocument,
} from "./contract"

export type BehaviorContractRegistryIssue = {
  readonly contractId: string | null
  readonly detail: string
  readonly kind:
    | "duplicate_contract_id"
    | "duplicate_oracle_id"
    | "empty_statement"
    | "empty_verification"
    | "enforced_with_blockers"
    | "enforced_without_oracle"
    | "enforced_without_sweep_tier"
    | "invalid_contract_id"
    | "invalid_version"
    | "seam_field_without_seam_id"
    | "seam_missing_artifacts"
}

export type BehaviorContractRegistryValidation = {
  readonly issues: ReadonlyArray<BehaviorContractRegistryIssue>
  readonly ok: boolean
}

/**
 * Mechanical registry checks, mirroring the product-promise transition
 * checks: the "good" state (`enforced`) requires a non-empty statement, a
 * named verification, at least one oracle, an automated enforcement tier,
 * and zero blocker refs.
 */
export const validateBehaviorContractRegistry = (
  document: BehaviorContractRegistryDocument,
): BehaviorContractRegistryValidation => {
  const issues: BehaviorContractRegistryIssue[] = []

  if (!/^\d{4}-\d{2}-\d{2}\.\d+$/u.test(document.version)) {
    issues.push({
      contractId: null,
      detail: `registry version "${document.version}" is not YYYY-MM-DD.N`,
      kind: "invalid_version",
    })
  }

  const seenContractIds = new Set<string>()
  for (const contract of document.contracts) {
    if (seenContractIds.has(contract.contractId)) {
      issues.push({
        contractId: contract.contractId,
        detail: "contractId appears more than once in the registry",
        kind: "duplicate_contract_id",
      })
    }
    seenContractIds.add(contract.contractId)

    if (!behaviorContractIdPattern.test(contract.contractId)) {
      issues.push({
        contractId: contract.contractId,
        detail: "contractId must match <area>.<slug>.v<N> in lower snake case",
        kind: "invalid_contract_id",
      })
    }
    if (contract.statement.trim() === "") {
      issues.push({
        contractId: contract.contractId,
        detail: "statement must record the stated expectation verbatim",
        kind: "empty_statement",
      })
    }
    if (contract.verification.trim() === "") {
      issues.push({
        contractId: contract.contractId,
        detail: "verification must name how the contract is checked",
        kind: "empty_verification",
      })
    }

    // Seam-contract convention (ST-5 #8511): a `seam` id segment and the
    // two-sided `seam` field must appear together, and both artifact paths
    // must be named — a seam contract that only names one side is exactly the
    // one-sided-contract failure mode this convention exists to eliminate.
    if (isSeamBehaviorContractId(contract.contractId)) {
      if (
        contract.seam === undefined ||
        contract.seam.client.trim() === "" ||
        contract.seam.server.trim() === ""
      ) {
        issues.push({
          contractId: contract.contractId,
          detail:
            "seam contracts (id contains a `seam` segment) must name both sides via seam.client and seam.server",
          kind: "seam_missing_artifacts",
        })
      }
    } else if (contract.seam !== undefined) {
      issues.push({
        contractId: contract.contractId,
        detail:
          "contracts carrying a seam field must use the seam id convention (<area>.seam.<slug>.v<N>)",
        kind: "seam_field_without_seam_id",
      })
    }

    const seenOracleIds = new Set<string>()
    for (const oracle of contract.oracles) {
      if (seenOracleIds.has(oracle.id)) {
        issues.push({
          contractId: contract.contractId,
          detail: `oracle id "${oracle.id}" appears more than once`,
          kind: "duplicate_oracle_id",
        })
      }
      seenOracleIds.add(oracle.id)
    }

    if (contract.state === "enforced") {
      if (contract.oracles.length === 0) {
        issues.push({
          contractId: contract.contractId,
          detail: "enforced contracts need at least one oracle",
          kind: "enforced_without_oracle",
        })
      }
      if (contract.blockerRefs.length > 0) {
        issues.push({
          contractId: contract.contractId,
          detail: "enforced contracts must have zero blocker refs",
          kind: "enforced_with_blockers",
        })
      }
      if (
        contract.enforcementTier !== "test-sweep" &&
        contract.enforcementTier !== "nightly"
      ) {
        issues.push({
          contractId: contract.contractId,
          detail:
            "enforced contracts must run on an automated tier (test-sweep or nightly)",
          kind: "enforced_without_sweep_tier",
        })
      }
    }
  }

  return { issues, ok: issues.length === 0 }
}

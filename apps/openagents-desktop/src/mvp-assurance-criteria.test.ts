import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"

type CriterionContract = Readonly<{
  criterion: string
  evidencePath: string
  requiredTokens: ReadonlyArray<string>
}>

const root = resolve(import.meta.dirname, "../../..")

/**
 * Criterion-local anchors complement, but never replace, the complete Desktop
 * suite and installed RC9 journal. Each oracle binds one ProductSpec criterion
 * to the exact implementation/test/receipt bytes named by the accepted MVP
 * completion audit. Its falsifier removes a required anchor and must be
 * rejected, giving the Assurance adapter a deterministic sensitivity pair.
 */
export const mvpCriterionContracts: ReadonlyArray<CriterionContract> = [
  { criterion: "CW-AC-01", evidencePath: "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-candidate-receipt.md", requiredTokens: ["signed/notarized", "stapled", "acceptance PASSED"] },
  { criterion: "CW-AC-02", evidencePath: "apps/openagents-desktop/src/mvp-proof.test.ts", requiredTokens: ["ordinary Codex session", "shell-codex-engine", "not.toContain(\"CODEX_HOME\")"] },
  { criterion: "CW-AC-03", evidencePath: "apps/openagents-desktop/src/product-spec-workroom.test.ts", requiredTokens: ["workContextRef", "runRef", "packetRef"] },
  { criterion: "CW-AC-04", evidencePath: "packages/product-spec/test/product-spec.test.ts", requiredTokens: ["the MVP spec is executable with unique author-visible criteria", "duplicate criterion IDs refuse executable admission"] },
  { criterion: "CW-AC-05", evidencePath: "apps/openagents-desktop/src/product-spec-workroom.test.ts", requiredTokens: ["requires a confirmed revision bump", "criterion reconciliation", "revision_not_incremented"] },
  { criterion: "CW-AC-06", evidencePath: "apps/openagents-desktop/src/product-spec-workroom.test.ts", requiredTokens: ["rejects duplicate/cyclic packets", "leaseRef", "dependencyRefs"] },
  { criterion: "CW-AC-07", evidencePath: "apps/openagents-desktop/src/builtin-productspec-skill.test.ts", requiredTokens: ["pins the product-owned proposal-only skills", "fails closed", "ProductSpecWorkSkillSha256"] },
  { criterion: "CW-AC-08", evidencePath: "apps/openagents-desktop/src/product-spec-app-server-tools.test.ts", requiredTokens: ["no authority-bearing approval operation", "record_evidence", "propose_plan"] },
  { criterion: "CW-AC-09", evidencePath: "apps/openagents-desktop/src/product-spec-workroom.test.ts", requiredTokens: ["stops dispatch on revision mismatch", "superseded", "expectedSpec"] },
  { criterion: "CW-AC-10", evidencePath: "apps/openagents-desktop/src/renderer/history-workspace.test.ts", requiredTokens: ["opens at the END", "scroll-up prepends older items", "restore plan"] },
  { criterion: "CW-AC-11", evidencePath: "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-candidate-receipt.md", requiredTokens: ["Real root Codex packet terminalized", "independent host verification", "non-text tool evidence"] },
  { criterion: "CW-AC-12", evidencePath: "apps/openagents-desktop/src/product-spec-workroom.test.ts", requiredTokens: ["persists idempotent failed, cancelled, and superseded", "lease.cancelled", "disposePacket"] },
  { criterion: "CW-AC-13", evidencePath: "apps/openagents-desktop/src/local-runtime-event-persistence.test.ts", requiredTokens: ["retains nested child identity", "independent transcript", "parent"] },
  { criterion: "CW-AC-14", evidencePath: "apps/openagents-desktop/src/renderer/shell.test.ts", requiredTokens: ["grant-scoped relative tree entries", "typed Git panel", "legacy absolute-path editor"] },
  { criterion: "CW-AC-15", evidencePath: "apps/openagents-desktop/src/codex-handoff.test.ts", requiredTokens: ["quiesces the exact OpenAgents packet", "reconciles an exact retry after restart", "does not silently retarget"] },
  { criterion: "CW-AC-16", evidencePath: "apps/openagents-desktop/src/update-staging-host.test.ts", requiredTokens: ["survives restart", "pointer mismatch", "retained rollback"] },
  { criterion: "CW-AC-17", evidencePath: "apps/openagents-desktop/src/renderer/diagnostics.test.ts", requiredTokens: ["privacy: no rendered text carries a path", "public-safe", "unavailable"] },
  { criterion: "CW-AC-18", evidencePath: "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-candidate-receipt.md", requiredTokens: ["All 12 required journal steps passed", "passed all 11 entries", "Uninstalled the reversible proof app"] },
]

const satisfies = (source: string, requiredTokens: ReadonlyArray<string>): boolean =>
  requiredTokens.every((token) => source.includes(token))

describe("accepted OpenAgents Desktop MVP criterion evidence", () => {
  for (const contract of mvpCriterionContracts) {
    test(`${contract.criterion} candidate evidence remains bound`, () => {
      const source = readFileSync(resolve(root, contract.evidencePath), "utf8")
      expect(satisfies(source, contract.requiredTokens)).toBe(true)
    })

    test(`${contract.criterion} missing-anchor falsifier is rejected`, () => {
      const source = readFileSync(resolve(root, contract.evidencePath), "utf8")
      const first = contract.requiredTokens[0]!
      const mutated = source.replaceAll(first, "")
      expect(satisfies(mutated, contract.requiredTokens)).toBe(false)
    })
  }
})

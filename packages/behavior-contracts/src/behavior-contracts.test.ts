import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  BehaviorContractSchemaVersion,
  decodeBehaviorContractRegistryDocument,
  type BehaviorContract,
  type BehaviorContractRegistryDocument,
} from "./contract"
import {
  checkBehaviorContractCoverage,
  inMemoryOracleSourceLayer,
} from "./coverage"
import { buildBehaviorContractReceipts } from "./receipt"
import { validateBehaviorContractRegistry } from "./registry"
import { renderBehaviorContractMarkdown } from "./report"

const contract = (overrides: Partial<BehaviorContract> = {}): BehaviorContract => ({
  blockerRefs: [],
  contractId: "khala_code.chat.example_behavior.v1",
  enforcementTier: "test-sweep",
  evidenceRefs: ["clients/khala-code-desktop/tests/ux-contracts.test.ts"],
  oracles: [
    {
      description: "DOM oracle for the example behavior",
      id: "example.dom",
      kind: "bun-test",
      mode: "dom",
      ref: "clients/khala-code-desktop/tests/ux-contracts.test.ts",
    },
  ],
  productArea: "chat",
  source: {
    channel: "khala-code-session",
    statedBy: "owner",
    statedOn: "2026-07-03",
  },
  state: "enforced",
  statement: "Example stated behavior.",
  surface: "khala-code-desktop",
  verification: "bun test tests/ux-contracts.test.ts in clients/khala-code-desktop",
  ...overrides,
})

const document = (
  contracts: ReadonlyArray<BehaviorContract>,
): BehaviorContractRegistryDocument => ({
  contracts,
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-03.1",
})

describe("behavior contract registry", () => {
  test("decodes a well-formed registry document", () => {
    const decoded = decodeBehaviorContractRegistryDocument(document([contract()]))
    expect(decoded.contracts).toHaveLength(1)
    expect(decoded.contracts[0]?.contractId).toBe("khala_code.chat.example_behavior.v1")
  })

  test("rejects unknown states at decode time", () => {
    expect(() =>
      decodeBehaviorContractRegistryDocument(
        document([{ ...contract(), state: "green" as never }]),
      ),
    ).toThrow()
  })

  test("accepts an enforced contract with oracle, sweep tier, and no blockers", () => {
    const validation = validateBehaviorContractRegistry(document([contract()]))
    expect(validation.ok).toBe(true)
    expect(validation.issues).toHaveLength(0)
  })

  test("flags enforced contracts without oracles", () => {
    const validation = validateBehaviorContractRegistry(
      document([contract({ oracles: [] })]),
    )
    expect(validation.ok).toBe(false)
    expect(validation.issues.map(issue => issue.kind)).toContain("enforced_without_oracle")
  })

  test("flags enforced contracts with blocker refs", () => {
    const validation = validateBehaviorContractRegistry(
      document([contract({ blockerRefs: ["blocker.example"] })]),
    )
    expect(validation.issues.map(issue => issue.kind)).toContain("enforced_with_blockers")
  })

  test("flags enforced contracts on manual or unenforced tiers", () => {
    const validation = validateBehaviorContractRegistry(
      document([contract({ enforcementTier: "manual" })]),
    )
    expect(validation.issues.map(issue => issue.kind)).toContain(
      "enforced_without_sweep_tier",
    )
  })

  test("flags duplicate contract ids and malformed ids", () => {
    const validation = validateBehaviorContractRegistry(
      document([
        contract(),
        contract(),
        contract({ contractId: "Not A Valid Id" }),
      ]),
    )
    const kinds = validation.issues.map(issue => issue.kind)
    expect(kinds).toContain("duplicate_contract_id")
    expect(kinds).toContain("invalid_contract_id")
  })

  test("allows pending contracts without oracles", () => {
    const validation = validateBehaviorContractRegistry(
      document([
        contract({
          enforcementTier: "unenforced",
          oracles: [],
          state: "pending",
        }),
      ]),
    )
    expect(validation.ok).toBe(true)
  })
})

describe("behavior contract coverage", () => {
  test("covered when the oracle source references the contract id", async () => {
    const registry = document([contract()])
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(registry).pipe(
        Effect.provide(
          inMemoryOracleSourceLayer({
            "clients/khala-code-desktop/tests/ux-contracts.test.ts":
              "// khala_code.chat.example_behavior.v1 oracle body",
          }),
        ),
      ),
    )
    expect(report.ok).toBe(true)
    expect(report.results[0]?.status).toBe("covered")
  })

  test("fails when the oracle source file is missing", async () => {
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(document([contract()])).pipe(
        Effect.provide(inMemoryOracleSourceLayer({})),
      ),
    )
    expect(report.ok).toBe(false)
    expect(report.results[0]?.status).toBe("missing_source")
  })

  test("fails when the oracle source never references the contract id", async () => {
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(document([contract()])).pipe(
        Effect.provide(
          inMemoryOracleSourceLayer({
            "clients/khala-code-desktop/tests/ux-contracts.test.ts":
              "// unrelated test body",
          }),
        ),
      ),
    )
    expect(report.ok).toBe(false)
    expect(report.results[0]?.status).toBe("missing_contract_reference")
  })

  test("skips non-bun-test oracles and non-enforced contracts", async () => {
    const registry = document([
      contract({
        oracles: [
          {
            description: "qa harness scenario",
            id: "example.scenario",
            kind: "qa-scenario",
            mode: "rpc",
            ref: "scenario.khala_code.seed.example.v1",
          },
        ],
      }),
      contract({
        contractId: "khala_code.chat.pending_behavior.v1",
        state: "pending",
      }),
    ])
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(registry).pipe(
        Effect.provide(inMemoryOracleSourceLayer({})),
      ),
    )
    expect(report.ok).toBe(true)
    expect(report.results.map(result => result.status)).toEqual([
      "skipped_kind",
      "skipped_state",
    ])
  })
})

describe("behavior contract report", () => {
  test("renders each contract with statement, oracle, and tier", () => {
    const markdown = renderBehaviorContractMarkdown(document([contract()]))
    expect(markdown).toContain("khala_code.chat.example_behavior.v1")
    expect(markdown).toContain("ENFORCED")
    expect(markdown).toContain("Example stated behavior.")
    expect(markdown).toContain("test-sweep")
    expect(markdown).toContain("clients/khala-code-desktop/tests/ux-contracts.test.ts")
  })
})

describe("behavior contract receipts", () => {
  test("records per-contract pass and fail checks without changing registry state", async () => {
    const registry = document([contract()])
    const registryValidation = validateBehaviorContractRegistry(registry)
    const coverage = await Effect.runPromise(
      checkBehaviorContractCoverage(registry).pipe(
        Effect.provide(
          inMemoryOracleSourceLayer({
            "clients/khala-code-desktop/tests/ux-contracts.test.ts":
              "// khala_code.chat.example_behavior.v1 oracle body",
          }),
        ),
      ),
    )

    const [receipt] = buildBehaviorContractReceipts(registry, {
      checkedAt: "2026-07-03T12:00:00.000Z",
      coverage,
      registryValidation,
      runId: "nightly-1",
      sweepChecks: [
        {
          evidenceRefs: ["var/qa-nightly/nightly-1/logs/desktop-verify.log"],
          id: "nightly_step.desktop_verify",
          status: "fail",
          summary: "desktop verify failed",
        },
      ],
    })

    expect(receipt?.schema).toBe("openagents.behavior_contract_receipt.v1")
    expect(receipt?.contractId).toBe("khala_code.chat.example_behavior.v1")
    expect(receipt?.statement).toBe("Example stated behavior.")
    expect(receipt?.status).toBe("fail")
    expect(receipt?.checks.map(check => check.id)).toContain("registry_entry_valid")
    expect(receipt?.checks.map(check => check.id)).toContain("oracle_coverage_linked")
    expect(receipt?.checks.map(check => check.id)).toContain("nightly_step.desktop_verify")
    expect(registry.contracts[0]?.state).toBe("enforced")
  })
})

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
import {
  CustomerBehaviorContractEngagementSchemaVersion,
  decodeCustomerBehaviorContractEngagementDocument,
  renderCustomerBehaviorContractEngagementMarkdown,
  validateCustomerBehaviorContractEngagement,
  type CustomerBehaviorContractEngagementDocument,
} from "./customer-engagement"
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

const engagement = (
  overrides: Partial<CustomerBehaviorContractEngagementDocument> = {},
): CustomerBehaviorContractEngagementDocument => {
  const registry = document([
    contract({
      blockerRefs: ["blocker.qa_swarm.example.oracle_pending"],
      contractId: "qa_swarm_demo.login.example_behavior.v1",
      enforcementTier: "unenforced",
      oracles: [
        {
          description: "Planned customer cadence oracle",
          id: "example.planned",
          kind: "planned",
          mode: "headless",
          ref: "scenario.qa_swarm_demo.login.example_behavior.v1",
        },
      ],
      productArea: "login flow",
      source: {
        channel: "qa-swarm-public-demo-intake",
        statedBy: "pilot-demo-operator",
        statedOn: "2026-07-03",
      },
      state: "pending",
      statement: "The public demo login page loads with the expected title.",
      surface: "openagents.com/login",
      verification:
        "Pending until the planned oracle runs in the customer cadence.",
    }),
  ])

  return {
    cadence: {
      alertChannel: "private-forum-thread",
      alertDestinationRef: "forum.private.qa-swarm.public-demo",
      tiers: ["nightly", "weekly"],
    },
    engagementId: "qa_swarm.public_demo_login.v1",
    receiptPack: {
      latestSweepRef: "receipt.qa_swarm.public_demo_login.seed.20260703",
      receipts: [
        {
          checkedAt: "2026-07-03T00:00:00.000Z",
          contractId: "qa_swarm_demo.login.example_behavior.v1",
          evidenceRefs: ["docs/qa-demo/result.json"],
          receiptRef:
            "receipt.qa_swarm.public_demo_login.example_behavior.seed.20260703",
          status: "pending",
          summary: "Oracle specified but not yet wired into the customer cadence.",
        },
      ],
    },
    registry,
    schemaVersion: CustomerBehaviorContractEngagementSchemaVersion,
    selectedCatalogCategories: [
      "stated-flow-availability",
      "stated-expectation-pinning",
    ],
    target: {
      baseUrl: "https://openagents.com/login",
      clientRef: "public-demo.openagents",
      environment: "production-public-demo",
      evidenceUrl: "/qa/qa-run.public-demo.behavior-contracts.latest",
      surface: "openagents.com/login",
      visibility: "public-demo",
    },
    version: "2026-07-03.1",
    ...overrides,
  }
}

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

describe("customer behavior contract engagements", () => {
  test("accepts a public-demo pilot with pending blocker-backed contracts", () => {
    const validation = validateCustomerBehaviorContractEngagement(engagement())
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)
  })

  test("keeps public customer registries opt-in and cadence-backed", () => {
    const validation = validateCustomerBehaviorContractEngagement(
      engagement({
        cadence: {
          alertChannel: "manual",
          alertDestinationRef: "manual.qa-swarm.public-demo",
          tiers: [],
        },
        target: {
          baseUrl: "https://client.example.test",
          clientRef: "client.example",
          environment: "staging",
          evidenceUrl: "",
          surface: "client-web",
          visibility: "public-opt-in",
        },
      }),
    )
    expect(validation.ok).toBe(false)
    expect(validation.issues.map(issue => issue.kind)).toEqual(
      expect.arrayContaining([
        "empty_cadence",
        "empty_evidence_url",
        "public_opt_in_without_evidence",
      ]),
    )
  })

  test("requires pending customer contracts to name blocker refs", () => {
    const base = engagement()
    const validation = validateCustomerBehaviorContractEngagement({
      ...base,
      registry: document([
        contract({
          contractId: "qa_swarm_demo.login.missing_blocker.v1",
          enforcementTier: "unenforced",
          oracles: [],
          state: "pending",
        }),
      ]),
      receiptPack: {
        latestSweepRef: "receipt.qa_swarm.public_demo_login.seed.20260703",
        receipts: [
          {
            checkedAt: "2026-07-03T00:00:00.000Z",
            contractId: "qa_swarm_demo.login.missing_blocker.v1",
            evidenceRefs: [],
            receiptRef: "receipt.qa_swarm.public_demo_login.missing_blocker",
            status: "pending",
            summary: "No oracle yet.",
          },
        ],
      },
    })
    expect(validation.ok).toBe(false)
    expect(validation.issues.map(issue => issue.kind)).toContain(
      "pending_without_blocker",
    )
  })

  test("validates the committed public-demo pilot registry for issue 8186", async () => {
    const raw = await Bun.file(
      new URL("../../../docs/qa-demo/customer-behavior-contract-pilot.json", import.meta.url),
    ).json()
    const pilot = decodeCustomerBehaviorContractEngagementDocument(raw)
    const validation = validateCustomerBehaviorContractEngagement(pilot)

    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)
    expect(pilot.engagementId).toBe("qa_swarm.public_demo_login.v1")
    expect(pilot.target.visibility).toBe("public-demo")
    expect(pilot.registry.contracts.every(contract => contract.state === "pending")).toBe(
      true,
    )
    expect(pilot.receiptPack.receipts.map(receipt => receipt.status)).toEqual([
      "pending",
      "pending",
      "pending",
      "pending",
    ])
  })

  test("renders customer engagement evidence without private raw artifacts", () => {
    const markdown = renderCustomerBehaviorContractEngagementMarkdown(engagement())
    expect(markdown).toContain("qa_swarm.public_demo_login.v1")
    expect(markdown).toContain("Visibility: `public-demo`")
    expect(markdown).toContain("/qa/qa-run.public-demo.behavior-contracts.latest")
    expect(markdown).not.toContain("raw")
  })
})

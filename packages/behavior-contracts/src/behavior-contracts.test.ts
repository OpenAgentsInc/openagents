import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  BehaviorContractSchemaVersion,
  decodeBehaviorContractRegistryDocument,
  type BehaviorContract,
  type BehaviorContractRegistryDocument,
} from "./contract"
import {
  BACKGROUND_AGENTS_CONTRACT_DOC_PATH,
  backgroundAgentsContractRegistry,
} from "./background-agents"
import { khalaSyncContractRegistry } from "./khala-sync"
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

  test("requires seam contracts (seam id segment) to name both sides", () => {
    const validation = validateBehaviorContractRegistry(
      document([
        contract({
          contractId: "khala_sync.seam.example_two_sided.v1",
        }),
      ]),
    )
    expect(validation.ok).toBe(false)
    expect(validation.issues.map(issue => issue.kind)).toContain(
      "seam_missing_artifacts",
    )
  })

  test("rejects seam artifacts with an empty side", () => {
    const validation = validateBehaviorContractRegistry(
      document([
        contract({
          contractId: "khala_sync.seam.example_two_sided.v1",
          seam: { client: "packages/example/src/client.ts", server: "  " },
        }),
      ]),
    )
    expect(validation.issues.map(issue => issue.kind)).toContain(
      "seam_missing_artifacts",
    )
  })

  test("rejects a seam field on a non-seam contract id", () => {
    const validation = validateBehaviorContractRegistry(
      document([
        contract({
          seam: {
            client: "packages/example/src/client.ts",
            server: "apps/example/src/server.ts",
          },
        }),
      ]),
    )
    expect(validation.issues.map(issue => issue.kind)).toContain(
      "seam_field_without_seam_id",
    )
  })

  test("accepts an enforced seam contract naming both sides with an e2e oracle", () => {
    const validation = validateBehaviorContractRegistry(
      document([
        contract({
          contractId: "khala_sync.seam.example_two_sided.v1",
          oracles: [
            {
              description: "two-sided e2e oracle",
              id: "example.seam_e2e",
              kind: "bun-test",
              mode: "rpc",
              ref: "packages/example/src/example-seam.e2e.test.ts",
            },
          ],
          seam: {
            client: "packages/example/src/client.ts",
            server: "apps/example/src/server.ts",
          },
        }),
      ]),
    )
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)
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

  test("covers qa-scenario oracles when the scenario source exists", async () => {
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
    ])
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(registry).pipe(
        Effect.provide(
          inMemoryOracleSourceLayer({
            "scenario.khala_code.seed.example.v1": JSON.stringify({
              id: "scenario.khala_code.seed.example.v1",
            }),
          }),
        ),
      ),
    )
    expect(report.ok).toBe(true)
    expect(report.results[0]?.status).toBe("covered")
  })

  test("fails qa-scenario oracles when the scenario source is missing", async () => {
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
    ])
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(registry).pipe(
        Effect.provide(inMemoryOracleSourceLayer({})),
      ),
    )
    expect(report.ok).toBe(false)
    expect(report.results[0]?.status).toBe("missing_source")
  })

  test("fails an enforced seam contract whose bun-test oracle is not an e2e suite", async () => {
    const registry = document([
      contract({
        contractId: "khala_sync.seam.example_two_sided.v1",
        oracles: [
          {
            description: "one-sided fake-transport unit suite — never seam proof",
            id: "example.seam_unit",
            kind: "bun-test",
            mode: "unit",
            ref: "packages/example/src/session.test.ts",
          },
        ],
        seam: {
          client: "packages/example/src/client.ts",
          server: "apps/example/src/server.ts",
        },
      }),
    ])
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(registry).pipe(
        Effect.provide(
          inMemoryOracleSourceLayer({
            "packages/example/src/session.test.ts":
              "// khala_sync.seam.example_two_sided.v1 referenced, but this is a fake-transport suite",
          }),
        ),
      ),
    )
    expect(report.ok).toBe(false)
    expect(report.results[0]?.status).toBe("seam_oracle_not_e2e")
  })

  test("covers an enforced seam contract whose e2e oracle exists and references it", async () => {
    const registry = document([
      contract({
        contractId: "khala_sync.seam.example_two_sided.v1",
        oracles: [
          {
            description: "two-sided e2e oracle",
            id: "example.seam_e2e",
            kind: "bun-test",
            mode: "rpc",
            ref: "packages/example/src/example-seam.e2e.test.ts",
          },
        ],
        seam: {
          client: "packages/example/src/client.ts",
          server: "apps/example/src/server.ts",
        },
      }),
    ])
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(registry).pipe(
        Effect.provide(
          inMemoryOracleSourceLayer({
            "packages/example/src/example-seam.e2e.test.ts":
              "// khala_sync.seam.example_two_sided.v1 oracle body importing real client + server",
          }),
        ),
      ),
    )
    expect(report.ok).toBe(true)
    expect(report.results[0]?.status).toBe("covered")
  })

  test("a pending seam contract is skipped, not failed, by seam coverage", async () => {
    const registry = document([
      contract({
        blockerRefs: ["blocker.example.depends_on_e2e_landing"],
        contractId: "khala_sync.seam.example_two_sided.v1",
        enforcementTier: "unenforced",
        oracles: [
          {
            description: "planned two-sided oracle",
            id: "example.seam_planned",
            kind: "bun-test",
            mode: "rpc",
            ref: "packages/example/src/example-seam.e2e.test.ts",
          },
        ],
        seam: {
          client: "packages/example/src/client.ts",
          server: "apps/example/src/server.ts",
        },
        state: "pending",
      }),
    ])
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(registry).pipe(
        Effect.provide(inMemoryOracleSourceLayer({})),
      ),
    )
    expect(report.ok).toBe(true)
    expect(report.results[0]?.status).toBe("skipped_state")
  })

  test("skips non-source-backed oracles and non-enforced contracts", async () => {
    const registry = document([
      contract({
        oracles: [
          {
            description: "manual acceptance check",
            id: "example.manual",
            kind: "manual-check",
            mode: "unit",
            ref: "manual.example",
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

const repoPath = (ref: string): string =>
  new URL(`../../../${ref}`, import.meta.url).pathname

describe("background agent contract registry", () => {
  test("registers the headline background-agent invariants with enforced oracles", () => {
    const validation = validateBehaviorContractRegistry(backgroundAgentsContractRegistry)
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)
    expect(backgroundAgentsContractRegistry.contracts.map(contract => contract.contractId)).toEqual([
      "background_agents.dispatch.budget_caps_enforced.v1",
      "background_agents.dispatch.lane_account_breaker.v1",
      "background_agents.toolset.compiled_policy_enforced.v1",
      "lead_gen_agent.drafting_only_toolset.v1",
      "lead_gen_agent.no_send_without_approval_receipt.v1",
      "background_agents.credentials.brokered_scm_helper.v1",
      "background_agents.credentials.no_long_lived_tokens_in_workspaces.v1",
      "background_agents.warm_dispatch.prepared_worktree_cache.v1",
      "background_agents.warm_dispatch.prebuilt_baseline_cache.v1",
      "background_agents.integrations.forum_trigger_callback.v1",
      "background_agents.integrations.github_mention_callback.v1",
      "background_agents.inbox.event_ledger_owner_scoped_private.v1",
      "background_agents.inbox.event_ledger_handled_gateway_redacted.v1",
      "background_agents.inbox.slack_event_ledger_ingest.v1",
      "background_agents.definitions.harness_swap.v1",
      "background_agents.agents_panel.run_status_indicators_truthful.v1",
      "background_agents.warm_dispatch.honest_no_op_without_warm_path.v1",
    ])
    const enforcedContractIds = backgroundAgentsContractRegistry.contracts
      .filter(contract => contract.state === "enforced")
      .map(contract => contract.contractId)
    expect(enforcedContractIds).toEqual([
      "background_agents.dispatch.budget_caps_enforced.v1",
      "background_agents.dispatch.lane_account_breaker.v1",
      "background_agents.toolset.compiled_policy_enforced.v1",
      "lead_gen_agent.drafting_only_toolset.v1",
      "lead_gen_agent.no_send_without_approval_receipt.v1",
      "background_agents.credentials.brokered_scm_helper.v1",
      "background_agents.credentials.no_long_lived_tokens_in_workspaces.v1",
      "background_agents.warm_dispatch.prepared_worktree_cache.v1",
      "background_agents.warm_dispatch.prebuilt_baseline_cache.v1",
      "background_agents.integrations.forum_trigger_callback.v1",
      "background_agents.integrations.github_mention_callback.v1",
      "background_agents.inbox.event_ledger_owner_scoped_private.v1",
      "background_agents.inbox.event_ledger_handled_gateway_redacted.v1",
      "background_agents.inbox.slack_event_ledger_ingest.v1",
    ])
    for (const contract of backgroundAgentsContractRegistry.contracts.filter(
      contract => enforcedContractIds.includes(contract.contractId),
    )) {
      expect(contract).toMatchObject({
        blockerRefs: [],
        enforcementTier: "test-sweep",
        state: "enforced",
      })
      expect(contract.oracles.length).toBeGreaterThan(0)
    }
    const pendingContracts = backgroundAgentsContractRegistry.contracts.filter(
      contract =>
        !enforcedContractIds.includes(contract.contractId),
    )
    expect(
      pendingContracts.every(
        contract =>
          contract.state === "pending" &&
          contract.enforcementTier === "unenforced" &&
          contract.blockerRefs.length > 0,
      ),
    ).toBe(true)
  })

  test("background-agent oracle coverage covers enforced entries and skips pending entries", async () => {
    const enforcedContracts = backgroundAgentsContractRegistry.contracts.filter(
      contract => contract.state === "enforced",
    )
    const sourceByRef = new Map<string, string[]>()
    for (const contract of enforcedContracts) {
      for (const oracle of contract.oracles) {
        sourceByRef.set(oracle.ref, [
          ...(sourceByRef.get(oracle.ref) ?? []),
          `// ${contract.contractId}`,
        ])
      }
    }
    const sources = Object.fromEntries(
      [...sourceByRef.entries()].map(([ref, contractRefs]) => [ref, contractRefs.join("\n")]),
    )
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(backgroundAgentsContractRegistry).pipe(
        Effect.provide(inMemoryOracleSourceLayer(sources)),
      ),
    )
    expect(report.ok).toBe(true)
    expect(report.results.map(result => result.status)).toEqual(
      Array.from({ length: report.results.length }, () => "covered"),
    )
  })

  test("the background-agent human contract doc stays in sync with the registry", async () => {
    const doc = await Bun.file(repoPath(BACKGROUND_AGENTS_CONTRACT_DOC_PATH)).text()
    expect(doc).toContain(
      `Registry version: \`${backgroundAgentsContractRegistry.version}\``,
    )
    expect(doc).toContain(
      renderBehaviorContractMarkdown(backgroundAgentsContractRegistry).split("\n")[0] ??
        "",
    )
    for (const contract of backgroundAgentsContractRegistry.contracts) {
      expect(doc).toContain(contract.contractId)
      expect(doc).toContain(contract.statement)
      for (const blockerRef of contract.blockerRefs) {
        expect(doc).toContain(blockerRef)
      }
    }
  })
})

describe("khala sync contract registry", () => {
  test("registers the sync-engine + synced-surface contracts as enforced test-sweep entries", () => {
    const validation = validateBehaviorContractRegistry(khalaSyncContractRegistry)
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)

    expect(khalaSyncContractRegistry.contracts.map(contract => contract.contractId)).toEqual([
      "khala_sync.push.validation_never_blocks_queue.v1",
      "khala_sync.client.offline_pushes_queue_honestly.v1",
      "khala_sync.client.staleness_never_fabricated.v1",
      "khala_sync.access.revocation_clears_synced_state.v1",
      "khala_sync.seam.bearer_ws_connect_reaches_live.v1",
      "khala_sync.hosted_chat.send_yields_assistant_reply.v1",
      "khala_sync.credit_balance.change_delivers_live.v1",
    ])
    for (const contract of khalaSyncContractRegistry.contracts.filter(
      contract => contract.state === "enforced",
    )) {
      expect(contract).toMatchObject({
        blockerRefs: [],
        enforcementTier: "test-sweep",
        state: "enforced",
      })
      expect(contract.oracles.length).toBeGreaterThan(0)
    }
    // The ST-5 seam contract (#8511) stays pending until ST-1's live-seam
    // smoke (#8507) lands on main; it must name both sides of the seam and
    // carry the blocker that gates its flip to enforced.
    const seamContract = khalaSyncContractRegistry.contracts.find(
      contract =>
        contract.contractId === "khala_sync.seam.bearer_ws_connect_reaches_live.v1",
    )
    expect(seamContract).toMatchObject({
      blockerRefs: ["blocker.khala_sync.depends_on_8507_live_seam_smoke_landing"],
      enforcementTier: "unenforced",
      seam: {
        client: "packages/khala-sync-client/src/transport.ts",
        server: "apps/openagents.com/workers/api/src/khala-sync-connect-routes.ts",
      },
      state: "pending",
    })
    // The push statement is the SPEC §2.4 acceptance rule, kept verbatim.
    expect(khalaSyncContractRegistry.contracts[0]!.statement).toBe(
      "Acceptance is synchronous with the transaction; validation failures ack the mutation and report the error in-band — they never 4xx/block the queue.",
    )
    // The KS-9.2 synced-surface contracts (#8311) keep the owner's stated
    // expectations verbatim at their heads.
    expect(khalaSyncContractRegistry.contracts[1]!.statement).toStartWith(
      "Offline pushes queue honestly:",
    )
    expect(khalaSyncContractRegistry.contracts[2]!.statement).toStartWith(
      "Synced staleness is never fabricated:",
    )
    expect(khalaSyncContractRegistry.contracts[3]!.statement).toStartWith(
      "Access revocation clears synced state:",
    )
    // The staleness contract references the desktop indicator contract
    // (KS-6.2, #8303) instead of duplicating its DOM oracle.
    expect(khalaSyncContractRegistry.contracts[2]!.evidenceRefs).toContain(
      "contract:khala_code.fleet.khala_sync_indicator_truthful.v1",
    )
  })

  test("khala sync oracle coverage links against the REAL push-engine test source on disk", async () => {
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(khalaSyncContractRegistry).pipe(
        Effect.provide(
          inMemoryOracleSourceLayer(
            Object.fromEntries(
              await Promise.all(
                khalaSyncContractRegistry.contracts
                  .flatMap(contract => contract.oracles)
                  .map(async oracle => {
                    const file = Bun.file(repoPath(oracle.ref))
                    return [
                      oracle.ref,
                      (await file.exists()) ? await file.text() : "",
                    ] as const
                  }),
              ),
            ),
          ),
        ),
      ),
    )
    // Coverage must be proven against the actual committed test files: each
    // exists AND references the owning contractId, so the contract cannot
    // silently drift away from the sweep that claims to enforce it.
    expect(report.results.map(result => result.status)).toEqual(
      Array.from({ length: report.results.length }, () => "covered"),
    )
    expect(report.results.length).toBeGreaterThanOrEqual(5)
    expect(report.ok).toBe(true)
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

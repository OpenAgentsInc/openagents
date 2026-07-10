import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import type { BootstrapSummary } from "../src/bootstrap.js"
import { hashPylonAccountRef, type PylonAccountRegistryEntry } from "../src/account-registry.js"
import type { PylonAccountUsageStore } from "../src/account-usage.js"
import { classifyPylonDispatchFailure } from "../src/dispatch-failure-taxonomy.js"
import {
  PYLON_FLEET_CAPACITY_DIAGNOSTIC_SCHEMA,
  createPylonOwnedFleetRunSupervisorCapacity,
} from "../src/orchestration/fleet-run-owned-capacity.js"
import {
  createPylonOrchestrationStore,
  type FleetRun,
  type PylonOrchestrationStore,
} from "../src/orchestration/store.js"

const fixedNow = new Date("2026-07-09T16:00:00.000Z")
const summary = {
  paths: {
    config: "/private/pylon/config.json",
    home: "/private/pylon",
  },
} as unknown as Pick<BootstrapSummary, "paths">

const account = (
  ref: string,
  provider: "claude_agent" | "codex" | "grok",
  input: { readonly home?: string; readonly paused?: boolean; readonly marginalCostClass?: PylonAccountRegistryEntry["marginalCostClass"] } = {},
): PylonAccountRegistryEntry => ({
  ref,
  provider,
  home: input.home ?? `/private/accounts/${provider}/${ref}`,
  ...(input.paused === true ? { paused: true } : {}),
  openAgentsProviderAccountRef: null,
  hourlyCap: null,
  weeklyCap: null,
  manualResetsRemaining: null,
  marginalCostClass: input.marginalCostClass ?? "not_measured",
})

const emptyUsage = (): PylonAccountUsageStore => ({
  schema: "openagents.pylon.account_usage_store.v0.3",
  accounts: {},
  updatedAt: fixedNow.toISOString(),
})

const exhaustedUsage = (entry: PylonAccountRegistryEntry): PylonAccountUsageStore => {
  const accountRefHash = hashPylonAccountRef(entry.provider, entry.ref)
  return {
    ...emptyUsage(),
    accounts: {
      [accountRefHash]: {
        provider: entry.provider,
        selector: "registry_ref",
        accountRefHash,
        providerTruth: {
          observedAt: fixedNow.toISOString(),
          snapshots: [{
            provider: entry.provider,
            limitId: "weekly",
            limitName: "weekly",
            primary: {
              usedPercent: 100,
              remainingPercent: 0,
              windowMinutes: 10_080,
              resetsAt: null,
              label: "weekly",
            },
            secondary: null,
            credits: null,
            planType: null,
            rateLimitReachedType: null,
          }],
        },
        localSessionTruth: null,
        updatedAt: fixedNow.toISOString(),
      },
    },
  }
}

const createRun = (
  store: PylonOrchestrationStore,
  runRef: string,
  workerKind: "auto" | "claude" | "codex" | "grok" = "auto",
): FleetRun => store.createFleetRun({
  runRef,
  objective: "Measure named owner-local fleet capacity.",
  workSource: "fixture",
  targetConcurrency: 4,
  workerKind,
  state: "running",
  now: fixedNow,
})

const capacityFixture = (input: {
  readonly registry: readonly PylonAccountRegistryEntry[]
  readonly store?: PylonOrchestrationStore
  readonly usage?: PylonAccountUsageStore
  readonly readiness?: (account: PylonAccountRegistryEntry) => string | Promise<string>
  readonly slots?: (account: PylonAccountRegistryEntry) => number | null
  readonly grokExecutionAvailable?: boolean
}) => {
  const store = input.store ?? createPylonOrchestrationStore(new Database(":memory:"))
  const capacity = createPylonOwnedFleetRunSupervisorCapacity({
    store,
    summary,
    defaultHomes: {
      codex: "/private/default/.codex",
      claudeAgent: "/private/default/.claude",
      grok: "/private/default/.grok",
    },
    grokExecutionAvailable: input.grokExecutionAvailable,
    loadRegistry: async () => input.registry,
    loadUsage: async () => input.usage ?? emptyUsage(),
    probeReadiness: async ({ account }) => await (input.readiness?.(account) ?? "ready"),
    advertisedSlotsForAccount: input.slots ?? (() => 1),
  })
  return { capacity, store }
}

describe("Pylon-owned FleetRun account capacity", () => {
  test("preserves mixed named Codex and Claude slots and data-driven cost classes", async () => {
    const codex = account("codex-owner", "codex", { marginalCostClass: "subscription" })
    const claude = account("claude-owner", "claude_agent")
    const { capacity, store } = capacityFixture({
      registry: [codex, claude],
      slots: (entry) => entry.provider === "codex" ? 2 : 1,
    })
    const run = createRun(store, "fleet_run.capacity.mixed")

    expect(await capacity.accounts({ run, now: fixedNow })).toEqual([
      {
        accountRef: "codex-owner",
        advertisedCapacity: 2,
        marginalCostClass: "subscription",
        workerKind: "codex",
      },
      {
        accountRef: "claude-owner",
        advertisedCapacity: 1,
        marginalCostClass: "not_measured",
        workerKind: "claude",
      },
    ])
    expect(capacity.diagnostics()).toEqual([])
  })

  test("reports named ready Grok custody but admits zero work until its executor is composed", async () => {
    let registryReads = 0
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const capacity = createPylonOwnedFleetRunSupervisorCapacity({
      store,
      summary,
      defaultHomes: {
        codex: "/private/default/.codex",
        claudeAgent: "/private/default/.claude",
        grok: "/private/default/.grok",
      },
      loadRegistry: async () => {
        registryReads += 1
        return [
          account("grok-owner", "grok"),
          account("codex-must-not-substitute", "codex"),
        ]
      },
      loadUsage: async () => emptyUsage(),
      probeReadiness: async () => "ready",
      advertisedSlotsForAccount: () => 3,
    })
    const run = createRun(store, "fleet_run.capacity.grok_unavailable", "grok")

    expect(await capacity.accounts({ run, now: fixedNow })).toEqual([{
      accountRef: "grok-owner",
      advertisedCapacity: 0,
      marginalCostClass: "not_measured",
      unavailabilityReason: "account_unavailable",
      workerKind: "grok",
    }])
    expect(registryReads).toBe(1)
    expect(capacity.diagnostics()).toEqual([{
      schema: PYLON_FLEET_CAPACITY_DIAGNOSTIC_SCHEMA,
      kind: "grok_executor_unavailable",
      blockerRefs: ["blocker.pylon.fleet_capacity.grok_executor_unavailable"],
    }])
  })

  test("maps exact bounded Grok slots only through the explicit executor gate", async () => {
    const grok = account("grok-owner", "grok", {
      marginalCostClass: "api_metered",
    })
    const { capacity, store } = capacityFixture({
      registry: [grok],
      grokExecutionAvailable: true,
      slots: () => 65_000,
    })
    const run = createRun(store, "fleet_run.capacity.grok_composed", "grok")

    expect(await capacity.accounts({ run, now: fixedNow })).toEqual([{
      accountRef: "grok-owner",
      advertisedCapacity: 64,
      marginalCostClass: "api_metered",
      workerKind: "grok",
    }])
    expect(capacity.diagnostics()).toEqual([])
  })

  test("excludes a default Grok home and never probes it", async () => {
    const defaultGrok = account("grok-default", "grok", {
      home: "/private/default/.grok",
    })
    let probes = 0
    const { capacity, store } = capacityFixture({
      registry: [defaultGrok],
      grokExecutionAvailable: true,
      readiness: () => {
        probes += 1
        return "ready"
      },
    })
    const run = createRun(store, "fleet_run.capacity.grok_default", "grok")

    expect(await capacity.accounts({ run, now: fixedNow })).toEqual([])
    expect(probes).toBe(0)
  })

  test("subtracts external durable load while leaving current-run subtraction to the supervisor", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const runA = createRun(store, "fleet_run.capacity.shared_a", "codex")
    const runB = createRun(store, "fleet_run.capacity.shared_b", "codex")
    const claim = store.tryClaimWorkUnit({
      claimRef: "claim.capacity.shared_a",
      workUnitRef: "fixture:shared-a",
      runRef: runA.runRef,
      workerAccountRef: "codex-shared",
      ttl: 60_000,
      now: fixedNow,
    })
    if (claim === null) throw new Error("expected shared capacity claim")
    store.updateWorkClaimState(claim.claimRef, "in_progress", fixedNow)
    const { capacity } = capacityFixture({
      registry: [account("codex-shared", "codex")],
      slots: () => 2,
      store,
    })

    expect(await capacity.accounts({ run: runA, now: fixedNow })).toEqual([
      expect.objectContaining({ accountRef: "codex-shared", advertisedCapacity: 2 }),
    ])
    expect(await capacity.accounts({ run: runB, now: fixedNow })).toEqual([
      expect.objectContaining({ accountRef: "codex-shared", advertisedCapacity: 1 }),
    ])
  })

  test("serializes account inspection across concurrent capacity reads", async () => {
    let active = 0
    let maximumActive = 0
    const { capacity, store } = capacityFixture({
      registry: [account("codex-a", "codex"), account("codex-b", "codex")],
      readiness: async () => {
        active += 1
        maximumActive = Math.max(maximumActive, active)
        await Bun.sleep(5)
        active -= 1
        return "ready"
      },
    })
    const run = createRun(store, "fleet_run.capacity.serialized", "codex")

    await Promise.all([
      capacity.accounts({ run, now: fixedNow }),
      capacity.accounts({ run, now: fixedNow }),
    ])
    expect(maximumActive).toBe(1)
  })

  test("rotates around paused, revoked, exhausted, rate-limited, and circuit-open accounts", async () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const healthy = account("codex-healthy", "codex")
    const paused = account("codex-paused", "codex", { paused: true })
    const revoked = account("codex-revoked", "codex")
    const limited = account("codex-rate-limited", "codex")
    const exhausted = account("codex-exhausted", "codex")
    const circuit = account("codex-circuit", "codex")
    store.recordDispatchBreakerFailure({
      accountRefHash: hashPylonAccountRef("codex", circuit.ref),
      classification: classifyPylonDispatchFailure({ error: "rate limited" }),
      lane: "codex",
      now: fixedNow,
    })
    const { capacity } = capacityFixture({
      registry: [paused, revoked, limited, exhausted, circuit, healthy],
      readiness: (entry) =>
        entry === revoked ? "credentials_revoked" :
        entry === limited ? "rate_limited" :
        "ready",
      usage: exhaustedUsage(exhausted),
      store,
    })
    const run = createRun(store, "fleet_run.capacity.health_rotation", "codex")

    expect(await capacity.accounts({ run, now: fixedNow })).toEqual([
      {
        accountRef: "codex-paused",
        advertisedCapacity: 0,
        marginalCostClass: "not_measured",
        unavailabilityReason: "account_unavailable",
        workerKind: "codex",
      },
      {
        accountRef: "codex-revoked",
        advertisedCapacity: 0,
        marginalCostClass: "not_measured",
        unavailabilityReason: "account_requires_reauth",
        workerKind: "codex",
      },
      {
        accountRef: "codex-rate-limited",
        advertisedCapacity: 0,
        marginalCostClass: "not_measured",
        unavailabilityReason: "account_rate_limited",
        workerKind: "codex",
      },
      {
        accountRef: "codex-exhausted",
        advertisedCapacity: 0,
        marginalCostClass: "not_measured",
        unavailabilityReason: "account_exhausted",
        workerKind: "codex",
      },
      {
        accountRef: "codex-circuit",
        advertisedCapacity: 0,
        marginalCostClass: "not_measured",
        unavailabilityReason: "account_rate_limited",
        workerKind: "codex",
      },
      {
        accountRef: "codex-healthy",
        advertisedCapacity: 1,
        marginalCostClass: "not_measured",
        workerKind: "codex",
      },
    ])
  })

  test("retains a Grok rate-limit breaker across restart and rotates to another named Grok account", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-grok-breaker-restart-"))
    const databasePath = join(root, "orchestration.sqlite")
    const throttled = account("grok-throttled", "grok")
    const healthy = account("grok-healthy", "grok")
    try {
      const firstDatabase = new Database(databasePath, { create: true })
      const firstStore = createPylonOrchestrationStore(firstDatabase)
      firstStore.recordDispatchBreakerFailure({
        accountRefHash: hashPylonAccountRef("grok", throttled.ref),
        classification: classifyPylonDispatchFailure({
          blockerRefs: ["blocker.pylon.fleet_runner.grok_account_rate_limited"],
        }),
        lane: "grok",
        now: fixedNow,
      })
      firstDatabase.close()

      const reopenedDatabase = new Database(databasePath)
      const reopenedStore = createPylonOrchestrationStore(reopenedDatabase)
      const { capacity } = capacityFixture({
        registry: [throttled, healthy],
        grokExecutionAvailable: true,
        store: reopenedStore,
      })
      const run = createRun(
        reopenedStore,
        "fleet_run.capacity.grok_breaker_restart",
        "grok",
      )
      try {
        expect(await capacity.accounts({ run, now: fixedNow })).toEqual([
          {
            accountRef: "grok-throttled",
            advertisedCapacity: 0,
            marginalCostClass: "not_measured",
            unavailabilityReason: "account_rate_limited",
            workerKind: "grok",
          },
          {
            accountRef: "grok-healthy",
            advertisedCapacity: 1,
            marginalCostClass: "not_measured",
            workerKind: "grok",
          },
        ])
      } finally {
        reopenedDatabase.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("fails unknown capacity to zero and excludes default homes without projecting private data", async () => {
    const defaultCodex = account("default", "codex", { home: "/private/default/.codex" })
    const named = account("codex-named", "codex", { home: "/private/isolated/codex-named" })
    const { capacity, store } = capacityFixture({
      registry: [defaultCodex, named],
      slots: () => null,
    })
    const run = createRun(store, "fleet_run.capacity.default_exclusion", "codex")
    const result = await capacity.accounts({ run, now: fixedNow })
    const encoded = JSON.stringify(result)

    expect(result).toEqual([{
      accountRef: "codex-named",
      advertisedCapacity: 0,
      marginalCostClass: "not_measured",
      unavailabilityReason: "account_unavailable",
      workerKind: "codex",
    }])
    expect(encoded).not.toMatch(/private|home|credential|token|auth\.json/i)
  })

  test("fails a corrupt readiness dependency closed with fixed public diagnostics", async () => {
    const diagnostics: unknown[] = []
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    const capacity = createPylonOwnedFleetRunSupervisorCapacity({
      store,
      summary,
      loadRegistry: async () => [account("codex-private-failure", "codex")],
      loadUsage: async () => emptyUsage(),
      probeReadiness: async () => {
        throw new Error("raw readiness failure /private/accounts/codex/auth.json token-secret")
      },
      onDiagnostic: (entry) => {
        diagnostics.push(entry)
        throw new Error("diagnostic sink /private/output must not own capacity")
      },
    })
    const run = createRun(store, "fleet_run.capacity.corrupt_readiness", "codex")

    expect(await capacity.accounts({ run, now: fixedNow })).toEqual([{
      accountRef: "codex-private-failure",
      advertisedCapacity: 0,
      marginalCostClass: "not_measured",
      unavailabilityReason: "account_unavailable",
      workerKind: "codex",
    }])
    expect(diagnostics).toEqual([{
      schema: PYLON_FLEET_CAPACITY_DIAGNOSTIC_SCHEMA,
      kind: "account_inspection_unavailable",
      blockerRefs: ["blocker.pylon.fleet_capacity.account_inspection_unavailable"],
    }])
    expect(JSON.stringify(capacity.diagnostics())).not.toMatch(/private|auth|token|raw readiness/i)
  })

  test("omits every duplicate bare account ref across providers with a fixed diagnostic", async () => {
    const { capacity, store } = capacityFixture({
      registry: [
        account("shared-ref", "codex"),
        account("shared-ref", "claude_agent"),
      ],
    })
    const run = createRun(store, "fleet_run.capacity.duplicate_ref")

    expect(await capacity.accounts({ run, now: fixedNow })).toEqual([])
    expect(capacity.diagnostics()).toEqual([
      expect.objectContaining({ kind: "duplicate_account_ref" }),
    ])
  })

  test("fails a throwing per-account slot source closed without failing the inventory read", async () => {
    const healthy = account("codex-healthy-slot", "codex")
    const broken = account("codex-broken-slot", "codex")
    const { capacity, store } = capacityFixture({
      registry: [broken, healthy],
      slots: (entry) => {
        if (entry === broken) throw new Error("private slot source /private/capacity")
        return 1
      },
    })
    const run = createRun(store, "fleet_run.capacity.slot_failure", "codex")

    expect(await capacity.accounts({ run, now: fixedNow })).toEqual([
      {
        accountRef: broken.ref,
        advertisedCapacity: 0,
        marginalCostClass: "not_measured",
        unavailabilityReason: "account_unavailable",
        workerKind: "codex",
      },
      expect.objectContaining({ accountRef: healthy.ref, advertisedCapacity: 1 }),
    ])
    expect(JSON.stringify(capacity.diagnostics())).not.toContain("/private/capacity")
    expect(capacity.diagnostics()).toEqual([
      expect.objectContaining({ kind: "account_inspection_unavailable" }),
    ])
  })

  test("strict default loaders distinguish malformed registry and usage state without leaking errors", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-capacity-strict-"))
    try {
      const strictSummary = {
        paths: {
          config: join(root, "config.json"),
          home: root,
        },
      } as unknown as Pick<BootstrapSummary, "paths">
      const store = createPylonOrchestrationStore(new Database(":memory:"))
      const run = createRun(store, "fleet_run.capacity.strict_loaders", "codex")
      await writeFile(strictSummary.paths.config, "{ malformed private registry")
      const malformedRegistry = createPylonOwnedFleetRunSupervisorCapacity({ store, summary: strictSummary })

      expect(await malformedRegistry.accounts({ run, now: fixedNow })).toEqual([])
      expect(malformedRegistry.diagnostics()).toEqual([
        expect.objectContaining({ kind: "account_registry_unavailable" }),
      ])

      await writeFile(strictSummary.paths.config, JSON.stringify({ dev: { accounts: [] } }))
      await writeFile(join(root, "account-usage.json"), "{ malformed private usage")
      const malformedUsage = createPylonOwnedFleetRunSupervisorCapacity({ store, summary: strictSummary })

      expect(await malformedUsage.accounts({ run, now: fixedNow })).toEqual([])
      expect(malformedUsage.diagnostics()).toEqual([
        expect.objectContaining({ kind: "account_usage_unavailable" }),
      ])

      const named = account("codex-malformed-credits", "codex", { home: join(root, "codex-malformed-credits") })
      await writeFile(strictSummary.paths.config, JSON.stringify({ dev: { accounts: [named] } }))
      await writeFile(join(root, "account-usage.json"), JSON.stringify({
        ...emptyUsage(),
        accounts: {
          [hashPylonAccountRef("codex", named.ref)]: {
            provider: "codex",
            selector: "registry_ref",
            accountRefHash: hashPylonAccountRef("codex", named.ref),
            providerTruth: {
              observedAt: fixedNow.toISOString(),
              snapshots: [{
                provider: "codex",
                limitId: "codex",
                limitName: null,
                primary: null,
                secondary: null,
                credits: { hasCredits: "private-unknown", unlimited: false, balance: null },
                planType: null,
                rateLimitReachedType: null,
              }],
            },
            localSessionTruth: null,
            updatedAt: fixedNow.toISOString(),
          },
        },
      }))
      const malformedCredits = createPylonOwnedFleetRunSupervisorCapacity({ store, summary: strictSummary })

      expect(await malformedCredits.accounts({ run, now: fixedNow })).toEqual([])
      expect(malformedCredits.diagnostics()).toEqual([
        expect.objectContaining({ kind: "account_usage_unavailable" }),
      ])
      expect(JSON.stringify([
        ...malformedRegistry.diagnostics(),
        ...malformedUsage.diagnostics(),
        ...malformedCredits.diagnostics(),
      ])).not.toMatch(/private|registry path|usage path|malformed private/i)
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("default readiness inspection is local-only and does not refresh provider usage", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-capacity-local-readiness-"))
    try {
      const accountHome = join(root, "accounts", "codex", "codex-local")
      await mkdir(accountHome, { recursive: true })
      await writeFile(join(accountHome, "auth.json"), "{}")
      const localSummary = {
        paths: {
          config: join(root, "config.json"),
          home: root,
        },
      } as unknown as Pick<BootstrapSummary, "paths">
      await writeFile(localSummary.paths.config, JSON.stringify({
        dev: {
          accounts: [{
            ref: "codex-local",
            provider: "codex",
            home: accountHome,
          }],
        },
      }))
      const store = createPylonOrchestrationStore(new Database(":memory:"))
      const run = createRun(store, "fleet_run.capacity.local_readiness", "codex")
      const capacity = createPylonOwnedFleetRunSupervisorCapacity({
        store,
        summary: localSummary,
        advertisedSlotsForAccount: () => 1,
      })

      expect(await capacity.accounts({ run, now: fixedNow })).toEqual([
        expect.objectContaining({ accountRef: "codex-local", advertisedCapacity: 1 }),
      ])
      expect(await Bun.file(join(root, "account-usage.json")).exists()).toBe(false)
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("reconstructs external busy load after reopening orchestration.sqlite", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-capacity-restart-"))
    const databasePath = join(root, "orchestration.sqlite")
    try {
      const firstDatabase = new Database(databasePath)
      const firstStore = createPylonOrchestrationStore(firstDatabase)
      const ownerRun = createRun(firstStore, "fleet_run.capacity.restart_owner", "codex")
      const claim = firstStore.tryClaimWorkUnit({
        claimRef: "claim.capacity.restart_owner",
        workUnitRef: "fixture:restart-owner",
        runRef: ownerRun.runRef,
        workerAccountRef: "codex-restart",
        ttl: 60_000,
        now: fixedNow,
      })
      if (claim === null) throw new Error("expected restart claim")
      firstStore.updateWorkClaimState(claim.claimRef, "in_progress", fixedNow)
      firstDatabase.close()

      const reopenedDatabase = new Database(databasePath)
      const reopenedStore = createPylonOrchestrationStore(reopenedDatabase)
      const waitingRun = createRun(reopenedStore, "fleet_run.capacity.restart_waiter", "codex")
      const { capacity } = capacityFixture({
        registry: [account("codex-restart", "codex")],
        slots: () => 1,
        store: reopenedStore,
      })

      expect(await capacity.accounts({ run: waitingRun, now: fixedNow })).toEqual([
        expect.objectContaining({ accountRef: "codex-restart", advertisedCapacity: 0 }),
      ])
      reopenedDatabase.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})

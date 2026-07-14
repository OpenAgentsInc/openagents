import { Runtime } from "@openagentsinc/runtime-platform"
import { readFile } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"

import {
  hashPylonAccountRef,
  isDefaultGrokAccountHome,
  loadPylonAccountRegistryEffect,
  normalizeAccountHome,
  PylonAccountRegistryError,
  type PylonAccountProvider,
  type PylonAccountRegistryEntry,
} from "../account-registry.js"
import {
  PYLON_ACCOUNT_USAGE_STORE_SCHEMA,
  readinessForTarget,
  type AccountDiscoveryTarget,
  type PylonAccountUsageStore,
  type PylonAccountUsageStoreEntry,
} from "../account-usage.js"
import {
  claudePerAccountConcurrency,
  codexPerAccountConcurrency,
  grokPerAccountConcurrency,
  MAX_GROK_PER_ACCOUNT_CONCURRENCY,
} from "../presence.js"
import { assertPublicProjectionSafe } from "../state.js"
import type { BootstrapSummary } from "../bootstrap.js"
import {
  mapPylonFleetSupervisorCapacity,
  type PublicSafePylonFleetCapacityAccount,
} from "./fleet-run-capacity.js"
import type {
  FleetRunSupervisorCapacity,
  FleetRunSupervisorConcreteWorkerKind,
} from "./fleet-run-supervisor.js"
import type { FleetRun, PylonOrchestrationStore } from "./store.js"

export const PYLON_FLEET_CAPACITY_DIAGNOSTIC_SCHEMA =
  "openagents.pylon.fleet_capacity_diagnostic.v0.1" as const

export type PylonFleetCapacityDiagnosticKind =
  | "account_inspection_unavailable"
  | "account_registry_unavailable"
  | "account_usage_unavailable"
  | "duplicate_account_ref"
  | "grok_account_inspection_unavailable"
  | "grok_executor_unavailable"

export type PylonFleetCapacityDiagnostic = {
  readonly schema: typeof PYLON_FLEET_CAPACITY_DIAGNOSTIC_SCHEMA
  readonly kind: PylonFleetCapacityDiagnosticKind
  readonly blockerRefs: readonly string[]
}

export type PylonOwnedFleetRunSupervisorCapacity = FleetRunSupervisorCapacity & {
  readonly diagnostics: () => readonly PylonFleetCapacityDiagnostic[]
}

type PylonFleetAccountReadinessProbe = (input: {
  readonly account: PylonAccountRegistryEntry
  readonly env: Record<string, string | undefined>
  readonly now: Date
  readonly summary: Pick<BootstrapSummary, "paths">
}) => Promise<string>

export type CreatePylonOwnedFleetRunSupervisorCapacityInput = {
  readonly store: PylonOrchestrationStore
  readonly summary: Pick<BootstrapSummary, "paths">
  readonly env?: Record<string, string | undefined> | undefined
  readonly defaultHomes?: {
    readonly claudeAgent: string
    readonly codex: string
    readonly grok?: string | undefined
  } | undefined
  readonly grokExecutionAvailable?: boolean | undefined
  readonly advertisedSlotsForAccount?: ((account: PylonAccountRegistryEntry) => number | null) | undefined
  readonly loadRegistry?: (() => Promise<readonly PylonAccountRegistryEntry[]>) | undefined
  readonly loadUsage?: (() => Promise<PylonAccountUsageStore>) | undefined
  readonly probeReadiness?: PylonFleetAccountReadinessProbe | undefined
  readonly onDiagnostic?: ((diagnostic: PylonFleetCapacityDiagnostic) => void | Promise<void>) | undefined
}

const diagnosticBlockerRefs: Readonly<Record<PylonFleetCapacityDiagnosticKind, string>> = {
  account_inspection_unavailable: "blocker.pylon.fleet_capacity.account_inspection_unavailable",
  account_registry_unavailable: "blocker.pylon.fleet_capacity.account_registry_unavailable",
  account_usage_unavailable: "blocker.pylon.fleet_capacity.account_usage_unavailable",
  duplicate_account_ref: "blocker.pylon.fleet_capacity.duplicate_account_ref",
  grok_account_inspection_unavailable: "blocker.pylon.fleet_capacity.grok_account_inspection_unavailable",
  grok_executor_unavailable: "blocker.pylon.fleet_capacity.grok_executor_unavailable",
}

const diagnostic = (kind: PylonFleetCapacityDiagnosticKind): PylonFleetCapacityDiagnostic => {
  const value = {
    schema: PYLON_FLEET_CAPACITY_DIAGNOSTIC_SCHEMA,
    kind,
    blockerRefs: [diagnosticBlockerRefs[kind]],
  } satisfies PylonFleetCapacityDiagnostic
  assertPublicProjectionSafe(value, "pylonFleetCapacityDiagnostic")
  return value
}

const accountTarget = (
  account: PylonAccountRegistryEntry,
  accountRefHash: string,
): AccountDiscoveryTarget => ({
  provider: account.provider,
  selector: "registry_ref",
  accountRef: account.ref,
  accountRefHash,
  home: account.home,
  homeRef: accountRefHash,
  account: {
    provider: account.provider,
    selector: "registry_ref",
    accountRef: account.ref,
    accountRefHash,
    home: account.home,
    openAgentsProviderAccountRef: account.openAgentsProviderAccountRef,
  },
})

const defaultReadinessProbe: PylonFleetAccountReadinessProbe = async (input) => {
  const accountRefHash = hashPylonAccountRef(input.account.provider, input.account.ref)
  // `readinessForTarget` is a local-only check: isolated credential-file
  // presence, SDK/config readiness, and persisted health/quota ledgers. It
  // never performs the spendful `accounts usage --refresh` provider call.
  return (
    await readinessForTarget(
      input.summary,
      accountTarget(input.account, accountRefHash),
      input.env,
    )
  ).readiness.state
}

const emptyUsageStore = (): PylonAccountUsageStore => ({
  schema: PYLON_ACCOUNT_USAGE_STORE_SCHEMA,
  accounts: {},
  updatedAt: "1970-01-01T00:00:00.000Z",
})

const usageWindowIsValid = (value: unknown): boolean => {
  if (value === null) return true
  if (typeof value !== "object" || Array.isArray(value)) return false
  const window = value as Record<string, unknown>
  return typeof window.usedPercent === "number" && Number.isFinite(window.usedPercent) &&
    typeof window.remainingPercent === "number" && Number.isFinite(window.remainingPercent)
}

const usageCreditsAreValid = (value: unknown): boolean => {
  if (value === null) return true
  if (typeof value !== "object" || Array.isArray(value)) return false
  const credits = value as Record<string, unknown>
  return typeof credits.hasCredits === "boolean" &&
    typeof credits.unlimited === "boolean" &&
    (credits.balance === null || typeof credits.balance === "string")
}

const usageStoreIsValid = (value: unknown): value is PylonAccountUsageStore => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false
  const store = value as Record<string, unknown>
  if (store.schema !== PYLON_ACCOUNT_USAGE_STORE_SCHEMA || typeof store.updatedAt !== "string") return false
  if (store.accounts === null || typeof store.accounts !== "object" || Array.isArray(store.accounts)) return false
  return Object.entries(store.accounts as Record<string, unknown>).every(([accountRefHash, value]) => {
    if (value === null || typeof value !== "object" || Array.isArray(value)) return false
    const entry = value as Record<string, unknown>
    if (entry.accountRefHash !== accountRefHash) return false
    if (entry.provider !== "codex" && entry.provider !== "claude_agent") return false
    if (entry.providerTruth === null) return true
    if (typeof entry.providerTruth !== "object" || Array.isArray(entry.providerTruth)) {
      return false
    }
    const snapshots = (entry.providerTruth as Record<string, unknown>).snapshots
    if (!Array.isArray(snapshots)) return false
    return snapshots.every((snapshot) => {
      if (snapshot === null || typeof snapshot !== "object" || Array.isArray(snapshot)) return false
      const source = snapshot as Record<string, unknown>
      return (source.provider === "codex" || source.provider === "claude_agent") &&
        (source.rateLimitReachedType === null || typeof source.rateLimitReachedType === "string") &&
        usageCreditsAreValid(source.credits) &&
        usageWindowIsValid(source.primary) &&
        usageWindowIsValid(source.secondary)
    })
  })
}

const strictLoadUsageStore = async (
  summary: Pick<BootstrapSummary, "paths">,
): Promise<PylonAccountUsageStore> => {
  let raw: string
  try {
    raw = await readFile(join(summary.paths.home, "account-usage.json"), "utf8")
  } catch (error) {
    if (error !== null && typeof error === "object" && (error as { code?: unknown }).code === "ENOENT") {
      return emptyUsageStore()
    }
    throw error
  }
  const parsed = JSON.parse(raw) as unknown
  if (!usageStoreIsValid(parsed)) throw new Error("invalid Pylon account usage store")
  return parsed
}

const strictLoadRegistry = async (
  summary: Pick<BootstrapSummary, "paths">,
): Promise<readonly PylonAccountRegistryEntry[]> => {
  try {
    return await Effect.runPromise(loadPylonAccountRegistryEffect(summary))
  } catch (error) {
    if (error instanceof PylonAccountRegistryError && error.kind === "not_found") return []
    throw error
  }
}

const usageCapacityState = (
  entry: PylonAccountUsageStoreEntry | undefined,
): "rate_limited" | "usage_limited" | null => {
  let exhausted = false
  for (const snapshot of entry?.providerTruth?.snapshots ?? []) {
    if (snapshot.rateLimitReachedType !== null && snapshot.rateLimitReachedType.trim() !== "") {
      return "rate_limited"
    }
    if (snapshot.credits !== null && !snapshot.credits.unlimited && !snapshot.credits.hasCredits) {
      exhausted = true
    }
    if ([snapshot.primary, snapshot.secondary].some((window) =>
      window !== null && (window.usedPercent >= 100 || window.remainingPercent <= 0)
    )) exhausted = true
  }
  return exhausted ? "usage_limited" : null
}

const laneForProvider = (
  provider: PylonAccountProvider,
): "claude_agent" | "codex" | "grok" =>
  provider === "claude_agent"
    ? "claude_agent"
    : provider === "grok"
      ? "grok"
      : "codex"

const workerKindForProvider = (provider: PylonAccountProvider): FleetRunSupervisorConcreteWorkerKind =>
  provider === "claude_agent"
    ? "claude"
    : provider === "grok"
      ? "grok"
      : "codex"

const runAcceptsProvider = (run: FleetRun, provider: PylonAccountProvider): boolean =>
  run.workerKind === "auto" || workerKindForProvider(provider) === run.workerKind

const boundedSlots = (
  value: number | null,
  provider: PylonAccountProvider,
): number | null =>
  value === null || !Number.isFinite(value)
    ? null
    : Math.max(
        0,
        Math.min(
          provider === "grok" ? MAX_GROK_PER_ACCOUNT_CONCURRENCY : 10_000,
          Math.trunc(value),
        ),
      )

/**
 * Connected-account capacity for the standing Pylon FleetRun supervisor.
 *
 * Registry homes and credential evidence stay local. Current-run claims are
 * deliberately left for `fleet-run-supervisor` to subtract; this adapter
 * subtracts durable busy claims from every other run so capacity is composed
 * once rather than double-counted after restart.
 */
export function createPylonOwnedFleetRunSupervisorCapacity(
  input: CreatePylonOwnedFleetRunSupervisorCapacityInput,
): PylonOwnedFleetRunSupervisorCapacity {
  const env = input.env ?? (Runtime.env as Record<string, string | undefined>)
  const defaultHomes = {
    codex: normalizeAccountHome(input.defaultHomes?.codex ?? join(homedir(), ".codex")),
    claudeAgent: normalizeAccountHome(input.defaultHomes?.claudeAgent ?? join(homedir(), ".claude")),
    grok: normalizeAccountHome(
      input.defaultHomes?.grok ?? env.GROK_HOME ?? join(homedir(), ".grok"),
    ),
  }
  const grokExecutionAvailable = input.grokExecutionAvailable ?? false
  const loadRegistry = input.loadRegistry ?? (() => strictLoadRegistry(input.summary))
  const loadUsage = input.loadUsage ?? (() => strictLoadUsageStore(input.summary))
  const probeReadiness = input.probeReadiness ?? defaultReadinessProbe
  const advertisedSlotsForAccount = input.advertisedSlotsForAccount ?? ((account) => {
    if (account.provider === "claude_agent") {
      return claudePerAccountConcurrency(env as NodeJS.ProcessEnv)
    }
    if (account.provider === "grok") {
      return grokPerAccountConcurrency(env as NodeJS.ProcessEnv)
    }
    return codexPerAccountConcurrency(env as NodeJS.ProcessEnv)
  })
  let latestDiagnostics: readonly PylonFleetCapacityDiagnostic[] = []
  let serialized = Promise.resolve()

  const accounts: FleetRunSupervisorCapacity["accounts"] = (request) => {
    const operation = async () => {
      const diagnostics: PylonFleetCapacityDiagnostic[] = []
      const addDiagnostic = async (kind: PylonFleetCapacityDiagnosticKind) => {
        if (diagnostics.some((entry) => entry.kind === kind)) return
        const entry = diagnostic(kind)
        diagnostics.push(entry)
        if (input.onDiagnostic !== undefined) {
          try {
            await input.onDiagnostic(entry)
          } catch {
            // Diagnostic observers are telemetry only and never capacity authority.
          }
        }
      }

      let registry: readonly PylonAccountRegistryEntry[]
      try {
        registry = await loadRegistry()
      } catch {
        await addDiagnostic("account_registry_unavailable")
        latestDiagnostics = diagnostics
        return []
      }

      let usage: PylonAccountUsageStore
      try {
        usage = await loadUsage()
      } catch {
        await addDiagnostic("account_usage_unavailable")
        latestDiagnostics = diagnostics
        return []
      }

      const activeBreakers = input.store.listActiveDispatchBreakers(request.now)
      const duplicateAccountRefs = new Set(
        registry
          .map((account) => account.ref)
          .filter((ref, index, refs) => refs.indexOf(ref) !== index),
      )
      if (duplicateAccountRefs.size > 0) await addDiagnostic("duplicate_account_ref")
      const externalBusy = new Map<string, number>()
      for (const claim of input.store.listLiveWorkClaims(request.now)) {
        if (claim.runRef === request.run.runRef || claim.state === "closeout") continue
        externalBusy.set(claim.workerAccountRef, (externalBusy.get(claim.workerAccountRef) ?? 0) + 1)
      }

      const rows: PublicSafePylonFleetCapacityAccount[] = []
      for (const account of registry) {
        if (duplicateAccountRefs.has(account.ref)) continue
        if (!runAcceptsProvider(request.run, account.provider)) continue
        const home = normalizeAccountHome(account.home)
        const isDefaultAccount = /^(?:\(default\)|default)$/iu.test(account.ref.trim()) ||
          home === (
            account.provider === "claude_agent"
              ? defaultHomes.claudeAgent
              : account.provider === "grok"
                ? defaultHomes.grok
                : defaultHomes.codex
          ) || (account.provider === "grok" && isDefaultGrokAccountHome(home, env))
        // Default Grok state is outside named account custody. Do not even
        // probe it: only registry-named isolated GROK_HOME accounts can
        // become owned fleet capacity.
        if (account.provider === "grok" && isDefaultAccount) continue
        if (account.provider === "grok" && !grokExecutionAvailable) {
          await addDiagnostic("grok_executor_unavailable")
        }
        const accountRefHash = hashPylonAccountRef(account.provider, account.ref)
        let readiness: string
        try {
          readiness = await probeReadiness({
            account,
            env,
            now: request.now,
            summary: input.summary,
          })
        } catch {
          await addDiagnostic(
            account.provider === "grok"
              ? "grok_account_inspection_unavailable"
              : "account_inspection_unavailable",
          )
          rows.push({
            accountRef: account.ref,
            capacity: { ready: 0, available: 0 },
            isDefaultAccount,
            marginalCostClass: account.marginalCostClass,
            paused: account.paused === true,
            provider: account.provider,
            readiness: "unavailable",
          })
          continue
        }
        const lane = laneForProvider(account.provider)
        const activeBreaker = activeBreakers.find((breaker) =>
          breaker.lane === lane &&
          (breaker.accountRefHash === accountRefHash ||
            (breaker.accountRefHash === null && breaker.contextId === null))
        )
        const usageState = usageCapacityState(usage.accounts[accountRefHash])
        if (usageState !== null) readiness = usageState
        if (activeBreaker !== undefined) readiness = activeBreaker.reason
        let ready: number | null
        try {
          ready = boundedSlots(
            advertisedSlotsForAccount(account),
            account.provider,
          )
        } catch {
          await addDiagnostic("account_inspection_unavailable")
          rows.push({
            accountRef: account.ref,
            capacity: { ready: 0, available: 0 },
            isDefaultAccount,
            marginalCostClass: account.marginalCostClass,
            paused: account.paused === true,
            provider: account.provider,
            readiness: "unavailable",
          })
          continue
        }
        const busy = externalBusy.get(account.ref) ?? 0
        rows.push({
          accountRef: account.ref,
          capacity: ready === null
            ? null
            : { ready, available: Math.max(0, ready - busy) },
          isDefaultAccount,
          marginalCostClass: account.marginalCostClass,
          paused: account.paused === true,
          provider: account.provider,
          readiness,
        })
      }

      latestDiagnostics = diagnostics
      return mapPylonFleetSupervisorCapacity(rows, {
        allowDefaultAccount: false,
        grokExecutionAvailable,
        includeUnavailableCandidates: true,
      })
    }

    const inspected = serialized.then(operation, operation)
    const result = inspected.catch(async () => {
      const entry = diagnostic("account_inspection_unavailable")
      latestDiagnostics = [entry]
      if (input.onDiagnostic !== undefined) {
        try {
          await input.onDiagnostic(entry)
        } catch {
          // Diagnostic observers are telemetry only and never capacity authority.
        }
      }
      return []
    })
    serialized = result.then(() => undefined, () => undefined)
    return result
  }

  return {
    accounts,
    diagnostics: () => [...latestDiagnostics],
  }
}

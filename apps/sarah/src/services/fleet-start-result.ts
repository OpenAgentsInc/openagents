import { FleetPublicRef, SyncScope, fleetRunScope } from "@openagentsinc/khala-sync"
import { Schema as S } from "effect"

import type {
  SarahFleetBrowserConfig,
  SarahFleetBrowserCoordinator,
} from "./fleet-browser-host.ts"

const SafeString = S.Trim.check(S.isMinLength(1), S.isMaxLength(1_000))
const SafeRef = S.Trim.check(
  S.isMinLength(1),
  S.isMaxLength(180),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:/#-]*$/u),
)
const IssueRef = S.String.check(S.isPattern(/^#[1-9]\d*$/u))
const Repository = S.Struct({
  owner: SafeRef,
  name: SafeRef,
  branch: SafeRef,
  commit: S.String.check(S.isPattern(/^[0-9a-f]{40}$/u)),
})
const Verifier = S.Struct({
  kind: S.Literal("command"),
  command: SafeString,
})
const WorkSource = S.Union([
  S.Struct({
    kind: S.Literal("issue_list"),
    issueRefs: S.Array(IssueRef),
  }),
  S.Struct({
    kind: S.Literal("plan_dag"),
    planRef: SafeRef,
    units: S.Array(
      S.Struct({
        unitRef: SafeRef,
        title: SafeString,
        dependsOn: S.Array(SafeRef),
      }),
    ),
  }),
])
const WorkerPolicy = S.Struct({
  workerKind: S.Literals(["codex", "claude", "grok", "auto"]),
  targetPreference: S.Literals(["owner_local", "managed_cloud", "auto"]),
})
const RelationshipPolicy = S.Struct({
  source: S.Literal("openagents_server_policy"),
  relationshipMode: S.Literals(["customer", "operator", "administrator"]),
  codingFleetStartAllowed: S.Literal(true),
  fleetObservationAllowed: S.Literal(true),
  retrievalScope: S.Literal("owner_fleet_runs"),
  responsePosture: S.Literals(["guided", "state_oriented"]),
  uiDensity: S.Literals(["standard", "dense"]),
  administratorToolsAllowed: S.Boolean,
})
const PublicRun = S.Struct({
  runRef: FleetPublicRef,
  scope: SyncScope,
  status: S.Literals(["pending_executor", "claimed_by_pylon"]),
  objective: SafeString,
  repository: Repository,
  verifier: Verifier,
  workSource: WorkSource,
  workerPolicy: WorkerPolicy,
  targetConcurrency: S.Int.check(
    S.isGreaterThanOrEqualTo(1),
    S.isLessThanOrEqualTo(8),
  ),
  createdAt: SafeString,
  updatedAt: SafeString,
  privateMaterialExcluded: S.Literal(true),
})

const SuccessfulFleetStartToolResult = S.Struct({
  toolCallId: SafeRef,
  toolName: S.Literal("coding_fleet_start"),
  ok: S.Literal(true),
  output: S.Struct({
    ok: S.Literal(true),
    duplicate: S.Boolean,
    policy: RelationshipPolicy,
    routeRef: S.Literal("route.sarah.fleet_runs.authority.v1"),
    run: PublicRun,
  }),
})

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null

const decodeSuccessfulResult = (
  value: unknown,
): SarahFleetBrowserConfig | null => {
  const raw = asRecord(value)
  if (raw?.toolName !== "coding_fleet_start" || raw.ok !== true) return null
  try {
    const decoded = S.decodeUnknownSync(SuccessfulFleetStartToolResult)(value, {
      onExcessProperty: "error",
    })
    const expectedScope = S.decodeUnknownSync(SyncScope)(
      fleetRunScope(decoded.output.run.runRef),
    )
    if (decoded.output.run.scope !== expectedScope) return null
    return {
      runRef: decoded.output.run.runRef,
      scope: expectedScope,
    }
  } catch {
    return null
  }
}

/**
 * Select only an exact, successful coding tool result. A malformed/failing
 * coding result poisons the response, and conflicting successes select
 * nothing. Model prose is intentionally not an input to this boundary.
 */
export const selectSarahFleetStartConfig = (
  toolResults: unknown,
): SarahFleetBrowserConfig | null => {
  if (!Array.isArray(toolResults)) return null
  let selected: SarahFleetBrowserConfig | null = null
  for (const result of toolResults) {
    const raw = asRecord(result)
    if (raw?.toolName !== "coding_fleet_start") continue
    if (raw.ok !== true) return null
    const decoded = decodeSuccessfulResult(result)
    if (decoded === null) return null
    if (selected !== null && selected.scope !== decoded.scope) return null
    selected = decoded
  }
  return selected
}

const ALTERNATE_FLEET_SELECTORS = [
  "fleet_scope",
  "fleet_run_ref",
  "fleetRun",
  "fleetRunRef",
  "run",
  "runRef",
  "run_ref",
  "scope",
] as const

export const canonicalSarahFleetRunUrl = (
  currentUrl: string,
  config: SarahFleetBrowserConfig,
): string => {
  const url = new URL(currentUrl)
  for (const selector of ALTERNATE_FLEET_SELECTORS) {
    url.searchParams.delete(selector)
  }
  url.searchParams.delete("fleet_run")
  url.searchParams.set("fleet_run", config.runRef)
  return url.toString()
}

/** Binds the exact result decoder to the one-runtime coordinator and URL. */
export const makeSarahFleetStartConfigHandler = (input: Readonly<{
  coordinator: Pick<SarahFleetBrowserCoordinator, "current" | "setConfig">
  currentUrl: () => string
  navigate: (url: string) => void
}>): ((config: SarahFleetBrowserConfig) => void) => {
  let lastNavigationUrl: string | null = null
  return (config) => {
    const previouslySelectedScope = input.coordinator.current()?.config.scope
    input.coordinator.setConfig(config)
    const destination = canonicalSarahFleetRunUrl(input.currentUrl(), config)
    if (
      destination !== input.currentUrl() &&
      (destination !== lastNavigationUrl ||
        previouslySelectedScope !== config.scope)
    ) {
      lastNavigationUrl = destination
      input.navigate(destination)
    }
  }
}

export const makeSarahFleetStartResultHandler = (input: Readonly<{
  coordinator: Pick<SarahFleetBrowserCoordinator, "current" | "setConfig">
  currentUrl: () => string
  navigate: (url: string) => void
}>): ((toolResults: unknown) => SarahFleetBrowserConfig | null) => {
  const handleConfig = makeSarahFleetStartConfigHandler(input)
  return (toolResults) => {
    const config = selectSarahFleetStartConfig(toolResults)
    if (config === null) return null
    handleConfig(config)
    return config
  }
}
